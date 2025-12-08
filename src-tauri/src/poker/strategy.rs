// src-tauri/src/poker/strategy.rs
// GTO-based poker strategy engine for hand evaluation and action recommendations
use serde::{Deserialize, Serialize};
use crate::poker_types::{PokerState, Card, Rank, Suit, LegalAction};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Action {
    Fold,
    Check,
    Call,
    Bet(f64),
    Raise(f64),
    NoRecommendation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendedAction {
    pub action: Action,
    pub reasoning: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HandRanking {
    HighCard = 0,
    OnePair = 1,
    TwoPair = 2,
    ThreeOfAKind = 3,
    Straight = 4,
    Flush = 5,
    FullHouse = 6,
    FourOfAKind = 7,
    StraightFlush = 8,
}

#[derive(Debug, Clone)]
pub struct HandStrength {
    pub ranking: HandRanking,
    pub kickers: Vec<Rank>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HandCategory {
    HighCard = 0,
    OnePair = 1,
    TwoPair = 2,
    ThreeOfAKind = 3,
    Straight = 4,
    Flush = 5,
    FullHouse = 6,
    FourOfAKind = 7,
    StraightFlush = 8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawType {
    None,
    FlushDraw,
    Oesd,
    Gutshot,
    ComboDraw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Street {
    Preflop,
    Flop,
    Turn,
    River,
}

/// Board texture classification for GTO c-bet strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardTexture {
    Dry,        // Disconnected, rainbow (A-9-2r) → high freq, small size
    SemiWet,    // Some connectivity or 2-flush
    Wet,        // Connected + flush draw (8-7-5 two-tone) → low freq, large size
    Monotone,   // 3+ same suit
}

#[derive(Debug, Clone)]
pub struct HandEvaluation {
    pub category: HandCategory,
    pub description: String,
    pub strength_score: u32,
    pub kickers: Vec<Rank>,
    pub draw_type: DrawType,
    pub outs: u32,
}

#[derive(Debug, PartialEq, Eq)]
enum PairSource {
    Pocket,
    HoleMatched,
    Board,
}

// =============================================================================
// PARSING & UTILITY
// =============================================================================

pub fn parse_legal_actions(ai_actions: &[String], amount_to_call: f64) -> Vec<LegalAction> {
    let mut legal_actions = Vec::new();
    for action_str in ai_actions {
        let normalized = action_str.to_uppercase().trim().to_string();
        if normalized.starts_with("FOLD") {
            legal_actions.push(LegalAction::Fold);
        } else if normalized.starts_with("CHECK") {
            legal_actions.push(LegalAction::Check);
        } else if normalized.starts_with("CALL") {
            legal_actions.push(LegalAction::Call(amount_to_call));
        } else if normalized.starts_with("BET") {
            legal_actions.push(LegalAction::Bet);
        } else if normalized.starts_with("RAISE") || normalized.contains("ALL-IN") || normalized.contains("ALL IN") {
            legal_actions.push(LegalAction::Raise);
        }
    }
    legal_actions
}

pub fn rank_value(rank: Rank) -> u8 {
    match rank {
        Rank::Two => 2, Rank::Three => 3, Rank::Four => 4, Rank::Five => 5,
        Rank::Six => 6, Rank::Seven => 7, Rank::Eight => 8, Rank::Nine => 9,
        Rank::Ten => 10, Rank::Jack => 11, Rank::Queen => 12, Rank::King => 13,
        Rank::Ace => 14,
    }
}

fn rank_name(rank: Rank) -> &'static str {
    match rank {
        Rank::Ace => "aces", Rank::King => "kings", Rank::Queen => "queens",
        Rank::Jack => "jacks", Rank::Ten => "tens", Rank::Nine => "nines",
        Rank::Eight => "eights", Rank::Seven => "sevens", Rank::Six => "sixes",
        Rank::Five => "fives", Rank::Four => "fours", Rank::Three => "threes",
        Rank::Two => "twos",
    }
}

fn rank_name_singular(rank: Rank) -> &'static str {
    match rank {
        Rank::Ace => "ace", Rank::King => "king", Rank::Queen => "queen",
        Rank::Jack => "jack", Rank::Ten => "ten", Rank::Nine => "nine",
        Rank::Eight => "eight", Rank::Seven => "seven", Rank::Six => "six",
        Rank::Five => "five", Rank::Four => "four", Rank::Three => "three",
        Rank::Two => "two",
    }
}

fn get_street(board_count: usize) -> Street {
    match board_count {
        0 => Street::Preflop,
        3 => Street::Flop,
        4 => Street::Turn,
        5 => Street::River,
        _ => Street::Preflop,
    }
}

// =============================================================================
// HAND EVALUATION (unchanged logic)
// =============================================================================

pub fn evaluate_hand_strength(hole_cards: &[Card], community_cards: &[Card]) -> HandStrength {
    let mut all_cards = Vec::new();
    all_cards.extend_from_slice(hole_cards);
    all_cards.extend_from_slice(community_cards);

    if all_cards.is_empty() {
        return HandStrength { ranking: HandRanking::HighCard, kickers: vec![] };
    }

    let mut rank_counts: HashMap<Rank, usize> = HashMap::new();
    for card in &all_cards {
        *rank_counts.entry(card.rank).or_insert(0) += 1;
    }

    let mut suit_counts: HashMap<Suit, usize> = HashMap::new();
    for card in &all_cards {
        *suit_counts.entry(card.suit).or_insert(0) += 1;
    }

    let flush_suit = suit_counts.iter()
        .find(|(_, &count)| count >= 5)
        .map(|(suit, _)| *suit);

    let mut unique_ranks: Vec<Rank> = rank_counts.keys().copied().collect();
    unique_ranks.sort_by(|a, b| b.cmp(a));

    let (has_straight, straight_high_rank) = check_straight(&unique_ranks);

    // Straight flush check
    if let Some(suit) = flush_suit {
        let flush_cards: Vec<Rank> = all_cards.iter()
            .filter(|c| c.suit == suit)
            .map(|c| c.rank)
            .collect();
        let mut flush_ranks: Vec<Rank> = flush_cards.iter().copied().collect();
        flush_ranks.sort_by(|a, b| b.cmp(a));
        flush_ranks.dedup();
        let (has_sf, sf_high_rank) = check_straight(&flush_ranks);
        if has_sf {
            return HandStrength { ranking: HandRanking::StraightFlush, kickers: vec![sf_high_rank] };
        }
    }

    let mut counts: Vec<(Rank, usize)> = rank_counts.into_iter().collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)));

    if counts.is_empty() {
        return HandStrength { ranking: HandRanking::HighCard, kickers: vec![] };
    }

    // Four of a kind
    if counts[0].1 == 4 {
        let kickers = all_cards.iter()
            .filter(|c| c.rank != counts[0].0)
            .map(|c| c.rank)
            .max()
            .map_or(vec![counts[0].0], |k| vec![counts[0].0, k]);
        return HandStrength { ranking: HandRanking::FourOfAKind, kickers };
    }

    // Full house
    if counts.len() >= 2 && counts[0].1 == 3 && counts[1].1 >= 2 {
        return HandStrength { ranking: HandRanking::FullHouse, kickers: vec![counts[0].0, counts[1].0] };
    }

    // Flush
    if let Some(suit) = flush_suit {
        let mut flush_ranks: Vec<Rank> = all_cards.iter()
            .filter(|c| c.suit == suit)
            .map(|c| c.rank)
            .collect();
        flush_ranks.sort_by(|a, b| b.cmp(a));
        return HandStrength { ranking: HandRanking::Flush, kickers: flush_ranks.into_iter().take(5).collect() };
    }

    // Straight
    if has_straight {
        return HandStrength { ranking: HandRanking::Straight, kickers: vec![straight_high_rank] };
    }

    // Three of a kind
    if counts[0].1 == 3 {
        let mut kickers = all_cards.iter()
            .filter(|c| c.rank != counts[0].0)
            .map(|c| c.rank)
            .collect::<Vec<Rank>>();
        kickers.sort_by(|a, b| b.cmp(a));
        let mut final_kickers = vec![counts[0].0];
        final_kickers.extend(kickers.into_iter().take(2));
        return HandStrength { ranking: HandRanking::ThreeOfAKind, kickers: final_kickers };
    }

    // Two pair
    if counts.len() >= 2 && counts[0].1 == 2 && counts[1].1 == 2 {
        let kickers = all_cards.iter()
            .filter(|c| c.rank != counts[0].0 && c.rank != counts[1].0)
            .map(|c| c.rank)
            .max()
            .map_or(vec![counts[0].0, counts[1].0], |k| vec![counts[0].0, counts[1].0, k]);
        return HandStrength { ranking: HandRanking::TwoPair, kickers };
    }

    // One pair
    if counts[0].1 == 2 {
        let pair_rank = counts[0].0;
        let mut kickers = all_cards.iter()
            .filter(|c| c.rank != pair_rank)
            .map(|c| c.rank)
            .collect::<Vec<Rank>>();
        kickers.sort_by(|a, b| b.cmp(a));
        let mut final_kickers = vec![pair_rank];
        final_kickers.extend(kickers.into_iter().take(3));
        return HandStrength { ranking: HandRanking::OnePair, kickers: final_kickers };
    }

    // High card
    let mut kickers: Vec<Rank> = all_cards.iter().map(|c| c.rank).collect();
    kickers.sort_by(|a, b| b.cmp(a));
    HandStrength { ranking: HandRanking::HighCard, kickers: kickers.into_iter().take(5).collect() }
}

