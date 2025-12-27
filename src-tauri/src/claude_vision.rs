// src-tauri/src/claude_vision.rs

use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose};
use crate::poker_types::PokerState;
use crate::vision::openai_o4mini::RawVisionData;

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
    tier1_output: &str,
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

Step 1 is a cheaper/faster model (OpenAI o4-mini) that reads the screenshot first.
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
type ActionButton = "fold" | "call" | "raise" | "check" | "all-in";

type AIRecommendation = {{
  action: "FOLD" | "CHECK" | "CALL" | "RAISE";
  amount: number | null;
  reasoning: string;
}};

type PokerState = {{
  heroCards: Card[];
  boardCards: Card[];
  potSize: number | null;
  heroPosition: string | null;
  street: Street | null;
  heroToAct: boolean | null;
  callAmount: number | null;
  facingBet: boolean | null;
  recommendedAction: "fold" | "call" | "raise" | "check" | null;
  aiRecommendation: AIRecommendation | null;
  availableActions: ActionButton[] | null;
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

TIER1_OUTPUT_JSON (from OpenAI o4-mini):
{}

ISSUE_LIST:
{}

YOUR TASK
---------
1. Carefully re-analyze the screenshot.
2. Use PREVIOUS_STATE_JSON to maintain continuity when appropriate (hero cards stable within a hand, board cards only growing).
3. Use TIER1_OUTPUT_JSON as a noisy first draft: fix all inconsistencies, illegal values, and low-confidence mistakes.
4. Ensure:
   - heroCards and boardCards are arrays (never null).
   - No Card object has null rank or suit.
   - No duplicate cards exist across hero and board.
   - The board length is consistent with the street.
   - callAmount: Look for action buttons on screen. If you see "CALL $X.XX" button, extract the dollar amount (e.g., if button shows "CALL $2.50", return 2.50). Set to null if no CALL button visible or amount not shown.
   - facingBet: Determines if hero is facing a bet/raise. TRUE if CALL button shows a dollar amount (e.g., "CALL $2.50") - this means someone bet/raised. FALSE if only CHECK button is available (no CALL button with amount) - this means no bet to call. Set to null if unclear.
   - availableActions: Look for action buttons visible on screen (typically at bottom). Return array like ["fold", "call", "raise"] or ["check", "raise", "fold"], or null if not clearly visible. Common buttons: "fold", "call", "raise", "check", "all-in".
   - heroPosition: Detect hero's position by:
     1. Look for the dealer button (white/yellow chip marked "D", "DEALER", or dealer icon)
     2. Find which player seat has the dealer button
     3. Count seats clockwise from dealer to hero:
        - Has dealer button = "BTN" (Button)
        - 1 seat left of dealer = "SB" (Small Blind)
        - 2 seats left = "BB" (Big Blind)
        - 3 seats left (6-max) = "UTG" (Under the Gun)
        - 4 seats left (6-max) = "MP" (Middle Position)
        - 5 seats left (6-max, right of dealer) = "CO" (Cutoff)
        - For 9-max: add "HJ" (Hijack) between MP and CO
     4. Alternative indicators:
        - Position labels on/near seats (BTN, SB, BB, CO, etc.)
        - Blind chips (smaller = SB, larger = BB)
        - Hero's seat may be highlighted
     5. Return null ONLY if no position indicators are visible
   - aiRecommendation: CRITICAL - Verify TIER1's recommendation is valid. Check these rules IN ORDER:

     HAND STRENGTH EVALUATION (fix common TIER1 mistakes):
     1. STRAIGHT: 5 consecutive ranks from all 7 cards (2 hero + 5 board)
        Example: A♠K♥ on Q♣J♦T♥ = STRAIGHT, NOT "top pair"
     2. THREE OF A KIND: Exactly 3 cards of same rank (check all 7 cards)
        Example: K♠K♥ on J♣K♦T♥ = THREE OF A KIND (3 kings)
        Counter: K♠K♥ on J♣2♥8♦ = OVERPAIR (only 2 kings, NOT trips)
     3. PREFLOP (0 board cards):
        - ONLY pocket pairs count as pairs (board is empty!)
        - Non-pairs are "high card" (e.g., A♣K♦ = "Ace high", 7♥9♠ = "Nine high")
        - NEVER say "pair" or "two pair" preflop unless hero has pocket pair
     4. POSTFLOP (3+ board cards):
        - ONE PAIR: hero must hold a card matching ONE board card
          Example: A♥6♠ on T♣4♦A♦ = "Pair of aces" ✓
          Counter: A♣J♣ on 6♦4♠T♥ = "Ace high" (NO pair) ✗
        - TWO PAIR: BOTH hero cards must match DIFFERENT board cards
          Example: A♦6♣ on 6♦A♣J♣ = "Two pair, aces and sixes" ✓
          Counter: A♣J♣ on 6♦4♠T♥ = "Ace high" (NOT two pair) ✗

     ACTION VALIDATION (based on facingBet):
     - If facingBet is TRUE: Valid actions are FOLD, CALL, or RAISE only (CANNOT CHECK)
     - If facingBet is FALSE: Valid actions are CHECK or RAISE only (CANNOT CALL)
     - PREFLOP: BB with facingBet=false can CHECK; all other positions must CALL blind or RAISE
     - NEVER recommend CHECK when facingBet is true
     - NEVER recommend CALL when facingBet is false

     For RAISE use 66-75% of pot with $0.15 minimum. Reasoning MUST accurately describe hand strength (use correct terminology from above). Keep reasoning under 15 words but BE ACCURATE. Set to null if heroToAct is false or cards unclear.
5. Set perFieldConfidence and overallConfidence to reflect your true certainty.
6. If you cannot confidently read hero cards or board cards, return [] for that array and low confidence for that field.

OUTPUT FORMAT
-------------
Return ONLY a single valid PokerState JSON object. No markdown, no comments, no text."#,
        previous_state_json,
        tier1_output,
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

/// Analyze with Claude and return RawVisionData format (for cascade fallback)
/// This function is called when OpenAI validation fails
pub async fn analyze_with_claude_raw(
    image_data: &[u8],
    tier1_output: &str,
    issues: &[String],
) -> Result<RawVisionData, String> {
    let api_key = std::env::var("CLAUDE_API_KEY")
        .or_else(|_| std::env::var("ANTHROPIC_API_KEY"))
        .map_err(|_| "CLAUDE_API_KEY or ANTHROPIC_API_KEY not found in environment".to_string())?;

    let base64_image = general_purpose::STANDARD.encode(image_data);

    // Determine which specialized prompt to use based on the issue type
    let is_new_hand_verification = issues.iter().any(|i| i == "new_hand_verification");
    let is_community_verification = issues.iter().any(|i| i.starts_with("community_card_verification:"));
    let is_duplicate_resolution = issues.iter().any(|i| i == "duplicate_resolution");

    let prompt = if is_new_hand_verification {
        // Special prompt for new hand verification - focus on hero card suits
        format!(r#"VERIFY the hero cards (hole cards) in this poker screenshot.

OpenAI detected these hero cards: {}

TASK: Look at the 2 cards at BOTTOM CENTER of screen and verify they are correct.

CRITICAL - Pay close attention to SUIT identification:
1. SUIT SHAPES:
   - ♠ SPADES: Black, POINTED shape pointing UP (like an upside-down heart with stem)
   - ♣ CLUBS: Black, THREE-LEAF CLOVER shape
   - ♥ HEARTS: Red, classic heart shape
   - ♦ DIAMONDS: Red, ROTATED SQUARE shape

2. SUIT COLORS:
   - BLACK suits: ♠ spades and ♣ clubs
   - RED suits: ♥ hearts and ♦ diamonds

3. CARD RANKS: A, K, Q, J, T(10), 9, 8, 7, 6, 5, 4, 3, 2

If OpenAI detected wrong suits (e.g., spades instead of diamonds), CORRECT them.

Return ONLY JSON with the verified/corrected cards:
{{
  "heroCards": ["X♠", "Y♥"],
  "communityCards": [],
  "pot": null,
  "position": null,
  "availableActions": [],
  "amountToCall": 0,
  "heroStack": null
}}

Return ONLY the JSON object, nothing else."#, tier1_output)
    } else if is_community_verification {
        // Extract the street from the issue (e.g., "community_card_verification:flop")
        let street = issues.iter()
            .find(|i| i.starts_with("community_card_verification:"))
            .and_then(|i| i.split(':').nth(1))
            .unwrap_or("unknown");

        let expected_count = match street {
            "flop" => "3",
            "turn" => "4",
            "river" => "5",
            _ => "unknown number of",
        };

        // Special prompt for community card verification - focus on board cards
        format!(r#"VERIFY the community cards ({}) in this poker screenshot.

OpenAI detected: {}

TASK: Look at the {} community cards in the HORIZONTAL ROW at TABLE CENTER.

CRITICAL - Pay close attention to SUIT identification:
1. SUIT SHAPES:
   - ♠ SPADES: Black, POINTED shape pointing UP (like an upside-down heart with stem)
   - ♣ CLUBS: Black, THREE-LEAF CLOVER shape
   - ♥ HEARTS: Red, classic heart shape
   - ♦ DIAMONDS: Red, ROTATED SQUARE shape

2. SUIT COLORS:
   - BLACK suits: ♠ spades and ♣ clubs
   - RED suits: ♥ hearts and ♦ diamonds

3. CARD RANKS: A, K, Q, J, T(10), 9, 8, 7, 6, 5, 4, 3, 2

4. NO DUPLICATES: Each card appears only once across hero + community cards

If OpenAI detected wrong suits or ranks, CORRECT them.

Return ONLY JSON with the verified/corrected cards:
{{
  "heroCards": [],
  "communityCards": ["X♠", "Y♥", "Z♦", null, null],
  "pot": null,
  "position": null,
  "availableActions": [],
  "amountToCall": 0,
  "heroStack": null
}}

Return ONLY the JSON object, nothing else."#, street, tier1_output, expected_count)
    } else if is_duplicate_resolution {
        // Special prompt for resolving duplicate card conflicts
        format!(r#"RESOLVE duplicate card conflict in this poker screenshot.

OpenAI detected (with DUPLICATE cards): {}

TASK: Re-analyze ALL cards carefully. A card can ONLY appear ONCE.

CARD LOCATIONS:
1. HERO CARDS: 2 larger cards at BOTTOM CENTER of screen
2. COMMUNITY CARDS: Up to 5 cards in HORIZONTAL ROW at TABLE CENTER

CRITICAL - When you see a "duplicate":
- One detection is WRONG - the same physical card cannot be in two places
- Look carefully at BOTH locations to determine the correct cards
- Pay close attention to suit SHAPES and COLORS

SUIT IDENTIFICATION:
- ♠ SPADES: Black, POINTED shape
- ♣ CLUBS: Black, THREE-LEAF CLOVER shape
- ♥ HEARTS: Red, classic heart shape
- ♦ DIAMONDS: Red, ROTATED SQUARE shape

Return ONLY JSON with the CORRECTED cards (NO duplicates):
{{
  "heroCards": ["X♠", "Y♥"],
  "communityCards": ["A♦", "B♣", "C♥", null, null],
  "pot": null,
  "position": null,
  "availableActions": [],
  "amountToCall": 0,
  "heroStack": null
}}

Return ONLY the JSON object, nothing else."#, tier1_output)
    } else {
        // Standard error correction prompt
        let issues_str = if issues.is_empty() {
            "None".to_string()
        } else {
            issues.join(", ")
        };

        format!(r#"You are correcting card detection errors from a faster AI model.

The previous model (OpenAI o4-mini) made these errors: {}

Previous output: {}

TASK: Re-analyze the poker screenshot and return ONLY valid JSON with corrected card data.

CRITICAL RULES:
1. Hero cards are at BOTTOM CENTER of screen (2 larger cards with slight overlap)
2. Community cards are the HORIZONTAL ROW at TABLE CENTER (up to 5 cards)
3. A card can ONLY appear ONCE across all cards - NO DUPLICATES
4. Each card must be rank+suit: A♠, K♥, Qd, T♣, 9♦, etc.
5. Valid ranks: A, K, Q, J, T, 9, 8, 7, 6, 5, 4, 3, 2
6. Valid suits: ♠ ♥ ♦ ♣ (or s h d c)
7. If you cannot clearly read a card, use null for that position
8. NEVER return single letters like "S" or "D" as cards

OUTPUT FORMAT (JSON only, no markdown):
{{
  "heroCards": ["K♠", "K♥"],
  "communityCards": ["J♣", "K♦", "T♥", null, null],
  "pot": 0.26,
  "position": "BTN",
  "availableActions": ["FOLD", "CALL $0.10", "RAISE"],
  "amountToCall": 0.10,
  "heroStack": 27.35
}}

Return ONLY the JSON object, nothing else."#, issues_str, tier1_output)
    };

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

    let mut raw_data: RawVisionData = serde_json::from_str(clean_text)
        .map_err(|e| format!("Failed to parse Claude output: {}. Response: {}", e, clean_text))?;

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