import React, { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getDpiScaleFactor } from "../lib/dpi-utils";

interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface MonitorInfo {
  x: number;
  y: number;
  width: number;
  height: number;
  scale_factor: number;
}

const CalibrationOverlay: React.FC = () => {
  const [region, setRegion] = useState<Rect | null>(null);
  const [_isDrawing, setIsDrawing] = useState(false);
  const [_scaleFactor, setScaleFactor] = useState<number>(window.devicePixelRatio || 1);
  const [monitorInfo, setMonitorInfo] = useState<MonitorInfo | null>(null);

  // Use refs for coordinates to avoid stale closure issues
  const startCoordsRef = useRef({ x: 0, y: 0 });
  const currentRectRef = useRef<Rect>({ x: 0, y: 0, width: 0, height: 0 });
  const isDrawingRef = useRef(false);

  // Load DPI scale factor and monitor info on mount
  useEffect(() => {
    // Use window.devicePixelRatio directly for Retina displays
    const browserScale = window.devicePixelRatio || 1;
    setScaleFactor(browserScale);

    // Also try backend, but browser value is more reliable for window coordinates
    getDpiScaleFactor().then((backendFactor) => {
      // Use the higher of the two to ensure we don't under-scale
      if (backendFactor > browserScale) {
        setScaleFactor(backendFactor);
      }
    });

    // Get monitor info from Rust backend
    invoke("get_calibration_monitor").then((monitor) => {
      setMonitorInfo(monitor as MonitorInfo | null);
    }).catch((err) => {
      console.error("Failed to get calibration monitor:", err);
    });
  }, []);

  // Handle cancel
  const handleCancel = useCallback(async () => {
    try {
      await invoke("close_calibration");
    } catch (err) {
      console.error("Failed to close calibration:", err);
    }
  }, []);

  // Handle save
  const handleSave = useCallback(async () => {
    if (!region) {
      return;
    }

    try {
      // Use current devicePixelRatio at save time (more reliable)
      const currentScale = window.devicePixelRatio || 1;

      // Convert to physical coordinates for saving
      const physicalRegion = {
        name: "Poker Table",
        x: Math.round(region.x * currentScale),
        y: Math.round(region.y * currentScale),
        width: Math.round(region.width * currentScale),
        height: Math.round(region.height * currentScale),
      };

      await invoke("save_calibration_regions", {
        regions: [physicalRegion],
        windowWidth: Math.round(window.innerWidth * currentScale),
        windowHeight: Math.round(window.innerHeight * currentScale),
        monitor: monitorInfo,
      });

      await invoke("close_calibration");
    } catch (err) {
      console.error("Failed to save calibration:", err);
    }
  }, [region, monitorInfo]);

  // Keyboard handlers
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        handleCancel();
      } else if (e.key === "Enter") {
        e.preventDefault();
        handleSave();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [handleCancel, handleSave]);

  // Mouse down - start drawing
  const handleMouseDown = useCallback((e: MouseEvent) => {
    if (e.button !== 0) return;

    isDrawingRef.current = true;
    setIsDrawing(true);

    startCoordsRef.current = { x: e.clientX, y: e.clientY };
    currentRectRef.current = { x: e.clientX, y: e.clientY, width: 0, height: 0 };

    // Clear previous region when starting new draw
    setRegion(null);
  }, []);

  // Mouse move - update rectangle
  const handleMouseMove = useCallback((e: MouseEvent) => {
    if (!isDrawingRef.current) return;

    const startX = startCoordsRef.current.x;
    const startY = startCoordsRef.current.y;
    const currentX = e.clientX;
    const currentY = e.clientY;

    const x = Math.min(startX, currentX);
    const y = Math.min(startY, currentY);
    const width = Math.abs(currentX - startX);
    const height = Math.abs(currentY - startY);

    currentRectRef.current = { x, y, width, height };

    // Force re-render by updating state
    setRegion({ x, y, width, height });
  }, []);

  // Mouse up - finish drawing
  const handleMouseUp = useCallback((e: MouseEvent) => {
    if (!isDrawingRef.current) return;

    isDrawingRef.current = false;
    setIsDrawing(false);

    const startX = startCoordsRef.current.x;
    const startY = startCoordsRef.current.y;
    const currentX = e.clientX;
    const currentY = e.clientY;

    const x = Math.min(startX, currentX);
    const y = Math.min(startY, currentY);
    const width = Math.abs(currentX - startX);
    const height = Math.abs(currentY - startY);

    // Only save if region is large enough
    if (width >= 20 && height >= 20) {
      setRegion({ x, y, width, height });
    } else {
      setRegion(null);
    }
  }, []);

  // Attach mouse event listeners to window
  useEffect(() => {
    window.addEventListener("mousedown", handleMouseDown);
    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);

    return () => {
      window.removeEventListener("mousedown", handleMouseDown);
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };
  }, [handleMouseDown, handleMouseMove, handleMouseUp]);

  return (
    <div
      style={{
        position: "fixed",
        top: 0,
        left: 0,
        width: "100vw",
        height: "100vh",
        backgroundColor: "rgba(0, 0, 0, 0.7)",
        cursor: "crosshair",
        userSelect: "none",
        overflow: "hidden",
      }}
    >
      {/* Instructions */}
      <div
        style={{
          position: "fixed",
          top: 20,
          left: "50%",
          transform: "translateX(-50%)",
          backgroundColor: "rgba(0, 0, 0, 0.8)",
          color: "white",
          padding: "12px 24px",
          borderRadius: 8,
          fontSize: 14,
          fontFamily: "system-ui, sans-serif",
          pointerEvents: "none",
          zIndex: 5000,
          textAlign: "center",
        }}
      >
        Click and drag to select your poker table. Press <strong>Enter</strong> to save, <strong>ESC</strong> to cancel.
      </div>

      {/* Selection Rectangle */}
      {region && region.width > 0 && region.height > 0 && (
        <div
          style={{
            position: "absolute",
            left: region.x,
            top: region.y,
            width: region.width,
            height: region.height,
            border: "2px solid #22c55e",
            backgroundColor: "rgba(34, 197, 94, 0.1)",
            pointerEvents: "none",
            zIndex: 4000,
          }}
        >
          {/* Dimensions label */}
          <div
            style={{
              position: "absolute",
              bottom: -24,
              left: "50%",
              transform: "translateX(-50%)",
              backgroundColor: "#22c55e",
              color: "white",
              padding: "2px 8px",
              borderRadius: 4,
              fontSize: 12,
              fontFamily: "monospace",
              whiteSpace: "nowrap",
            }}
          >
            {Math.round(region.width)} x {Math.round(region.height)}
          </div>
        </div>
      )}

      {/* Bottom buttons */}
      {region && region.width >= 20 && region.height >= 20 && (
        <div
          style={{
            position: "fixed",
            bottom: 30,
            left: "50%",
            transform: "translateX(-50%)",
            display: "flex",
            gap: 12,
            zIndex: 5000,
          }}
        >
          <button
            onClick={(e) => {
              e.stopPropagation();
              handleCancel();
            }}
            onMouseDown={(e) => e.stopPropagation()}
            style={{
              padding: "10px 24px",
              backgroundColor: "rgba(239, 68, 68, 0.9)",
              color: "white",
              border: "none",
              borderRadius: 6,
              fontSize: 14,
              fontWeight: 500,
              cursor: "pointer",
              fontFamily: "system-ui, sans-serif",
            }}
          >
            Cancel (ESC)
          </button>
          <button
            onClick={(e) => {
              e.stopPropagation();
              handleSave();
            }}
            onMouseDown={(e) => e.stopPropagation()}
            style={{
              padding: "10px 24px",
              backgroundColor: "rgba(34, 197, 94, 0.9)",
              color: "white",
              border: "none",
              borderRadius: 6,
              fontSize: 14,
              fontWeight: 500,
              cursor: "pointer",
              fontFamily: "system-ui, sans-serif",
            }}
          >
            Save (Enter)
          </button>
        </div>
      )}

      {/* Global crosshair cursor */}
      <style>{`
        * { cursor: crosshair !important; }
      `}</style>
    </div>
  );
};

export default CalibrationOverlay;
