// src-tauri/src/poker/state_machine.rs
// State machine smoothing logic to prevent flickering and enforce valid transitions

use crate::poker_types::{Card, PokerState};

#[derive(Debug, Clone)]
pub struct StateTransitionResult {
    pub new_state: PokerState,
    pub is_new_hand: bool,
    pub corrections_applied: Vec<String>,
}

/// Detect if a new hand has started based on pot reset and board changes
pub fn detect_hand_transition(
    previous: Option<&PokerState>,
    current: &PokerState,
) -> bool {
    let Some(prev) = previous else {
        // No previous state - assume first hand
        return false;
    };

    // NEW HAND DETECTION RULES
    // 1. Pot reset: High pot (>$1000) drops to low pot (<$500)
    if let (Some(prev_pot), Some(curr_pot)) = (prev.pot_size, current.pot_size) {
        if prev_pot > 1000.0 && curr_pot < 500.0 {
            return true;
        }
    }

    // 2. Board reset: Had board cards (3+), now has 0
    if prev.board_cards.len() >= 3 && current.board_cards.is_empty() {
        return true;
    }

    // 3. Hero cards changed completely (both cards different)
    if prev.hero_cards.len() == 2 && current.hero_cards.len() == 2 {
        let prev_high_confidence = prev.per_field_confidence.hero_cards >= 0.85;
        let curr_high_confidence = current.per_field_confidence.hero_cards >= 0.85;

        if prev_high_confidence && curr_high_confidence {
            let cards_changed = !cards_equal(&prev.hero_cards[0], &current.hero_cards[0])
                && !cards_equal(&prev.hero_cards[1], &current.hero_cards[1])
                && !cards_equal(&prev.hero_cards[0], &current.hero_cards[1])
                && !cards_equal(&prev.hero_cards[1], &current.hero_cards[0]);

            if cards_changed {
                return true;
            }
        }
    }

    false
}

