// src-tauri/src/vision/openai_o4mini.rs
// OpenAI o4-mini vision model integration for poker screenshot analysis
// Pure data extraction - NO hand evaluation or strategy

use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose};

/// Raw vision output - pure data extraction from screenshot
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RawVisionData {
    /// Hero's hole cards - may contain null for undetected cards
    pub hero_cards: Vec<Option<String>>,
    /// Community cards - may contain null for undealt/undetected cards
    pub community_cards: Vec<Option<String>>,
    pub pot: Option<f64>,
    pub position: Option<String>,
    pub available_actions: Vec<String>,
    #[serde(default)]
    pub amount_to_call: f64,
    pub hero_stack: Option<f64>,
}

impl RawVisionData {
    /// Filter out null values from hero_cards, returning only valid card strings
    pub fn hero_cards_filtered(&self) -> Vec<String> {
        self.hero_cards
            .iter()
            .filter_map(|c| c.clone())
            .collect()
    }

    /// Filter out null values from community_cards, returning only valid card strings
    pub fn community_cards_filtered(&self) -> Vec<String> {
        self.community_cards
            .iter()
            .filter_map(|c| c.clone())
            .collect()
    }
}

#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: Vec<ContentPart>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum ContentPart {
    Text {
        #[serde(rename = "type")]
        content_type: String,
        text: String,
    },
    ImageUrl {
        #[serde(rename = "type")]
        content_type: String,
        image_url: ImageUrl,
    },
}

#[derive(Serialize)]
struct ImageUrl {
    url: String,
}

#[derive(Deserialize)]
struct OpenAIResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

use std::collections::HashSet;

/// Validate card format: must be rank+suit like "A♠", "Ks", "T♣"
pub fn is_valid_card(card: &str) -> bool {
    // Handle 2-3 character cards (e.g., "A♠", "Ks", "T♣", "10♠")
    let card = card.replace("10", "T");

    let chars: Vec<char> = card.chars().collect();
    if chars.len() < 2 {
        return false;
    }

    let rank = chars[0].to_ascii_uppercase();
    let suit_part: String = chars[1..].iter().collect();

    let valid_ranks = ['A', 'K', 'Q', 'J', 'T', '9', '8', '7', '6', '5', '4', '3', '2'];
    let valid_suits = ["♠", "♥", "♦", "♣", "s", "h", "d", "c", "S", "H", "D", "C"];

    valid_ranks.contains(&rank) && valid_suits.contains(&suit_part.as_str())
}

/// Check for duplicate cards across hero + community (both may contain null values)
pub fn has_duplicate_cards(hero: &[Option<String>], community: &[Option<String>]) -> bool {
    let mut seen = HashSet::new();

    for opt_card in hero {
        if let Some(card) = opt_card {
            let normalized = normalize_card(card);
            if !seen.insert(normalized) {
                return true;
            }
        }
    }

    for opt_card in community {
        if let Some(card) = opt_card {
            let normalized = normalize_card(card);
            if !seen.insert(normalized) {
                return true;
            }
        }
    }

    false
}

/// Normalize card string for comparison (lowercase, 10→T)
fn normalize_card(card: &str) -> String {
    card.to_lowercase()
        .replace("10", "t")
        .replace("♠", "s")
        .replace("♥", "h")
        .replace("♦", "d")
        .replace("♣", "c")
}

/// Validate OpenAI response, returns issues list
pub fn validate_vision_response(data: &RawVisionData) -> Vec<String> {
    let mut issues = Vec::new();

    // Check for malformed cards in hero hand (skip nulls)
    for opt_card in &data.hero_cards {
        if let Some(card) = opt_card {
            if !is_valid_card(card) {
                issues.push(format!("malformed_hero_card: {}", card));
            }
        }
    }

    // Check for malformed cards in community (skip nulls)
    for opt_card in &data.community_cards {
        if let Some(card) = opt_card {
            if !is_valid_card(card) {
                issues.push(format!("malformed_community_card: {}", card));
            }
        }
    }

    // Check for duplicates across all cards
    if has_duplicate_cards(&data.hero_cards, &data.community_cards) {
        issues.push("duplicate_cards_detected".to_string());
    }

    // Check hero card count (should be 0 or 2)
    if !data.hero_cards.is_empty() && data.hero_cards.len() != 2 {
        issues.push(format!("invalid_hero_count: {}", data.hero_cards.len()));
    }

    issues
}

