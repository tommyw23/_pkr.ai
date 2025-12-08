// src-tauri/src/poker/preflop_ranges.rs
// Hardcoded GTO preflop opening ranges for instant fallback when AI is unavailable

use std::collections::HashSet;
use once_cell::sync::Lazy;
use super::Action;

// BTN (Button) Opening Range
// All pairs, all suited aces, suited kings K2s+, suited queens Q5s+, suited jacks J7s+,
// suited connectors 54s+, offsuit broadways ATo+ KJo+ QJo
static BTN_RANGE: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut range = HashSet::new();

    // All pairs
    range.extend([
        "AA", "KK", "QQ", "JJ", "TT", "99", "88", "77", "66", "55", "44", "33", "22"
    ]);

    // All suited aces
    range.extend([
        "AKs", "AQs", "AJs", "ATs", "A9s", "A8s", "A7s", "A6s", "A5s", "A4s", "A3s", "A2s"
    ]);

    // Suited kings K2s+
    range.extend([
        "KQs", "KJs", "KTs", "K9s", "K8s", "K7s", "K6s", "K5s", "K4s", "K3s", "K2s"
    ]);

    // Suited queens Q5s+
    range.extend([
        "QJs", "QTs", "Q9s", "Q8s", "Q7s", "Q6s", "Q5s"
    ]);

    // Suited jacks J7s+
    range.extend([
        "JTs", "J9s", "J8s", "J7s"
    ]);

    // Suited connectors 54s+
    range.extend([
        "T9s", "98s", "87s", "76s", "65s", "54s"
    ]);

    // One gappers suited
    range.extend([
        "T8s", "97s", "86s", "75s"
    ]);

    // Two gappers suited (high cards)
    range.extend([
        "T7s", "96s"
    ]);

    // Offsuit broadways
    range.extend([
        "AKo", "AQo", "AJo", "ATo", "KQo", "KJo", "QJo"
    ]);

    range
});

// CO (Cutoff) Opening Range
// Pairs 22+, suited aces, suited kings K5s+, suited queens Q8s+, AJo+ KQo
static CO_RANGE: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut range = HashSet::new();

    // All pairs
    range.extend([
        "AA", "KK", "QQ", "JJ", "TT", "99", "88", "77", "66", "55", "44", "33", "22"
    ]);

    // All suited aces
    range.extend([
        "AKs", "AQs", "AJs", "ATs", "A9s", "A8s", "A7s", "A6s", "A5s", "A4s", "A3s", "A2s"
    ]);

    // Suited kings K5s+
    range.extend([
        "KQs", "KJs", "KTs", "K9s", "K8s", "K7s", "K6s", "K5s"
    ]);

    // Suited queens Q8s+
    range.extend([
        "QJs", "QTs", "Q9s", "Q8s"
    ]);

    // Suited jacks
    range.extend([
        "JTs", "J9s"
    ]);

    // Suited connectors
    range.extend([
        "T9s", "98s", "87s", "76s", "65s"
    ]);

    // One gappers suited
    range.extend([
        "T8s", "97s"
    ]);

    // Offsuit broadways
    range.extend([
        "AKo", "AQo", "AJo", "KQo"
    ]);

    range
});

// EP (Early Position: UTG, UTG+1) Opening Range
// Pairs 66+, AJs+ KQs, AQo+
static EP_RANGE: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut range = HashSet::new();

    // Pairs 66+
    range.extend([
        "AA", "KK", "QQ", "JJ", "TT", "99", "88", "77", "66"
    ]);

    // Premium suited aces AJs+
    range.extend([
        "AKs", "AQs", "AJs"
    ]);

    // Suited kings
    range.extend([
        "KQs"
    ]);

    // Medium suited aces (for balance)
    range.extend([
        "ATs", "A5s" // A5s for wheel potential
    ]);

    // Suited queens
    range.extend([
        "QJs"
    ]);

    // Suited jacks
    range.extend([
        "JTs"
    ]);

    // Offsuit broadways AQo+
    range.extend([
        "AKo", "AQo"
    ]);

    range
});

