// src/App.tsx
import React, { useState, useEffect } from "react";
import ControlBar from "./components/ControlBar";
import AnalysisPanel from "./components/AnalysisPanel";
import "./global.css";
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window';

// Get invoke directly from Tauri
const invoke = (window as any).__TAURI__?.core?.invoke;

// Add this NEW function to detect new hands and reset state
function isNewHand(currentCards: string[], previousCards: string[]): boolean {
  // New hand if we went from having cards to no cards, then back to having cards
  return currentCards.length > 0 && previousCards.length === 0;
}

// Convert backend recommendation to frontend format
function convertRecommendation(
  backendRec?: {
    action: "Fold" | "Check" | "Call" | "NoRecommendation" | { Bet: number } | { Raise: number };
    reasoning: string;
  }
): { kind: "raise"; amount: number } | { kind: "bet"; amount: number } | { kind: "call" } | { kind: "check" } | { kind: "fold" } | { kind: "none" } {
  if (!backendRec) {
    return { kind: "none" };
  }

  if (typeof backendRec.action === "string") {
    if (backendRec.action === "NoRecommendation") return { kind: "none" };
    if (backendRec.action === "Fold") return { kind: "fold" };
    if (backendRec.action === "Call") return { kind: "call" };
    if (backendRec.action === "Check") return { kind: "check" };
    return { kind: "fold" };
  } else if (typeof backendRec.action === "object") {
    if ("Raise" in backendRec.action) {
      return { kind: "raise", amount: Math.round(backendRec.action.Raise * 100) / 100 };
    } else if ("Bet" in backendRec.action) {
      return { kind: "bet", amount: Math.round(backendRec.action.Bet * 100) / 100 };
    }
  }

  return { kind: "fold" };
}

