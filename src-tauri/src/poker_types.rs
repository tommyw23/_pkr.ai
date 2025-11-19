// src-tauri/src/poker_types.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Card {
    pub rank: String, // "2"-"9", "T", "J", "Q", "K", "A"
    pub suit: String, // "c", "d", "h", "s"
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PerFieldConfidence {
    #[serde(rename = "heroCards")]
    pub hero_cards: f32,
    #[serde(rename = "boardCards")]
    pub board_cards: f32,
    #[serde(rename = "potSize")]
    pub pot_size: f32,
    #[serde(rename = "heroPosition")]
    pub hero_position: f32,
    pub street: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PokerState {
    #[serde(rename = "heroCards")]
    pub hero_cards: Vec<Card>,
    #[serde(rename = "boardCards")]
    pub board_cards: Vec<Card>,
    #[serde(rename = "potSize")]
    pub pot_size: Option<f64>,
    #[serde(rename = "heroPosition")]
    pub hero_position: Option<String>,
    pub street: Option<String>, // "preflop", "flop", "turn", "river", "showdown"
    #[serde(rename = "heroToAct")]
    pub hero_to_act: Option<bool>,
    #[serde(rename = "recommendedAction")]
    pub recommended_action: Option<String>, // "fold", "call", "raise", "check"
    #[serde(rename = "perFieldConfidence")]
    pub per_field_confidence: PerFieldConfidence,
    #[serde(rename = "overallConfidence")]
    pub overall_confidence: f32,
}

impl Card {
    pub fn to_display(&self) -> String {
        let suit_symbol = match self.suit.as_str() {
            "c" => "♣",
            "d" => "♦",
            "h" => "♥",
            "s" => "♠",
            _ => &self.suit,
        };
        format!("{}{}", self.rank, suit_symbol)
    }
}

impl PokerState {
    pub fn to_display_cards(cards: &[Card]) -> Vec<String> {
        cards.iter().map(|c| c.to_display()).collect()
    }
}