// MP (Middle Position) Opening Range
// Between EP and CO in terms of tightness
static MP_RANGE: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut range = HashSet::new();

    // All pairs 55+
    range.extend([
        "AA", "KK", "QQ", "JJ", "TT", "99", "88", "77", "66", "55"
    ]);

    // Suited aces A9s+
    range.extend([
        "AKs", "AQs", "AJs", "ATs", "A9s", "A8s", "A7s", "A6s", "A5s", "A4s", "A3s", "A2s"
    ]);

    // Suited kings K9s+
    range.extend([
        "KQs", "KJs", "KTs", "K9s"
    ]);

    // Suited queens Q9s+
    range.extend([
        "QJs", "QTs", "Q9s"
    ]);

    // Suited jacks
    range.extend([
        "JTs", "J9s"
    ]);

    // Suited connectors
    range.extend([
        "T9s", "98s", "87s", "76s"
    ]);

    // Offsuit broadways
    range.extend([
        "AKo", "AQo", "AJo", "KQo"
    ]);

    range
});

// SB (Small Blind) Opening Range vs BB
// More aggressive when heads up
static SB_RANGE: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    let mut range = HashSet::new();

    // All pairs
    range.extend([
        "AA", "KK", "QQ", "JJ", "TT", "99", "88", "77", "66", "55", "44", "33", "22"
    ]);

    // All suited aces
    range.extend([
        "AKs", "AQs", "AJs", "ATs", "A9s", "A8s", "A7s", "A6s", "A5s", "A4s", "A3s", "A2s"
    ]);

    // Suited kings
    range.extend([
        "KQs", "KJs", "KTs", "K9s", "K8s", "K7s", "K6s", "K5s", "K4s"
    ]);

    // Suited queens
    range.extend([
        "QJs", "QTs", "Q9s", "Q8s", "Q7s", "Q6s"
    ]);

    // Suited jacks
    range.extend([
        "JTs", "J9s", "J8s"
    ]);

    // Suited connectors and one gappers
    range.extend([
        "T9s", "T8s", "98s", "97s", "87s", "86s", "76s", "75s", "65s", "54s"
    ]);

    // Offsuit broadways
    range.extend([
        "AKo", "AQo", "AJo", "ATo", "A9o", "KQo", "KJo", "KTo", "QJo", "QTo", "JTo"
    ]);

    range
});

/// Normalizes a hand string to canonical format (higher rank first, suited indicator)
/// Examples: "Ah Kh" -> "AKs", "9c 9d" -> "99", "7s 2s" -> "72s"
fn normalize_hand(hand: &str) -> Option<String> {
    let hand = hand.trim();

    // Handle already normalized format (e.g., "AKs", "QQ", "T9o")
    if hand.len() >= 2 && hand.len() <= 3 {
        let chars: Vec<char> = hand.chars().collect();
        if chars.len() >= 2 && is_rank_char(chars[0]) && is_rank_char(chars[1]) {
            return Some(hand.to_uppercase());
        }
    }

    // Parse "Ah Kh" or "As Kd" format
    let parts: Vec<&str> = hand.split_whitespace().collect();
    if parts.len() != 2 {
        return None;
    }

    let card1 = parts[0];
    let card2 = parts[1];

    if card1.len() < 2 || card2.len() < 2 {
        return None;
    }

    let rank1 = &card1[0..1].to_uppercase();
    let suit1 = &card2[1..2].to_lowercase();
    let rank2 = &card2[0..1].to_uppercase();
    let suit2 = &card2[1..2].to_lowercase();

    // Normalize rank representations
    let rank1 = if rank1 == "10" { "T" } else { rank1 };
    let rank2 = if rank2 == "10" { "T" } else { rank2 };

    // Check if it's a pair
    if rank1 == rank2 {
        return Some(format!("{}{}", rank1, rank1));
    }

    // Determine if suited
    let suited = suit1 == suit2;
    let suffix = if suited { "s" } else { "o" };

    // Order by rank (higher rank first)
    let (high, low) = if rank_value(rank1) >= rank_value(rank2) {
        (rank1, rank2)
    } else {
        (rank2, rank1)
    };

    Some(format!("{}{}{}", high, low, suffix))
}

fn is_rank_char(c: char) -> bool {
    matches!(c.to_ascii_uppercase(), 'A' | 'K' | 'Q' | 'J' | 'T' | '9' | '8' | '7' | '6' | '5' | '4' | '3' | '2')
}