/// Get site-specific hints for the vision prompt
fn get_site_hints(site_name: Option<&str>) -> &'static str {
    match site_name {
        Some("replay") => r#"
SITE-SPECIFIC NOTES (Replay Poker):
- Browser-based free poker site with SMALLER card graphics
- Hero cards appear in the BOTTOM-LEFT area of the table (not center!)
- Suit icons are THINNER and may appear faded/lighter
- Pay close attention to suit COLORS: RED = hearts (♥) or diamonds (♦), BLACK = spades (♠) or clubs (♣)
- Clubs have a CLOVER shape (three-leaf), Spades are POINTED upward
- Cards may have a white or light background"#,
        Some("ignition") | Some("bovada") => r#"
SITE-SPECIFIC NOTES (Ignition/Bovada):
- SPATIAL LAYOUT:
  • Hero's 2 hole cards: BOTTOM CENTER of screen, larger cards with slight overlap
  • Community cards: 5-card HORIZONTAL ROW at TABLE CENTER (middle of screen)
  • DO NOT confuse these two areas - they are physically separated

- CRITICAL UNIQUENESS RULE:
  • A card can only appear ONCE across all 7 cards total
  • If you see 4♠ in hero hand, it CANNOT appear in community cards
  • If you detect a duplicate, re-examine - one detection is wrong

- CARD FORMAT REQUIREMENTS:
  • Each card must be: rank + suit (e.g., "A♠", "K♥", "Qd", "T♣", "2♠")
  • Valid ranks: A, K, Q, J, T, 9, 8, 7, 6, 5, 4, 3, 2
  • Valid suits: ♠ ♥ ♦ ♣ (or s h d c)
  • Single letters like "S", "D" alone are INVALID
  • "10" should be written as "T"

- UNCERTAINTY HANDLING:
  • If you cannot clearly read a card's rank or suit, return null for that position
  • Better to return null than guess wrong
  • DO NOT return partial cards like just a suit letter"#,
        Some("acr") => r#"
SITE-SPECIFIC NOTES (Americas Cardroom):
- Clear suit symbols, similar layout to Ignition
- Hero cards at bottom-center"#,
        _ => ""
    }
}