fn check_straight(ranks: &[Rank]) -> (bool, Rank) {
    if ranks.len() < 5 { return (false, Rank::Two); }

    // Wheel (A-2-3-4-5)
    if ranks.contains(&Rank::Ace) && ranks.contains(&Rank::Five) && ranks.contains(&Rank::Four)
        && ranks.contains(&Rank::Three) && ranks.contains(&Rank::Two) {
        return (true, Rank::Five);
    }

    for i in 0..=ranks.len().saturating_sub(5) {
        let mut is_straight = true;
        for j in 0..4 {
            let current = rank_value(ranks[i + j]);
            let next = rank_value(ranks[i + j + 1]);
            if current != next + 1 {
                is_straight = false;
                break;
            }
        }
        if is_straight { return (true, ranks[i]); }
    }
    (false, Rank::Two)
}

// =============================================================================
// BOARD TEXTURE ANALYSIS (GTO key concept)
// =============================================================================

/// Analyze board texture for c-bet strategy decisions
fn analyze_board_texture(board: &[Card]) -> BoardTexture {
    if board.len() < 3 { return BoardTexture::Dry; }

    // Count suits
    let mut suit_counts: HashMap<Suit, usize> = HashMap::new();
    for card in board {
        *suit_counts.entry(card.suit).or_insert(0) += 1;
    }
    let max_suit_count = *suit_counts.values().max().unwrap_or(&0);

    // Check connectivity
    let mut ranks: Vec<u8> = board.iter().map(|c| rank_value(c.rank)).collect();
    ranks.sort();
    ranks.dedup();

    let mut gaps = 0;
    let mut connected_count = 0;
    for i in 0..ranks.len().saturating_sub(1) {
        let diff = ranks[i + 1] - ranks[i];
        if diff == 1 { connected_count += 1; }
        else if diff == 2 { gaps += 1; } // One-gapper
    }

    // Monotone (3+ same suit)
    if max_suit_count >= 3 { return BoardTexture::Monotone; }

    // Wet: 2+ connected cards AND 2-flush, or 3+ connected
    let has_two_flush = max_suit_count >= 2;
    if connected_count >= 2 || (connected_count >= 1 && has_two_flush && gaps >= 1) {
        return BoardTexture::Wet;
    }

    // Semi-wet: Some connectivity or 2-flush
    if connected_count >= 1 || has_two_flush || gaps >= 2 {
        return BoardTexture::SemiWet;
    }

    BoardTexture::Dry
}

/// Check if we have range advantage on this board (simplified)
/// Returns true if the board favors the preflop aggressor (high cards)
fn has_range_advantage(board: &[Card], position: &str) -> bool {
    if board.is_empty() { return true; }

    let high_card_count = board.iter()
        .filter(|c| rank_value(c.rank) >= 10) // T+
        .count();

    let is_aggressor_position = matches!(
        position.to_lowercase().as_str(),
        "btn" | "button" | "co" | "cutoff" | "mp" | "hj" | "hijack" | "utg"
    );

    // High card boards favor preflop raiser
    is_aggressor_position && high_card_count >= 1
}

/// Check if we have nut advantage (can we have the absolute best hands?)
fn has_nut_advantage(hole_cards: &[Card], board: &[Card]) -> bool {
    // Simplified: Do we have top set, nut flush draw, or nut straight potential?
    if board.is_empty() { return false; }

    let board_high = board.iter().map(|c| rank_value(c.rank)).max().unwrap_or(0);
    let hero_has_top_pair_potential = hole_cards.iter().any(|c| rank_value(c.rank) == board_high);
    let hero_has_overpair = hole_cards.iter().all(|c| rank_value(c.rank) > board_high)
        && hole_cards[0].rank == hole_cards[1].rank;

    hero_has_top_pair_potential || hero_has_overpair
}

