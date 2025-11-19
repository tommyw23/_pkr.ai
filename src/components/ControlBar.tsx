import React from "react";
import logo from "../images/pkr-logo.jpg";

type ControlBarProps = {
  thinking: boolean;          // true = pause icon, false = play icon
  onToggle: () => void;       // called when Play/Pause is clicked
  onSettingsClick?: () => void;
  onLogClick?: () => void;
  onCloseClick?: () => void;
};

export const iconBtn: React.CSSProperties = {
  height: 28,
  width: 28,
  borderRadius: 8,
  border: "1px solid #FFFFFF1A",
  background: "#0C0F14E6",
  color: "#E8EEF5",
  cursor: "pointer",
  display: "inline-grid",
  placeItems: "center",
  fontSize: 14,
  lineHeight: 1,
};

export default function ControlBar({
  thinking,
  onToggle,
  onSettingsClick,
  onLogClick,
  onCloseClick,
}: ControlBarProps) {
  return (
    <div
      className="pkr-frost"
      style={{
        width: 560,
        height: 44,
        borderRadius: 22,
        display: "flex",
        alignItems: "center",
        padding: "0 12px",
        gap: 10,
      }}
    >
      {/* Logo on the left */}
      <img
        src={logo}
        alt="pkr.ai logo"
        style={{
          width: 26,
          height: 26,
          borderRadius: 6,
          objectFit: "cover",
          filter: "drop-shadow(0 0 6px rgba(255, 0, 80, 0.45))",
        }}
      />

      {/* Play / Pause */}
      <button
        className="pkr-no-select pkr-icon"
        onClick={onToggle}
        aria-label={thinking ? "Pause" : "Play"}
        title={thinking ? "Pause" : "Play"}
        style={{
          height: 28,
          padding: "0 10px",
          borderRadius: 8,
          border: "1px solid #FFFFFF1A",
          background: "#0C0F14E6",
          color: "#E8EEF5",
          cursor: "pointer",
          fontWeight: 600,
        }}
      >
        {thinking ? "⏸" : "▶"}
      </button>

      {/* Label */}
      <div style={{ fontSize: 14, color: "#E8EEF5" }}>Show/Hide</div>

      {/* Right controls */}
      <div style={{ marginLeft: "auto", display: "flex", gap: 8 }}>
        <button
          title="Settings"
          aria-label="Settings"
          className="pkr-icon"
          style={iconBtn}
          onClick={onSettingsClick}
        >
          ⚙
        </button>
        <button
          title="Game Log"
          aria-label="Game Log"
          className="pkr-icon"
          style={iconBtn}
          onClick={onLogClick}
        >
          📋
        </button>
        <button
          title="Close"
          aria-label="Close"
          className="pkr-icon"
          style={iconBtn}
          onClick={onCloseClick}
        >
          ✕
        </button>
      </div>
    </div>
  );
}
