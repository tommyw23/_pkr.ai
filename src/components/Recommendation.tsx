import React from "react";

export default function Recommendation() {
  return (
    <div style={{ marginTop: 12 }}>
      <h3
        style={{
          fontSize: 14,
          fontWeight: 700,
          color: "#00FFA6",
          letterSpacing: 0.5,
          marginBottom: 6,
        }}
      >
        RECOMMENDED MOVE
      </h3>
      <div
        style={{
          fontSize: 20,
          fontWeight: 800,
          color: "#E8EEF5",
        }}
      >
        → Raise to $800
      </div>
      <div
        style={{
          fontSize: 12,
          color: "#98A2B3",
          marginTop: 4,
        }}
      >
        SPR low — opponent capped — build pot now.
      </div>
    </div>
  );
}