// =============================================================================
// DRAW DETECTION
// =============================================================================

fn detect_draws(hole_cards: &[Card], board_cards: &[Card]) -> (DrawType, u32) {
    if board_cards.len() < 3 || board_cards.len() > 4 { return (DrawType::None, 0); }

    let mut all_cards = Vec::new();
    all_cards.extend_from_slice(hole_cards);
    all_cards.extend_from_slice(board_cards);

    let mut suit_counts: HashMap<Suit, usize> = HashMap::new();
    let mut hole_suits: HashSet<Suit> = HashSet::new();

    for card in hole_cards { hole_suits.insert(card.suit); }
    for card in &all_cards { *suit_counts.entry(card.suit).or_insert(0) += 1; }

    // Flush draw (4 to flush with hole card contributing)
    let mut has_flush_draw = false;
    for (suit, count) in &suit_counts {
        if *count == 4 && hole_suits.contains(suit) {
            has_flush_draw = true;
            break;
        }
    }

    // Straight draw detection
    let mut ranks: Vec<Rank> = all_cards.iter().map(|c| c.rank).collect();
    ranks.sort_by(|a, b| rank_value(*b).cmp(&rank_value(*a)));
    ranks.dedup();

    let mut has_oesd = false;
    let mut has_gutshot = false;

    for i in 0..ranks.len().saturating_sub(3) {
        let r1 = rank_value(ranks[i]);
        let r2 = rank_value(ranks[i + 1]);
        let r3 = rank_value(ranks[i + 2]);
        let r4 = rank_value(ranks[i + 3]);

        if r1 == r2 + 1 && r2 == r3 + 1 && r3 == r4 + 1 {
            if r1 != 14 && r4 != 2 { // Not A-high or wheel
                has_oesd = true;
                break;
            }
        }
    }

    if !has_oesd {
        for i in 0..ranks.len().saturating_sub(3) {
            let r1 = rank_value(ranks[i]);
            let r4 = rank_value(ranks[i + 3]);
            if r1 - r4 == 4 {
                has_gutshot = true;
                break;
            }
        }
    }

    match (has_flush_draw, has_oesd, has_gutshot) {
        (true, true, _) => (DrawType::ComboDraw, 15),
        (true, false, true) => (DrawType::ComboDraw, 12),
        (true, false, false) => (DrawType::FlushDraw, 9),
        (false, true, _) => (DrawType::Oesd, 8),
        (false, false, true) => (DrawType::Gutshot, 4),
        _ => (DrawType::None, 0),
    }
}

fn draw_name(draw_type: DrawType) -> String {
    match draw_type {
        DrawType::FlushDraw => "flush draw".to_string(),
        DrawType::Oesd => "open-ended straight draw".to_string(),
        DrawType::Gutshot => "gutshot".to_string(),
        DrawType::ComboDraw => "combo draw".to_string(),
        DrawType::None => "no draw".to_string(),
    }
}

// =============================================================================
// HAND EVALUATION WITH CONTEXT
// =============================================================================

fn identify_pair_source(pair_rank: Rank, hole: &[Card], board: &[Card]) -> PairSource {
    let hole_count = hole.iter().filter(|c| c.rank == pair_rank).count();
    let board_count = board.iter().filter(|c| c.rank == pair_rank).count();

    if hole_count == 2 { PairSource::Pocket }
    else if hole_count == 1 && board_count >= 1 { PairSource::HoleMatched }
    else { PairSource::Board }
}

fn is_playing_the_board(best_hand_ranks: &[Rank], _hole_cards: &[Card], board_cards: &[Card]) -> bool {
    if board_cards.len() < 5 { return false; }

    let mut board_counts: HashMap<Rank, usize> = HashMap::new();
    for card in board_cards {
        *board_counts.entry(card.rank).or_insert(0) += 1;
    }

    for &rank in best_hand_ranks {
        let count = board_counts.entry(rank).or_insert(0);
        if *count > 0 { *count -= 1; }
        else { return false; }
    }
    true
}

fn classify_pair_relative_to_board(pair_rank: Rank, board_cards: &[Card]) -> &'static str {
    let pair_value = rank_value(pair_rank);
    let mut higher_board_ranks = HashSet::new();
    for card in board_cards {
        if rank_value(card.rank) > pair_value {
            higher_board_ranks.insert(card.rank);
        }
    }

    match higher_board_ranks.len() {
        0 => {
            let max_board = board_cards.iter().map(|c| rank_value(c.rank)).max().unwrap_or(0);
            if pair_value > max_board { "overpair" } else { "top pair" }
        },
        1 => "second pair",
        2 => "third pair",
        _ => "bottom pair",
    }
}