function App() {
  // State
  const [isPlaying, setIsPlaying] = useState(false);
  const [showPanel, setShowPanel] = useState(false);
  const [showSettings, setShowSettings] = useState(false);

  // Real poker state from backend
  const [pokerState, setPokerState] = useState<{
    your_cards: string[];
    community_cards: string[];
    pot_size: number | null;
    position: string | null;
    recommendation?: {
      action: "Fold" | "Check" | "Call" | "NoRecommendation" | { Bet: number } | { Raise: number };
      reasoning: string;
    };
    strength_score?: number;
    win_percentage?: number;
    tie_percentage?: number;
    street?: string;
  }>({
    your_cards: [],
    community_cards: [],
    pot_size: null,
    position: null,
  });

  const [previousCards, setPreviousCards] = useState<string[]>([]);

  // Listen for poker data from Rust backend
  useEffect(() => {
    const unlisten = listen('poker-capture', (event: any) => {
      console.log('📩 Received poker data:', event.payload);
      
      // Detect new hand - reset if transitioning from empty to full
      if (isNewHand(event.payload.your_cards, previousCards)) {
        console.log('🆕 New hand detected! Resetting state...');
      }
      
      setPreviousCards(event.payload.your_cards);
      setPokerState(event.payload);
    });

    return () => {
      unlisten.then(fn => fn());
    };
  }, [previousCards]);

  // Dynamic window resizing based on panel visibility
  useEffect(() => {
    console.log('🔄 useEffect triggered - showPanel:', showPanel);

    const resizeWindow = async () => {
      try {
        const appWindow = getCurrentWindow();
        console.log('📦 getCurrentWindow() successful');

        const CONTROL_BAR_HEIGHT = 80;
        const FULL_PANEL_HEIGHT = 550;
        const WINDOW_WIDTH = 700;

        if (showPanel) {
          console.log(`🔼 Attempting to expand window to ${WINDOW_WIDTH}x${FULL_PANEL_HEIGHT}...`);
          const size = new LogicalSize(WINDOW_WIDTH, FULL_PANEL_HEIGHT);

          try {
            await appWindow.setSize(size);
            console.log('✅ Window expanded successfully using setSize');
          } catch (setSizeErr) {
            console.warn('⚠️ setSize failed, trying alternative approach:', setSizeErr);
            await appWindow.setMinSize(size);
            await appWindow.setMaxSize(size);
            console.log('✅ Window expanded using setMinSize/setMaxSize');
          }

          const currentSize = await appWindow.innerSize();
          console.log('📏 Current window size:', currentSize);
        } else {
          console.log(`🔽 Attempting to collapse window to ${WINDOW_WIDTH}x${CONTROL_BAR_HEIGHT}...`);
          const size = new LogicalSize(WINDOW_WIDTH, CONTROL_BAR_HEIGHT);

          try {
            await appWindow.setSize(size);
            console.log('✅ Window collapsed successfully using setSize');
          } catch (setSizeErr) {
            console.warn('⚠️ setSize failed, trying alternative approach:', setSizeErr);
            await appWindow.setMinSize(size);
            await appWindow.setMaxSize(size);
            console.log('✅ Window collapsed using setMinSize/setMaxSize');
          }

          const currentSize = await appWindow.innerSize();
          console.log('📏 Current window size:', currentSize);
        }
      } catch (err) {
        console.error('❌ Failed to resize window:', err);
        console.error('Error details:', JSON.stringify(err, null, 2));
      }
    };

    resizeWindow();
  }, [showPanel]);

  // Handlers
  const handleToggle = async () => {
    if (!invoke) {
      console.error("Tauri API not ready yet!");
      return;
    }

    const newPlaying = !isPlaying;
    setIsPlaying(newPlaying);
    setShowPanel(newPlaying);
    
    if (newPlaying) {
      console.log("Starting poker monitoring...");
      try {
        const windows = await invoke('find_poker_windows');
        console.log("Found poker windows:", windows);
        await invoke('start_poker_monitoring');
        console.log("Monitoring started!");
      } catch (err) {
        console.error("Error starting monitoring:", err);
      }
    } else {
      console.log("Stopping poker monitoring...");
      try {
        await invoke('stop_poker_monitoring');
        console.log("Monitoring stopped!");
      } catch (err) {
        console.error("Error stopping monitoring:", err);
      }
    }
  };

  const handleSettings = () => {
    setShowSettings(!showSettings);
    console.log("Settings clicked");
  };

  const handleClear = () => {
    console.log("Clear clicked - resetting poker state (monitoring continues)");
    setPokerState({
      your_cards: [],
      community_cards: [],
      pot_size: null,
      position: null,
    });
    setPreviousCards([]);
  };

  const handleClose = async () => {
    if (invoke) {
      try {
        await invoke("exit_app");
      } catch (err) {
        console.error("Failed to close app:", err);
        window.close();
      }
    } else {
      window.close();
    }
  };

  return (
    <div
      style={{
        position: "fixed",
        top: 0,
        left: 0,
        right: 0,
        padding: "12px 0",
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        // NO gap here - use margin on children instead
        fontFamily: "system-ui, -apple-system, sans-serif",
        background: "transparent",
        pointerEvents: "none",
      }}
    >
      {/* Control Bar - Always Visible */}
      <div style={{ pointerEvents: "auto" }}>
        <ControlBar
          thinking={isPlaying}
          onToggle={handleToggle}
          onSettingsClick={handleSettings}
          onClearClick={handleClear}
          onCloseClick={handleClose}
        />
      </div>

      {/* Analysis Panel - Dropdown Animation */}
      <div
        style={{
          marginTop: showPanel ? 12 : 0,
          maxHeight: showPanel ? "1000px" : "0",
          opacity: showPanel ? 1 : 0,
          visibility: showPanel ? "visible" : "hidden",
          transform: `translateY(${showPanel ? 0 : -20}px)`,
          transition: "all 0.3s cubic-bezier(0.4, 0, 0.2, 1)",
          overflow: "hidden",
          pointerEvents: showPanel ? "auto" : "none",
          width: "100%",
          display: "flex",
          justifyContent: "center",
        }}
      >
        <AnalysisPanel
          key={`${pokerState.your_cards.join('-')}-${pokerState.community_cards.join('-')}`}
          state={{
            hand: pokerState.your_cards.length === 2
              ? [pokerState.your_cards[0], pokerState.your_cards[1]] as [string, string]
              : ["--", "--"] as [string, string],
            pot: pokerState.pot_size || 0,
            position: pokerState.position || "Unknown",
            community: pokerState.community_cards.length > 0
              ? pokerState.community_cards
              : ["--", "--", "--", "--", "--"],
            strength: pokerState.strength_score || 0,
            win: pokerState.win_percentage || 0,
            tie: pokerState.tie_percentage || 0,
            action: convertRecommendation(pokerState.recommendation),
            opponentSnapshot: pokerState.recommendation?.reasoning || "Live game in progress"
          }}
        />
      </div>

      {/* Settings Panel */}
      {showSettings && (
        <div style={{ pointerEvents: "auto", marginTop: 12 }}>
          <div
            className="pkr-frost-strong"
            style={{
              width: 560,
              padding: 20,
              borderRadius: 16,
            }}
          >
            <h3 style={{ margin: "0 0 16px 0", fontSize: 18, fontWeight: 700 }}>
              Settings
            </h3>
            <p style={{ margin: 0, fontSize: 14, color: "#98A2B3" }}>
              Settings panel coming soon...
            </p>
            <button
              onClick={() => setShowSettings(false)}
              style={{
                marginTop: 16,
                padding: "8px 16px",
                background: "#0C0F14",
                border: "1px solid #FFFFFF1A",
                borderRadius: 8,
                color: "#E8EEF5",
                cursor: "pointer",
              }}
            >
              Close
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;