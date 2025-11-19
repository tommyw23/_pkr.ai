// src-tauri/src/gemini.rs

use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose};
use crate::poker_types::PokerState;

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<Content>,
}

#[derive(Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum Part {
    Text { text: String },
    InlineData { inline_data: InlineData },
}

#[derive(Serialize)]
struct InlineData {
    mime_type: String,
    data: String,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Vec<Candidate>,
}

#[derive(Deserialize)]
struct Candidate {
    content: ResponseContent,
}

#[derive(Deserialize)]
struct ResponseContent {
    parts: Vec<ResponsePart>,
}

#[derive(Deserialize)]
struct ResponsePart {
    text: String,
}

pub async fn analyze_poker_screenshot(image_data: &[u8]) -> Result<PokerState, String> {
    let api_key = std::env::var("GEMINI_API_KEY")
        .map_err(|_| "GEMINI_API_KEY not found in environment".to_string())?;
    
    let base64_image = general_purpose::STANDARD.encode(image_data);
    
    let prompt = r#"Analyze this poker screenshot and return ONLY a JSON object (no markdown, no explanations):

{
  "heroCards": [{"rank": "A", "suit": "s"}, {"rank": "K", "suit": "h"}],
  "boardCards": [{"rank": "Q", "suit": "c"}, {"rank": "J", "suit": "d"}, {"rank": "T", "suit": "h"}],
  "potSize": 1250,
  "heroPosition": "BTN",
  "street": "flop",
  "heroToAct": true,
  "recommendedAction": null,
  "perFieldConfidence": {
    "heroCards": 0.95,
    "boardCards": 0.92,
    "potSize": 0.88,
    "heroPosition": 0.90,
    "street": 0.95
  },
  "overallConfidence": 0.92
}

CRITICAL RULES:
- heroCards: 2 hole cards at bottom center. Ranks: "2"-"9","T","J","Q","K","A". Suits: "c","d","h","s"
- boardCards: 0-5 community cards in center of table
- street: "preflop" (0 board cards), "flop" (3), "turn" (4), "river" (5), "showdown" (5)
- potSize: numeric pot value, or null if not visible
- heroPosition: "BTN", "SB", "BB", "UTG", "MP", "CO", or null
- heroToAct: true if it's hero's turn, false otherwise
- If cards are unclear or face-down, use [] and set confidence < 0.80
- perFieldConfidence: score 0.0-1.0 for each field's accuracy
- overallConfidence: average confidence across all fields
- Return ONLY valid JSON, nothing else"#;

    let request = GeminiRequest {
        contents: vec![Content {
            parts: vec![
                Part::Text { text: prompt.to_string() },
                Part::InlineData {
                    inline_data: InlineData {
                        mime_type: "image/png".to_string(),
                        data: base64_image,
                    },
                },
            ],
        }],
    };

    let client = reqwest::Client::new();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash-lite:generateContent?key={}",
        api_key
    );

    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Gemini API error: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Gemini API error ({}): {}", status, error_text));
    }

    let gemini_response: GeminiResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Gemini response: {}", e))?;

    let response_text = gemini_response
        .candidates
        .first()
        .and_then(|c| c.content.parts.first())
        .map(|p| p.text.as_str())
        .ok_or("No response from Gemini")?;

    // Strip markdown
    let clean_text = response_text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let poker_state: PokerState = serde_json::from_str(clean_text)
        .map_err(|e| format!("Failed to parse poker state: {}. Response: {}", e, clean_text))?;

    Ok(poker_state)
}