pub fn evaluate_hand(hole_cards: &[Card], board_cards: &[Card]) -> HandEvaluation {
    if board_cards.is_empty() && hole_cards.len() == 2 {
        return evaluate_preflop_hand(hole_cards);
    }

    let strength = evaluate_hand_strength(hole_cards, board_cards);
    let (draw_type, outs) = detect_draws(hole_cards, board_cards);

    let category = match strength.ranking {
        HandRanking::HighCard => HandCategory::HighCard,
        HandRanking::OnePair => HandCategory::OnePair,
        HandRanking::TwoPair => HandCategory::TwoPair,
        HandRanking::ThreeOfAKind => HandCategory::ThreeOfAKind,
        HandRanking::Straight => HandCategory::Straight,
        HandRanking::Flush => HandCategory::Flush,
        HandRanking::FullHouse => HandCategory::FullHouse,
        HandRanking::FourOfAKind => HandCategory::FourOfAKind,
        HandRanking::StraightFlush => HandCategory::StraightFlush,
    };

    let (strength_score, description) = match category {
        HandCategory::HighCard => {
            let high_card_rank = strength.kickers.first().cloned().unwrap_or(Rank::Two);
            let score = 20 + (rank_value(high_card_rank) as u32 * 9 / 14);
            let desc = if draw_type != DrawType::None {
                draw_name(draw_type)
            } else {
                format!("{} high", rank_name_singular(high_card_rank))
            };
            (score, desc)
        },
        HandCategory::OnePair => {
            let pair_rank = strength.kickers.first().cloned().unwrap_or(Rank::Two);
            let pair_value = rank_value(pair_rank) as u32;
            let pair_source = identify_pair_source(pair_rank, hole_cards, board_cards);

            match pair_source {
                PairSource::Pocket => {
                    let is_overpair = classify_pair_relative_to_board(pair_rank, board_cards) == "overpair";
                    let score = if is_overpair { 60 + (pair_value * 14 / 14) } else { 45 + (pair_value * 14 / 14) };
                    let desc = if is_overpair {
                        format!("overpair, {}", rank_name(pair_rank))
                    } else {
                        format!("pocket pair, {}", rank_name(pair_rank))
                    };
                    (score, desc)
                },
                PairSource::HoleMatched => {
                    let classification = classify_pair_relative_to_board(pair_rank, board_cards);
                    let (base_score, desc) = match classification {
                        "top pair" => (55, format!("top pair, {}", rank_name(pair_rank))),
                        "second pair" => (42, format!("second pair, {}", rank_name(pair_rank))),
                        _ => (32, format!("bottom pair, {}", rank_name(pair_rank))),
                    };
                    let kicker = strength.kickers.get(1).cloned().unwrap_or(Rank::Two);
                    let kicker_boost = if rank_value(kicker) >= 12 { 5 } else if rank_value(kicker) >= 10 { 3 } else { 0 };
                    (base_score + kicker_boost, desc)
                },
                PairSource::Board => {
                    let best_kicker = strength.kickers.get(1).cloned().unwrap_or(Rank::Two);
                    let playing_the_board = is_playing_the_board(&strength.kickers, hole_cards, board_cards);
                    if playing_the_board {
                        (20, "playing the board".to_string())
                    } else {
                        (38, format!("board pair with {} kicker", rank_name_singular(best_kicker)))
                    }
                }
            }
        },
        HandCategory::TwoPair => {
            let playing_board = is_playing_the_board(&strength.kickers, hole_cards, board_cards);
            if playing_board { (28, "playing the board (two pair)".to_string()) }
            else { (68, "two pair".to_string()) }
        },
        HandCategory::ThreeOfAKind => {
            let rank = strength.kickers[0];
            let hole_count = hole_cards.iter().filter(|c| c.rank == rank).count();
            match hole_count {
                2 => (88, format!("set of {}", rank_name(rank))),
                1 => (82, format!("trips, {}", rank_name(rank))),
                _ => {
                    let playing_board = is_playing_the_board(&strength.kickers, hole_cards, board_cards);
                    if playing_board { (35, "playing the board (trips)".to_string()) }
                    else {
                        let kicker = strength.kickers.get(1).cloned().unwrap_or(Rank::Two);
                        (65, format!("board trips with {} kicker", rank_name_singular(kicker)))
                    }
                }
            }
        },
        HandCategory::Straight => (90, "straight".to_string()),
        HandCategory::Flush => (92, "flush".to_string()),
        HandCategory::FullHouse => (96, "full house".to_string()),
        HandCategory::FourOfAKind => (98, "four of a kind".to_string()),
        HandCategory::StraightFlush => (100, "straight flush".to_string()),
    };

    HandEvaluation { category, description, strength_score, kickers: strength.kickers, draw_type, outs }
}

// =============================================================================
// PREFLOP HAND EVALUATION (GTO ranges)
// =============================================================================

fn evaluate_preflop_hand(hole_cards: &[Card]) -> HandEvaluation {
    let card1 = &hole_cards[0];
    let card2 = &hole_cards[1];
    let rank1 = card1.rank;
    let rank2 = card2.rank;
    let is_pair = rank1 == rank2;
    let is_suited = card1.suit == card2.suit;
    let high_rank = rank1.max(rank2);
    let low_rank = rank1.min(rank2);
    let high_value = rank_value(high_rank);
    let low_value = rank_value(low_rank);
    let gap = high_value - low_value;

    let (category, description, strength_score) = if is_pair {
        // Pocket pairs: 22=40, AA=100
        let score = 40 + ((high_value as u32 - 2) * 60 / 12);
        (HandCategory::OnePair, format!("pocket {}", rank_name(rank1)), score)
    } else if is_suited {
        if high_rank == Rank::Ace {
            // Suited aces: A2s=60, AKs=95
            let score = 60 + (low_value as u32 * 3);
            (HandCategory::HighCard, format!("{}{} suited", rank_name_singular(high_rank), rank_name_singular(low_rank)), score)
        } else if high_value >= 10 && low_value >= 10 {
            // Suited broadways: KQs, QJs, JTs
            let score = 70 + (high_value as u32 - 10) * 3;
            (HandCategory::HighCard, format!("{}{} suited broadway", rank_name_singular(high_rank), rank_name_singular(low_rank)), score)
        } else if gap <= 1 {
            // Suited connectors: 76s=55, T9s=62
            let score = 45 + high_value as u32;
            (HandCategory::HighCard, format!("{}{} suited connector", rank_name_singular(high_rank), rank_name_singular(low_rank)), score)
        } else if gap <= 2 {
            // Suited one-gappers: 86s, 97s
            let score = 40 + high_value as u32;
            (HandCategory::HighCard, format!("{}{} suited gapper", rank_name_singular(high_rank), rank_name_singular(low_rank)), score)
        } else {
            // Suited trash
            let score = 30 + high_value as u32;
            (HandCategory::HighCard, format!("{}{} suited", rank_name_singular(high_rank), rank_name_singular(low_rank)), score)
        }
    } else {
        // Offsuit hands
        if high_value >= 10 && low_value >= 10 {
            // Broadway offsuit: AKo=80, KQo=65, JTo=55
            let score = 50 + high_value as u32 + (low_value as u32 / 2);
            (HandCategory::HighCard, format!("{}{} offsuit broadway", rank_name_singular(high_rank), rank_name_singular(low_rank)), score)
        } else if high_rank == Rank::Ace {
            // Ax offsuit: A9o=50, A2o=35
            let score = 30 + (low_value as u32 * 2);
            (HandCategory::HighCard, format!("{}{} offsuit", rank_name_singular(high_rank), rank_name_singular(low_rank)), score)
        } else if gap <= 1 && high_value >= 7 {
            // Offsuit connectors (only playable high): T9o, 98o
            let score = 35 + high_value as u32;
            (HandCategory::HighCard, format!("{}{} offsuit connector", rank_name_singular(high_rank), rank_name_singular(low_rank)), score)
        } else {
            // Offsuit junk
            let score = 15 + high_value as u32 + (low_value as u32 / 3);
            (HandCategory::HighCard, format!("{}{} offsuit", rank_name_singular(high_rank), rank_name_singular(low_rank)), score)
        }
    };

    HandEvaluation {
        category, description, strength_score,
        kickers: vec![high_rank, low_rank],
        draw_type: DrawType::None,
        outs: 0,
    }
}

// =============================================================================
// GTO SIZING FUNCTIONS
// =============================================================================

/// Calculate Minimum Defense Frequency: MDF = Pot / (Pot + Bet)
fn calculate_mdf(pot: f64, bet_size: f64) -> f64 {
    if bet_size <= 0.0 { return 1.0; }
    pot / (pot + bet_size)
}

