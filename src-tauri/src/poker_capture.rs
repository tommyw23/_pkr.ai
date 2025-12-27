// src-tauri/src/poker_capture.rs
use screenshots::Screen;
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tokio::time::sleep;
use tauri::{AppHandle, Emitter, Manager};
use once_cell::sync::Lazy;
use xcap::Monitor;
use crate::screen_capture::{get_dpi_scale_factor, logical_to_physical, LogicalCoordinates};
use crate::vision::{
    should_process_frame, reset_frame_state, print_frame_statistics,
    analyze_with_openai, FrameFilterConfig,
    preprocess_for_vision_api, PreprocessConfig
};
use crate::calibration::{CalibrationData, CalibrationRegion, MonitorInfo};

/// Fullscreen capture mode: bypasses window detection and captures entire primary monitor
/// Set to true to work around window bounds issues (-32000, -32000)
const FULLSCREEN_MODE: bool = true;

// Global state tracking for cascade inference
static PREVIOUS_STATE: Lazy<Mutex<Option<crate::vision::openai_o4mini::RawVisionData>>> =
    Lazy::new(|| Mutex::new(None));

// ============================================
// GENERATIONAL STATE MANAGEMENT
// ============================================

/// Global generation counter - incremented when significant visual changes detected
/// Used to discard stale API responses when table state has changed
static CURRENT_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Tracks the last significant visual state for change detection
static LAST_VISUAL_STATE: Lazy<Mutex<Option<SignificantTableState>>> =
    Lazy::new(|| Mutex::new(None));

/// Minimum time between generation increments (debounce)
const MIN_GENERATION_INCREMENT_MS: u64 = 500;

/// Last time generation was incremented
static LAST_GENERATION_INCREMENT: Lazy<Mutex<std::time::Instant>> =
    Lazy::new(|| Mutex::new(std::time::Instant::now()));

/// Pixel-based visual state for fast change detection (no OCR/LLM)
#[derive(Debug, Clone)]
pub struct SignificantTableState {
    /// Hash of pixels in pot/center table area
    pub pot_region_hash: u64,
    /// Hash of pixels in community card area
    pub board_region_hash: u64,
    /// Hash of pixels in action button area
    pub buttons_region_hash: u64,
    /// Timestamp when this state was captured
    pub captured_at: std::time::Instant,
}

/// Event emitted when generation changes
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GenerationChangeEvent {
    pub old_generation: u64,
    pub new_generation: u64,
    pub reason: String,
    pub timestamp_ms: u64,
}

// Generation management functions

/// Get the current generation ID
pub fn get_current_generation() -> u64 {
    CURRENT_GENERATION.load(Ordering::SeqCst)
}

/// Increment generation and return the new value (with debouncing)
pub fn increment_generation(reason: &str) -> Option<u64> {
    let now = std::time::Instant::now();

    // Check debounce
    {
        let last = LAST_GENERATION_INCREMENT.lock().unwrap();
        if now.duration_since(*last).as_millis() < MIN_GENERATION_INCREMENT_MS as u128 {
            return None;
        }
    }

    // Update last increment time
    {
        let mut last = LAST_GENERATION_INCREMENT.lock().unwrap();
        *last = now;
    }

    let new_gen = CURRENT_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    Some(new_gen)
}

/// Check if a request's generation is still valid
pub fn is_generation_valid(request_generation: u64) -> bool {
    let current = CURRENT_GENERATION.load(Ordering::SeqCst);
    request_generation == current
}

/// Reset generation counter (called when stopping monitoring)
pub fn reset_generation() {
    CURRENT_GENERATION.store(0, Ordering::SeqCst);
    *LAST_VISUAL_STATE.lock().unwrap() = None;
}

/// Simple hash function for pixel data (fast, not cryptographic)
fn hash_pixels(data: &[u8]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    // Sample every 4th pixel for speed (still captures changes)
    for (i, byte) in data.iter().enumerate() {
        if i % 4 == 0 {
            byte.hash(&mut hasher);
        }
    }
    hasher.finish()
}

/// Calculate Mean Squared Error between two hashes (normalized 0.0-1.0)
/// Returns the relative difference as a percentage
fn hash_difference_ratio(hash1: u64, hash2: u64) -> f64 {
    if hash1 == hash2 {
        return 0.0;
    }
    // XOR the hashes and count differing bits
    let diff = hash1 ^ hash2;
    let diff_bits = diff.count_ones() as f64;
    // Normalize to 0.0-1.0 (64 bits max)
    diff_bits / 64.0
}

