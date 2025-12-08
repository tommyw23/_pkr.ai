// src-tauri/src/validator.rs

use crate::poker_types::{Card, PokerState};
use std::collections::HashSet;

#[derive(Debug)]
pub struct ValidationIssues {
    pub issues: Vec<String>,
    pub is_valid: bool,
}

pub fn validate_poker_state(state: &PokerState) -> ValidationIssues {
    let mut issues = Vec::new();

    // Check overall confidence
    if state.overall_confidence < 0.80 {
        issues.push(format!("low_overall_confidence: {:.2}", state.overall_confidence));
    }

    // Check for duplicate cards
    let all_cards: Vec<&Card> = state.hero_cards.iter()
        .chain(state.board_cards.iter())
        .collect();
    
    let mut seen = HashSet::new();
    for card in &all_cards {
        let card_str = format!("{}{}", card.rank.to_str(), card.suit.to_str());
        if !seen.insert(card_str.clone()) {
            issues.push(format!("duplicate_card_detected: {}", card_str));
        }
    }

    // Check board length matches street
    if let Some(ref street) = state.street {
        let expected_board_len = match street.as_str() {
            "preflop" => 0,
            "flop" => 3,
            "turn" => 4,
            "river" => 5,
            "showdown" => 5,
            _ => state.board_cards.len(), // unknown street, skip validation
        };

        if state.board_cards.len() != expected_board_len {
            issues.push(format!(
                "inconsistent_board_length: expected {} for {}, got {}",
                expected_board_len,
                street,
                state.board_cards.len()
            ));
        }
    }

    // Check hero cards (should be 0 or 2)
    if !state.hero_cards.is_empty() && state.hero_cards.len() != 2 {
        issues.push(format!("invalid_hero_cards_count: {}", state.hero_cards.len()));
    }

    // Card validity is now guaranteed by the enum types - no need to validate

    ValidationIssues {
        is_valid: issues.is_empty(),
        issues,
    }
}