/// GTO c-bet sizing based on board texture
fn get_cbet_size(pot: f64, texture: BoardTexture) -> f64 {
    let fraction = match texture {
        BoardTexture::Dry => 0.33,       // Small, high frequency
        BoardTexture::SemiWet => 0.50,   // Medium
        BoardTexture::Wet => 0.66,       // Large, polarized
        BoardTexture::Monotone => 0.75,  // Large, very polarized
    };
    (pot * fraction).max(0.10)
}

/// GTO turn sizing (larger than flop)
fn get_turn_size(pot: f64, has_nut_advantage: bool) -> f64 {
    if has_nut_advantage {
        (pot * 1.0).max(0.15) // Pot-sized or overbet
    } else {
        (pot * 0.66).max(0.15)
    }
}

/// GTO river sizing
fn get_river_size(pot: f64, is_value: bool, is_thin_value: bool) -> f64 {
    if is_thin_value {
        (pot * 0.33).max(0.10) // Block bet
    } else if is_value {
        (pot * 0.75).max(0.15) // Standard value
    } else {
        (pot * 1.0).max(0.15) // Polarized (bluff or nuts)
    }
}

// =============================================================================
// GTO POSITION-BASED OPENING THRESHOLDS
// =============================================================================

/// Get the minimum strength score to open raise from a position
fn get_open_threshold(position: &str) -> u32 {
    match position.to_lowercase().as_str() {
        "utg" | "under the gun" | "ep" | "early position" => 65, // ~15% (77+, ATs+, AJo+)
        "utg+1" | "utg1" => 62,
        "mp" | "middle position" | "mp1" | "mp2" => 58, // ~20%
        "hj" | "hijack" => 55, // ~22%
        "co" | "cutoff" => 50, // ~28%
        "btn" | "button" => 42, // ~40%
        "sb" | "small blind" => 45, // ~40% (raise or fold)
        "bb" | "big blind" => 35, // Defend wide
        _ => 55,
    }
}

/// Get the minimum strength to 3-bet from a position
fn get_3bet_threshold(position: &str, is_vs_late_position: bool) -> u32 {
    // Linear 3-betting when IP, polarized when OOP
    let base = match position.to_lowercase().as_str() {
        "btn" | "button" => 55, // Linear
        "co" | "cutoff" => 60,
        "sb" | "small blind" => 70, // Polarized: only premiums (also 3-bet A5s type blockers at 45-55)
        "bb" | "big blind" => 65,
        _ => 70,
    };
    if is_vs_late_position { base - 5 } else { base }
}

// =============================================================================
// MAIN STRATEGY FUNCTION (GTO-based)
// =============================================================================

pub fn recommend_action(
    hand_eval: &HandEvaluation,
    legal_actions: &[LegalAction],
    position: &str,
    pot: f64,
    amount_to_call: f64,
    community_cards: &[Card],
) -> RecommendedAction {
    let street = get_street(community_cards.len());
    let facing_bet = amount_to_call > 0.01;

    // Showdown detection
    let is_showdown = street == Street::River &&
        (legal_actions.is_empty() || (legal_actions.len() == 1 && matches!(legal_actions[0], LegalAction::Fold)));

    if is_showdown {
        return RecommendedAction {
            action: Action::NoRecommendation,
            reasoning: "Showdown - all betting complete".to_string(),
        };
    }

    let (desired_action, reasoning) = match street {
        Street::Preflop => recommend_preflop(hand_eval, position, pot, amount_to_call, facing_bet),
        Street::Flop => recommend_flop(hand_eval, position, pot, amount_to_call, facing_bet, community_cards),
        Street::Turn => recommend_turn(hand_eval, position, pot, amount_to_call, facing_bet, community_cards),
        Street::River => recommend_river(hand_eval, position, pot, amount_to_call, facing_bet),
    };

    // Filter to legal actions
    let final_action = filter_to_legal(desired_action, &reasoning, legal_actions, hand_eval, pot, amount_to_call, street);
    final_action
}

// =============================================================================
// STREET-SPECIFIC GTO RECOMMENDATIONS
// =============================================================================

