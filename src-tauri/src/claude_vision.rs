// src-tauri/src/claude_vision.rs

use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose};
use crate::poker_types::PokerState;

#[derive(Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: Vec<ContentBlock>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum ContentBlock {
    Text { 
        #[serde(rename = "type")]
        content_type: String,
        text: String 
    },
    Image { 
        #[serde(rename = "type")]
        content_type: String,
        source: ImageSource 
    },
}

#[derive(Serialize)]
struct ImageSource {
    #[serde(rename = "type")]
    source_type: String,
    media_type: String,
    data: String,
}

#[derive(Deserialize)]
struct ClaudeResponse {
    content: Vec<ResponseContent>,
}

#[derive(Deserialize)]
struct ResponseContent {
    text: String,
}

pub async fn analyze_with_claude(
    image_data: &[u8],
    previous_state: Option<&PokerState>,
    gemini_output: &str,
    issues: &[String],
) -> Result<PokerState, String> {
    let api_key = std::env::var("CLAUDE_API_KEY")
        .map_err(|_| "CLAUDE_API_KEY not found in environment".to_string())?;
    
    let base64_image = general_purpose::STANDARD.encode(image_data);
    
    // Serialize previous state if available
    let previous_state_json = previous_state
        .map(|s| serde_json::to_string_pretty(s).unwrap_or_else(|_| "null".to_string()))
        .unwrap_or_else(|| "null".to_string());
    
    let issues_str = if issues.is_empty() {
        "None".to_string()
    } else {
        format!("[{}]", issues.iter().map(|i| format!("\"{}\"", i)).collect::<Vec<_>>().join(", "))
    };
    
    let prompt = format!(r#"You are the SECOND-STAGE vision model in a cascading poker HUD system.

Step 1 is a cheaper model (Gemini) that reads the screenshot first.
You are Step 2: you CORRECT its mistakes when the validation system detects problems or low confidence.

You are analyzing a screenshot of an online No-Limit Hold'em poker hand.

CRITICAL REQUIREMENTS
---------------------
1. You MUST return ONLY valid JSON. No explanations, no comments, no markdown.
2. Follow the schema EXACTLY (fields, types, and allowed values).
3. Do NOT invent cards or pot sizes that are not clearly visible on the screenshot.
4. If something is unclear, represent that by:
   - Returning an EMPTY ARRAY [] for cards you cannot confidently read (heroCards or boardCards).
   - Lowering the corresponding confidence field (e.g. heroCards confidence = 0.1).
   - Never by using null for rank, suit, or the card arrays themselves.

SCHEMA (STRICT)
---------------
type Card = {{
  rank: "2"|"3"|"4"|"5"|"6"|"7"|"8"|"9"|"T"|"J"|"Q"|"K"|"A",
  suit: "c"|"d"|"h"|"s"
}};

type Street = "preflop" | "flop" | "turn" | "river" | "showdown";

type PokerState = {{
  heroCards: Card[];
  boardCards: Card[];
  potSize: number | null;
  heroPosition: string | null;
  street: Street | null;
  heroToAct: boolean | null;
  recommendedAction: "fold" | "call" | "raise" | "check" | null;
  perFieldConfidence: {{
    heroCards: number;
    boardCards: number;
    potSize: number;
    heroPosition: number;
    street: number;
  }};
  overallConfidence: number;
}};

CONSTRAINTS ON CARDS
--------------------
- heroCards must ALWAYS be an array (0–2 cards for Hold'em). NEVER null.
- boardCards must ALWAYS be an array (0–5 cards). NEVER null.
- Each Card MUST have a non-null rank and suit using ONLY the allowed values above.
- If you are not confident about a card's rank or suit, OMIT that card from the array and lower the corresponding confidence.
- Do NOT output fake cards like "0" rank or any suit outside "c","d","h","s".
- Across heroCards + boardCards combined, there must be NO duplicate physical cards (same rank AND suit).

TEMPORAL / CONTINUITY CONSTRAINTS
---------------------------------
You may be given a previous state from an earlier frame of the SAME HAND.

- In a single hand:
  - Hero hole cards should NOT change once known with high confidence.
  - Board cards should only grow over time: 0 -> 3 -> 4 -> 5; they should not shrink or change to different cards.
- If the new screenshot clearly shows a NEW hand (e.g., hero cards look different and previous hand ended), you may reset heroCards/boardCards, but lower confidence accordingly.

Use this logic:
- If previous heroCards had high confidence (>= 0.9) and you are unsure now, it is better to KEEP the previous heroCards than to invent new ones.
- If previous boardCards had high confidence and the current image is ambiguous, KEEP the previous boardCards and set a lower boardCards confidence to reflect uncertainty.
- Only change a previously high-confidence card if the screenshot clearly shows that it is different (e.g., a new hand, a new board card is visibly added).

INPUTS YOU RECEIVE
------------------
PREVIOUS_STATE_JSON:
{}

GEMINI_OUTPUT_JSON:
{}

ISSUE_LIST:
{}

YOUR TASK
---------
1. Carefully re-analyze the screenshot.
2. Use PREVIOUS_STATE_JSON to maintain continuity when appropriate (hero cards stable within a hand, board cards only growing).
3. Use GEMINI_OUTPUT_JSON as a noisy first draft: fix all inconsistencies, illegal values, and low-confidence mistakes.
4. Ensure:
   - heroCards and boardCards are arrays (never null).
   - No Card object has null rank or suit.
   - No duplicate cards exist across hero and board.
   - The board length is consistent with the street.
5. Set perFieldConfidence and overallConfidence to reflect your true certainty.
6. If you cannot confidently read hero cards or board cards, return [] for that array and low confidence for that field.

OUTPUT FORMAT
-------------
Return ONLY a single valid PokerState JSON object. No markdown, no comments, no text."#,
        previous_state_json,
        gemini_output,
        issues_str
    );

    // Rest of the function stays the same...

    let request = ClaudeRequest {
        model: "claude-3-5-haiku-20241022".to_string(),
        max_tokens: 1024,
        messages: vec![Message {
            role: "user".to_string(),
            content: vec![
                ContentBlock::Image {
                    content_type: "image".to_string(),
                    source: ImageSource {
                        source_type: "base64".to_string(),
                        media_type: "image/png".to_string(),
                        data: base64_image,
                    },
                },
                ContentBlock::Text {
                    content_type: "text".to_string(),
                    text: prompt,
                },
            ],
        }],
    };

    let client = reqwest::Client::new();
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Claude API error: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Claude API error ({}): {}", status, error_text));
    }

    let claude_response: ClaudeResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Claude response: {}", e))?;

    let response_text = claude_response
        .content
        .first()
        .map(|c| c.text.as_str())
        .ok_or("No response from Claude")?;

    // Strip markdown if present
    let clean_text = response_text
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let poker_state: PokerState = serde_json::from_str(clean_text)
        .map_err(|e| format!("Failed to parse Claude output: {}. Response: {}", e, clean_text))?;

    Ok(poker_state)
}