/// Check if visual state has changed significantly (>threshold)
/// Uses pixel hashing - no OCR or LLM calls
pub fn is_significant_visual_change(
    current: &SignificantTableState,
    previous: &SignificantTableState,
    threshold: f64,
) -> Option<String> {
    let mut reasons = Vec::new();

    // Check pot region
    let pot_diff = hash_difference_ratio(current.pot_region_hash, previous.pot_region_hash);
    if pot_diff > threshold {
        reasons.push(format!("pot_change_{:.0}%", pot_diff * 100.0));
    }

    // Check board/community cards region
    let board_diff = hash_difference_ratio(current.board_region_hash, previous.board_region_hash);
    if board_diff > threshold {
        reasons.push(format!("board_change_{:.0}%", board_diff * 100.0));
    }

    // Check action buttons region
    let buttons_diff = hash_difference_ratio(current.buttons_region_hash, previous.buttons_region_hash);
    if buttons_diff > threshold {
        reasons.push(format!("buttons_change_{:.0}%", buttons_diff * 100.0));
    }

    if reasons.is_empty() {
        None
    } else {
        Some(reasons.join(", "))
    }
}

/// Capture visual state from calibrated regions (fast, pixel-based)
pub fn capture_visual_state_from_image(
    img: &image::DynamicImage,
    region: &CalibrationRegion,
) -> SignificantTableState {
    let img_width = img.width() as i32;
    let img_height = img.height() as i32;

    // Define sub-regions within the calibrated poker table
    // These are relative positions within the captured region
    // Pot region: center-top (where pot amount is displayed)
    let pot_region = extract_subregion_hash(img, 0.35, 0.25, 0.30, 0.10);

    // Board region: center (where community cards are)
    let board_region = extract_subregion_hash(img, 0.20, 0.35, 0.60, 0.15);

    // Buttons region: bottom-center (where action buttons are)
    let buttons_region = extract_subregion_hash(img, 0.25, 0.75, 0.50, 0.15);

    SignificantTableState {
        pot_region_hash: pot_region,
        board_region_hash: board_region,
        buttons_region_hash: buttons_region,
        captured_at: std::time::Instant::now(),
    }
}

/// Extract a sub-region from image and compute hash
/// x, y, width, height are relative (0.0-1.0)
fn extract_subregion_hash(
    img: &image::DynamicImage,
    rel_x: f64,
    rel_y: f64,
    rel_width: f64,
    rel_height: f64,
) -> u64 {
    let img_width = img.width();
    let img_height = img.height();

    let x = (rel_x * img_width as f64) as u32;
    let y = (rel_y * img_height as f64) as u32;
    let w = (rel_width * img_width as f64) as u32;
    let h = (rel_height * img_height as f64) as u32;

    // Clamp to image bounds
    let x = x.min(img_width.saturating_sub(1));
    let y = y.min(img_height.saturating_sub(1));
    let w = w.min(img_width - x);
    let h = h.min(img_height - y);

    if w == 0 || h == 0 {
        return 0;
    }

    // Crop and hash
    let cropped = img.crop_imm(x, y, w, h);
    let bytes = cropped.to_rgb8().into_raw();
    hash_pixels(&bytes)
}

/// Emit generation change event to frontend
pub fn emit_generation_change(app: &AppHandle, old_gen: u64, new_gen: u64, reason: &str) {
    let event = GenerationChangeEvent {
        old_generation: old_gen,
        new_generation: new_gen,
        reason: reason.to_string(),
        timestamp_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
    };
    let _ = app.emit("generation-change", &event);
}

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

/// Load calibration data from the app's data directory
fn load_calibration_data(app: &AppHandle) -> Option<CalibrationData> {
    let app_data_dir = match app.path().app_data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Failed to get app data dir: {:?}", e);
            return None;
        }
    };

    let calibration_path = app_data_dir.join("calibration.json");

    if !calibration_path.exists() {
        return None;
    }

    let json = match std::fs::read_to_string(&calibration_path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Failed to read calibration file: {:?}", e);
            return None;
        }
    };

    let data: CalibrationData = match serde_json::from_str(&json) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to parse calibration JSON: {:?}", e);
            return None;
        }
    };

    if data.regions.is_empty() {
        return None;
    }

    Some(data)
}