fn recommend_preflop(
    hand_eval: &HandEvaluation,
    position: &str,
    pot: f64,
    amount_to_call: f64,
    facing_bet: bool,
) -> (Action, String) {
    let score = hand_eval.strength_score;
    let open_threshold = get_open_threshold(position);
    let raise_size = (pot * 3.0).max(0.06); // Standard 3x open
    let pos_lower = position.to_lowercase();
    
    // Position flags
    let is_bb = pos_lower.contains("bb") || pos_lower.contains("big blind");
    let is_sb = pos_lower.contains("sb") || pos_lower.contains("small blind");
    let is_blind = is_bb || is_sb;

    if !facing_bet || amount_to_call < 0.03 {
        // RFI (Raise First In) spot
        if score >= open_threshold {
            (Action::Bet(raise_size), format!("{}, open from {}", hand_eval.description, position))
        } else if amount_to_call < 0.01 {
            // BB can check
            (Action::Check, format!("{}, check option", hand_eval.description))
        } else {
            (Action::Fold, format!("{}, below opening range for {}", hand_eval.description, position))
        }
    } else {
        // Facing a raise: 3-bet, call, or fold
        let is_vs_late = pos_lower.contains("btn") || pos_lower.contains("sb");
        let three_bet_threshold = get_3bet_threshold(position, is_vs_late);
        let pot_odds = amount_to_call / (pot + amount_to_call);
        
        // Classify bet size relative to pot
        let bet_to_pot = if pot > 0.0 { amount_to_call / pot } else { 1.0 };
        let is_min_raise = bet_to_pot < 0.5;  // Min-raise or small raise
        let is_big_raise = bet_to_pot > 1.5;  // 3-bet or larger

        // Premium hands: 3-bet for value
        if score >= three_bet_threshold {
            let reraise = (pot + amount_to_call) * 3.0;
            (Action::Raise(reraise), format!("{}, 3-bet for value", hand_eval.description))
        }
        // Suited wheel aces (A2s-A5s): 3-bet as blockers (polarized range)
        else if hand_eval.description.contains("suited") && hand_eval.description.contains("ace")
            && score >= 60 && score < 70 {
            let reraise = (pot + amount_to_call) * 3.0;
            (Action::Raise(reraise), format!("{}, 3-bet as blocker", hand_eval.description))
        }
        // === BLIND DEFENSE LOGIC (GTO) ===
        // BB defends very wide vs min-raises (MDF ~67%)
        else if is_bb && is_min_raise {
            if score >= 35 {
                // Defend wide: any Ax, Kx, pairs, suited connectors, broadway
                (Action::Call, format!("{}, defend BB vs min-raise", hand_eval.description))
            } else if score >= 28 {
                // Marginal: suited gappers, weak offsuit - defend if great odds
                if pot_odds < 0.25 {
                    (Action::Call, format!("{}, defend BB (getting {:.0}%)", hand_eval.description, pot_odds * 100.0))
                } else {
                    (Action::Fold, format!("{}, fold BB to raise", hand_eval.description))
                }
            } else {
                (Action::Fold, format!("{}, fold trash in BB", hand_eval.description))
            }
        }
        // BB vs normal raise (2.5-3x)
        else if is_bb && !is_big_raise {
            if score >= 40 {
                // Ax, decent pairs, suited broadways
                (Action::Call, format!("{}, defend BB", hand_eval.description))
            } else if score >= 35 && pot_odds < 0.30 {
                // Speculative hands with good odds
                (Action::Call, format!("{}, defend BB (pot odds)", hand_eval.description))
            } else {
                (Action::Fold, format!("{}, fold BB to raise", hand_eval.description))
            }
        }
        // BB vs 3-bet or big raise - tighten up significantly
        else if is_bb && is_big_raise {
            if score >= 55 {
                (Action::Call, format!("{}, call 3-bet in BB", hand_eval.description))
            } else {
                (Action::Fold, format!("{}, fold BB to 3-bet", hand_eval.description))
            }
        }
        // SB defense (tighter than BB, no discount)
        else if is_sb {
            if is_min_raise && score >= 42 {
                (Action::Call, format!("{}, complete SB vs min-raise", hand_eval.description))
            } else if !is_big_raise && score >= 48 {
                (Action::Call, format!("{}, defend SB", hand_eval.description))
            } else {
                (Action::Fold, format!("{}, fold SB to raise", hand_eval.description))
            }
        }
        // === NON-BLIND POSITIONS ===
        // Medium hands: flat call for implied odds
        else if score >= 50 {
            // Pocket pairs: set mine
            if hand_eval.category == HandCategory::OnePair {
                (Action::Call, format!("{}, call to set mine", hand_eval.description))
            }
            // Suited connectors: implied odds
            else if hand_eval.description.contains("suited") {
                (Action::Call, format!("{}, call for implied odds", hand_eval.description))
            }
            // Broadway hands
            else if score >= 55 {
                (Action::Call, format!("{}, defend vs raise", hand_eval.description))
            } else {
                (Action::Fold, format!("{}, fold to raise", hand_eval.description))
            }
        }
        // Weak hands: fold
        else {
            (Action::Fold, format!("{}, fold to aggression", hand_eval.description))
        }
    }
}

fn recommend_flop(
    hand_eval: &HandEvaluation,
    position: &str,
    pot: f64,
    amount_to_call: f64,
    facing_bet: bool,
    board: &[Card],
) -> (Action, String) {
    let score = hand_eval.strength_score;
    let texture = analyze_board_texture(board);
    let has_range_adv = has_range_advantage(board, position);
    
    // Texture-based sizing (core GTO concept)
    let cbet_size = get_cbet_size(pot, texture);
    let texture_desc = match texture {
        BoardTexture::Dry => "dry board",
        BoardTexture::SemiWet => "semi-wet board",
        BoardTexture::Wet => "wet board",
        BoardTexture::Monotone => "monotone board",
    };

    if !facing_bet {
        // C-bet decision based on board texture
        // GTO: High frequency + small size on dry, low frequency + large size on wet
        
        if score >= 85 {
            // Monster: always bet for value
            let bet = (pot * 0.66).max(0.10);
            (Action::Bet(bet), format!("{}, value bet on {}", hand_eval.description, texture_desc))
        } else if score >= 55 {
            // Top pair / overpair
            match texture {
                BoardTexture::Dry => {
                    // Dry board: bet small, high frequency (range bet)
                    (Action::Bet(cbet_size), format!("{}, range bet on {}", hand_eval.description, texture_desc))
                },
                BoardTexture::SemiWet => {
                    (Action::Bet(cbet_size), format!("{}, value bet on {}", hand_eval.description, texture_desc))
                },
                BoardTexture::Wet | BoardTexture::Monotone => {
                    // Wet board: bet larger but more selectively
                    if score >= 60 || has_range_adv {
                        (Action::Bet(cbet_size), format!("{}, value bet on {}", hand_eval.description, texture_desc))
                    } else {
                        // Check back some top pairs on wet boards to protect checking range
                        (Action::Check, format!("{}, check to protect range on {}", hand_eval.description, texture_desc))
                    }
                }
            }
        } else if hand_eval.draw_type == DrawType::ComboDraw || hand_eval.draw_type == DrawType::FlushDraw {
            // Strong draws: semi-bluff (more on wet boards where draws are disguised)
            match texture {
                BoardTexture::Wet | BoardTexture::Monotone => {
                    (Action::Bet(cbet_size), format!("{}, semi-bluff on {}", hand_eval.description, texture_desc))
                },
                _ => {
                    // On dry boards, draws are obvious - check sometimes
                    if score >= 35 {
                        (Action::Bet(cbet_size), format!("{}, semi-bluff", hand_eval.description))
                    } else {
                        (Action::Check, format!("{}, check with draw", hand_eval.description))
                    }
                }
            }
        } else if score >= 40 {
            // Medium strength: pot control
            (Action::Check, format!("{}, pot control on {}", hand_eval.description, texture_desc))
        } else if hand_eval.draw_type == DrawType::Oesd || hand_eval.draw_type == DrawType::Gutshot {
            // Weaker draws
            match texture {
                BoardTexture::Dry => {
                    // Can bluff on dry boards
                    if has_range_adv {
                        (Action::Bet(cbet_size), format!("{}, bluff c-bet on {}", hand_eval.description, texture_desc))
                    } else {
                        (Action::Check, format!("{}, check with draw", hand_eval.description))
                    }
                },
                _ => (Action::Check, format!("{}, check with draw on {}", hand_eval.description, texture_desc))
            }
        } else {
            // Air: check back (but can range bet on dry boards)
            match texture {
                BoardTexture::Dry if has_range_adv => {
                    // GTO: range bet 100% on dry A-high boards
                    (Action::Bet(cbet_size), format!("range bet on {}", texture_desc))
                },
                _ => (Action::Check, format!("{}, check back on {}", hand_eval.description, texture_desc))
            }
        }
    } else {
        // Facing a bet: use MDF and equity
        let mdf = calculate_mdf(pot, amount_to_call);
        let equity = estimate_equity(hand_eval, Street::Flop);
        let pot_odds = amount_to_call / (pot + amount_to_call);

        if score >= 85 {
            // Monster: raise for value
            let raise = (pot + amount_to_call) * 2.5;
            (Action::Raise(raise), format!("{}, raise for value on {}", hand_eval.description, texture_desc))
        } else if equity > pot_odds {
            if score >= 68 {
                // Strong hand: can raise
                let raise = (pot + amount_to_call) * 2.5;
                (Action::Raise(raise), format!("{}, raise for value", hand_eval.description))
            } else {
                // Drawing or medium: call
                (Action::Call, format!("{}, call, equity {:.0}% > pot odds {:.0}%", hand_eval.description, equity * 100.0, pot_odds * 100.0))
            }
        } else {
            // Below pot odds
            (Action::Fold, format!("{}, fold on {}, insufficient equity", hand_eval.description, texture_desc))
        }
    }
}

