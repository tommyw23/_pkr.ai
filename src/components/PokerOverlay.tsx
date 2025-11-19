import React from "react";

export default function PokerOverlay() {
  return (
    <div
      style={{
        padding: 16,
        borderRadius: 16,
        border: "1px solid rgba(255,255,255,0.12)",
        background: "rgba(0,0,0,0.7)",
        width: 288,
        color: "#fff",
        boxShadow: "0 20px 60px rgba(0,0,0,0.6)",
        backdropFilter: "blur(6px)"
      }}
    >
      <div style={{ color: "#00FFA6", fontSize: 12, textTransform: "uppercase", fontWeight: 700, letterSpacing: 1 }}>
        pkr.ai
      </div>
      <div style={{ marginTop: 8, display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <div style={{ fontSize: 16, fontWeight: 600 }}>Win Chance</div>
        <div style={{ fontSize: 24, fontWeight: 800, color: "#00FFA6" }}>78%</div>
      </div>
      <div style={{ marginTop: 10 }}>
        <div style={{ fontSize: 11, color: "#9aa" }}>Your Hand</div>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div style={{ fontSize: 16, fontWeight: 600 }}>Q♠ Q♦</div>
          <div style={{ fontSize: 12, color: "#9aa", textAlign: "right" }}>
            <div>Pot: $18.50</div>
            <div>To Call: $4.00</div>
          </div>
        </div>
      </div>
      <div style={{ marginTop: 10 }}>
        <div style={{ fontSize: 11, color: "#9aa" }}>Recommended Action</div>
        <div style={{ fontSize: 18, fontWeight: 700, color: "#00FFA6" }}>RAISE TO $12</div>
        <div style={{ fontSize: 12, color: "#9aa", marginTop: 4 }}>SPR low. Villain capped. Build pot now.</div>
      </div>
      <div style={{ marginTop: 10 }}>
        <div style={{ fontSize: 11, color: "#9aa" }}>Villain Profile</div>
        <div style={{ display: "flex", justifyContent: "space-between" }}>
          <div>
            <div style={{ fontSize: 14, fontWeight: 600 }}>Loose / Aggro</div>
            <div style={{ fontSize: 12, color: "#9aa" }}>3-bets light · overbluffs turn</div>
          </div>
          <div style={{ fontSize: 12, color: "#9aa" }}>BTN vs SB</div>
        </div>
      </div>
      <div style={{ marginTop: 10, height: 1, background: "rgba(255,255,255,0.1)" }} />
      <div style={{ marginTop: 8, fontSize: 10, textAlign: "center", color: "#8a8a8a" }}>
        advisory mode · not autopilot
      </div>
    </div>
  );
}
