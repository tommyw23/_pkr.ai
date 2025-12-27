// src/App.tsx
import { useState, useEffect, useRef } from "react";
import ControlBar from "./components/ControlBar";
import AnalysisPanel from "./components/AnalysisPanel";
import LoginScreen from "./components/LoginScreen";
import LowHandsWarning from "./components/LowHandsWarning";
import LimitReachedToast from "./components/LimitReachedDialog";
import { useAuthContext } from "./context/AuthContext";
import { useSubscription } from "./hooks/useSubscription";
import "./global.css";
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window';

// Get invoke directly from Tauri
const invoke = (window as any).__TAURI__?.core?.invoke;

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
  // Auth state
  const { user, loading: authLoading } = useAuthContext();

  // Subscription and usage tracking
  const {
    canAnalyzeHand,
    incrementHandsUsed,
    isNearLimit,
    handsRemaining,
    tierLimit,
    plan,
  } = useSubscription();

  // State - must be declared before any conditional returns
  const [isPlaying, setIsPlaying] = useState(false);
  const [showPanel, setShowPanel] = useState(false);
  const [showCalibrationWarning, setShowCalibrationWarning] = useState(false);
  const [showLimitReached, setShowLimitReached] = useState(false);
  const [showLowHandsWarning, setShowLowHandsWarning] = useState(false);

  // Generation tracking for stale result detection
  const [currentGeneration, setCurrentGeneration] = useState<number>(0);
  const [isAnalyzing, setIsAnalyzing] = useState<boolean>(false);
  const [wasInterrupted, setWasInterrupted] = useState<boolean>(false);

  // Track last counted hand to prevent double-counting
  const lastCountedHandRef = useRef<string>('');

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
    generation_id?: number;
    analysis_duration_ms?: number;
  }>({
    your_cards: [],
    community_cards: [],
    pot_size: null,
    position: null,
  });

  const [previousCards, setPreviousCards] = useState<string[]>([]);

  // Listen for poker data from Rust backend
  useEffect(() => {
    const unlistenCapture = listen('poker-capture', async (event: any) => {
      const payloadGeneration = event.payload.generation_id ?? 0;

      // Only update if generation matches or is newer (handles stale results)
      if (payloadGeneration >= currentGeneration) {
        setCurrentGeneration(payloadGeneration);
        setIsAnalyzing(false);
        setWasInterrupted(false);

        setPreviousCards(event.payload.your_cards);
        setPokerState(event.payload);

        // Increment hands used when we receive a valid recommendation
        const hasValidRecommendation =
          event.payload.recommendation &&
          event.payload.recommendation.action !== 'NoRecommendation' &&
          event.payload.your_cards.length === 2;

        if (hasValidRecommendation) {
          const handKey = event.payload.your_cards.sort().join('-');
          if (handKey !== lastCountedHandRef.current) {
            const success = await incrementHandsUsed();
            if (success) {
              lastCountedHandRef.current = handKey;
              if (isNearLimit && !showLowHandsWarning) {
                setShowLowHandsWarning(true);
              }
            }
          }
        }
      }
    });

    // Listen for generation changes (table state changed during analysis)
    const unlistenGenChange = listen('generation-change', (event: any) => {
      const { new_generation } = event.payload;
      setCurrentGeneration(new_generation);
      setWasInterrupted(true);
      setTimeout(() => setWasInterrupted(false), 3000);
    });

    // Listen for analysis-started (fires before each API call)
    const unlistenAnalysisStarted = listen('analysis-started', () => {
      setIsAnalyzing(true);
    });

    return () => {
      unlistenCapture.then(fn => fn());
      unlistenGenChange.then(fn => fn());
      unlistenAnalysisStarted.then(fn => fn());
    };
  }, [previousCards, currentGeneration, incrementHandsUsed, isNearLimit, showLowHandsWarning]);

  // Window sizing constants
  const LOGIN_WINDOW_WIDTH = 500;
  const LOGIN_WINDOW_HEIGHT = 600;
  const CONTROL_BAR_HEIGHT = 80;
  const TOAST_HEIGHT = 100;
  const FULL_PANEL_HEIGHT = 550;
  const MAIN_WINDOW_WIDTH = 850;

  // Resize window based on auth state (login screen vs main app)
  useEffect(() => {
    if (authLoading) return;

    const resizeForAuthState = async () => {
      try {
        const appWindow = getCurrentWindow();

        if (!user) {
          const size = new LogicalSize(LOGIN_WINDOW_WIDTH, LOGIN_WINDOW_HEIGHT);
          await appWindow.setSize(size);
          await appWindow.center();
        } else {
          const size = new LogicalSize(MAIN_WINDOW_WIDTH, CONTROL_BAR_HEIGHT);
          await appWindow.setSize(size);
        }
      } catch (err) {
        console.error('Failed to resize window:', err);
      }
    };

    resizeForAuthState();
  }, [user, authLoading]);

  // Dynamic window resizing based on panel visibility and warnings
  useEffect(() => {
    if (!user || authLoading) return;

    const resizeWindow = async () => {
      try {
        const appWindow = getCurrentWindow();

        let targetHeight = CONTROL_BAR_HEIGHT;
        if (showPanel) {
          targetHeight = FULL_PANEL_HEIGHT;
        } else if (showCalibrationWarning || showLowHandsWarning || showLimitReached) {
          targetHeight = CONTROL_BAR_HEIGHT + TOAST_HEIGHT;
        }

        const size = new LogicalSize(MAIN_WINDOW_WIDTH, targetHeight);

        try {
          await appWindow.setSize(size);
        } catch {
          await appWindow.setMinSize(size);
          await appWindow.setMaxSize(size);
        }
      } catch (err) {
        console.error('Failed to resize window:', err);
      }
    };

    resizeWindow();
  }, [showPanel, showCalibrationWarning, showLowHandsWarning, showLimitReached, user, authLoading]);

  // Show loading state while checking auth
  if (authLoading) {
    return (
      <div
        style={{
          width: "100vw",
          height: "100vh",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          background: "#0C0F14",
        }}
      >
        <div style={{ color: "#98A2B3", fontSize: 14 }}>Loading...</div>
      </div>
    );
  }

  // Show login screen if not authenticated
  if (!user) {
    return <LoginScreen />;
  }

  // Handlers
  const handleToggle = async () => {
    if (!invoke) {
      return;
    }

    // If we're about to start playing, check limits first
    if (!isPlaying) {
      if (!canAnalyzeHand()) {
        setShowLimitReached(true);
        return;
      }

      try {
        const calibrationData = await invoke('load_calibration_regions') as { regions: any[] };
        if (!calibrationData.regions || calibrationData.regions.length === 0) {
          setShowCalibrationWarning(true);
          return;
        }
      } catch {
        setShowCalibrationWarning(true);
        return;
      }
    }

    const newPlaying = !isPlaying;
    setIsPlaying(newPlaying);
    setShowPanel(newPlaying);

    if (newPlaying) {
      setIsAnalyzing(true);
      setWasInterrupted(false);
      try {
        await invoke('find_poker_windows');
        await invoke('start_poker_monitoring');
      } catch (err) {
        console.error("Error starting monitoring:", err);
        setIsAnalyzing(false);
      }
    } else {
      setIsAnalyzing(false);
      setWasInterrupted(false);
      try {
        await invoke('stop_poker_monitoring');
      } catch (err) {
        console.error("Error stopping monitoring:", err);
      }
    }
  };

  const handleClear = () => {
    setPokerState({
      your_cards: [],
      community_cards: [],
      pot_size: null,
      position: null,
    });
    setPreviousCards([]);
    lastCountedHandRef.current = '';
  };

  const handleClose = async () => {
    if (invoke) {
      try {
        await invoke("exit_app");
      } catch {
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
        fontFamily: "system-ui, -apple-system, sans-serif",
        background: "transparent",
        pointerEvents: "none",
      }}
    >
      {/* Control Bar - Always Visible */}
      <div style={{ pointerEvents: "auto" }}>
        <ControlBar
          thinking={isPlaying}
          isAnalyzing={isAnalyzing}
          wasInterrupted={wasInterrupted}
          onToggle={handleToggle}
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

      {/* Calibration Warning Toast */}
      {showCalibrationWarning && (
        <div
          style={{
            marginTop: 12,
            pointerEvents: "auto",
            animation: "slideDown 0.2s ease-out",
          }}
        >
          <div
            className="pkr-frost-strong"
            style={{
              width: 560,
              padding: "12px 16px",
              borderRadius: 12,
              display: "flex",
              alignItems: "center",
              gap: 12,
            }}
          >
            <div
              style={{
                width: 36,
                height: 36,
                borderRadius: 8,
                background: "#2563EB",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                fontSize: 18,
                flexShrink: 0,
              }}
            >
              ⊞
            </div>
            <div style={{ flex: 1 }}>
              <div style={{ fontSize: 14, fontWeight: 600, color: "#E8EEF5", marginBottom: 2 }}>
                Calibration Required
              </div>
              <div style={{ fontSize: 12, color: "#98A2B3" }}>
                Click the <strong style={{ color: "#3B82F6" }}>⊞</strong> button to select your poker table region
              </div>
            </div>
            <button
              onClick={() => setShowCalibrationWarning(false)}
              style={{
                padding: "6px 12px",
                background: "#2563EB",
                border: "none",
                borderRadius: 6,
                color: "#FFFFFF",
                cursor: "pointer",
                fontSize: 12,
                fontWeight: 600,
                flexShrink: 0,
              }}
            >
              Got it
            </button>
          </div>
        </div>
      )}

      {/* Low Hands Warning Toast */}
      {showLowHandsWarning && !showCalibrationWarning && !showPanel && !showLimitReached && (
        <LowHandsWarning
          handsRemaining={handsRemaining}
          onDismiss={() => setShowLowHandsWarning(false)}
        />
      )}

      {/* Limit Reached Toast */}
      {showLimitReached && !showCalibrationWarning && !showPanel && (
        <LimitReachedToast
          tierLimit={tierLimit}
          currentPlan={plan}
          onDismiss={() => setShowLimitReached(false)}
        />
      )}
    </div>
  );
}

export default App;
