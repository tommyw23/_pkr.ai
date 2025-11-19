import React, { useMemo, useState } from "react";

type Recommendation =
  | { kind: "raise"; amount: number }
  | { kind: "call" }
  | { kind: "fold" };

type PanelState = {
  hand: [string, string]; // e.g., ["Q♣", "A♡"]
  pot: number;            // e.g., 1500
  position?: string;      // optional
  community: string[];    // ADD THIS LINE
  strength: number;       // 0..100
  win: number;            // win %
  tie: number;            // tie %
  potOdds?: number;
  accuracy?: number;
  action: Recommendation; // recommended move
  opponentSnapshot?: string;
};

export default function AnalysisPanel({ state }: { state: PanelState }) {
  const [tipOpen, setTipOpen] = useState(false);

  const strengthBar = useMemo(() => {
    const pct = Math.max(0, Math.min(100, state.strength));
    return (
      <div style={{ display: "flex", gap: 4, flexWrap: "wrap", maxWidth: 240 }}>
        <div className="pkrc-strength-label" style={{ textAlign: "right" }}>
          {pct}%
        </div>
        <div
          aria-label="Strength"
          role="meter"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={pct}
          style={{
            height: 12,
            borderRadius: 8,
            background: "rgba(255,255,255,0.08)",
            border: "1px solid rgba(255,255,255,0.12)",
            position: "relative",
            overflow: "hidden",
            width: 240,
          }}
        >
          <div
            style={{
              width: `${pct}%`,
              height: "100%",
              borderRadius: 8,
              background:
               "linear-gradient(90deg, rgba(0,80,40,1), rgba(0,200,120,0.9))",
              boxShadow: "inset 0 0 0 1px rgba(0,0,0,0.3)",
            }}
          />
        </div>
      </div>
    );
  }, [state.strength]);

  const moveText = useMemo(() => {
    switch (state.action.kind) {
      case "raise":
        return (
          <>
            <span className="pkrc-arrow">→</span>{" "}
            <span className="pkrc-action-text">
              Raise <span className="pkrc-amt">to ${state.action.amount}</span>
            </span>
          </>
        );
      case "call":
        return (
          <>
            <span className="pkrc-arrow">→</span>{" "}
            <span className="pkrc-action-text">Call</span>
          </>
        );
      case "fold":
        return (
          <>
            <span className="pkrc-arrow">→</span>{" "}
            <span className="pkrc-action-text">Fold</span>
          </>
        );
    }
  }, [state.action]);

  return (
    <div className="pkr-frost-strong pkrc" style={{ width: 560 }}>
      {/* Header row: title (top-left), chips (below title), strength (top-right) */}
      <div
        className="pkrc-head"
        style={{
          display: "grid",
          gridTemplateColumns: "minmax(0,1fr) 280px",
          gridTemplateAreas: `"title strength" "chips strength"`,
          columnGap: 16,
          rowGap: 10,
          alignItems: "start",
        }}
      >
        {/* Title */}
        <div style={{ gridArea: "title", display: "grid", gap: 8 }}>
          <div style={{ fontSize: 22, fontWeight: 800, letterSpacing: 0.2, color: "#E8EEF5" }}>
            pkr.ai's response ↓
          </div>
        </div>

        {/* Strength block (top-right) */}
        <div
          className="pkrc-strength"
          style={{ gridArea: "strength", justifySelf: "end" }}
        >
          <div className="pkrc-strength-label" style={{ textAlign: "right" }}>
            STRENGTH
          </div>
          {strengthBar}
          <div
            className="pkrc-substats"
            style={{ justifyContent: "flex-end", gap: 16, display: "flex" }}
          >
            <span>Win: {state.win}%</span>
            <span>Tie: {state.tie}%</span>
          </div>
        </div>

        {/* Chips row (under title, left side) */}
        <div className="pkrc-info" style={{ gridArea: "chips", display: "grid", gap: 6 }}>
          {/* Row 1: HAND + POT */}
          <div style={{ display: "flex", gap: 14, flexWrap: "wrap" }}>
            <span className="pkrc-chip">
              HAND:&nbsp;&nbsp;{state.hand[0]}&nbsp;{state.hand[1]}
            </span>
            <span className="pkrc-chip">POT:&nbsp;&nbsp;${state.pot}</span>
          </div>

          {/* Row 2: POS + COMMUNITY */}
          <div style={{
            display: "flex",
            gap: 12,
            flexWrap: "wrap",
            alignItems: "center",
            minWidth: 0
          }}>
            {state.position ? (
              <span className="pkrc-chip">POS:&nbsp;&nbsp;{state.position}</span>
            ) : null}
            <span className="pkrc-chip" style={{ display: "flex", gap: 6, alignItems: "center" }}>
              COMMUNITY:&nbsp;&nbsp;
              {state.community && state.community.length > 0 ? (
                state.community.slice(0, 5).map((card, i) => (
                  <span key={i}>{card}</span>
                ))
              ) : (
                <>--&nbsp;&nbsp;--&nbsp;&nbsp;--&nbsp;&nbsp;--&nbsp;&nbsp;--</>
              )}
            </span>
          </div>
        </div>
      </div>

      {/* Recommend Move box */}
      <div className="pkrc-action" style={{ marginTop: 14 }}>
        <div className="pkrc-action-glow" />
        <div
          className="pkrc-row"
          style={{ alignItems: "center", gap: 8, fontSize: 13, color: "#98A2B3" }}
        >
          <span>RECOMMEND MOVE</span>
          <button
            className="pkrc-tip-btn"
            onClick={() => setTipOpen((v) => !v)}
            title="Why?"
            aria-expanded={tipOpen}
          >
            ?
          </button>
        </div>

        <div
          className="pkrc-row pkrc-move"
          data-move={state.action.kind}
          style={{ marginTop: 6, alignItems: "baseline" }}
        >
          {moveText}
        </div>

        <div className={`pkrc-tip ${tipOpen ? "open" : ""}`}>
          {state.opponentSnapshot ??
            "Model favors aggression given current equity vs. ranges."}
        </div>
      </div>
    </div>
  );
}