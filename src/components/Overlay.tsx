import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MousePointer2 } from "lucide-react";

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
      const scaleFactor = window.devicePixelRatio || 1;
      const coords: SelectionCoords = {
        x: Math.round(x * scaleFactor),
        y: Math.round(y * scaleFactor),
        width: Math.round(width * scaleFactor),
        height: Math.round(height * scaleFactor),
      };
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

      {/* Cancel Button (keeps pointer visible on hover) */}
      <button
        onClick={handleCancel}
        onMouseDown={(e) => {
          e.preventDefault();
          e.stopPropagation();
          handleCancel();
        }}
        className="fixed top-5 right-5 bg-red-500 hover:bg-red-600 text-white border-none px-4 py-2 rounded-md font-sans text-sm z-[5000] transition-colors duration-200"
        style={{ cursor: "default" }}
        title="Cancel (ESC)"
      >
        Cancel (ESC)
      </button>

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
   