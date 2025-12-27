# Architecture

## Overview
pkr.ai is a Tauri desktop app with a React frontend and Rust backend. It uses a hybrid cloud approach for card detection and strategy analysis.

```
┌─────────────────────────────────────────────────────────────┐
│                     Desktop (Tauri)                         │
│  ┌─────────────────┐    ┌─────────────────────────────────┐ │
│  │  React Frontend │◄──►│         Rust Backend            │ │
│  │  - UI/Settings  │    │  - Screen capture               │ │
│  │  - Calibration  │    │  - Image processing             │ │
│  │  - Results view │    │  - API orchestration            │ │
│  │  - Auth flow    │    │  - State management             │ │
│  └─────────────────┘    └──────────┬────────────────────┬─┘ │
└─────────────────────────────────────┼────────────────────┼───┘
                                      │                    │
                                      ▼                    ▼
                            ┌─────────────────┐  ┌─────────────────┐
                            │  GPT-4o-mini    │  │    o1-mini      │
                            │  (Vision OCR)   │  │  (Strategy)     │
                            │  - Card detect  │  │  - Range calc   │
                            │  - Board read   │  │  - Action rec   │
                            └─────────────────┘  └─────────────────┘
```

## Data Flow
1. **Capture:** Rust backend captures screen region at configured interval
2. **Detect:** Image sent to GPT-4o-mini for card/board OCR
3. **Parse:** Rust parses structured response into game state
4. **Analyze:** Game state sent to o1-mini for strategic analysis
5. **Display:** Recommendations pushed to React UI via Tauri commands

## Directory Structure (Target)
```
/pkr-ai
├── src/                    # React frontend
│   ├── components/
│   │   ├── CalibrationOverlay.tsx
│   │   ├── GameStateDisplay.tsx
│   │   └── SettingsPanel.tsx
│   ├── hooks/
│   ├── lib/
│   │   ├── supabase.ts
│   │   └── stripe.ts
│   └── App.tsx
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── main.rs
│   │   ├── capture.rs      # Screen capture logic
│   │   ├── vision.rs       # GPT-4o-mini integration
│   │   ├── strategy.rs     # o1-mini integration
│   │   └── state.rs        # Game state management
│   └── Cargo.toml
├── docs/                   # This documentation
└── package.json
```

## Key Technical Decisions

### Why GPT-4o-mini for Vision (not local YOLO)
- YOLO achieved 99% mAP in training but failed on real poker client UIs
- Card rendering varies wildly across clients (skins, sizes, angles)
- GPT-4o-mini handles visual variation robustly
- Cost acceptable at ~720 calls/hour active use

### Why o1-mini for Strategy (not GPT-4)
- Reasoning-optimized model for complex decisions
- Better at range calculations and EV analysis
- Slower but accuracy matters more than speed for strategy

### Why Tauri (not Electron)
- Smaller bundle size
- Better performance for screen capture
- Rust backend enables fast image processing

## External Services
| Service | Purpose | Config Location |
|---------|---------|-----------------|
| Supabase | Auth + user data | `src/lib/supabase.ts` |
| Stripe | Subscriptions | `src/lib/stripe.ts` |
| OpenAI | Vision + Strategy | `src-tauri/src/vision.rs`, `strategy.rs` |

## Environment Variables
```
VITE_SUPABASE_URL=
VITE_SUPABASE_ANON_KEY=
STRIPE_SECRET_KEY=
STRIPE_WEBHOOK_SECRET=
OPENAI_API_KEY=
```