fn recommend_turn(
    hand_eval: &HandEvaluation,
    position: &str,
    pot: f64,
    amount_to_call: f64,
    facing_bet: bool,
    board: &[Card],
) -> (Action, String) {
    let score = hand_eval.strength_score;
    let texture = analyze_board_texture(board);
    
    // Check for nut advantage scenarios (paired boards, etc.)
    let board_paired = {
        let mut rank_counts: std::collections::HashMap<Rank, usize> = std::collections::HashMap::new();
        for card in board {
            *rank_counts.entry(card.rank).or_insert(0) += 1;
        }
        rank_counts.values().any(|&c| c >= 2)
    };

    // GTO Turn: Polarize. Bet strong value and draws. Check medium.
    if !facing_bet {
        if score >= 88 {
            // Monster (set+): can overbet on paired/dry boards (nut advantage)
            if board_paired || texture == BoardTexture::Dry {
                let bet = (pot * 1.25).max(0.20); // Overbet
                (Action::Bet(bet), format!("{}, overbet for value (nut advantage)", hand_eval.description))
            } else {
                let bet = (pot * 0.75).max(0.15);
                (Action::Bet(bet), format!("{}, value bet", hand_eval.description))
            }
        } else if score >= 68 {
            // Strong (two pair, overpair): standard value bet
            let bet = get_turn_size(pot, false);
            (Action::Bet(bet), format!("{}, value bet", hand_eval.description))
        } else if hand_eval.draw_type == DrawType::ComboDraw || hand_eval.draw_type == DrawType::FlushDraw {
            // Strong draws: semi-bluff with equity
            let bet = (pot * 0.66).max(0.15);
            (Action::Bet(bet), format!("{}, semi-bluff ({} outs)", hand_eval.description, hand_eval.outs))
        } else if score >= 42 {
            // Medium strength (middle pair, weak top pair): CHECK
            // GTO key concept: don't bet medium hands on turn
            (Action::Check, format!("{}, pot control (GTO: check medium)", hand_eval.description))
        } else if hand_eval.draw_type == DrawType::Oesd {
            // OESD: can semi-bluff
            let bet = (pot * 0.50).max(0.10);
            (Action::Bet(bet), format!("{}, semi-bluff ({} outs)", hand_eval.description, hand_eval.outs))
        } else {
            // Weak: give up or check with gutshot
            if hand_eval.draw_type == DrawType::Gutshot {
                (Action::Check, format!("{}, check with {} outs", hand_eval.description, hand_eval.outs))
            } else {
                (Action::Check, format!("{}, check/give up", hand_eval.description))
            }
        }
    } else {
        // Facing bet: equity vs pot odds
        let equity = estimate_equity(hand_eval, Street::Turn);
        let pot_odds = amount_to_call / (pot + amount_to_call);

        if score >= 88 {
            // Monster: raise
            let raise = (pot + amount_to_call) * 2.5;
            (Action::Raise(raise), format!("{}, raise for value", hand_eval.description))
        } else if equity > pot_odds {
            if score >= 75 {
                // Strong: can raise
                let raise = (pot + amount_to_call) * 2.2;
                (Action::Raise(raise), format!("{}, raise for value", hand_eval.description))
            } else {
                (Action::Call, format!("{}, call, equity {:.0}% > pot odds {:.0}%", hand_eval.description, equity * 100.0, pot_odds * 100.0))
            }
        } else {
            (Action::Fold, format!("{}, fold, equity {:.0}% < pot odds {:.0}%", hand_eval.description, equity * 100.0, pot_odds * 100.0))
        }
    }
}

fn recommend_river(
    hand_eval: &HandEvaluation,
    position: &str,
    pot: f64,
    amount_to_call: f64,
    facing_bet: bool,
) -> (Action, String) {
    let score = hand_eval.strength_score;

    // River: Draws have 0 equity. Pure value vs bluff.
    if !facing_bet {
        if score >= 85 {
            // Monster: bet big for value
            let bet = (pot * 0.75).max(0.15);
            (Action::Bet(bet), format!("{}, value bet", hand_eval.description))
        } else if score >= 68 {
            // Strong: value bet
            let bet = (pot * 0.66).max(0.15);
            (Action::Bet(bet), format!("{}, value bet", hand_eval.description))
        } else if score >= 55 {
            // Top pair type: thin value (block bet)
            let bet = (pot * 0.33).max(0.10);
            (Action::Bet(bet), format!("{}, thin value", hand_eval.description))
        } else {
            // Weak: check, give up
            (Action::Check, format!("{}, check, showdown value", hand_eval.description))
        }
    } else {
        // Facing river bet: MDF-based decision
        let mdf = calculate_mdf(pot, amount_to_call);
        let equity = estimate_equity(hand_eval, Street::River);
        let pot_odds = amount_to_call / (pot + amount_to_call);

        if score >= 85 {
            // Monster: raise for value
            let raise = (pot + amount_to_call) * 2.5;
            (Action::Raise(raise), format!("{}, raise for value", hand_eval.description))
        } else if score >= 55 && equity > pot_odds {
            // Bluff catcher with showdown value: call
            (Action::Call, format!("{}, call as bluff catcher", hand_eval.description))
        } else if score >= 45 && equity > pot_odds * 1.2 {
            // Marginal: close decision, lean call
            (Action::Call, format!("{}, marginal call", hand_eval.description))
        } else {
            (Action::Fold, format!("{}, fold to river aggression", hand_eval.description))
        }
    }
}

// =============================================================================
// EQUITY ESTIMATION
// =============================================================================

