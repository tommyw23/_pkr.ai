import React, { useCallback, useEffect, useRef, useState } from "react";
import AnalysisPanel from "./AnalysisPanel";
import type { GameState } from "../types/poker";

type Props = {
  initial?: Partial<GameState>;
  onClose?: () => void;
};

export default function OverlayRoot({ initial, onClose: _onClose }: Props) {
  const [state, setState] = useState<GameState>(() => ({
    running: initial?.running ?? true,
    visible: initial?.visible ?? true,
    holeCards: (initial?.holeCards as any) ?? ["As", "Kd"],
    pot: initial?.pot ?? 1850,
    confidence: initial?.confidence ?? 0.85,
    recommendation: initial?.recommendation ?? {
      action: "RAISE",
      amount: 800,
      reason: "SPR low; villain capped; build pot",
    },
    context: initial?.context,
  }));

  // Position (draggable)
  const [pos, setPos] = useState<{ x: number; y: number }>({
    x: window.innerWidth / 2 - 200,
    y: 12,
  });
  const dragRef = useRef<HTMLDivElement>(null);
  const isDragging = useRef(false);
  const dragOffset = useRef<{ dx: number; dy: number }>({ dx: 0, dy: 0 });

  const onMouseDown = useCallback(
    (e: React.MouseEvent) => {
      const target = e.target as HTMLElement;
      if (!target.closest("[data-drag-handle]")) return;
      isDragging.current = true;
      dragOffset.current = { dx: e.clientX - pos.x, dy: e.clientY - pos.y };
    },
    [pos.x, pos.y]
  );

  const onMouseMove = useCallback((e: MouseEvent) => {
    if (!isDragging.current) return;
    const x = e.clientX - dragOffset.current.dx;
    const y = e.clientY - dragOffset.current.dy;
    setPos({ x, y });
  }, []);

  const onMouseUp = useCallback(() => {
    isDragging.current = false;
  }, []);

  useEffect(() => {
    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, [onMouseMove, onMouseUp]);

  // Double-click the handle to toggle panel visibility
  const onHandleDoubleClick = () =>
    setState((s) => ({ ...s, visible: !s.visible }));

  return (
    <div
      ref={dragRef}
      onMouseDown={onMouseDown}
      className="pkr-no-select"
      style={{
        position: "fixed",
        left: pos.x,
        top: pos.y,
        width: 400,
        zIndex: 2147483000,
        display: "flex",
        alignItems: "flex-start",
        flexDirection: "column",
        gap: 8,
      }}
    >
      {/* Lightweight drag handle instead of a second ControlBar */}
      <div
        data-drag-handle
        onDoubleClick={onHandleDoubleClick}
        title="Drag (double-click to show/hide)"
        style={{
          width: 560,
          height: 10,
          borderRadius: 6,
          background: "rgba(255,255,255,0.08)",
          border: "1px solid rgba(255,255,255,0.12)",
          cursor: "move",
        }}
      />

      <div
        style={{
          transition: "opacity 150ms ease, transform 150ms ease, height 150ms ease",
          opacity: state.visible ? 1 : 0,
          transform: `translateY(${state.visible ? 0 : -6}px)`,
          pointerEvents: state.visible ? "auto" : "none",
        }}
      >
        <AnalysisPanel state={state as any} />
      </div>
    </div>
  );
}
