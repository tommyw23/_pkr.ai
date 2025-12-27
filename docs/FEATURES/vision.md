# Vision OCR (Card Detection)

## Purpose
Detect playing cards and board state from screen captures using GPT-4o-mini.

## Provider
**OpenAI GPT-4o-mini** (vision capability)

## Why GPT-4o-mini (not local YOLO)
| Approach | Result |
|----------|--------|
| YOLO V1 | 95.7% mAP50 - too low |
| YOLO V2 | 99.07% mAP50 - training success |
| YOLO V3 | Added card backs - 98.85% validation |
| **Real-world test** | **Failed** - poker client UIs too varied |

GPT-4o-mini handles visual variation across poker clients (different skins, sizes, lighting) much better than a trained YOLO model.

## API Cost
- ~720 calls/hour at active use
- GPT-4o-mini pricing makes this sustainable for 70% margin target

## Implementation

### Request Format
```typescript
const response = await openai.chat.completions.create({
  model: "gpt-4o-mini",
  messages: [
    {
      role: "user",
      content: [
        {
          type: "text",
          text: `Identify the playing cards in this image. 
                 Return JSON: { "cards": ["Ah", "Kd", ...] }
                 Use format: rank + suit (A/K/Q/J/T/9-2 + h/d/c/s)
                 If no cards visible, return { "cards": [] }`
        },
        {
          type: "image_url",
          image_url: { url: `data:image/png;base64,${base64Image}` }
        }
      ]
    }
  ],
  max_tokens: 100
});
```

### Response Parsing
```typescript
interface VisionResponse {
  cards: string[];  // e.g., ["Ah", "Kd", "Qc", "Js", "Th"]
}

function parseCards(response: string): string[] {
  const json = JSON.parse(response);
  return json.cards.filter(isValidCard);
}

function isValidCard(card: string): boolean {
  const ranks = 'AKQJT98765432';
  const suits = 'hdcs';
  return card.length === 2 
    && ranks.includes(card[0]) 
    && suits.includes(card[1]);
}
```

## Capture Regions
Vision OCR is called separately for:
1. **Hole cards** - Player's 2 cards
2. **Community cards** - Board (flop/turn/river)
3. **Pot size** - Text OCR for dollar amount (optional)

## Error Handling
- Retry 3x on API failure with exponential backoff
- If detection confidence low, skip frame and wait for next capture
- Log failures for debugging

## Files Involved
- `src-tauri/src/vision.rs` - Rust integration with OpenAI API
- Called from `src-tauri/src/capture.rs` after screen grab

## Performance Target
- Detection latency: <500ms per frame
- Accuracy: >95% on real poker client UIs
- Capture interval: Every 1-2 seconds during active play

## Status
🚧 Not yet implemented - pending calibration fix
