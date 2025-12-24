// src-tauri/src/poker_capture.rs
use screenshots::Screen;
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::time::sleep;
use tauri::{AppHandle, Emitter, Manager};
use once_cell::sync::Lazy;
use crate::screen_capture::{get_dpi_scale_factor, logical_to_physical, LogicalCoordinates};
use crate::vision::{
    should_process_frame, reset_frame_state, print_frame_statistics,
    analyze_with_openai, FrameFilterConfig,
    preprocess_for_vision_api, PreprocessConfig
};

/// Fullscreen capture mode: bypasses window detection and captures entire primary monitor
/// Set to true to work around window bounds issues (-32000, -32000)
const FULLSCREEN_MODE: bool = true;

// Global state tracking for cascade inference
static PREVIOUS_STATE: Lazy<Mutex<Option<crate::vision::openai_o4mini::RawVisionData>>> =
    Lazy::new(|| Mutex::new(None));

/// Parse and validate hero and community cards from vision response
/// Returns None if cards cannot be parsed or are invalid

/// Parse card string like "A♠" or "K♥" into Card struct
fn parse_card_string(card_str: &str) -> Option<crate::poker_types::Card> {
    if card_str.is_empty() {
        return None;
    }

    // Parse rank (first character(s))
    let rank_str = if card_str.starts_with("10") {
        "T"
    } else {
        &card_str[0..1]
    };

    // Parse suit (unicode symbol)
    let suit_char = card_str.chars().last()?;
    let suit_str = match suit_char {
        '♠' => "s",
        '♥' => "h",
        '♦' => "d",
        '♣' => "c",
        _ => return None,
    };

    crate::poker_types::Card::from_str(rank_str, suit_str)
}

/// Detect and normalize poker site name from window title
/// Returns a consistent site name for logging and potential future site-specific handling
fn detect_poker_site(window_title: &str) -> &'static str {
    let title_lower = window_title.to_lowercase();

    // Americas Cardroom (ACR) - Winning Poker Network
    if title_lower.contains("americas cardroom") ||
       title_lower.contains("americas card room") ||
       title_lower.contains("acr poker") ||
       (title_lower.contains("acr") && (title_lower.contains("poker") || title_lower.contains("hold"))) {
        return "Americas Cardroom";
    }

    // Ignition Casino - PaiWangLuo Network
    if title_lower.contains("ignition") {
        return "Ignition Casino";
    }

    // Bovada - PaiWangLuo Network (same as Ignition)
    if title_lower.contains("bovada") {
        return "Bovada";
    }

    // WSOP.com - 888/WSOP Network
    if title_lower.contains("wsop") {
        return "WSOP.com";
    }

    // PokerStars
    if title_lower.contains("pokerstars") {
        return "PokerStars";
    }

    // GGPoker - GG Network
    if title_lower.contains("ggpoker") || title_lower.contains("gg poker") {
        return "GGPoker";
    }

    // 888poker
    if title_lower.contains("888poker") || title_lower.contains("888 poker") {
        return "888poker";
    }

    // partypoker
    if title_lower.contains("partypoker") || title_lower.contains("party poker") {
        return "partypoker";
    }

    // BetOnline
    if title_lower.contains("betonline") {
        return "BetOnline";
    }

    // Replay Poker (play money)
    if title_lower.contains("replay poker") {
        return "Replay Poker";
    }

    // Global Poker (sweepstakes)
    if title_lower.contains("global poker") {
        return "Global Poker";
    }

    // Generic poker window
    if title_lower.contains("poker") || title_lower.contains("hold'em") || title_lower.contains("holdem") {
        return "Unknown Poker Site";
    }

    "Unknown"
}

/// Normalize site name to a short identifier for site-specific handling
/// Used for resolution selection and prompt hints
fn normalize_site_name(site_name: &str) -> &'static str {
    let lower = site_name.to_lowercase();
    if lower.contains("replay") {
        "replay"
    } else if lower.contains("ignition") {
        "ignition"
    } else if lower.contains("bovada") {
        "bovada"
    } else if lower.contains("acr") || lower.contains("americas cardroom") {
        "acr"
    } else if lower.contains("pokerstars") {
        "pokerstars"
    } else if lower.contains("ggpoker") {
        "ggpoker"
    } else if lower.contains("wsop") {
        "wsop"
    } else if lower.contains("888") {
        "888poker"
    } else if lower.contains("party") {
        "partypoker"
    } else if lower.contains("betonline") {
        "betonline"
    } else if lower.contains("global") {
        "globalpoker"
    } else {
        "generic"
    }
}

fn parse_and_validate_cards(
    raw_data: &crate::vision::openai_o4mini::RawVisionData,
) -> Option<(Vec<crate::poker_types::Card>, Vec<crate::poker_types::Card>)> {
    // Validate hero cards
    if raw_data.hero_cards.is_empty() {
        println!("⚠️  No hero cards detected, skipping recommendation");
        return None;
    }

    if raw_data.hero_cards.len() != 2 {
        println!("⚠️  Invalid number of hero cards: {}, expected 2", raw_data.hero_cards.len());
        return None;
    }

    // Parse hero cards
    let mut hero_cards = Vec::new();
    for card_str in &raw_data.hero_cards {
        match parse_card_string(card_str) {
            Some(card) => hero_cards.push(card),
            None => {
                println!("⚠️  Failed to parse hero card: {}", card_str);
                return None;
            }
        }
    }

    // Parse community cards (filter out nulls)
    let mut community_cards = Vec::new();
    for opt_card_str in &raw_data.community_cards {
        if let Some(card_str) = opt_card_str {
            match parse_card_string(card_str) {
                Some(card) => community_cards.push(card),
                None => {
                    println!("⚠️  Failed to parse community card: {}", card_str);
                    return None;
                }
            }
        }
    }

    // Check for duplicate cards (impossible in real poker)
    let mut all_cards = hero_cards.clone();
    all_cards.extend_from_slice(&community_cards);

    for i in 0..all_cards.len() {
        for j in (i + 1)..all_cards.len() {
            if all_cards[i].rank == all_cards[j].rank && all_cards[i].suit == all_cards[j].suit {
                println!("⚠️  Duplicate card detected: {:?}, invalid hand", all_cards[i]);
                return None;
            }
        }
    }

    // Check for impossible card counts (e.g., 5 kings)
    use std::collections::HashMap;
    let mut rank_counts: HashMap<crate::poker_types::Rank, usize> = HashMap::new();
    for card in &all_cards {
        *rank_counts.entry(card.rank).or_insert(0) += 1;
        if rank_counts[&card.rank] > 4 {
            println!("⚠️  Impossible card count: more than 4 cards of rank {:?}", card.rank);
            return None;
        }
    }

    Some((hero_cards, community_cards))
}