fn rank_value(rank: &str) -> u8 {
    match rank {
        "A" => 14,
        "K" => 13,
        "Q" => 12,
        "J" => 11,
        "T" => 10,
        "9" => 9,
        "8" => 8,
        "7" => 7,
        "6" => 6,
        "5" => 5,
        "4" => 4,
        "3" => 3,
        "2" => 2,
        _ => 0,
    }
}

/// Returns the appropriate opening range for a given position
fn get_range_for_position(position: &str) -> Option<&'static HashSet<&'static str>> {
    let pos = position.to_uppercase();
    match pos.as_str() {
        "BTN" | "BUTTON" | "BU" => Some(&*BTN_RANGE),
        "CO" | "CUTOFF" => Some(&*CO_RANGE),
        "EP" | "UTG" | "UTG+1" | "UTG1" | "EARLY" => Some(&*EP_RANGE),
        "MP" | "MP1" | "MP2" | "MIDDLE" => Some(&*MP_RANGE),
        "SB" | "SMALL_BLIND" | "SMALLBLIND" => Some(&*SB_RANGE),
        _ => None,
    }
}

/// Returns the recommended preflop action based on GTO ranges
///
/// # Arguments
/// * `hand` - Hand in format "AKs", "QQ", "T9o", or "Ah Kh", "9c 9d", etc.
/// * `position` - Position string: "BTN", "CO", "EP", "MP", "SB"
///
/// # Returns
/// * `Some(Action::Raise(2.5))` if hand is in the opening range for that position
/// * `Some(Action::Fold)` if hand is not in the range
/// * `None` if position is invalid or hand cannot be parsed
pub fn get_preflop_action(hand: &str, position: &str) -> Option<Action> {
    let normalized = normalize_hand(hand)?;
    let range = get_range_for_position(position)?;

    // Check if normalized hand is in range (try both suited and offsuit variants for pairs)
    let in_range = range.contains(normalized.as_str()) || {
        // For pairs, the notation is just "AA", "KK", etc. without 's' or 'o'
        if normalized.len() == 2 && normalized.chars().nth(0) == normalized.chars().nth(1) {
            range.contains(normalized.as_str())
        } else {
            false
        }
    };

    if in_range {
        // Standard opening raise size is 2.5bb
        Some(Action::Raise(2.5))
    } else {
        Some(Action::Fold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_hand() {
        assert_eq!(normalize_hand("AKs"), Some("AKS".to_string()));
        assert_eq!(normalize_hand("Ah Kh"), Some("AKs".to_string()));
        assert_eq!(normalize_hand("As Kd"), Some("AKo".to_string()));
        assert_eq!(normalize_hand("9c 9d"), Some("99".to_string()));
        assert_eq!(normalize_hand("7s 2s"), Some("72s".to_string()));
        assert_eq!(normalize_hand("2s 7s"), Some("72s".to_string()));
    }

    #[test]
    fn test_btn_range() {
        // Should raise premium hands
        assert!(matches!(get_preflop_action("AA", "BTN"), Some(Action::Raise(_))));
        assert!(matches!(get_preflop_action("AKs", "BTN"), Some(Action::Raise(_))));

        // Should raise suited connectors
        assert!(matches!(get_preflop_action("54s", "BTN"), Some(Action::Raise(_))));

        // Should fold weak hands
        assert!(matches!(get_preflop_action("72o", "BTN"), Some(Action::Fold)));
        assert!(matches!(get_preflop_action("J2o", "BTN"), Some(Action::Fold)));
    }

    #[test]
    fn test_ep_range() {
        // Should raise premium hands
        assert!(matches!(get_preflop_action("AA", "EP"), Some(Action::Raise(_))));
        assert!(matches!(get_preflop_action("AKs", "EP"), Some(Action::Raise(_))));

        // Should fold marginal hands that BTN would raise
        assert!(matches!(get_preflop_action("54s", "EP"), Some(Action::Fold)));
        assert!(matches!(get_preflop_action("22", "EP"), Some(Action::Fold)));
    }

    #[test]
    fn test_co_range() {
        // Between EP and BTN
        assert!(matches!(get_preflop_action("22", "CO"), Some(Action::Raise(_))));
        assert!(matches!(get_preflop_action("K5s", "CO"), Some(Action::Raise(_))));
        assert!(matches!(get_preflop_action("K4s", "CO"), Some(Action::Fold)));
    }
}
