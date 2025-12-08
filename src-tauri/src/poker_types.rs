// src-tauri/src/poker_types.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rank {
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Jack,
    Queen,
    King,
    Ace,
}

impl Rank {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "2" => Some(Rank::Two),
            "3" => Some(Rank::Three),
            "4" => Some(Rank::Four),
            "5" => Some(Rank::Five),
            "6" => Some(Rank::Six),
            "7" => Some(Rank::Seven),
            "8" => Some(Rank::Eight),
            "9" => Some(Rank::Nine),
            "T" | "10" => Some(Rank::Ten),
            "J" => Some(Rank::Jack),
            "Q" => Some(Rank::Queen),
            "K" => Some(Rank::King),
            "A" => Some(Rank::Ace),
            _ => None,
        }
    }

    pub fn to_str(&self) -> &'static str {
        match self {
            Rank::Two => "2",
            Rank::Three => "3",
            Rank::Four => "4",
            Rank::Five => "5",
            Rank::Six => "6",
            Rank::Seven => "7",
            Rank::Eight => "8",
            Rank::Nine => "9",
            Rank::Ten => "T",
            Rank::Jack => "J",
            Rank::Queen => "Q",
            Rank::King => "K",
            Rank::Ace => "A",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

impl Suit {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "c" | "clubs" => Some(Suit::Clubs),
            "d" | "diamonds" => Some(Suit::Diamonds),
            "h" | "hearts" => Some(Suit::Hearts),
            "s" | "spades" => Some(Suit::Spades),
            _ => None,
        }
    }

    pub fn to_str(&self) -> &'static str {
        match self {
            Suit::Clubs => "c",
            Suit::Diamonds => "d",
            Suit::Hearts => "h",
            Suit::Spades => "s",
        }
    }

    pub fn to_symbol(&self) -> &'static str {
        match self {
            Suit::Clubs => "♣",
            Suit::Diamonds => "♦",
            Suit::Hearts => "♥",
            Suit::Spades => "♠",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub rank: Rank,
    pub suit: Suit,
}

impl Card {
    pub fn from_str(rank: &str, suit: &str) -> Option<Self> {
        let rank = Rank::from_str(rank)?;
        let suit = Suit::from_str(suit)?;
        Some(Card { rank, suit })
    }
}

// Custom serialization to maintain compatibility with JSON API
impl Serialize for Card {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Card", 2)?;
        state.serialize_field("rank", self.rank.to_str())?;
        state.serialize_field("suit", self.suit.to_str())?;
        state.end()
    }
}

// Custom deserialization to parse from JSON string fields
impl<'de> Deserialize<'de> for Card {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct CardVisitor;

        impl<'de> Visitor<'de> for CardVisitor {
            type Value = Card;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a card with rank and suit fields")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Card, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut rank: Option<String> = None;
                let mut suit: Option<String> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "rank" => rank = Some(map.next_value()?),
                        "suit" => suit = Some(map.next_value()?),
                        _ => { let _: serde::de::IgnoredAny = map.next_value()?; }
                    }
                }

                let rank_str = rank.ok_or_else(|| de::Error::missing_field("rank"))?;
                let suit_str = suit.ok_or_else(|| de::Error::missing_field("suit"))?;

                let rank = Rank::from_str(&rank_str)
                    .ok_or_else(|| de::Error::custom(format!("invalid rank: {}", rank_str)))?;
                let suit = Suit::from_str(&suit_str)
                    .ok_or_else(|| de::Error::custom(format!("invalid suit: {}", suit_str)))?;

                Ok(Card { rank, suit })
            }
        }

        deserializer.deserialize_map(CardVisitor)
    }
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
pub struct AIRecommendation {
    pub action: String, // "FOLD", "CHECK", "CALL", "RAISE"
    pub amount: Option<f64>,
    pub reasoning: String,
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
    #[serde(rename = "callAmount", skip_serializing_if = "Option::is_none", default)]
    pub call_amount: Option<f64>, // Dollar amount shown on CALL button if visible
    #[serde(rename = "facingBet", skip_serializing_if = "Option::is_none", default)]
    pub facing_bet: Option<bool>, // True if there's a bet/raise to respond to
    #[serde(rename = "recommendedAction")]
    pub recommended_action: Option<String>, // "fold", "call", "raise", "check" (deprecated - use ai_recommendation)
    #[serde(rename = "aiRecommendation", skip_serializing_if = "Option::is_none", default)]
    pub ai_recommendation: Option<AIRecommendation>, // AI-generated recommendation
    #[serde(rename = "availableActions", skip_serializing_if = "Option::is_none", default)]
    pub available_actions: Option<Vec<String>>, // e.g., ["fold", "call", "raise"] or ["check", "raise"]
    #[serde(rename = "amountToCall", skip_serializing_if = "Option::is_none", default)]
    pub amount_to_call: Option<f64>, // Amount needed to call (from callAmount or extracted from available actions)
    #[serde(rename = "heroStack", skip_serializing_if = "Option::is_none", default)]
    pub hero_stack: Option<f64>, // Hero's remaining chip stack
    #[serde(rename = "perFieldConfidence")]
    pub per_field_confidence: PerFieldConfidence,
    #[serde(rename = "overallConfidence")]
    pub overall_confidence: f32,
}

impl Card {
    pub fn to_display(&self) -> String {
        format!("{}{}", self.rank.to_str(), self.suit.to_symbol())
    }
}

impl PokerState {
    pub fn to_display_cards(cards: &[Card]) -> Vec<String> {
        cards.iter().map(|c| c.to_display()).collect()
    }
}

/// Legal actions parsed from AI's available actions
#[derive(Debug, Clone, PartialEq)]
pub enum LegalAction {
    Fold,
    Check,
    Call(f64), // Amount to call
    Bet,
    Raise,
}