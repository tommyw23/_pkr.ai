import React from "react";
import logo from "../images/pkr-logo.png";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

type ControlBarProps = {
  thinking: boolean;          // true = pause icon, false = play icon
  isAnalyzing?: boolean;      // true = currently processing API call
  wasInterrupted?: boolean;   // true = table state changed during analysis
  onToggle: () => void;       // called when Play/Pause is clicked
  onClearClick?: () => void;
  onCloseClick?: () => void;
};

export const iconBtn: React.CSSProperties = {
  height: 26,
  width: 26,
  borderRadius: 6,
  border: "1px solid #FFFFFF1A",
  background: "#0C0F14E6",
  color: "#E8EEF5",
  cursor: "pointer",
  display: "inline-grid",
  placeItems: "center",
  fontSize: 13,
  lineHeight: 1,
};

export default function ControlBar({
  thinking,
  isAnalyzing,
  wasInterrupted,
  onToggle,
  onClearClick,
  onCloseClick,
}: ControlBarProps) {
  return (
    <div
      className="pkr-frost"
      style={{
        width: 560,
        height: 38,
        borderRadius: 19,
        display: "flex",
        alignItems: "center",
        padding: "0 10px",
        gap: 8,
      }}
    >
      {/* Left group: Logo, Play/Pause, and branding text */}
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        {/* Logo on the left */}
        <img
          src={logo}
          alt="pkr.ai logo"
          style={{
            width: 24,
            height: 24,
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
            height: 26,
            padding: "0 9px",
            borderRadius: 6,
            border: "1px solid #FFFFFF1A",
            background: "#0C0F14E6",
            color: "#E8EEF5",
            cursor: "pointer",
            fontWeight: 600,
            fontSize: 13,
          }}
        >
          {thinking ? "⏸" : "▶"}
        </button>

        {/* Branding text */}
        <span
          style={{
            fontSize: 13,
            color: "#999999",
            fontWeight: 500,
            marginLeft: 2,
            userSelect: "none"
          }}
        >
          pkr.ai
        </span>

        {/* Status indicators */}
        {thinking && isAnalyzing && (
          <span
            style={{
              fontSize: 11,
              color: "#98A2B3",
              marginLeft: 8,
              display: "flex",
              alignItems: "center",
              gap: 4,
            }}
          >
            <span
              style={{
                width: 6,
                height: 6,
                borderRadius: "50%",
                background: "#3B82F6",
                animation: "pulse 1.5s ease-in-out infinite",
              }}
            />
            Analyzing...
          </span>
        )}

        {wasInterrupted && !isAnalyzing && (
          <span
            style={{
              fontSize: 11,
              color: "#F59E0B",
              marginLeft: 8,
              animation: "flash 0.5s ease-in-out 2",
            }}
          >
            New action detected
          </span>
        )}
      </div>

      {/* Spacer */}
      <div style={{ flex: 1 }} />

      {/* Right controls */}
      <div style={{ display: "flex", gap: 8 }}>
        {/* Clear button */}
        <button
          title="Clear data"
          aria-label="Clear data"
          className="pkr-icon"
          style={{
            ...iconBtn,
            background: "#4B5563E6",
            transition: "background 0.2s ease",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
          onMouseEnter={(e) => (e.currentTarget.style.background = "#6B7280E6")}
          onMouseLeave={(e) => (e.currentTarget.style.background = "#4B5563E6")}
          onClick={onClearClick}
        >
          🗑
        </button>
        {/* Calibration button */}
        <button
          title="Calibrate"
          aria-label="Calibrate"
          className="pkr-icon"
          style={{
            ...iconBtn,
            background: "#2563EBE6",
            transition: "background 0.2s ease",
          }}
          onMouseEnter={(e) => (e.currentTarget.style.background = "#3B82F6E6")}
          onMouseLeave={(e) => (e.currentTarget.style.background = "#2563EBE6")}
          onClick={async () => {
            try {
              await invoke("start_calibration");
            } catch (err) {
              console.error("Failed to start calibration:", err);
            }
          }}
        >
          ⊞
        </button>
        {/* Drag Grip - enables window dragging */}
        <button
          title="Drag to move window"
          aria-label="Drag to move window"
          className="pkr-icon"
          style={{
            ...iconBtn,
            cursor: "move",
            opacity: 0.6,
            transition: "opacity 0.2s ease",
          }}
          onMouseEnter={(e) => (e.currentTarget.style.opacity = "1")}
          onMouseLeave={(e) => (e.currentTarget.style.opacity = "0.6")}
          onMouseDown={async (e) => {
            e.preventDefault();
            e.stopPropagation();
            try {
              await getCurrentWindow().startDragging();
            } catch (err) {
              console.error("Failed to start window dragging:", err);
            }
          }}
        >
          ⋮⋮
        </button>
        <button
          title="Visit website"
          aria-label="Visit website"
          className="pkr-icon"
          style={{
            ...iconBtn,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
          onClick={async () => {
            try {
              await openUrl("https://usepkr.ai");
            } catch (err) {
              console.error("Failed to open website:", err);
            }
          }}
        >
          🏠
        </button>
        <button
          title="Minimize"
          aria-label="Minimize"
          className="pkr-icon"
          style={iconBtn}
          onClick={async () => {
            try {
              await getCurrentWindow().minimize();
            } catch (err) {
              console.error("Failed to minimize:", err);
            }
          }}
        >
          −
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