fn estimate_equity(hand_eval: &HandEvaluation, street: Street) -> f64 {
    // River: draws have 0 equity
    if street == Street::River {
        return match hand_eval.strength_score {
            s if s >= 90 => 0.98,
            s if s >= 80 => 0.92,
            s if s >= 68 => 0.80,
            s if s >= 55 => 0.60,
            s if s >= 45 => 0.40,
            s if s >= 35 => 0.25,
            _ => 0.10,
        };
    }

    // Flop/Turn: draws have equity
    let outs = hand_eval.outs as f64;
    let draw_equity = if street == Street::Turn {
        outs * 2.2 / 100.0 // ~2.2% per out (1 card)
    } else {
        outs * 4.0 / 100.0 // ~4% per out (2 cards)
    };

    let made_hand_equity = match hand_eval.strength_score {
        s if s >= 90 => 0.95,
        s if s >= 80 => 0.88,
        s if s >= 68 => 0.75,
        s if s >= 55 => 0.58,
        s if s >= 45 => 0.40,
        s if s >= 35 => 0.25,
        _ => 0.12,
    };

    f64::max(draw_equity, made_hand_equity)
}

// =============================================================================
// LEGAL ACTION FILTERING
// =============================================================================

fn filter_to_legal(
    desired: Action,
    reasoning: &str,
    legal_actions: &[LegalAction],
    hand_eval: &HandEvaluation,
    pot: f64,
    amount_to_call: f64,
    street: Street,
) -> RecommendedAction {
    let has_fold = legal_actions.iter().any(|a| matches!(a, LegalAction::Fold));
    let has_check = legal_actions.iter().any(|a| matches!(a, LegalAction::Check) || matches!(a, LegalAction::Call(amt) if *amt == 0.0));
    let has_call = legal_actions.iter().any(|a| matches!(a, LegalAction::Call(_)));
    let has_bet = legal_actions.iter().any(|a| matches!(a, LegalAction::Bet));
    let has_raise = legal_actions.iter().any(|a| matches!(a, LegalAction::Raise));

    let mut final_reasoning = reasoning.to_string();

    let final_action = match desired {
        Action::Bet(amt) => {
            if has_bet { Action::Bet(amt) }
            else if has_raise { Action::Raise(amt) }
            else if has_check { final_reasoning = format!("{} (bet N/A, check)", reasoning); Action::Check }
            else { Action::Fold }
        },
        Action::Raise(amt) => {
            if has_raise { Action::Raise(amt) }
            else if has_bet { Action::Bet(amt) }
            else if has_call { final_reasoning = format!("{} (raise N/A, call)", reasoning); Action::Call }
            else if has_check { Action::Check }
            else { Action::Fold }
        },
        Action::Call => {
            if amount_to_call < 0.01 {
                if has_check { Action::Check } else { Action::Fold }
            } else if has_call { Action::Call }
            else { Action::Fold }
        },
        Action::Check => {
            if has_check { Action::Check }
            else if has_call && amount_to_call > 0.0 {
                let pot_odds = amount_to_call / (pot + amount_to_call);
                let equity = estimate_equity(hand_eval, street);
                if equity > pot_odds && hand_eval.strength_score >= 35 {
                    final_reasoning = format!("{} (check N/A, call)", reasoning);
                    Action::Call
                } else {
                    final_reasoning = format!("{} (check N/A, fold)", reasoning);
                    Action::Fold
                }
            } else { Action::Fold }
        },
        Action::Fold => {
            if has_check {
                final_reasoning = "Check for free".to_string();
                Action::Check
            } else { Action::Fold }
        },
        Action::NoRecommendation => Action::NoRecommendation,
    };

    RecommendedAction { action: final_action, reasoning: final_reasoning }
}

// Legacy alias - now takes community_cards directly
pub fn recommend_action_v2(
    hand_eval: &HandEvaluation,
    legal_actions: &[LegalAction],
    position: &str,
    pot: f64,
    amount_to_call: f64,
    community_cards: &[Card],
) -> RecommendedAction {
    recommend_action(hand_eval, legal_actions, position, pot, amount_to_call, community_cards)
}

/// Calculate win and tie percentages (simplified)
pub fn calculate_win_tie_percentages(
    hole_cards: &[Card],
    community_cards: &[Card],
    _num_simulations: u32,
) -> (f32, f32) {
    let hand_eval = evaluate_hand(hole_cards, community_cards);
    let street = match community_cards.len() {
        0 => Street::Preflop,
        3 => Street::Flop,
        4 => Street::Turn,
        _ => Street::River,
    };
    let base_equity = estimate_equity(&hand_eval, street);
    let tie_percentage = 3.0;
    let win_percentage = (base_equity * 100.0) - (tie_percentage / 2.0);
    (win_percentage as f32, tie_percentage as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preflop_pocket_aces() {
        let hole = vec![
            Card { rank: Rank::Ace, suit: Suit::Spades },
            Card { rank: Rank::Ace, suit: Suit::Hearts },
        ];
        let eval = evaluate_hand(&hole, &[]);
        assert_eq!(eval.category, HandCategory::OnePair);
        assert!(eval.strength_score >= 95);
        assert!(eval.description.contains("pocket"));
    }

    #[test]
    fn test_preflop_suited_connector() {
        let hole = vec![
            Card { rank: Rank::Eight, suit: Suit::Hearts },
            Card { rank: Rank::Seven, suit: Suit::Hearts },
        ];
        let eval = evaluate_hand(&hole, &[]);
        assert!(eval.description.contains("suited connector"));
    }

    #[test]
    fn test_top_pair() {
        let hole = vec![
            Card { rank: Rank::Ace, suit: Suit::Clubs },
            Card { rank: Rank::King, suit: Suit::Diamonds },
        ];
        let board = vec![
            Card { rank: Rank::Ace, suit: Suit::Hearts },
            Card { rank: Rank::Seven, suit: Suit::Spades },
            Card { rank: Rank::Two, suit: Suit::Clubs },
        ];
        let eval = evaluate_hand(&hole, &board);
        assert_eq!(eval.category, HandCategory::OnePair);
        assert!(eval.description.contains("top pair"));
        assert!(eval.strength_score >= 55);
    }

    #[test]
    fn test_flush_draw_equity() {
        let hole = vec![
            Card { rank: Rank::Ace, suit: Suit::Hearts },
            Card { rank: Rank::King, suit: Suit::Hearts },
        ];
        let board = vec![
            Card { rank: Rank::Seven, suit: Suit::Hearts },
            Card { rank: Rank::Two, suit: Suit::Hearts },
            Card { rank: Rank::Nine, suit: Suit::Clubs },
        ];
        let eval = evaluate_hand(&hole, &board);
        assert_eq!(eval.draw_type, DrawType::FlushDraw);
        assert_eq!(eval.outs, 9);
    }

    #[test]
    fn test_mdf_calculation() {
        // Half pot bet: MDF = 100 / (100 + 50) = 66.7%
        let mdf = calculate_mdf(100.0, 50.0);
        assert!((mdf - 0.667).abs() < 0.01);
    }
}