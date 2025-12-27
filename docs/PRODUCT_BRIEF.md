# Product Brief: pkr.ai

## Goal
Build a real-time poker AI assistant desktop application that captures screen regions, detects cards via computer vision, and provides strategic analysis to help players make optimal decisions.

## Target User
- **Who:** Online poker players (recreational to semi-pro)
- **Pain:** Making suboptimal decisions in real-time, difficulty tracking pot odds and ranges
- **Success:** Improved win rate, faster decision-making, learning optimal strategy through AI guidance

## Core User Flow
1. User downloads and installs pkr.ai desktop app (Tauri)
2. User signs up / logs in (Supabase auth)
3. User subscribes to a tier ($29/$59/$99 monthly via Stripe)
4. User opens their poker client
5. User calibrates screen regions (fullscreen overlay to draw capture zones)
6. App captures cards + board in real-time via GPT-4o-mini vision OCR
7. App sends game state to o1-mini for strategic analysis
8. User receives real-time recommendations

## Tech Stack
- **Frontend:** React + TypeScript + Tailwind
- **Desktop:** Tauri (Rust backend)
- **Auth:** Supabase
- **Payments:** Stripe (3-tier subscriptions)
- **Vision OCR:** GPT-4o-mini
- **Strategy Engine:** o1-mini
- **Local fallback (deprecated):** YOLO card detection (reached 99% mAP but failed in real-world poker UIs)

## Non-Goals (for now)
- Multi-table support
- Hand history import/analysis
- Mobile app
- Tournament-specific features
- Social/community features

## Success Criteria
- Hard launch: **January 1st, 2025**
- Calibration overlay works fullscreen over poker clients
- Card detection >95% accuracy in real poker client UIs
- Strategy response latency <2 seconds
- 70% profit margin on API costs
- Functional subscription flow end-to-end

## Architecture Approach
Hybrid cloud model:
- GPT-4o-mini handles vision (cheap, fast OCR)
- o1-mini handles reasoning (strategic analysis)
- Rust backend manages screen capture, state, and API orchestration
- React frontend for UI/settings/calibration
