// src/App.tsx
import React, { useState, useEffect } from "react";
import ControlBar from "./components/ControlBar";
import AnalysisPanel from "./components/AnalysisPanel";
import "./global.css";
import { listen } from '@tauri-apps/api/event';

// Get invoke directly from Tauri
const invoke = (window as any).__TAURI__?.core?.invoke;

// Improved hand strength calculator with community cards
function calculateStrength(yourCards: string[], community: string[]): number {
  if (yourCards.length === 0) return 0;
  
  const allCards = [...yourCards, ...community];
  
  // Parse ranks
  const parseRank = (card: string) => {
    const rank = card.slice(0, -1);
    if (rank === 'A') return 14;
    if (rank === 'K') return 13;
    if (rank === 'Q') return 12;
    if (rank === 'J') return 11;
    return parseInt(rank) || 10;
  };
  
  const parseSuit = (card: string) => card.slice(-1);
  
  const ranks = allCards.map(parseRank).sort((a, b) => b - a);
  const suits = allCards.map(parseSuit);
  
  // Count rank occurrences
  const rankCounts: { [key: number]: number } = {};
  ranks.forEach(r => rankCounts[r] = (rankCounts[r] || 0) + 1);
  const counts = Object.values(rankCounts).sort((a, b) => b - a);
  
  // Check for flush
  const suitCounts: { [key: string]: number } = {};
  suits.forEach(s => suitCounts[s] = (suitCounts[s] || 0) + 1);
  const hasFlush = Object.values(suitCounts).some(count => count >= 5);
  
  // Check for straight
  const uniqueRanks = [...new Set(ranks)].sort((a, b) => b - a);
  let hasStraight = false;
  for (let i = 0; i <= uniqueRanks.length - 5; i++) {
    if (uniqueRanks[i] - uniqueRanks[i + 4] === 4) {
      hasStraight = true;
      break;
    }
  }
  
  // Evaluate hand
  if (hasStraight && hasFlush) return 95; // Straight flush
  if (counts[0] === 4) return 90; // Four of a kind
  if (counts[0] === 3 && counts[1] === 2) return 85; // Full house
  if (hasFlush) return 80; // Flush
  if (hasStraight) return 75; // Straight
  if (counts[0] === 3) return 65; // Three of a kind
  if (counts[0] === 2 && counts[1] === 2) return 55; // Two pair
  if (counts[0] === 2) {
    // Pair - strength based on rank
    const pairRank = parseInt(Object.keys(rankCounts).find(k => rankCounts[parseInt(k)] === 2) || '0');
    return 35 + (pairRank * 2); // 35-63%
  }
  
  // High card - based on highest card
  return 15 + (ranks[0] * 1.5); // 15-36%
}

// Get recommended action with proper check/fold logic
function getRecommendedAction(
  yourCards: string[], 
  community: string[], 
  pot: number
): { kind: "raise"; amount: number } | { kind: "call" } | { kind: "fold" } {
  if (yourCards.length === 0) {
    return { kind: "fold" }; // No cards yet
  }
  
  const strength = calculateStrength(yourCards, community);
  
  // Determine if we can check (free, no bet to call)
  // Simplified: if pot is very small relative to normal, assume it's a check situation
  const canCheck = pot < 1000; // Adjust threshold as needed
  
  if (strength >= 70) {
    // Strong hand - raise aggressively
    const raiseAmount = Math.round(pot * 2);
    return { kind: "raise", amount: raiseAmount };
  } else if (strength >= 50) {
    // Medium-strong hand - smaller raise or call
    const raiseAmount = Math.round(pot * 1.2);
    return { kind: "raise", amount: raiseAmount };
  } else if (strength >= 35) {
    // Medium hand - call/check
    return { kind: "call" };
  } else {
    // Weak hand - fold unless we can check
    if (canCheck) {
      return { kind: "call" }; // Display as "call" but means check
    }
    return { kind: "fold" };
  }
}

// Add this NEW function to detect new hands and reset state
function isNewHand(currentCards: string[], previousCards: string[]): boolean {
  // New hand if we went from having cards to no cards, then back to having cards
  return currentCards.length > 0 && previousCards.length === 0;
}

function App() {
  // State
  const [isPlaying, setIsPlaying] = useState(false);
  const [showPanel, setShowPanel] = useState(false);
  const [showSettings, setShowSettings] = useState(false);

  // Real poker state from Gemini
const [pokerState, setPokerState] = useState({
  your_cards: [] as string[],
  community_cards: [] as string[],
  pot_size: null as number | null,
  position: null as string | null,
});

// Listen for poker data from Rust backend
// Listen for poker data from Rust backend
const [previousCards, setPreviousCards] = useState<string[]>([]);

// Listen for poker data from Rust backend
useEffect(() => {
  const unlisten = listen('poker-capture', (event: any) => {
    console.log('📩 Received poker data:', event.payload);
    
    // Detect new hand - reset if transitioning from empty to full
    if (isNewHand(event.payload.your_cards, previousCards)) {
      console.log('🆕 New hand detected! Resetting state...');
      // Reset happens automatically by setting new state
    }
    
    setPreviousCards(event.payload.your_cards);
    setPokerState(event.payload);
  });

  return () => {
    unlisten.then(fn => fn());
  };
}, [previousCards]);

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
        // Find poker windows
        const windows = await invoke('find_poker_windows');
        console.log("Found poker windows:", windows);
        
        // Start monitoring
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

  const handleLog = () => {
    // Copy current analysis to clipboard
    const analysis = `
pkr.ai Hand Analysis
═══════════════════════════════════
Hand: ${pokerState.your_cards.join(" ")}
Pot: $${pokerState.pot_size || 0}
Position: ${pokerState.position || "Unknown"}
Strength: ${calculateStrength(pokerState.your_cards, pokerState.community_cards)}%
Community: ${pokerState.community_cards.join(" ")}
Recommendation: Analyzing...
Notes: Live game in progress
    `.trim();

    navigator.clipboard.writeText(analysis).then(() => {
      console.log("Analysis copied to clipboard!");
    }).catch(err => {
      console.error("Failed to copy:", err);
    });
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
        gap: 12,
        fontFamily: "system-ui, -apple-system, sans-serif",
      }}
    >
      {/* Control Bar - Always Visible */}
      <ControlBar
        thinking={isPlaying}
        onToggle={handleToggle}
        onSettingsClick={handleSettings}
        onLogClick={handleLog}
        onCloseClick={handleClose}
      />

      {/* Analysis Panel - Dropdown Animation */}
      <div
        style={{
          maxHeight: showPanel ? "1000px" : "0",
          opacity: showPanel ? 1 : 0,
          transform: `translateY(${showPanel ? 0 : -20}px)`,
          transition: "all 0.3s cubic-bezier(0.4, 0, 0.2, 1)",
          overflow: "hidden",
          pointerEvents: showPanel ? "auto" : "none",
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
    strength: calculateStrength(pokerState.your_cards, pokerState.community_cards),
    win: 0,
    tie: 0,
    action: getRecommendedAction(pokerState.your_cards, pokerState.community_cards, pokerState.pot_size || 0),
    opponentSnapshot: "Live game in progress"
  }}
/>
      </div>

      {/* Settings Panel */}
      {showSettings && (
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
      )}
    </div>
  );
}

export default App;