/// Extract raw data from poker screenshot using OpenAI o4-mini (GPT-4o-mini)
/// Pure data extraction - NO hand evaluation or strategy recommendations
pub async fn extract_poker_data(image_data: &[u8], site_name: Option<&str>) -> Result<RawVisionData, String> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .map_err(|_| "OPENAI_API_KEY not found in environment".to_string())?;

    let base64_image = general_purpose::STANDARD.encode(image_data);
    let data_url = format!("data:image/png;base64,{}", base64_image);

    // Get site-specific hints
    let site_hints = get_site_hints(site_name);
    let site_label = site_name.unwrap_or("unknown");

    let prompt = format!(r#"Extract poker data from this {} poker screenshot and return ONLY a JSON object (no markdown, no explanations):
{}

EXAMPLE OUTPUT:
{{
  "heroCards": ["K♠", "K♥"],
  "communityCards": ["J♣", "K♦", "T♥", null, null],
  "pot": 0.26,
  "position": "BTN",
  "availableActions": ["FOLD", "CALL $0.10", "RAISE"],
  "amountToCall": 0.10,
  "heroStack": 27.35
}}

CRITICAL - SUIT IDENTIFICATION (most common error source):

Both BLACK suits:
- ♠ SPADES = Single pointed top like an upside-down heart. Has a small stem at the bottom.
- ♣ CLUBS = THREE rounded lobes arranged like a clover/trefoil. No point at top.

Both RED suits:
- ♥ HEARTS = TWO rounded bumps at top, single point at bottom. Classic heart shape.
- ♦ DIAMONDS = FOUR pointed corners in a rotated square/rhombus. NO curves anywhere.

VERIFICATION STEPS before returning each card:
1. Is the suit BLACK or RED?
2. If BLACK: Does it have a POINTED top (spade) or THREE ROUNDED lobes (club)?
3. If RED: Does it have CURVED bumps at top (heart) or FOUR SHARP corners (diamond)?

If uncertain between ♣/♠: Look for the 3-lobe clover pattern. Spades are pointed, clubs are rounded.
If uncertain between ♦/♥: Look for sharp corners (diamond) vs curved top (heart).

EXTRACTION RULES:
- heroCards: 2 hole cards at bottom center. Use EXACT Unicode suit symbols:
  • ♠ (spades - pointed top, stem at bottom)
  • ♣ (clubs - three rounded lobes)
  • ♥ (hearts - curved top bumps)
  • ♦ (diamonds - four sharp corners)
  Double-check EVERY suit symbol before responding. Suit errors are the #1 cause of incorrect recommendations.
  If cards are face-down or unclear, use [].
- communityCards: Use exactly 5 entries in communityCards array. Use null for cards not visible yet. Examples: ["A♠", "K♥", "Q♦", null, null] for flop, or [null, null, null, null, null] for preflop.
- pot: Numeric pot value visible on screen, or null if not visible.
- position: Detect hero's position by:
  1. Look for the dealer button (white/yellow chip marked "D", "DEALER", or dealer chip icon)
  2. Find which player seat has the dealer button
  3. Determine hero's position based on seats from dealer:
     - Has dealer button = "BTN" (Button)
     - 1 seat left of dealer (clockwise) = "SB" (Small Blind)
     - 2 seats left of dealer = "BB" (Big Blind)
     - 3 seats left at 6-max table = "UTG" (Under the Gun)
     - 4 seats left at 6-max = "MP" (Middle Position)
     - 5 seats left at 6-max (right of dealer) = "CO" (Cutoff)
  4. Alternative indicators if dealer button not clear:
     - Look for position labels on or near seats (BTN, SB, BB, CO, etc.)
     - Check for blind chips (smaller chip = SB, larger = BB)
     - Hero's seat may be highlighted or marked differently
  5. Common visual cues: dealer button is usually a white/yellow disc, position may be shown as text overlay
  6. Return null ONLY if absolutely no position indicators are visible
- availableActions: Extract the EXACT text from each visible action button including dollar amounts (e.g., ["FOLD", "CHECK", "CALL $0.10", "RAISE TO $0.75", "BET", "ALL-IN"]). If a button is grayed out or disabled, do not include it. If not visible, use [].
- amountToCall: If there is a CALL button with a dollar amount, extract that number (e.g., "CALL $0.10" → 0.10). If there is a CHECK button and no CALL amount, set to 0. If you cannot read amountToCall from CALL button, set to 0.
- heroStack: Hero's chip stack amount if visible, or null if not visible.

HARD GUARDRAILS (CRITICAL):
- DO NOT evaluate hand strength.
- DO NOT say if it is a pair, straight, flush, etc.
- DO NOT recommend actions.
- DO NOT provide reasoning or explanations.
- ONLY read text and cards from the screen and output JSON.
- Do not guess hidden cards or stacks. If unclear, use null.

Return ONLY valid JSON, nothing else."#, site_label, site_hints);

    let request = OpenAIRequest {
        model: "gpt-4o-mini".to_string(), // o4-mini is accessed via gpt-4o-mini endpoint
        max_tokens: 1024,
        temperature: 0.0, // Deterministic for consistent results
        messages: vec![Message {
            role: "user".to_string(),
            content: vec![
                ContentPart::Text {
                    content_type: "text".to_string(),
                    text: prompt.to_string(),
                },
                ContentPart::ImageUrl {
                    content_type: "image_url".to_string(),
                    image_url: ImageUrl { url: data_url },
                },
            ],
        }],
    };

    let client = reqwest::Client::new();
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("OpenAI API error: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();

        // Check for rate limit
        if status.as_u16() == 429 {
            return Err("429_RATE_LIMIT".to_string());
        }

        return Err(format!("OpenAI API error ({}): {}", status, error_text));
    }

    let openai_response: OpenAIResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse OpenAI response: {}", e))?;

    let response_text = openai_response
        .choices
        .first()
        .map(|c| c.message.content.as_str())
        .ok_or("No response from OpenAI")?;

    // Strip markdown if present
    let clean_text = response_text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let mut raw_data: RawVisionData = serde_json::from_str(clean_text)
        .map_err(|e| format!("Failed to parse OpenAI output: {}. Response: {}", e, clean_text))?;

    // Post-process: normalize card strings ("10♠" → "T♠") and handle nulls
    raw_data.hero_cards = raw_data.hero_cards
        .into_iter()
        .map(|opt_card| opt_card.map(|card| card.replace("10", "T")))
        .collect();

    raw_data.community_cards = raw_data.community_cards
        .into_iter()
        .map(|opt_card| opt_card.map(|card| card.replace("10", "T")))
        .collect();

    // Clamp negative amountToCall to 0
    if raw_data.amount_to_call < 0.0 {
        raw_data.amount_to_call = 0.0;
    }

    Ok(raw_data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_url_format() {
        let test_data = b"test image data";
        let base64 = general_purpose::STANDARD.encode(test_data);
        let data_url = format!("data:image/png;base64,{}", base64);

        assert!(data_url.starts_with("data:image/png;base64,"));
        assert!(data_url.contains(&base64));
    }

    #[test]
    fn test_request_serialization() {
        let request = OpenAIRequest {
            model: "gpt-4o-mini".to_string(),
            max_tokens: 1024,
            temperature: 0.0,
            messages: vec![Message {
                role: "user".to_string(),
                content: vec![
                    ContentPart::Text {
                        content_type: "text".to_string(),
                        text: "test".to_string(),
                    },
                ],
            }],
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("gpt-4o-mini"));
        assert!(json.contains("\"temperature\":0.0"));
    }

    #[test]
    fn test_valid_cards() {
        // Unicode suits
        assert!(is_valid_card("A♠"));
        assert!(is_valid_card("K♥"));
        assert!(is_valid_card("Q♦"));
        assert!(is_valid_card("J♣"));
        assert!(is_valid_card("T♠"));
        assert!(is_valid_card("9♥"));
        assert!(is_valid_card("2♣"));

        // "10" should be converted to "T" and be valid
        assert!(is_valid_card("10♥"));
        assert!(is_valid_card("10s"));

        // Letter suits (case insensitive)
        assert!(is_valid_card("Td"));
        assert!(is_valid_card("As"));
        assert!(is_valid_card("KH"));
        assert!(is_valid_card("Qc"));

        // Invalid cards
        assert!(!is_valid_card("S"));      // Single letter
        assert!(!is_valid_card("D"));      // Single letter
        assert!(!is_valid_card(""));       // Empty
        assert!(!is_valid_card("X♠"));     // Invalid rank
        assert!(!is_valid_card("Ax"));     // Invalid suit
        assert!(!is_valid_card("1♠"));     // Invalid rank (1 is not valid)
    }

    #[test]
    fn test_normalize_card() {
        // Both "10" and "T" should normalize to the same value
        assert_eq!(normalize_card("10♠"), normalize_card("T♠"));
        assert_eq!(normalize_card("10s"), normalize_card("Ts"));

        // Unicode and letter suits should normalize the same
        assert_eq!(normalize_card("A♠"), normalize_card("As"));
        assert_eq!(normalize_card("K♥"), normalize_card("Kh"));
        assert_eq!(normalize_card("Q♦"), normalize_card("Qd"));
        assert_eq!(normalize_card("J♣"), normalize_card("Jc"));
    }

    #[test]
    fn test_duplicate_detection() {
        // Same card in hero and community (unicode)
        let hero = vec!["A♠".to_string(), "K♥".to_string()];
        let community = vec![Some("A♠".to_string()), None, None, None, None];
        assert!(has_duplicate_cards(&hero, &community));

        // Same card but different representations (unicode vs letter)
        let hero2 = vec!["A♠".to_string(), "K♥".to_string()];
        let community2 = vec![Some("As".to_string()), None, None, None, None];
        assert!(has_duplicate_cards(&hero2, &community2));

        // Same card but 10 vs T
        let hero3 = vec!["10♠".to_string(), "K♥".to_string()];
        let community3 = vec![Some("T♠".to_string()), None, None, None, None];
        assert!(has_duplicate_cards(&hero3, &community3));

        // No duplicates
        let hero4 = vec!["A♠".to_string(), "K♥".to_string()];
        let community4 = vec![Some("Q♦".to_string()), Some("J♣".to_string()), Some("T♠".to_string()), None, None];
        assert!(!has_duplicate_cards(&hero4, &community4));

        // Duplicate within hero cards
        let hero5 = vec!["A♠".to_string(), "A♠".to_string()];
        let community5: Vec<Option<String>> = vec![None, None, None, None, None];
        assert!(has_duplicate_cards(&hero5, &community5));
    }

    #[test]
    fn test_validate_vision_response() {
        // Valid response
        let valid_data = RawVisionData {
            hero_cards: vec!["A♠".to_string(), "K♥".to_string()],
            community_cards: vec![Some("Q♦".to_string()), Some("J♣".to_string()), Some("T♠".to_string()), None, None],
            pot: Some(10.0),
            position: Some("BTN".to_string()),
            available_actions: vec!["FOLD".to_string(), "CALL".to_string()],
            amount_to_call: 0.5,
            hero_stack: Some(100.0),
        };
        assert!(validate_vision_response(&valid_data).is_empty());

        // Malformed card
        let malformed_data = RawVisionData {
            hero_cards: vec!["A♠".to_string(), "S".to_string()],  // "S" is invalid
            community_cards: vec![None, None, None, None, None],
            pot: Some(10.0),
            position: None,
            available_actions: vec![],
            amount_to_call: 0.0,
            hero_stack: None,
        };
        let issues = validate_vision_response(&malformed_data);
        assert!(issues.iter().any(|i| i.contains("malformed_hero_card")));

        // Duplicate cards
        let duplicate_data = RawVisionData {
            hero_cards: vec!["A♠".to_string(), "K♥".to_string()],
            community_cards: vec![Some("A♠".to_string()), None, None, None, None],  // Duplicate A♠
            pot: Some(10.0),
            position: None,
            available_actions: vec![],
            amount_to_call: 0.0,
            hero_stack: None,
        };
        let issues2 = validate_vision_response(&duplicate_data);
        assert!(issues2.iter().any(|i| i.contains("duplicate_cards_detected")));

        // Invalid hero count (1 card)
        let invalid_count = RawVisionData {
            hero_cards: vec!["A♠".to_string()],  // Only 1 card
            community_cards: vec![None, None, None, None, None],
            pot: Some(10.0),
            position: None,
            available_actions: vec![],
            amount_to_call: 0.0,
            hero_stack: None,
        };
        let issues3 = validate_vision_response(&invalid_count);
        assert!(issues3.iter().any(|i| i.contains("invalid_hero_count")));
    }
}
