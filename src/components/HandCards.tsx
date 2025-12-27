import type { Card } from "../types/poker";

function suitGlyph(s: string) {
  if (s === "s") return "♠";
  if (s === "h") return "♥";
  if (s === "d") return "♦";
  return "♣";
}
function suitColor(s: string) {
  if (s === "s") return "#9FB3FF";
  if (s === "h") return "#FF8FA3";
  if (s === "d") return "#7DE1FF";
  return "#B5F7B7";
}

function renderCard(c: Card) {
  if (!c) {
    return (
      <div style={{
        width: 28, height: 20, borderRadius: 4, display: "grid", placeItems: "center",
        border: "1px dashed #FFFFFF26", background: "#FFFFFF0D", color: "#E8EEF5", fontSize: 12, fontWeight: 700
      }}>?</div>
    );
  }
  const rank = c[0];
  const suit = c[1];
  return (
    <div style={{
      width: 28, height: 20, borderRadius: 4, display: "grid", placeItems: "center",
      background: suitColor(suit), color: "#0E1117", fontSize: 12, fontWeight: 800
    }}>
      {rank}{suitGlyph(suit)}
    </div>
  );
}

export default function HandCards({ cards }: { cards: [Card, Card] }) {
  return (
    <div style={{ display: "flex", gap: 6 }}>
      {renderCard(cards[0])}
      {renderCard(cards[1])}
    </div>
  );
}