/// Validate that the state transition follows poker rules
pub fn validate_state_transition(
    previous: Option<&PokerState>,
    current: &PokerState,
    is_new_hand: bool,
) -> Result<(), Vec<String>> {
    let mut issues = Vec::new();

    // If new hand, no continuity checks needed
    if is_new_hand || previous.is_none() {
        return Ok(());
    }

    let prev = previous.unwrap();

    // RULE 1: Hero cards should not vanish or change mid-hand (unless low confidence)
    if prev.hero_cards.len() == 2
        && prev.per_field_confidence.hero_cards >= 0.85
        && current.hero_cards.len() < 2
    {
        issues.push(format!(
            "hero_cards_vanished: {} cards → {} cards",
            prev.hero_cards.len(),
            current.hero_cards.len()
        ));
    }

    // RULE 2: Board cards can only grow (0→3→4→5), not shrink
    if prev.board_cards.len() > current.board_cards.len() {
        issues.push(format!(
            "board_cards_decreased: {} cards → {} cards",
            prev.board_cards.len(),
            current.board_cards.len()
        ));
    }

    // RULE 3: Board cards progression must follow street rules
    let prev_board_len = prev.board_cards.len();
    let curr_board_len = current.board_cards.len();

    if curr_board_len > prev_board_len {
        // Check valid transitions
        let valid_transition = match (prev_board_len, curr_board_len) {
            (0, 3) => true,  // Preflop → Flop
            (3, 4) => true,  // Flop → Turn
            (4, 5) => true,  // Turn → River
            (0, 4) => true,  // Skip flop detection (rare but possible)
            (0, 5) => true,  // Skip to river (rare but possible)
            (3, 5) => true,  // Skip turn (rare but possible)
            _ => false,
        };

        if !valid_transition {
            issues.push(format!(
                "invalid_board_progression: {} → {} cards",
                prev_board_len, curr_board_len
            ));
        }
    }

    // RULE 4: Pot should not decrease mid-hand (can stay same or increase)
    if let (Some(prev_pot), Some(curr_pot)) = (prev.pot_size, current.pot_size) {
        // Allow 5% tolerance for OCR fluctuations
        if curr_pot < prev_pot * 0.95 && prev_pot > 100.0 {
            issues.push(format!(
                "pot_decreased: ${:.0} → ${:.0}",
                prev_pot, curr_pot
            ));
        }
    }

    // RULE 5: Street should progress forward or stay same
    if let (Some(ref prev_street), Some(ref curr_street)) = (&prev.street, &current.street) {
        let prev_idx = street_to_index(prev_street);
        let curr_idx = street_to_index(curr_street);

        if curr_idx < prev_idx {
            issues.push(format!(
                "street_regressed: {} → {}",
                prev_street, curr_street
            ));
        }
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

/// Smooth state transition by correcting invalid changes
pub fn smooth_state_transition(
    previous: Option<&PokerState>,
    current: PokerState,
) -> StateTransitionResult {
    let is_new_hand = detect_hand_transition(previous, &current);

    // If new hand, accept current state as-is
    if is_new_hand || previous.is_none() {
        return StateTransitionResult {
            new_state: current,
            is_new_hand,
            corrections_applied: vec![],
        };
    }

    let prev = previous.unwrap();
    let mut smoothed = current.clone();
    let mut corrections = Vec::new();

    // CORRECTION 1: Prevent hero cards from vanishing
    if prev.hero_cards.len() == 2
        && prev.per_field_confidence.hero_cards >= 0.85
        && smoothed.hero_cards.len() < 2
    {
        smoothed.hero_cards = prev.hero_cards.clone();
        smoothed.per_field_confidence.hero_cards = prev.per_field_confidence.hero_cards * 0.9;
        corrections.push("restored_hero_cards".to_string());
    }

    // CORRECTION 2: Prevent board cards from decreasing
    if prev.board_cards.len() > smoothed.board_cards.len() {
        smoothed.board_cards = prev.board_cards.clone();
        smoothed.per_field_confidence.board_cards = prev.per_field_confidence.board_cards * 0.9;
        corrections.push("restored_board_cards".to_string());
    }

    // CORRECTION 3: Prevent invalid board progression (e.g., 3→6 cards)
    if smoothed.board_cards.len() > prev.board_cards.len() {
        let valid = match (prev.board_cards.len(), smoothed.board_cards.len()) {
            (0, 3) | (3, 4) | (4, 5) | (0, 4) | (0, 5) | (3, 5) => true,
            _ => false,
        };

        if !valid {
            smoothed.board_cards = prev.board_cards.clone();
            smoothed.per_field_confidence.board_cards = prev.per_field_confidence.board_cards * 0.85;
            corrections.push("fixed_invalid_board_progression".to_string());
        }
    }

    // CORRECTION 4: Prevent pot decrease (use max of previous and current)
    if let (Some(prev_pot), Some(curr_pot)) = (prev.pot_size, smoothed.pot_size) {
        if curr_pot < prev_pot * 0.95 && prev_pot > 100.0 {
            smoothed.pot_size = Some(prev_pot);
            smoothed.per_field_confidence.pot_size = prev.per_field_confidence.pot_size * 0.9;
            corrections.push("prevented_pot_decrease".to_string());
        }
    }

    // CORRECTION 5: Prevent street regression
    if let (Some(ref prev_street), Some(ref curr_street)) = (&prev.street, &smoothed.street) {
        let prev_idx = street_to_index(prev_street);
        let curr_idx = street_to_index(curr_street);

        if curr_idx < prev_idx {
            smoothed.street = prev.street.clone();
            smoothed.per_field_confidence.street = prev.per_field_confidence.street * 0.9;
            corrections.push("prevented_street_regression".to_string());
        }
    }

    // CORRECTION 6: Ensure board length matches street
    if let Some(ref street) = smoothed.street {
        let expected_board_len = match street.as_str() {
            "preflop" => 0,
            "flop" => 3,
            "turn" => 4,
            "river" | "showdown" => 5,
            _ => smoothed.board_cards.len(), // Unknown street, keep current
        };

        if smoothed.board_cards.len() != expected_board_len
            && smoothed.per_field_confidence.street >= 0.80
            && smoothed.per_field_confidence.board_cards >= 0.80
        {
            // Trust board cards over street if board confidence is higher
            if smoothed.per_field_confidence.board_cards
                > smoothed.per_field_confidence.street
            {
                let new_street = match smoothed.board_cards.len() {
                    0 => "preflop",
                    3 => "flop",
                    4 => "turn",
                    5 => "river",
                    _ => street.as_str(),
                };
                smoothed.street = Some(new_street.to_string());
                corrections.push("corrected_street_from_board".to_string());
            }
        }
    }

    // CORRECTION 7: Carry forward high-confidence previous board cards if current is subset
    if prev.board_cards.len() == smoothed.board_cards.len()
        && prev.board_cards.len() >= 3
        && prev.per_field_confidence.board_cards >= 0.90
        && smoothed.per_field_confidence.board_cards < 0.80
    {
        // Check if first N cards match
        let prev_first_n = &prev.board_cards[..smoothed.board_cards.len()];
        let all_match = prev_first_n
            .iter()
            .zip(smoothed.board_cards.iter())
            .all(|(a, b)| cards_equal(a, b));

        if !all_match {
            smoothed.board_cards = prev.board_cards.clone();
            smoothed.per_field_confidence.board_cards = prev.per_field_confidence.board_cards;
            corrections.push("kept_high_confidence_board".to_string());
        }
    }

    // Update overall confidence if corrections were made
    if !corrections.is_empty() {
        smoothed.overall_confidence = (smoothed.per_field_confidence.hero_cards
            + smoothed.per_field_confidence.board_cards
            + smoothed.per_field_confidence.pot_size
            + smoothed.per_field_confidence.hero_position
            + smoothed.per_field_confidence.street)
            / 5.0;
    }

    StateTransitionResult {
        new_state: smoothed,
        is_new_hand,
        corrections_applied: corrections,
    }
}

/// Helper: Compare two cards for equality
fn cards_equal(a: &Card, b: &Card) -> bool {
    a.rank == b.rank && a.suit == b.suit
}

/// Helper: Convert street name to index for progression checking
fn street_to_index(street: &str) -> usize {
    match street {
        "preflop" => 0,
        "flop" => 1,
        "turn" => 2,
        "river" => 3,
        "showdown" => 4,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::poker_types::PerFieldConfidence;

    fn create_test_state(
        hero_cards: Vec<Card>,
        board_cards: Vec<Card>,
        pot: Option<f64>,
        street: Option<&str>,
        confidence: f32,
    ) -> PokerState {
        PokerState {
            hero_cards,
            board_cards,
            pot_size: pot,
            hero_position: Some("BTN".to_string()),
            street: street.map(|s| s.to_string()),
            hero_to_act: Some(true),
            recommended_action: None,
            per_field_confidence: PerFieldConfidence {
                hero_cards: confidence,
                board_cards: confidence,
                pot_size: confidence,
                hero_position: confidence,
                street: confidence,
            },
            overall_confidence: confidence,
        }
    }

    fn create_card(rank: &str, suit: &str) -> Card {
        Card {
            rank: rank.to_string(),
            suit: suit.to_string(),
        }
    }

    #[test]
    fn test_detect_pot_reset() {
        let prev = create_test_state(
            vec![create_card("A", "s"), create_card("K", "h")],
            vec![
                create_card("Q", "c"),
                create_card("J", "d"),
                create_card("T", "h"),
            ],
            Some(2500.0),
            Some("flop"),
            0.9,
        );

        let curr = create_test_state(
            vec![create_card("7", "s"), create_card("2", "h")],
            vec![],
            Some(200.0),
            Some("preflop"),
            0.9,
        );

        assert!(detect_hand_transition(Some(&prev), &curr));
    }

    #[test]
    fn test_prevent_pot_decrease() {
        let prev = create_test_state(
            vec![create_card("A", "s"), create_card("K", "h")],
            vec![
                create_card("Q", "c"),
                create_card("J", "d"),
                create_card("T", "h"),
            ],
            Some(1500.0),
            Some("flop"),
            0.9,
        );

        let curr = create_test_state(
            vec![create_card("A", "s"), create_card("K", "h")],
            vec![
                create_card("Q", "c"),
                create_card("J", "d"),
                create_card("T", "h"),
            ],
            Some(1200.0), // Pot decreased
            Some("flop"),
            0.9,
        );

        let result = smooth_state_transition(Some(&prev), curr);

        assert_eq!(result.new_state.pot_size, Some(1500.0));
        assert!(result.corrections_applied.contains(&"prevented_pot_decrease".to_string()));
    }

    #[test]
    fn test_prevent_board_decrease() {
        let prev = create_test_state(
            vec![create_card("A", "s"), create_card("K", "h")],
            vec![
                create_card("Q", "c"),
                create_card("J", "d"),
                create_card("T", "h"),
                create_card("9", "s"),
            ],
            Some(1500.0),
            Some("turn"),
            0.9,
        );

        let curr = create_test_state(
            vec![create_card("A", "s"), create_card("K", "h")],
            vec![create_card("Q", "c"), create_card("J", "d")], // Board decreased
            Some(1500.0),
            Some("turn"),
            0.9,
        );

        let result = smooth_state_transition(Some(&prev), curr);

        assert_eq!(result.new_state.board_cards.len(), 4);
        assert!(result.corrections_applied.contains(&"restored_board_cards".to_string()));
    }

    #[test]
    fn test_valid_board_progression() {
        let prev = create_test_state(
            vec![create_card("A", "s"), create_card("K", "h")],
            vec![
                create_card("Q", "c"),
                create_card("J", "d"),
                create_card("T", "h"),
            ],
            Some(1500.0),
            Some("flop"),
            0.9,
        );

        let curr = create_test_state(
            vec![create_card("A", "s"), create_card("K", "h")],
            vec![
                create_card("Q", "c"),
                create_card("J", "d"),
                create_card("T", "h"),
                create_card("9", "s"),
            ],
            Some(1800.0),
            Some("turn"),
            0.9,
        );

        let result = validate_state_transition(Some(&prev), &curr, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_new_hand_allows_all_changes() {
        let prev = create_test_state(
            vec![create_card("A", "s"), create_card("K", "h")],
            vec![
                create_card("Q", "c"),
                create_card("J", "d"),
                create_card("T", "h"),
            ],
            Some(2500.0),
            Some("flop"),
            0.9,
        );

        let curr = create_test_state(
            vec![create_card("2", "c"), create_card("7", "d")],
            vec![],
            Some(100.0),
            Some("preflop"),
            0.9,
        );

        let result = smooth_state_transition(Some(&prev), curr.clone());

        assert!(result.is_new_hand);
        assert!(result.corrections_applied.is_empty());
        assert_eq!(result.new_state.hero_cards, curr.hero_cards);
    }
}
