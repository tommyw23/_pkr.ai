# Strategy Engine

## Purpose
Analyze game state and provide optimal poker decisions using o1-mini reasoning model.

## Provider
**OpenAI o1-mini** (reasoning-optimized)

## Why o1-mini
- Superior at multi-step reasoning (range analysis, EV calculations)
- Better strategic depth than GPT-4o
- Acceptable latency for non-real-time strategic advice
- Cost-effective for reasoning tasks

## Input: Game State
```typescript
interface GameState {
  holeCards: [string, string];      // e.g., ["Ah", "Kd"]
  communityCards: string[];          // e.g., ["Qc", "Js", "Th"]
  potSize: number;                   // Current pot in dollars/BB
  street: 'preflop' | 'flop' | 'turn' | 'river';
  position: string;                  // e.g., "BTN", "BB", "UTG"
  stackSize: number;                 // Player's stack
  villainCount: number;              // Players still in hand
  action: 'check' | 'bet' | 'call' | 'raise' | 'fold' | null;
  betToCall: number;                 // Amount to call (if any)
}
```

## Output: Strategy Response
```typescript
interface StrategyResponse {
  recommendedAction: 'fold' | 'check' | 'call' | 'bet' | 'raise';
  betSize?: number;                  // If betting/raising
  confidence: 'high' | 'medium' | 'low';
  reasoning: string;                 // Brief explanation
  handStrength: string;              // e.g., "Top pair, top kicker"
  equity?: number;                   // Win % against typical range
}
```

## Prompt Template
```typescript
const prompt = `You are a professional poker strategist. Analyze this hand:

Hole Cards: ${gameState.holeCards.join(', ')}
Board: ${gameState.communityCards.join(', ') || 'Preflop'}
Street: ${gameState.street}
Position: ${gameState.position}
Pot: ${gameState.potSize}
Stack: ${gameState.stackSize}
Facing: ${gameState.betToCall > 0 ? `$${gameState.betToCall} to call` : 'No bet'}
Villains: ${gameState.villainCount}

Provide your analysis in JSON format:
{
  "recommendedAction": "fold|check|call|bet|raise",
  "betSize": <number if betting>,
  "confidence": "high|medium|low",
  "reasoning": "<1-2 sentence explanation>",
  "handStrength": "<hand description>",
  "equity": <0-100>
}`;
```

## API Call
```typescript
const response = await openai.chat.completions.create({
  model: "o1-mini",
  messages: [{ role: "user", content: prompt }],
  max_tokens: 300
});
```

## Integration Flow
1. Vision OCR detects cards → updates `GameState`
2. User action detected (bet sizing, timing) → triggers strategy call
3. o1-mini analyzes position → returns recommendation
4. UI displays recommendation with reasoning

## Display Modes
- **Compact:** Just show action + bet size
- **Detailed:** Show reasoning, equity, confidence
- **Training:** Show full range analysis (future feature)

## Error Handling
- If parsing fails, show raw reasoning text
- Timeout after 10s, show "Analysis unavailable"
- Cache recent analyses to reduce API calls

## Files Involved
- `src-tauri/src/strategy.rs` - o1-mini integration
- `src/components/StrategyDisplay.tsx` - UI component

## Performance Target
- Response latency: <3 seconds
- Should not block UI (async with loading state)

## Status
🚧 Not yet implemented - pending vision integration