/// Capture screenshot from the calibrated region
/// Returns the cropped image as a DynamicImage
fn capture_calibrated_region(
    region: &CalibrationRegion,
    saved_monitor: Option<&MonitorInfo>,
) -> Result<image::DynamicImage, String> {
    use image::GenericImageView;

    // Get all monitors
    let monitors = Monitor::all().map_err(|e| format!("Failed to get monitors: {}", e))?;

    // Find the correct monitor based on saved calibration data
    let target_monitor = if let Some(saved) = saved_monitor {
        monitors
            .iter()
            .find(|m| m.x() == saved.x && m.y() == saved.y)
            .or_else(|| {
                monitors.iter().find(|m| m.is_primary())
            })
            .ok_or("No matching monitor found")?
    } else {
        monitors
            .iter()
            .find(|m| m.is_primary())
            .ok_or("No primary monitor found")?
    };

    let full_screenshot = target_monitor
        .capture_image()
        .map_err(|e| format!("Failed to capture screen: {}", e))?;

    // Crop to the calibrated region (coordinates are already in physical pixels)
    let x = region.x as u32;
    let y = region.y as u32;
    let width = region.width as u32;
    let height = region.height as u32;

    // Validate bounds
    if x + width > full_screenshot.width() || y + height > full_screenshot.height() {
        return Err(format!(
            "Calibrated region ({},{} {}x{}) exceeds screen bounds ({}x{})",
            x, y, width, height, full_screenshot.width(), full_screenshot.height()
        ));
    }

    let cropped = full_screenshot.view(x, y, width, height).to_image();

    Ok(image::DynamicImage::ImageRgba8(cropped))
}

/// Process a capture from the calibrated region through the cascade vision pipeline
/// This uses the same OpenAI → Claude cascade as the window-based capture
pub async fn process_calibrated_capture(
    app: &AppHandle,
    cancel_flag: Option<&Arc<AtomicBool>>,
) -> Result<ParsedPokerData, String> {
    // Load calibration data
    let calibration = load_calibration_data(app)
        .ok_or("No calibration data found. Please calibrate first.")?;

    let region = &calibration.regions[0];

    // Check for cancellation
    if let Some(flag) = cancel_flag {
        if flag.load(Ordering::Relaxed) {
            return Err("Capture cancelled".to_string());
        }
    }

    let capture_start = std::time::Instant::now();
    let analysis_start = std::time::Instant::now();

    // Capture generation at the start of analysis
    let request_generation = get_current_generation();

    // Capture from calibrated region
    let screenshot_start = std::time::Instant::now();
    let window_img = capture_calibrated_region(region, calibration.monitor.as_ref())?;

    // Frame filtering
    let filter_start = std::time::Instant::now();
    let filter_config = FrameFilterConfig {
        min_diff_threshold: 0.02,
        min_green_ratio: 0.0,
        max_skip_duration_secs: 15,
        use_perceptual_hash: true,
    };
    let filter_result = should_process_frame(&window_img, &filter_config);

    if !filter_result.should_process {
        // Return previous state if available
        let prev_state_guard = PREVIOUS_STATE.lock().unwrap();
        if let Some(ref prev_raw_data) = *prev_state_guard {
            return build_parsed_data_from_raw(prev_raw_data, request_generation, analysis_start);
        } else {
            return Err("Frame filtered and no previous state available".to_string());
        }
    }

    // Image preprocessing
    let preprocess_start = std::time::Instant::now();
    let preprocess_config = PreprocessConfig::for_site(Some("unknown"));
    let final_img = preprocess_for_vision_api(&window_img, &preprocess_config);

    // Convert to PNG bytes
    let mut png_bytes = Vec::new();
    final_img.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;

    let size_kb = png_bytes.len() as f32 / 1024.0;

    // OpenAI o4-mini (Step 1)
    let openai_start = std::time::Instant::now();
    let openai_result = match analyze_with_openai(&png_bytes, Some("unknown")).await {
        Ok(result) => Some(result),
        Err(e) => {
            None
        }
    };

    // Check for cancellation
    if let Some(flag) = cancel_flag {
        if flag.load(Ordering::Relaxed) {
            return Err("Capture cancelled after API call".to_string());
        }
    }

    // Validate and fallback to Claude if needed (Step 2)
    let raw_data = if let Some(ref data) = openai_result {
        let issues = crate::vision::openai_o4mini::validate_vision_response(data);

        if issues.is_empty() {
            data.clone()
        } else {
            let claude_start = std::time::Instant::now();
            let tier1_json = serde_json::to_string(data).unwrap_or_default();
            match crate::claude_vision::analyze_with_claude_raw(&png_bytes, &tier1_json, &issues).await {
                Ok(claude_data) => {
                    claude_data
                }
                Err(e) => {
                    data.clone() // Use OpenAI result anyway
                }
            }
        }
    } else {
        // OpenAI failed completely, try Claude directly
        let claude_start = std::time::Instant::now();
        match crate::claude_vision::analyze_with_claude_raw(&png_bytes, "", &["openai_unavailable".to_string()]).await {
            Ok(claude_data) => {
                claude_data
            }
            Err(e) => {
                return Err(format!("Both OpenAI and Claude failed: {}", e));
            }
        }
    };

    // Update previous state
    {
        let mut prev_state = PREVIOUS_STATE.lock().unwrap();
        *prev_state = Some(raw_data.clone());
    }

    // Check if generation is still valid before returning result
    if !is_generation_valid(request_generation) {
        let current_gen = get_current_generation();
    }

    build_parsed_data_from_raw(&raw_data, request_generation, analysis_start)
}