/// Parse available actions and amount to call from vision response
/// Returns (legal_actions, amount_to_call)
fn parse_legal_actions(
    available_actions: &Option<Vec<String>>,
    call_amount: Option<f64>,
    facing_bet: Option<bool>,
) -> (Vec<String>, Option<f64>) {
    let actions = available_actions.clone().unwrap_or_else(|| {
        // If no actions detected, infer from facingBet
        match facing_bet {
            Some(true) => vec!["fold".to_string(), "call".to_string(), "raise".to_string()],
            Some(false) => vec!["check".to_string(), "raise".to_string()],
            None => vec![], // Unknown state
        }
    });

    (actions, call_amount)
}

/// Generate recommendation using ONLY Rust evaluation (never trust AI's hand description)
/// Uses the new v2 API that enforces legal actions
fn generate_rust_recommendation(
    hero_cards: &[crate::poker_types::Card],
    community_cards: &[crate::poker_types::Card],
    pot_size: Option<f64>,
    position: Option<&str>,
    call_amount: Option<f64>,
    available_actions: &[String],
) -> (crate::poker::RecommendedAction, crate::poker::HandEvaluation) {
    // STEP 1: Evaluate hand strength using Rust (ONLY source of truth)
    let hand_eval = crate::poker::evaluate_hand(hero_cards, community_cards);

    println!("🎯 Rust evaluated: {} (score: {})", hand_eval.description, hand_eval.strength_score);

    // STEP 2: Parse legal actions from AI's detected buttons
    let amount_to_call = call_amount.unwrap_or(0.0);
    let legal_actions = crate::poker::parse_legal_actions(available_actions, amount_to_call);

    println!("✅ Legal actions: {:?}", legal_actions);
    if amount_to_call > 0.0 {
        println!("💵 Amount to call: ${:.2}", amount_to_call);
    }

    // STEP 3: Get recommendation from Rust strategy engine using new v2 API
    // This ensures we ONLY recommend legal actions
    let pot = pot_size.unwrap_or(0.0);
    let pos = position.unwrap_or("unknown");

    let recommendation = crate::poker::recommend_action_v2(
        &hand_eval,
        &legal_actions,
        pos,
        pot,
        amount_to_call,
        &community_cards,
    );

    (recommendation, hand_eval)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PokerRegions {
    pub hole_cards: String,
    pub community_cards: String,
    pub pot: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ParsedPokerData {
    pub your_cards: Vec<String>,
    pub community_cards: Vec<String>,
    pub pot_size: Option<f64>,
    pub position: Option<String>,
    pub recommendation: crate::poker::RecommendedAction,
    pub strength_score: u32,       // Hand strength 0-100
    pub win_percentage: f32,       // Win percentage 0-100
    pub tie_percentage: f32,       // Tie percentage 0-100
    pub street: String,            // preflop/flop/turn/river
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PokerWindow {
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CapturedGameState {
    pub image_base64: String,
    pub timestamp: u64,
    pub window_title: String,
    pub ocr_text: Option<String>,
    pub cards_detected: Vec<String>,
    pub pot_size: Option<f64>,
    pub position: Option<String>,
}

pub struct MonitoringState {
    pub is_running: Arc<Mutex<bool>>,
    pub cancel_requested: Arc<AtomicBool>,
}

impl Default for MonitoringState {
    fn default() -> Self {
        Self {
            is_running: Arc::new(Mutex::new(false)),
            cancel_requested: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// Helper: Check if this looks like a new hand
fn is_likely_new_hand(
    current: &crate::vision::openai_o4mini::RawVisionData,
    previous: &crate::vision::openai_o4mini::RawVisionData,
) -> bool {
    // Pot dropped significantly (more than 70% drop suggests hand ended)
    if let (Some(prev_pot), Some(curr_pot)) = (previous.pot, current.pot) {
        if curr_pot < prev_pot * 0.3 {
            return true;
        }
    }

    // Community cards reset (had 3+, now 0)
    let prev_community = previous.community_cards.iter().filter(|c| c.is_some()).count();
    let curr_community = current.community_cards.iter().filter(|c| c.is_some()).count();
    if prev_community >= 3 && curr_community == 0 {
        return true;
    }

    false
}

/// Normalize card for comparison (handles unicode vs letter suits, 10 vs T)
fn normalize_card_for_comparison(card: &str) -> String {
    card.to_lowercase()
        .replace("10", "t")
        .replace("♠", "s")
        .replace("♥", "h")
        .replace("♦", "d")
        .replace("♣", "c")
}

/// Check if two card sets match (accounting for different representations)
fn cards_match(cards1: &[String], cards2: &[String]) -> bool {
    if cards1.len() != cards2.len() {
        return false;
    }

    let set1: std::collections::HashSet<String> = cards1.iter()
        .map(|c| normalize_card_for_comparison(c))
        .collect();
    let set2: std::collections::HashSet<String> = cards2.iter()
        .map(|c| normalize_card_for_comparison(c))
        .collect();

    set1 == set2
}

/// Validate that card detection is temporally consistent
/// Returns Ok(()) if consistent, Err(reason) if inconsistent
fn validate_temporal_consistency(
    current: &crate::vision::openai_o4mini::RawVisionData,
    previous: &crate::vision::openai_o4mini::RawVisionData,
) -> Result<(), String> {
    // Skip validation if this looks like a new hand
    if is_likely_new_hand(current, previous) {
        println!("🆕 New hand detected - skipping temporal consistency check");
        return Ok(());
    }

    // Rule 1: Hero cards cannot change mid-hand
    // If both frames have 2 hero cards and pot didn't reset, cards must match
    if previous.hero_cards.len() == 2 && current.hero_cards.len() == 2 {
        if !cards_match(&previous.hero_cards, &current.hero_cards) {
            return Err(format!(
                "Hero cards changed {:?} -> {:?} but pot didn't reset",
                previous.hero_cards, current.hero_cards
            ));
        }
    }

    // Rule 2: Community cards can only increase, never decrease (unless new hand)
    let prev_community_count = previous.community_cards.iter().filter(|c| c.is_some()).count();
    let curr_community_count = current.community_cards.iter().filter(|c| c.is_some()).count();

    if curr_community_count < prev_community_count {
        return Err(format!(
            "Community cards decreased {} -> {} but not a new hand",
            prev_community_count, curr_community_count
        ));
    }

    // Rule 3: Existing community cards shouldn't change suits/ranks
    if prev_community_count > 0 && curr_community_count >= prev_community_count {
        for i in 0..prev_community_count {
            if let (Some(prev_card), Some(curr_card)) = (&previous.community_cards[i], &current.community_cards[i]) {
                let prev_norm = normalize_card_for_comparison(prev_card);
                let curr_norm = normalize_card_for_comparison(curr_card);
                if prev_norm != curr_norm {
                    return Err(format!(
                        "Community card {} changed: {} -> {}",
                        i + 1, prev_card, curr_card
                    ));
                }
            }
        }
    }

    Ok(())
}

/// Apply temporal consistency correction to raw data
/// Returns corrected data that merges previous stable cards with current dynamic data
fn apply_temporal_correction(
    current: &crate::vision::openai_o4mini::RawVisionData,
    previous: &crate::vision::openai_o4mini::RawVisionData,
) -> crate::vision::openai_o4mini::RawVisionData {
    let mut corrected = previous.clone();

    // Keep dynamic data from current detection (pot, actions, stack)
    corrected.pot = current.pot;
    corrected.available_actions = current.available_actions.clone();
    corrected.amount_to_call = current.amount_to_call;
    corrected.hero_stack = current.hero_stack;
    corrected.position = current.position.clone();

    // For community cards: keep previous stable cards, add any new ones from current
    let prev_count = previous.community_cards.iter().filter(|c| c.is_some()).count();
    let curr_count = current.community_cards.iter().filter(|c| c.is_some()).count();

    if curr_count > prev_count {
        // New community cards appeared - keep previous + add new ones
        for i in prev_count..curr_count.min(5) {
            if i < current.community_cards.len() && current.community_cards[i].is_some() {
                if i < corrected.community_cards.len() {
                    corrected.community_cards[i] = current.community_cards[i].clone();
                }
            }
        }
    }

    corrected
}

/// Verify hero cards with Claude when a new hand is detected
/// This is critical because wrong first detection = wrong entire hand
/// NOTE: Currently unused - kept for potential future use if accuracy issues arise
#[allow(dead_code)]
async fn verify_new_hand_with_claude(
    png_bytes: &[u8],
    openai_data: &crate::vision::openai_o4mini::RawVisionData,
    _site_name: &str,
) -> Result<crate::vision::openai_o4mini::RawVisionData, String> {
    // Skip verification if no hero cards detected by OpenAI
    if openai_data.hero_cards.is_empty() {
        println!("   No hero cards to verify, skipping Claude check");
        return Ok(openai_data.clone());
    }

    let claude_start = std::time::Instant::now();

    // Call Claude for verification
    let openai_json = serde_json::to_string(openai_data).unwrap_or_default();
    let issues = vec!["new_hand_verification".to_string()];

    match crate::claude_vision::analyze_with_claude_raw(png_bytes, &openai_json, &issues).await {
        Ok(claude_data) => {
            println!("⏱️  Claude verification took: {:.2}s", claude_start.elapsed().as_secs_f64());

            // Compare hero cards between OpenAI and Claude
            let openai_normalized: std::collections::HashSet<String> = openai_data.hero_cards.iter()
                .map(|c| normalize_card_for_comparison(c))
                .collect();
            let claude_normalized: std::collections::HashSet<String> = claude_data.hero_cards.iter()
                .map(|c| normalize_card_for_comparison(c))
                .collect();

            if openai_normalized != claude_normalized {
                println!("⚠️  OpenAI/Claude disagree on new hand cards:");
                println!("   OpenAI: {:?}", openai_data.hero_cards);
                println!("   Claude: {:?}", claude_data.hero_cards);
                println!("   Using Claude's detection (better suit accuracy)");

                // Return Claude data (trust Claude for suit accuracy)
                Ok(claude_data)
            } else {
                println!("✅ OpenAI/Claude agree on hero cards: {:?}", openai_data.hero_cards);
                Ok(openai_data.clone())
            }
        }
        Err(e) => {
            println!("⏱️  Claude verification took: {:.2}s", claude_start.elapsed().as_secs_f64());
            Err(e)
        }
    }
}

/// Verify community cards with Claude when street transitions (flop/turn/river appears)
/// This prevents wrong community card detection from persisting for entire hand
/// NOTE: Currently unused - kept for potential future use if accuracy issues arise
#[allow(dead_code)]
async fn verify_community_cards_with_claude(
    png_bytes: &[u8],
    openai_data: &crate::vision::openai_o4mini::RawVisionData,
    prev_community_count: usize,
    _site_name: &str,
) -> Result<crate::vision::openai_o4mini::RawVisionData, String> {
    let curr_community_count = openai_data.community_cards.iter().filter(|c| c.is_some()).count();

    // Determine what street transition this is
    let transition = match (prev_community_count, curr_community_count) {
        (0, 3) => "flop",
        (3, 4) => "turn",
        (4, 5) => "river",
        _ => return Ok(openai_data.clone()), // No significant transition
    };

    println!("🃏 {} detected ({} → {} community cards) - verifying with Claude...",
        transition.to_uppercase(), prev_community_count, curr_community_count);

    let claude_start = std::time::Instant::now();

    // Call Claude for verification with community_card_verification issue
    let openai_json = serde_json::to_string(openai_data).unwrap_or_default();
    let issues = vec![format!("community_card_verification:{}", transition)];

    match crate::claude_vision::analyze_with_claude_raw(png_bytes, &openai_json, &issues).await {
        Ok(claude_data) => {
            println!("⏱️  Claude verification took: {:.2}s", claude_start.elapsed().as_secs_f64());

            // Compare community cards between OpenAI and Claude
            let openai_community: Vec<String> = openai_data.community_cards.iter()
                .filter_map(|c| c.clone())
                .map(|c| normalize_card_for_comparison(&c))
                .collect();
            let claude_community: Vec<String> = claude_data.community_cards.iter()
                .filter_map(|c| c.clone())
                .map(|c| normalize_card_for_comparison(&c))
                .collect();

            if openai_community != claude_community {
                println!("⚠️  OpenAI/Claude disagree on {} cards:", transition);
                println!("   OpenAI: {:?}", openai_data.community_cards);
                println!("   Claude: {:?}", claude_data.community_cards);
                println!("   Using Claude's detection (better accuracy)");

                // Return Claude data for community cards, but keep OpenAI's hero cards
                // (hero cards were already verified on new hand)
                let mut merged_data = openai_data.clone();
                merged_data.community_cards = claude_data.community_cards;
                Ok(merged_data)
            } else {
                println!("✅ OpenAI/Claude agree on {} cards: {:?}", transition, openai_data.community_cards);
                Ok(openai_data.clone())
            }
        }
        Err(e) => {
            println!("⏱️  Claude verification took: {:.2}s", claude_start.elapsed().as_secs_f64());
            println!("⚠️  Claude verification failed: {}, using OpenAI result", e);
            Ok(openai_data.clone())
        }
    }
}

/// Resolve duplicate cards by calling Claude for verification
/// Called when validation detects duplicate cards between hero and community
async fn resolve_duplicate_cards_with_claude(
    png_bytes: &[u8],
    openai_data: &crate::vision::openai_o4mini::RawVisionData,
    _site_name: &str,
) -> Result<crate::vision::openai_o4mini::RawVisionData, String> {
    println!("🔄 Resolving duplicate cards with Claude...");

    let claude_start = std::time::Instant::now();

    // Call Claude with duplicate_resolution issue
    let openai_json = serde_json::to_string(openai_data).unwrap_or_default();
    let issues = vec!["duplicate_resolution".to_string()];

    match crate::claude_vision::analyze_with_claude_raw(png_bytes, &openai_json, &issues).await {
        Ok(claude_data) => {
            println!("⏱️  Claude duplicate resolution took: {:.2}s", claude_start.elapsed().as_secs_f64());

            // Check if Claude's result has no duplicates
            let claude_has_dupes = crate::vision::has_duplicate_cards(
                &claude_data.hero_cards,
                &claude_data.community_cards
            );

            if !claude_has_dupes {
                println!("✅ Claude resolved duplicate - detection corrected");
                println!("   Corrected hero: {:?}", claude_data.hero_cards);
                println!("   Corrected community: {:?}", claude_data.community_cards);
                Ok(claude_data)
            } else {
                println!("⚠️  Claude also detected duplicates - visual ambiguity in screenshot");
                // Return Claude's result anyway, might be better than OpenAI's
                Ok(claude_data)
            }
        }
        Err(e) => {
            println!("⏱️  Claude duplicate resolution took: {:.2}s", claude_start.elapsed().as_secs_f64());
            Err(e)
        }
    }
}

/// Detect if a new hand has started (pot reset)
#[allow(dead_code)]
fn detect_new_hand(current_state: &crate::vision::openai_o4mini::RawVisionData) -> bool {
    let prev_state_clone = {
        let prev_state_guard = PREVIOUS_STATE.lock().unwrap();
        prev_state_guard.clone()
    }; // Lock dropped here

    if let Some(prev) = prev_state_clone {
        // New hand detected if:
        // 1. Pot resets from high to low (hand ended)
        // 2. OR board cards disappear (new deal)

        if let (Some(prev_pot), Some(curr_pot)) = (prev.pot, current_state.pot) {
            // Pot reset: was high (>$2000), now low (<$1000)
            if prev_pot > 2000.0 && curr_pot < 1000.0 {
                return true;
            }
        }

        // Board cards reset (3+ visible cards -> 0 visible cards)
        let prev_visible = prev.community_cards.iter().filter(|c| c.is_some()).count();
        let curr_visible = current_state.community_cards.iter().filter(|c| c.is_some()).count();
        if prev_visible >= 3 && curr_visible == 0 {
            return true;
        }
    }

    false
}

/// Capture poker window and analyze with CASCADE INFERENCE + YOLO CROPPING
pub async fn capture_poker_regions(
    window_title: String,
    app_handle: Option<&AppHandle>,
    cancel_flag: Option<&Arc<AtomicBool>>,
) -> Result<ParsedPokerData, String> {
    // Check for cancellation at start
    if let Some(flag) = cancel_flag {
        if flag.load(Ordering::Relaxed) {
            println!("🚫 Capture cancelled at start");
            return Err("Capture cancelled".to_string());
        }
    }

    // ============================================
    // TIMING DIAGNOSTICS - START
    // ============================================
    let capture_start = std::time::Instant::now();
    println!("\n⏱️  ========== CAPTURE START ==========");

    // ============================================
    // FULLSCREEN MODE CHECK
    // ============================================
    let screenshot_start = std::time::Instant::now();
    let window_img = if FULLSCREEN_MODE {
        println!("🖥️  FULLSCREEN MODE: Detecting pkr.ai window monitor...");

        let screens = Screen::all().map_err(|e| format!("Failed to get screens: {}", e))?;

        // Default to primary screen (index 0)
        let mut target_screen_index = 0;

        // If we have app_handle, detect which monitor the pkr.ai window is on
        if let Some(app) = app_handle {
            if let Some(main_window) = app.get_webview_window("main") {
                match main_window.outer_position() {
                    Ok(position) => {
                        let window_x = position.x;
                        let window_y = position.y;
                        println!("📍 pkr.ai window position: ({}, {})", window_x, window_y);

                        // Find which screen contains the window center point
                        for (index, screen) in screens.iter().enumerate() {
                            let display = screen.display_info;
                            let screen_x = display.x;
                            let screen_y = display.y;
                            let screen_width = display.width;
                            let screen_height = display.height;

                            println!("🖥️  Screen {}: x={}, y={}, w={}, h={}",
                                index, screen_x, screen_y, screen_width, screen_height);

                            // Check if window position is within this screen's bounds
                            if window_x >= screen_x
                                && window_x < screen_x + screen_width as i32
                                && window_y >= screen_y
                                && window_y < screen_y + screen_height as i32
                            {
                                target_screen_index = index;
                                println!("✅ Found pkr.ai window on Screen {}", index);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        println!("⚠️  Could not get window position: {}, using primary monitor", e);
                    }
                }
            } else {
                println!("⚠️  Main window not found, using primary monitor");
            }
        } else {
            println!("⚠️  No app handle provided, using primary monitor");
        }

        let screen = screens.get(target_screen_index)
            .ok_or_else(|| format!("Screen {} not found", target_screen_index))?;

        println!("🖥️  Capturing Screen {}", target_screen_index);

        let full_image = screen.capture()
            .map_err(|e| format!("Failed to capture screen: {}", e))?;

        println!("📸 Full screenshot size: {}x{}", full_image.width(), full_image.height());

        let img_buffer = image::RgbaImage::from_raw(
            full_image.width(),
            full_image.height(),
            full_image.rgba().to_vec(),
        ).ok_or("Failed to create image buffer")?;

        image::DynamicImage::ImageRgba8(img_buffer)
    } else {
        // Original window detection logic
        let windows = find_poker_windows().await?;
        let poker_window = windows.iter()
            .find(|w| w.title == window_title)
            .ok_or("Poker window not found")?;

        // Get DPI scale factor for coordinate conversion
        let scale_factor = get_dpi_scale_factor().unwrap_or(1.0);

        println!("📐 Window bounds (logical): x={}, y={}, w={}, h={}",
            poker_window.x, poker_window.y, poker_window.width, poker_window.height);

        // Convert logical window coordinates to physical screen coordinates
        let logical_coords = LogicalCoordinates {
            x: poker_window.x,
            y: poker_window.y,
            width: poker_window.width,
            height: poker_window.height,
        };
        let physical_coords = logical_to_physical(&logical_coords, scale_factor);

        println!("📐 Physical coords ({}x scale): x={}, y={}, w={}, h={}",
            scale_factor, physical_coords.x, physical_coords.y,
            physical_coords.width, physical_coords.height);

        let screens = Screen::all().map_err(|e| format!("Failed to get screens: {}", e))?;
        let screen = screens.first().ok_or("No screens found")?;
        let full_image = screen.capture()
            .map_err(|e| format!("Failed to capture screen: {}", e))?;

        println!("📸 Full screenshot size: {}x{}", full_image.width(), full_image.height());

        let img_buffer = image::RgbaImage::from_raw(
            full_image.width(),
            full_image.height(),
            full_image.rgba().to_vec(),
        ).ok_or("Failed to create image buffer")?;

        let dynamic_img = image::DynamicImage::ImageRgba8(img_buffer);

        // Use physical coordinates for cropping
        let crop_x = physical_coords.x.min(dynamic_img.width().saturating_sub(1));
        let crop_y = physical_coords.y.min(dynamic_img.height().saturating_sub(1));
        let crop_width = physical_coords.width.min(dynamic_img.width() - crop_x);
        let crop_height = physical_coords.height.min(dynamic_img.height() - crop_y);

        println!("✂️  Cropping to: x={}, y={}, w={}, h={}", crop_x, crop_y, crop_width, crop_height);

        dynamic_img.crop_imm(crop_x, crop_y, crop_width, crop_height)
    };

    println!("🎯 Image size: {}x{}", window_img.width(), window_img.height());
    println!("⏱️  Screenshot capture took: {:.2}s", screenshot_start.elapsed().as_secs_f64());

    // ============================================
    // FRAME FILTERING PIPELINE
    // ============================================
    let filter_start = std::time::Instant::now();
    let filter_config = FrameFilterConfig {
        min_diff_threshold: 0.02,  // 2% change threshold
        min_green_ratio: 0.0,      // Disabled: poker sites use varying felt colors (red, green, blue, etc)
        max_skip_duration_secs: 15, // Force process after 15 seconds (3x capture interval to allow hash comparison to work)
        use_perceptual_hash: true,
    };
    let filter_result = should_process_frame(&window_img, &filter_config);
    println!("⏱️  Frame filtering took: {:.2}s", filter_start.elapsed().as_secs_f64());

    if !filter_result.should_process {
        println!("⏭️  Frame skipped: {} (diff: {:.1}%, green: {})",
            filter_result.reason,
            filter_result.diff_percentage * 100.0,
            filter_result.green_felt_detected
        );
        // Return previous state if available, or error if first frame was filtered
        let prev_state_guard = PREVIOUS_STATE.lock().unwrap();
        if let Some(ref prev_raw_data) = *prev_state_guard {
            let your_cards = prev_raw_data.hero_cards.clone();
            let community_cards: Vec<String> = prev_raw_data.community_cards
                .iter()
                .filter_map(|opt| opt.clone())
                .collect();

            // Always use Rust strategy (never trust AI hand descriptions)
            let (recommendation, hand_eval, win_pct, tie_pct, street) = match parse_and_validate_cards(prev_raw_data) {
                Some((hero_cards, community_cards_parsed)) => {
                    let (legal_actions, call_amount) = parse_legal_actions(
                        &Some(prev_raw_data.available_actions.clone()),
                        Some(prev_raw_data.amount_to_call),
                        None,
                    );

                    let (rec, eval) = generate_rust_recommendation(
                        &hero_cards,
                        &community_cards_parsed,
                        prev_raw_data.pot,
                        prev_raw_data.position.as_deref(),
                        call_amount,
                        &legal_actions,
                    );

                    // Calculate win/tie percentages
                    let (win_pct, tie_pct) = crate::poker::calculate_win_tie_percentages(
                        &hero_cards,
                        &community_cards_parsed,
                        1000, // num_simulations
                    );

                    // Determine street
                    let street = match community_cards_parsed.len() {
                        0 => "preflop".to_string(),
                        3 => "flop".to_string(),
                        4 => "turn".to_string(),
                        5 => "river".to_string(),
                        _ => "unknown".to_string(),
                    };

                    (rec, eval, win_pct, tie_pct, street)
                }
                None => {
                    let default_eval = crate::poker::HandEvaluation {
                        category: crate::poker::HandCategory::HighCard,
                        description: "Unable to evaluate".to_string(),
                        strength_score: 0,
                        kickers: vec![],
                        draw_type: crate::poker::DrawType::None,
                        outs: 0,
                    };
                    (
                        crate::poker::RecommendedAction {
                            action: crate::poker::Action::NoRecommendation,
                            reasoning: "No recommendation available - unable to detect cards".to_string(),
                        },
                        default_eval,
                        0.0,
                        0.0,
                        "unknown".to_string(),
                    )
                }
            };

            return Ok(ParsedPokerData {
                your_cards,
                community_cards,
                pot_size: prev_raw_data.pot,
                position: prev_raw_data.position.clone(),
                recommendation,
                strength_score: hand_eval.strength_score,
                win_percentage: win_pct,
                tie_percentage: tie_pct,
                street,
            });
        } else {
            return Err("Frame filtered and no previous state available".to_string());
        }
    }

    println!("✅ Frame will be processed: {} (diff: {:.1}%)",
        filter_result.reason, filter_result.diff_percentage * 100.0);

    // ============================================
    // YOLO PANEL DETECTION + CROPPING (DISABLED - TOO SLOW)
    // ============================================
    // Panel detection adds 15-20 seconds per capture and often fails
    // Using full captured image instead for much faster processing
    let panel_start = std::time::Instant::now();
    let panel_img = window_img.clone();
    println!("⏱️  Panel detection took: {:.2}s (skipped - using full image)", panel_start.elapsed().as_secs_f64());

    // ============================================
    // SITE-SPECIFIC CONFIGURATION
    // ============================================
    let detected_site = detect_poker_site(&window_title);
    let normalized_site = normalize_site_name(detected_site);
    println!("🎯 Site detected: {} → normalized: {}", detected_site, normalized_site);

    // ============================================
    // IMAGE PREPROCESSING FOR VISION API
    // ============================================
    let preprocess_start = std::time::Instant::now();
    let preprocess_config = PreprocessConfig::for_site(Some(normalized_site));
    println!("⚡ Using resolution: {}x{} for {}", preprocess_config.target_width, preprocess_config.target_height, normalized_site);
    let final_img = preprocess_for_vision_api(&panel_img, &preprocess_config);
    println!("⏱️  Image preprocessing took: {:.2}s", preprocess_start.elapsed().as_secs_f64());
    // ============================================

    // Convert to PNG bytes
    let mut png_bytes = Vec::new();
    final_img.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;

    let size_kb = png_bytes.len() as f32 / 1024.0;
    println!("📦 Final image size: {:.1} KB ({}x{})", size_kb, final_img.width(), final_img.height());

    println!("🤖 Step 1: Analyzing with OpenAI o4-mini (fast)...");

    // STEP 1: Try OpenAI o4-mini first (cheap and fast)
    let openai_start = std::time::Instant::now();
    let openai_result = match analyze_with_openai(&png_bytes, Some(normalized_site)).await {
        Ok(result) => Some(result),
        Err(e) => {
            if e.contains("429") || e.contains("RATE_LIMIT") {
                println!("⚠️  OpenAI rate limit hit! Will try Claude...");
                None
            } else {
                println!("❌ OpenAI error: {}", e);
                None
            }
        }
    };
    println!("⏱️  OpenAI API call took: {:.2}s", openai_start.elapsed().as_secs_f64());

    // Check for cancellation after OpenAI API call
    if let Some(flag) = cancel_flag {
        if flag.load(Ordering::Relaxed) {
            println!("🚫 Capture cancelled after OpenAI API call");
            return Err("Capture cancelled".to_string());
        }
    }

    // STEP 2: Validate OpenAI result and fallback to Claude if needed
    let raw_data = if let Some(ref data) = openai_result {
        let issues = crate::vision::openai_o4mini::validate_vision_response(data);

        if issues.is_empty() {
            println!("✅ OpenAI validation passed");
            data.clone()
        } else {
            println!("⚠️  OpenAI validation failed: {:?}", issues);
            println!("🤖 Step 2: Falling back to Claude for correction...");

            // Try Claude fallback
            let claude_start = std::time::Instant::now();
            let tier1_json = serde_json::to_string(data).unwrap_or_default();
            match crate::claude_vision::analyze_with_claude_raw(
                &png_bytes,
                &tier1_json,
                &issues,
            ).await {
                Ok(claude_data) => {
                    println!("⏱️  Claude API call took: {:.2}s", claude_start.elapsed().as_secs_f64());
                    println!("✅ Claude correction complete");
                    claude_data
                }
                Err(e) => {
                    println!("⏱️  Claude API call took: {:.2}s", claude_start.elapsed().as_secs_f64());
                    println!("❌ Claude fallback failed: {}", e);
                    // Return OpenAI data anyway, let downstream handle it
                    data.clone()
                }
            }
        }
    } else {
        // OpenAI completely failed, try Claude directly
        println!("🤖 Trying Claude as primary (OpenAI unavailable)...");
        let claude_start = std::time::Instant::now();
        match crate::claude_vision::analyze_with_claude_raw(
            &png_bytes,
            "{}",
            &["openai_unavailable".to_string()],
        ).await {
            Ok(claude_data) => {
                println!("⏱️  Claude API call took: {:.2}s", claude_start.elapsed().as_secs_f64());
                claude_data
            }
            Err(e) => {
                println!("⏱️  Claude API call took: {:.2}s", claude_start.elapsed().as_secs_f64());
                return Err(format!("Both OpenAI and Claude failed: {}", e));
            }
        }
    };

    // ============================================
    // TEMPORAL CONSISTENCY VALIDATION
    // ============================================
    // Cards cannot flip-flop between frames during the same hand
    // This is a FREE check in Rust - no API calls needed
    let raw_data = {
        let (is_new_hand, prev_clone) = {
            let prev_state_guard = PREVIOUS_STATE.lock().unwrap();
            if let Some(ref prev) = *prev_state_guard {
                (is_likely_new_hand(&raw_data, prev), Some(prev.clone()))
            } else {
                (true, None) // First frame ever = treat as new hand
            }
        }; // Lock released here

        if is_new_hand {
            // NEW HAND: Trust OpenAI result, temporal consistency will protect future frames
            println!("🆕 New hand detected - using OpenAI result");
            raw_data
        } else if let Some(ref prev) = prev_clone {
            // SAME HAND: Apply temporal consistency check (FREE - runs in Rust)
            match validate_temporal_consistency(&raw_data, prev) {
                Ok(()) => {
                    println!("✅ Temporal consistency check passed");
                    raw_data
                }
                Err(reason) => {
                    println!("⚠️  Temporal inconsistency detected: {}", reason);
                    println!("   Using corrected state (previous cards + current pot/actions)");
                    apply_temporal_correction(&raw_data, prev)
                }
            }
        } else {
            // No previous state (shouldn't reach here, but fallback)
            raw_data
        }
    };

    // Final duplicate check - call Claude ONLY if duplicates detected
    let raw_data = {
        let has_duplicates = crate::vision::has_duplicate_cards(
            &raw_data.hero_cards,
            &raw_data.community_cards
        );

        if has_duplicates {
            println!("⚠️  Duplicate cards detected - calling Claude for resolution...");
            match resolve_duplicate_cards_with_claude(&png_bytes, &raw_data, normalized_site).await {
                Ok(resolved_data) => resolved_data,
                Err(e) => {
                    println!("⚠️  Claude duplicate resolution failed: {}", e);
                    raw_data
                }
            }
        } else {
            raw_data
        }
    };

    println!("✅ Vision data extraction complete!");
    println!("   Hero cards: {:?}", raw_data.hero_cards);
    println!("   Community cards: {:?}", raw_data.community_cards);
    println!("   Pot: {:?}", raw_data.pot);
    println!("   Position: {:?}", raw_data.position);
    println!("   Available actions: {:?}", raw_data.available_actions);
    println!("   Amount to call: {:.2}", raw_data.amount_to_call);

    // Validate and log position detection
    if let Some(ref pos) = raw_data.position {
        let valid_positions = vec!["BTN", "SB", "BB", "UTG", "MP", "CO", "HJ"];
        if valid_positions.contains(&pos.as_str()) {
            println!("✅ Position detected successfully: {}", pos);
        } else {
            println!("⚠️  Unexpected position value: {} (expected one of: {:?})", pos, valid_positions);
        }
    } else {
        println!("⚠️  Position not detected - AI couldn't identify dealer button or position indicators");
        println!("    Tip: Check if dealer button is visible in the screenshot");
    }

    // ============================================
    // RUST STRATEGY RECOMMENDATION (PRIMARY PATH)
    // ============================================
    println!("🧠 Generating strategy recommendation with Rust evaluation...");
    let strategy_start = std::time::Instant::now();

    // STEP 1: Parse and validate cards
    let (recommendation, hand_eval, win_pct, tie_pct, street) = match parse_and_validate_cards(&raw_data) {
        Some((hero_cards, community_cards)) => {
            // STEP 2: Parse legal actions
            let (legal_actions, call_amount) = parse_legal_actions(
                &Some(raw_data.available_actions.clone()),
                Some(raw_data.amount_to_call),
                None, // facing_bet not in RawVisionData
            );

            // STEP 3: Generate recommendation using ONLY Rust evaluation
            let (rec, eval) = generate_rust_recommendation(
                &hero_cards,
                &community_cards,
                raw_data.pot,
                raw_data.position.as_deref(),
                call_amount,
                &legal_actions,
            );

            // Calculate win/tie percentages
            let (win_pct, tie_pct) = crate::poker::calculate_win_tie_percentages(
                &hero_cards,
                &community_cards,
                1000, // num_simulations
            );

            // Determine street
            let street = match community_cards.len() {
                0 => "preflop".to_string(),
                3 => "flop".to_string(),
                4 => "turn".to_string(),
                5 => "river".to_string(),
                _ => "unknown".to_string(),
            };

            (rec, eval, win_pct, tie_pct, street)
        }
        None => {
            // Card parsing failed - cannot generate recommendation
            println!("❌ Card parsing/validation failed, cannot generate recommendation");
            let default_eval = crate::poker::HandEvaluation {
                category: crate::poker::HandCategory::HighCard,
                description: "Unable to evaluate".to_string(),
                strength_score: 0,
                kickers: vec![],
                draw_type: crate::poker::DrawType::None,
                outs: 0,
            };
            (
                crate::poker::RecommendedAction {
                    action: crate::poker::Action::NoRecommendation,
                    reasoning: "No recommendation available - unable to detect cards".to_string(),
                },
                default_eval,
                0.0,
                0.0,
                "unknown".to_string(),
            )
        }
    };

    println!("⏱️  Strategy recommendation took: {:.2}s", strategy_start.elapsed().as_secs_f64());

    // Save current state for next iteration
    *PREVIOUS_STATE.lock().unwrap() = Some(raw_data.clone());

    // Display format uses the raw string cards from vision API
    let your_cards = raw_data.hero_cards.clone();
    let community_cards: Vec<String> = raw_data.community_cards
        .iter()
        .filter_map(|opt| opt.clone())
        .collect();

    println!("🃏 Your cards: {:?}", your_cards);
    println!("🎴 Community: {:?}", community_cards);
    if let Some(pot) = raw_data.pot {
        println!("💰 Pot: ${}", pot);
    }
    if let Some(ref pos) = raw_data.position {
        println!("📍 Position: {}", pos);
    }

    // Print strategy recommendation
    let action_str = match &recommendation.action {
        crate::poker::Action::Fold => "FOLD".to_string(),
        crate::poker::Action::Check => "CHECK".to_string(),
        crate::poker::Action::Call => "CALL".to_string(),
        crate::poker::Action::Bet(amount) => format!("BET ${:.2}", amount),
        crate::poker::Action::Raise(amount) => format!("RAISE ${:.2}", amount),
        crate::poker::Action::NoRecommendation => "NO RECOMMENDATION".to_string(),
    };
    println!("♠️  Rust Recommendation: {} - {}", action_str, recommendation.reasoning);

    // ============================================
    // TIMING DIAGNOSTICS - END
    // ============================================
    let total_time = capture_start.elapsed().as_secs_f64();
    println!("⏱️  ========== TOTAL CAPTURE TIME: {:.2}s ==========\n", total_time);

    Ok(ParsedPokerData {
        your_cards,
        community_cards,
        pot_size: raw_data.pot,
        position: raw_data.position.clone(),
        recommendation,
        strength_score: hand_eval.strength_score,
        win_percentage: win_pct,
        tie_percentage: tie_pct,
        street,
    })
}

#[tauri::command]
pub async fn find_poker_windows() -> Result<Vec<PokerWindow>, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
        use windows::Win32::UI::WindowsAndMessaging::{
            EnumWindows, GetWindowTextW, GetWindowRect, IsWindowVisible,
        };
        use std::sync::Mutex;

        let windows: Mutex<Vec<PokerWindow>> = Mutex::new(Vec::new());

        unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let windows_ptr = lparam.0 as *const Mutex<Vec<PokerWindow>>;
            let windows = &*windows_ptr;

            if !IsWindowVisible(hwnd).as_bool() {
                return BOOL(1);
            }

            let mut title: [u16; 512] = [0; 512];
            let len = GetWindowTextW(hwnd, &mut title);
            
            if len == 0 {
                return BOOL(1);
            }

            let title_str = String::from_utf16_lossy(&title[..len as usize]);

            // Poker site keywords for window detection (case-insensitive matching)
            let poker_keywords = [
                "pokerstars",
                "ggpoker",
                "888poker",
                "partypoker",
                "acr",
                "americas cardroom",  // ACR full name
                "americas card room", // ACR alternate spelling
                "betonline",
                "ignition",
                "bovada",
                "wsop",
                "replay poker",
                "global poker",
                "poker",
            ];

            // Case-insensitive matching for better site detection
            let title_lower = title_str.to_lowercase();
            let is_poker = poker_keywords.iter().any(|&kw| title_lower.contains(kw));

            if !is_poker {
                return BOOL(1);
            }

            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_ok() {
                let window = PokerWindow {
                    title: title_str,
                    x: rect.left,
                    y: rect.top,
                    width: (rect.right - rect.left) as u32,
                    height: (rect.bottom - rect.top) as u32,
                };

                if let Ok(mut vec) = windows.lock() {
                    vec.push(window);
                }
            }

            BOOL(1)
        }

        unsafe {
            let _ = EnumWindows(
                Some(enum_callback),
                LPARAM(&windows as *const _ as isize),
            );
        }

        Ok(windows.into_inner().unwrap())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(vec![])
    }
}

#[tauri::command]
pub async fn capture_poker_window(window_title: String) -> Result<CapturedGameState, String> {
    let windows = find_poker_windows().await?;
    let poker_window = windows.iter()
        .find(|w| w.title == window_title)
        .ok_or("Poker window not found")?;

    // Get DPI scale factor for coordinate conversion
    let scale_factor = get_dpi_scale_factor().unwrap_or(1.0);

    println!("📐 Window bounds (logical): x={}, y={}, w={}, h={}",
        poker_window.x, poker_window.y, poker_window.width, poker_window.height);

    // Convert logical window coordinates to physical screen coordinates
    let logical_coords = LogicalCoordinates {
        x: poker_window.x,
        y: poker_window.y,
        width: poker_window.width,
        height: poker_window.height,
    };
    let physical_coords = logical_to_physical(&logical_coords, scale_factor);

    println!("📐 Physical coords ({}x scale): x={}, y={}, w={}, h={}",
        scale_factor, physical_coords.x, physical_coords.y,
        physical_coords.width, physical_coords.height);

    let screens = Screen::all().map_err(|e| format!("Failed to get screens: {}", e))?;
    let screen = screens.first().ok_or("No screens found")?;
    let full_image = screen.capture()
        .map_err(|e| format!("Failed to capture screen: {}", e))?;

    println!("📸 Full screenshot size: {}x{}", full_image.width(), full_image.height());

    let img_buffer = image::RgbaImage::from_raw(
        full_image.width(),
        full_image.height(),
        full_image.rgba().to_vec(),
    ).ok_or("Failed to create image buffer")?;

    let dynamic_img = image::DynamicImage::ImageRgba8(img_buffer);

    // Use physical coordinates for cropping
    let crop_x = physical_coords.x.min(dynamic_img.width().saturating_sub(1));
    let crop_y = physical_coords.y.min(dynamic_img.height().saturating_sub(1));
    let crop_width = physical_coords.width.min(dynamic_img.width() - crop_x);
    let crop_height = physical_coords.height.min(dynamic_img.height() - crop_y);

    println!("✂️  Cropping to: x={}, y={}, w={}, h={}", crop_x, crop_y, crop_width, crop_height);

    let cropped_img = dynamic_img.crop_imm(crop_x, crop_y, crop_width, crop_height);

    let mut png_bytes = Vec::new();
    cropped_img.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;

    let base64_image = general_purpose::STANDARD.encode(&png_bytes);

    Ok(CapturedGameState {
        image_base64: base64_image,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        window_title,
        ocr_text: None,
        cards_detected: vec![],
        pot_size: None,
        position: None,
    })
}

#[tauri::command]
pub async fn start_poker_monitoring(
    app: AppHandle,
    state: tauri::State<'_, MonitoringState>,
) -> Result<(), String> {
    {
        let mut is_running = state.is_running.lock().unwrap();
        if *is_running {
            return Ok(());
        }
        *is_running = true;
    }

    println!("Starting poker monitoring background task...");

    // Reset frame filter state for new monitoring session
    reset_frame_state();

    let is_running = Arc::clone(&state.is_running);
    let cancel_flag = Arc::clone(&state.cancel_requested);
    let app_clone = app.clone();

    tauri::async_runtime::spawn(async move {
        let mut capture_count = 0;

        while *is_running.lock().unwrap() {
            // Check for cancellation at start of each iteration
            if cancel_flag.load(Ordering::Relaxed) {
                println!("🚫 Monitoring loop detected cancellation - aborting");
                cancel_flag.store(false, Ordering::Relaxed); // Clear flag
                break;
            }

            capture_count += 1;
            println!("📸 Capture #{}: Taking screenshot...", capture_count);

            match find_poker_windows().await {
                Ok(windows) => {
                    if windows.is_empty() {
                        println!("⚠️  No poker windows found");
                    } else {
                        let window = &windows[0];
                        let site_name = detect_poker_site(&window.title);
                        println!("🎯 Found: {} ({})", site_name, window.title);
                        println!("⚡ Using optimization: 1280x720, o4-mini, 2.0% threshold");

                        match capture_poker_regions(window.title.clone(), Some(&app_clone), Some(&cancel_flag)).await {
                            Ok(parsed_data) => {
                                println!("✅ Analysis complete!");
                                println!("🃏 Your cards: {:?}", parsed_data.your_cards);
                                println!("🎴 Community: {:?}", parsed_data.community_cards);
                                if let Some(pot) = parsed_data.pot_size {
                                    println!("💰 Pot: ${}", pot);
                                }
                                if let Some(ref pos) = parsed_data.position {
                                    println!("📍 Position: {}", pos);
                                }

                                let _ = app_clone.emit("poker-capture", &parsed_data);
                            }
                            Err(e) => {
                                println!("❌ Capture error: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Window detection error: {}", e);
                }
            }

            sleep(Duration::from_secs(5)).await;
        }

        println!("🛑 Monitoring stopped");
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_poker_monitoring(
    state: tauri::State<'_, MonitoringState>,
) -> Result<(), String> {
    println!("Stopping poker monitoring...");
    let mut is_running = state.is_running.lock().unwrap();
    *is_running = false;

    // Print frame filtering statistics
    print_frame_statistics();

    // Clear previous state when stopping
    *PREVIOUS_STATE.lock().unwrap() = None;

    // Reset frame filter state
    reset_frame_state();

    Ok(())
}

#[tauri::command]
pub async fn cancel_capture(
    state: tauri::State<'_, MonitoringState>,
) -> Result<(), String> {
    println!("🚫 Cancel capture requested - setting cancel flag");
    state.cancel_requested.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub async fn capture_poker_region(
    _x: i32,
    _y: i32,
    _width: u32,
    _height: u32,
) -> Result<String, String> {
    let screens = Screen::all().map_err(|e| format!("Failed to get screens: {}", e))?;

    let screen = screens
        .first()
        .ok_or("No screens found")?;

    let image = screen
        .capture()
        .map_err(|e| format!("Failed to capture screen: {}", e))?;

    let png_bytes = image
        .to_png()
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;

    Ok(general_purpose::STANDARD.encode(&png_bytes))
}