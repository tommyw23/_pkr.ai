import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MousePointer2, GripVertical } from "lucide-react";
import { getDpiScaleFactor } from "../lib/dpi-utils";

interface SelectionCoords {
  x: number;
  y: number;
  width: number;
  height: number;
}

const MIN_SEL_SIZE = 10; // px

const Overlay: React.FC = () => {
  const [isSelecting, setIsSelecting] = useState(false);
  const [startCoords, setStartCoords] = useState({ x: 0, y: 0 });
  const [scaleFactor, setScaleFactor] = useState<number>(window.devicePixelRatio || 1);

  const [selectionStyle, setSelectionStyle] = useState<{
    left: number;
    top: number;
    width: number;
    height: number;
    display: "none" | "block";
  }>({
    left: 0,
    top: 0,
    width: 0,
    height: 0,
    display: "none",
  });

  const [cursorPosition, setCursorPosition] = useState({ x: 0, y: 0 });
  const [cursorVisible, setCursorVisible] = useState(false);

  const selectionRef = useRef<HTMLDivElement>(null);

  // Cancel selection / close overlay
  const handleCancel = async () => {
    setIsSelecting(false);
    setSelectionStyle((s) => ({ ...s, display: "none", width: 0, height: 0 }));
    try {
      await invoke("close_overlay_window", { reason: "User cancelled" });
    } catch (err) {
      console.error("Failed to close overlay:", err);
    }
  };

  // Complete selection (send to backend)
  const handleSelectionComplete = async (x: number, y: number, width: number, height: number) => {
    try {
      // Use the backend DPI scale factor for accurate coordinate conversion
      const coords: SelectionCoords = {
        x: Math.round(x * scaleFactor),
        y: Math.round(y * scaleFactor),
        width: Math.round(width * scaleFactor),
        height: Math.round(height * scaleFactor),
      };
      console.log(`📐 Overlay selection - Logical: ${x},${y} ${width}x${height}`);
      console.log(`📐 Overlay selection - Physical (${scaleFactor}x): ${coords.x},${coords.y} ${coords.width}x${coords.height}`);
      await invoke("capture_selected_area", { coords });
    } catch (err) {
      console.error("Failed to capture selected area:", err);
    }
  };

  // Mouse handlers
  const handleMouseDown = (e: React.MouseEvent) => {
    // Ignore non-left clicks
    if (e.button !== 0) return;

    setIsSelecting(true);
    const x = e.clientX;
    const y = e.clientY;
    setStartCoords({ x, y });
    setCursorPosition({ x, y });
    setCursorVisible(true);

    setSelectionStyle({
      left: x,
      top: y,
      width: 0,
      height: 0,
      display: "block",
    });

    e.preventDefault();
    e.stopPropagation();
  };

  const handleMouseMove = (e: React.MouseEvent) => {
    const x = e.clientX;
    const y = e.clientY;

    // always update cursor
    setCursorPosition({ x, y });
    if (!cursorVisible) setCursorVisible(true);

    if (!isSelecting) return;

    const width = Math.abs(x - startCoords.x);
    const height = Math.abs(y - startCoords.y);
    const left = Math.min(x, startCoords.x);
    const top = Math.min(y, startCoords.y);

    setSelectionStyle((prev) => ({
      ...prev,
      width,
      height,
      left,
      top,
    }));

    e.preventDefault();
    e.stopPropagation();
  };

  const handleMouseUp = (e: React.MouseEvent) => {
    const x = e.clientX;
    const y = e.clientY;
    setCursorPosition({ x, y });

    if (!isSelecting) return;
    setIsSelecting(false);

    const left = Math.min(x, startCoords.x);
    const top = Math.min(y, startCoords.y);
    const width = Math.abs(x - startCoords.x);
    const height = Math.abs(y - startCoords.y);

    e.preventDefault();
    e.stopPropagation();

    if (width >= MIN_SEL_SIZE && height >= MIN_SEL_SIZE) {
      handleSelectionComplete(left, top, width, height);
    } else {
      handleCancel();
    }
  };

  // ESC to cancel
  const handleEscapeKey = (e: KeyboardEvent) => {
    if (e.key === "Escape" || e.keyCode === 27) {
      e.preventDefault();
      e.stopImmediatePropagation();
      handleCancel();
    }
  };

  // Load DPI scale factor from backend on mount
  useEffect(() => {
    getDpiScaleFactor().then((factor) => {
      setScaleFactor(factor);
      console.log(`🔍 Overlay using DPI scale factor: ${factor}`);
    });
  }, []);

  useEffect(() => {
    document.addEventListener("keydown", handleEscapeKey, true);
    window.addEventListener("keydown", handleEscapeKey, true);
    return () => {
      document.removeEventListener("keydown", handleEscapeKey, true);
      window.removeEventListener("keydown", handleEscapeKey, true);
    };
  }, []);

  return (
    <div
      className="fixed inset-0 w-screen h-screen bg-black/10 overflow-hidden"
      style={{ cursor: "none", userSelect: "none" }}
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
    >
      {/* Instructions */}
      <div className="fixed top-5 left-1/2 -translate-x-1/2 bg-black/50 text-white px-6 py-3 rounded-lg font-sans text-xs pointer-events-none z-[5000]">
        Click and drag to select area, press ESC to cancel
      </div>

      {/* Control Bar - Top Right */}
      <div className="fixed top-5 right-5 flex items-center gap-2 z-[5000]">
        {/* Drag Grip Icon */}
        <button
          data-tauri-drag-region
          onMouseDown={(e) => {
            e.preventDefault();
            e.stopPropagation();
          }}
          className="bg-black/50 hover:bg-black/70 text-white border-none p-2 rounded-md font-sans text-sm transition-colors duration-200 flex items-center justify-center"
          style={{ cursor: "grab" }}
          title="Drag to move window"
        >
          <GripVertical className="w-5 h-5" />
        </button>

        {/* Cancel Button */}
        <button
          onClick={handleCancel}
          onMouseDown={(e) => {
            e.preventDefault();
            e.stopPropagation();
            handleCancel();
          }}
          className="bg-red-500 hover:bg-red-600 text-white border-none px-4 py-2 rounded-md font-sans text-sm transition-colors duration-200"
          style={{ cursor: "default" }}
          title="Cancel (ESC)"
        >
          Cancel (ESC)
        </button>
      </div>

      {/* Selection Rectangle (outer strong border) */}
      <div
        ref={selectionRef}
        className="absolute rounded-3xl rounded-br-none pointer-events-none"
        style={{
          left: selectionStyle.left,
          top: selectionStyle.top,
          width: selectionStyle.width,
          height: selectionStyle.height,
          display: selectionStyle.display,
          zIndex: 4000,
          border: "2px solid rgba(255,255,255,0.9)",
          background: "rgba(0, 200, 150, 0.08)",
        }}
      />

      {/* Selection Rectangle (inner subtle outline) */}
      <div
        className="absolute rounded-3xl rounded-br-none pointer-events-none"
        style={{
          left: selectionStyle.left,
          top: selectionStyle.top,
          width: selectionStyle.width,
          height: selectionStyle.height,
          display: selectionStyle.display,
          zIndex: 4000,
          border: "0.5px solid rgba(0,0,0,0.7)",
          background: "transparent",
        }}
      />

      {/* Custom Cursor */}
      <div
        className="fixed pointer-events-none z-[9999] transition-opacity duration-100"
        style={{
          left: cursorPosition.x,
          top: cursorPosition.y,
          transform: "translate(-2px, -2px)",
          opacity: cursorVisible ? 1 : 0,
        }}
      >
        <MousePointer2 className="w-5 h-5 drop-shadow-2xl fill-secondary stroke-primary" />
      </div>
    </div>
  );
};

export default Overlay;
   