/// Build ParsedPokerData from RawVisionData with generation tracking
fn build_parsed_data_from_raw(
    raw_data: &crate::vision::openai_o4mini::RawVisionData,
    generation_id: u64,
    analysis_start: std::time::Instant,
) -> Result<ParsedPokerData, String> {
    // Filter out null values from hero_cards
    let your_cards: Vec<String> = raw_data.hero_cards
        .iter()
        .filter_map(|opt| opt.clone())
        .collect();
    let community_cards: Vec<String> = raw_data.community_cards
        .iter()
        .filter_map(|opt| opt.clone())
        .collect();

    // Generate recommendation using Rust strategy
    let (recommendation, hand_eval, win_pct, tie_pct, street) = match parse_and_validate_cards(raw_data) {
        Some((hero_cards, community_cards_parsed)) => {
            let (legal_actions, call_amount) = parse_legal_actions(
                &Some(raw_data.available_actions.clone()),
                Some(raw_data.amount_to_call),
                None,
            );

            let (rec, eval) = generate_rust_recommendation(
                &hero_cards,
                &community_cards_parsed,
                raw_data.pot,
                raw_data.position.as_deref(),
                call_amount,
                &legal_actions,
            );

            let (win_pct, tie_pct) = crate::poker::calculate_win_tie_percentages(
                &hero_cards,
                &community_cards_parsed,
                1000,
            );

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
                    reasoning: "Unable to detect cards".to_string(),
                },
                default_eval,
                0.0,
                0.0,
                "unknown".to_string(),
            )
        }
    };

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
        generation_id,
        analysis_duration_ms: analysis_start.elapsed().as_millis() as u64,
    })
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
    // Filter out null hero cards first
    let hero_card_strings: Vec<&String> = raw_data.hero_cards
        .iter()
        .filter_map(|opt| opt.as_ref())
        .collect();

    // Validate hero cards
    if hero_card_strings.is_empty() {
        return None;
    }

    if hero_card_strings.len() != 2 {
        return None;
    }

    // Parse hero cards
    let mut hero_cards = Vec::new();
    for card_str in &hero_card_strings {
        match parse_card_string(card_str) {
            Some(card) => hero_cards.push(card),
            None => {
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

    // STEP 2: Parse legal actions from AI's detected buttons
    let amount_to_call = call_amount.unwrap_or(0.0);
    let legal_actions = crate::poker::parse_legal_actions(available_actions, amount_to_call);

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
    // Generation tracking for stale result detection
    pub generation_id: u64,        // Generation when analysis started
    pub analysis_duration_ms: u64, // How long the analysis took
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

/// Check if two card sets match (accounting for different representations and null values)
fn cards_match(cards1: &[Option<String>], cards2: &[Option<String>]) -> bool {
    // Filter out None values and compare
    let filtered1: Vec<&String> = cards1.iter().filter_map(|c| c.as_ref()).collect();
    let filtered2: Vec<&String> = cards2.iter().filter_map(|c| c.as_ref()).collect();

    if filtered1.len() != filtered2.len() {
        return false;
    }

    let set1: std::collections::HashSet<String> = filtered1.iter()
        .map(|c| normalize_card_for_comparison(c))
        .collect();
    let set2: std::collections::HashSet<String> = filtered2.iter()
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
        return Ok(());
    }

    // Rule 1: Hero cards cannot change mid-hand
    // If both frames have 2 hero cards (non-null) and pot didn't reset, cards must match
    let prev_hero_count = previous.hero_cards.iter().filter(|c| c.is_some()).count();
    let curr_hero_count = current.hero_cards.iter().filter(|c| c.is_some()).count();

    if prev_hero_count == 2 && curr_hero_count == 2 {
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
        return Ok(openai_data.clone());
    }

    let claude_start = std::time::Instant::now();

    // Call Claude for verification
    let openai_json = serde_json::to_string(openai_data).unwrap_or_default();
    let issues = vec!["new_hand_verification".to_string()];

    match crate::claude_vision::analyze_with_claude_raw(png_bytes, &openai_json, &issues).await {
        Ok(claude_data) => {
            // Compare hero cards between OpenAI and Claude (filter out nulls)
            let openai_normalized: std::collections::HashSet<String> = openai_data.hero_cards.iter()
                .filter_map(|c| c.as_ref())
                .map(|c| normalize_card_for_comparison(c))
                .collect();
            let claude_normalized: std::collections::HashSet<String> = claude_data.hero_cards.iter()
                .filter_map(|c| c.as_ref())
                .map(|c| normalize_card_for_comparison(c))
                .collect();

            if openai_normalized != claude_normalized {
                // Return Claude data (trust Claude for suit accuracy)
                Ok(claude_data)
            } else {
                Ok(openai_data.clone())
            }
        }
        Err(e) => {
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

    let claude_start = std::time::Instant::now();

    // Call Claude for verification with community_card_verification issue
    let openai_json = serde_json::to_string(openai_data).unwrap_or_default();
    let issues = vec![format!("community_card_verification:{}", transition)];

    match crate::claude_vision::analyze_with_claude_raw(png_bytes, &openai_json, &issues).await {
        Ok(claude_data) => {
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
                // Return Claude data for community cards, but keep OpenAI's hero cards
                // (hero cards were already verified on new hand)
                let mut merged_data = openai_data.clone();
                merged_data.community_cards = claude_data.community_cards;
                Ok(merged_data)
            } else {
                Ok(openai_data.clone())
            }
        }
        Err(e) => {
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
    let claude_start = std::time::Instant::now();

    // Call Claude with duplicate_resolution issue
    let openai_json = serde_json::to_string(openai_data).unwrap_or_default();
    let issues = vec!["duplicate_resolution".to_string()];

    match crate::claude_vision::analyze_with_claude_raw(png_bytes, &openai_json, &issues).await {
        Ok(claude_data) => {
            // Check if Claude's result has no duplicates
            let claude_has_dupes = crate::vision::has_duplicate_cards(
                &claude_data.hero_cards,
                &claude_data.community_cards
            );

            if !claude_has_dupes {
                Ok(claude_data)
            } else {
                // Return Claude's result anyway, might be better than OpenAI's
                Ok(claude_data)
            }
        }
        Err(e) => {
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
            return Err("Capture cancelled".to_string());
        }
    }

    // ============================================
    // TIMING DIAGNOSTICS - START
    // ============================================
    let capture_start = std::time::Instant::now();
    let analysis_start = std::time::Instant::now();
    let request_generation = get_current_generation();

    // ============================================
    // FULLSCREEN MODE CHECK
    // ============================================
    let screenshot_start = std::time::Instant::now();
    let window_img = if FULLSCREEN_MODE {
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

                        // Find which screen contains the window center point
                        for (index, screen) in screens.iter().enumerate() {
                            let display = screen.display_info;
                            let screen_x = display.x;
                            let screen_y = display.y;
                            let screen_width = display.width;
                            let screen_height = display.height;

                            // Check if window position is within this screen's bounds
                            if window_x >= screen_x
                                && window_x < screen_x + screen_width as i32
                                && window_y >= screen_y
                                && window_y < screen_y + screen_height as i32
                            {
                                target_screen_index = index;
                                break;
                            }
                        }
                    }
                    Err(e) => {
                    }
                }
            }
        }

        let screen = screens.get(target_screen_index)
            .ok_or_else(|| format!("Screen {} not found", target_screen_index))?;

        let full_image = screen.capture()
            .map_err(|e| format!("Failed to capture screen: {}", e))?;

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

        // Convert logical window coordinates to physical screen coordinates
        let logical_coords = LogicalCoordinates {
            x: poker_window.x,
            y: poker_window.y,
            width: poker_window.width,
            height: poker_window.height,
        };
        let physical_coords = logical_to_physical(&logical_coords, scale_factor);

        let screens = Screen::all().map_err(|e| format!("Failed to get screens: {}", e))?;
        let screen = screens.first().ok_or("No screens found")?;
        let full_image = screen.capture()
            .map_err(|e| format!("Failed to capture screen: {}", e))?;

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

        dynamic_img.crop_imm(crop_x, crop_y, crop_width, crop_height)
    };

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

    if !filter_result.should_process {
        // Return previous state if available, or error if first frame was filtered
        let prev_state_guard = PREVIOUS_STATE.lock().unwrap();
        if let Some(ref prev_raw_data) = *prev_state_guard {
            // Filter out null values from hero_cards
            let your_cards: Vec<String> = prev_raw_data.hero_cards
                .iter()
                .filter_map(|opt| opt.clone())
                .collect();
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
                generation_id: request_generation,
                analysis_duration_ms: analysis_start.elapsed().as_millis() as u64,
            });
        } else {
            return Err("Frame filtered and no previous state available".to_string());
        }
    }

    // ============================================
    // YOLO PANEL DETECTION + CROPPING (DISABLED - TOO SLOW)
    // ============================================
    // Panel detection adds 15-20 seconds per capture and often fails
    // Using full captured image instead for much faster processing
    let panel_start = std::time::Instant::now();
    let panel_img = window_img.clone();

    // ============================================
    // SITE-SPECIFIC CONFIGURATION
    // ============================================
    let detected_site = detect_poker_site(&window_title);
    let normalized_site = normalize_site_name(detected_site);

    // ============================================
    // IMAGE PREPROCESSING FOR VISION API
    // ============================================
    let preprocess_start = std::time::Instant::now();
    let preprocess_config = PreprocessConfig::for_site(Some(normalized_site));
    let final_img = preprocess_for_vision_api(&panel_img, &preprocess_config);
    // ============================================

    // Convert to PNG bytes
    let mut png_bytes = Vec::new();
    final_img.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;

    let size_kb = png_bytes.len() as f32 / 1024.0;

    // STEP 1: Try OpenAI o4-mini first (cheap and fast)
    let openai_start = std::time::Instant::now();
    let openai_result = match analyze_with_openai(&png_bytes, Some(normalized_site)).await {
        Ok(result) => Some(result),
        Err(e) => {
            if e.contains("429") || e.contains("RATE_LIMIT") {
                None
            } else {
                None
            }
        }
    };

    // Check for cancellation after OpenAI API call
    if let Some(flag) = cancel_flag {
        if flag.load(Ordering::Relaxed) {
            return Err("Capture cancelled".to_string());
        }
    }

    // STEP 2: Validate OpenAI result and fallback to Claude if needed
    let raw_data = if let Some(ref data) = openai_result {
        let issues = crate::vision::openai_o4mini::validate_vision_response(data);

        if issues.is_empty() {
            data.clone()
        } else {
            // Try Claude fallback
            let claude_start = std::time::Instant::now();
            let tier1_json = serde_json::to_string(data).unwrap_or_default();
            match crate::claude_vision::analyze_with_claude_raw(
                &png_bytes,
                &tier1_json,
                &issues,
            ).await {
                Ok(claude_data) => {
                    claude_data
                }
                Err(e) => {
                    // Return OpenAI data anyway, let downstream handle it
                    data.clone()
                }
            }
        }
    } else {
        // OpenAI completely failed, try Claude directly
        let claude_start = std::time::Instant::now();
        match crate::claude_vision::analyze_with_claude_raw(
            &png_bytes,
            "{}",
            &["openai_unavailable".to_string()],
        ).await {
            Ok(claude_data) => {
                claude_data
            }
            Err(e) => {
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
            raw_data
        } else if let Some(ref prev) = prev_clone {
            // SAME HAND: Apply temporal consistency check (FREE - runs in Rust)
            match validate_temporal_consistency(&raw_data, prev) {
                Ok(()) => {
                    raw_data
                }
                Err(reason) => {
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
            match resolve_duplicate_cards_with_claude(&png_bytes, &raw_data, normalized_site).await {
                Ok(resolved_data) => resolved_data,
                Err(e) => {
                    raw_data
                }
            }
        } else {
            raw_data
        }
    };

    // Validate and log position detection
    if let Some(ref pos) = raw_data.position {
        let valid_positions = vec!["BTN", "SB", "BB", "UTG", "MP", "CO", "HJ"];
        if valid_positions.contains(&pos.as_str()) {
        } else {
        }
    }

    // ============================================
    // RUST STRATEGY RECOMMENDATION (PRIMARY PATH)
    // ============================================
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

    // Save current state for next iteration
    *PREVIOUS_STATE.lock().unwrap() = Some(raw_data.clone());

    // Display format uses the raw string cards from vision API (filter out nulls)
    let your_cards: Vec<String> = raw_data.hero_cards
        .iter()
        .filter_map(|opt| opt.clone())
        .collect();
    let community_cards: Vec<String> = raw_data.community_cards
        .iter()
        .filter_map(|opt| opt.clone())
        .collect();

    // ============================================
    // TIMING DIAGNOSTICS - END
    // ============================================
    let total_time = capture_start.elapsed().as_secs_f64();

    // Check if generation is still valid before returning result
    if !is_generation_valid(request_generation) {
        let current_gen = get_current_generation();
    }

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
        generation_id: request_generation,
        analysis_duration_ms: analysis_start.elapsed().as_millis() as u64,
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

    // Convert logical window coordinates to physical screen coordinates
    let logical_coords = LogicalCoordinates {
        x: poker_window.x,
        y: poker_window.y,
        width: poker_window.width,
        height: poker_window.height,
    };
    let physical_coords = logical_to_physical(&logical_coords, scale_factor);

    let screens = Screen::all().map_err(|e| format!("Failed to get screens: {}", e))?;
    let screen = screens.first().ok_or("No screens found")?;
    let full_image = screen.capture()
        .map_err(|e| format!("Failed to capture screen: {}", e))?;

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

    // Reset frame filter state for new monitoring session
    reset_frame_state();

    let is_running = Arc::clone(&state.is_running);
    let cancel_flag = Arc::clone(&state.cancel_requested);
    let app_clone = app.clone();

    // Check if calibration is available
    let has_calibration = load_calibration_data(&app).is_some();

    tauri::async_runtime::spawn(async move {
        let mut capture_count = 0;

        while *is_running.lock().unwrap() {
            // Check for cancellation at start of each iteration
            if cancel_flag.load(Ordering::Relaxed) {
                cancel_flag.store(false, Ordering::Relaxed); // Clear flag
                break;
            }

            capture_count += 1;

            // Use calibrated capture if available, otherwise fall back to window detection
            if has_calibration {
                // Emit analysis-started event before API call
                let _ = app_clone.emit("analysis-started", ());

                match process_calibrated_capture(&app_clone, Some(&cancel_flag)).await {
                    Ok(parsed_data) => {
                        let _ = app_clone.emit("poker-capture", &parsed_data);
                    }
                    Err(e) => {
                    }
                }
            } else {
                // Fallback: window detection mode
                match find_poker_windows().await {
                    Ok(windows) => {
                        if windows.is_empty() {
                        } else {
                            let window = &windows[0];
                            let site_name = detect_poker_site(&window.title);

                            // Emit analysis-started event before API call
                            let _ = app_clone.emit("analysis-started", ());

                            match capture_poker_regions(window.title.clone(), Some(&app_clone), Some(&cancel_flag)).await {
                                Ok(parsed_data) => {
                                    let _ = app_clone.emit("poker-capture", &parsed_data);
                                }
                                Err(e) => {
                                }
                            }
                        }
                    }
                    Err(e) => {
                    }
                }
            }

            sleep(Duration::from_secs(5)).await;
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_poker_monitoring(
    state: tauri::State<'_, MonitoringState>,
) -> Result<(), String> {
    let mut is_running = state.is_running.lock().unwrap();
    *is_running = false;

    // Print frame filtering statistics
    print_frame_statistics();

    // Clear previous state when stopping
    *PREVIOUS_STATE.lock().unwrap() = None;

    // Reset generation counter
    reset_generation();

    // Reset frame filter state
    reset_frame_state();

    Ok(())
}

#[tauri::command]
pub async fn cancel_capture(
    state: tauri::State<'_, MonitoringState>,
) -> Result<(), String> {
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
