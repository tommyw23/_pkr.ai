export type Rank = "A"|"K"|"Q"|"J"|"T"|"9"|"8"|"7"|"6"|"5"|"4"|"3"|"2";
export type Suit = "s"|"h"|"d"|"c"; // spade/heart/diamond/club
export type Card = `${Rank}${Suit}` | null;

export interface Recommendation {
  action: "FOLD" | "CALL" | "CHECK" | "BET" | "RAISE";
  amount?: number;
  reason?: string;
}

export interface GameState {
  running: boolean;          // Play/Pause
  visible: boolean;          // Show/Hide main panel
  holeCards: [Card, Card];   // hero hole cards (use null for unknown)
  pot: number;               // currency number
  confidence: number;        // 0..1
  recommendation: Recommendation;
  context?: { position?: string; villainPos?: string; street?: "PREFLOP"|"FLOP"|"TURN"|"RIVER" }
}
