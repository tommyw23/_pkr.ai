# Changelog

## 2024-12 (Pre-Launch Sprint)

### Dec 25
#### Calibration System Implementation
- Added calibration button to ControlBar (blue ⊞ icon)
- Created fullscreen transparent overlay for region selection (`CalibrationOverlay.tsx`)
- User can click-drag to select poker table region with crosshair cursor
- Saves calibration to `~/Library/Application Support/com.srikanthnani.pluely/calibration.json`
- Coordinates saved in physical pixels (DPI-scaled for Retina displays)

#### Calibration → Vision Pipeline Integration
- Added `load_calibration_data()` to load saved regions
- Added `capture_calibrated_region()` to capture from saved coordinates
- Added `process_calibrated_capture()` to run calibrated images through cascade
- Modified `start_poker_monitoring()` to use calibration when available
- Falls back to window detection if no calibration exists
- Test capture button (orange 📷) for debugging

#### Files Modified
- `src/components/ControlBar.tsx` - Added calibration + test capture buttons
- `src/components/CalibrationOverlay.tsx` - New calibration UI component
- `src/main.tsx` - Routes to CalibrationOverlay for calibration-overlay window
- `src-tauri/src/calibration.rs` - Calibration commands (start, close, save, load, test)
- `src-tauri/src/poker_capture.rs` - Integrated calibration with cascade pipeline
- `src-tauri/src/lib.rs` - Registered calibration commands
- `src-tauri/capabilities/default.json` - Added window size permissions

### Week of Dec 23
- Initialized documentation system for AI-assisted development
- Created PRODUCT_BRIEF, ROADMAP, ARCHITECTURE docs
- Documented all major features: auth, billing, calibration, vision, strategy

### Earlier in December
- Pivoted from local YOLO to cloud GPT-4o-mini for card detection
- YOLO V3 achieved 98.85% validation accuracy but failed on real poker client UIs
- Decision: Hybrid approach with GPT-4o-mini (vision) + o1-mini (strategy)
- Started 8-phase rebuild plan across 3 parallel tracks

## 2024-11 (Foundation)

### Authentication
- Implemented Supabase auth (signup/login/logout)
- Protected routes requiring authentication

### Billing
- Integrated Stripe subscriptions
- 3-tier pricing: $29 / $59 / $99 monthly
- Checkout flow with Stripe-hosted pages
- Webhook handling for subscription events

### Landing Page
- Marketing page completed
- Pricing section with tier comparison

### Screen Capture
- Basic screen capture functionality in Rust backend
- Initial capture interval: 2 seconds (needs optimization)

## 2024-10 (Exploration)

### YOLO Training
- V1: 95.7% mAP50 - insufficient accuracy
- V2: 99.07% mAP50 - excellent training metrics
- V3: Added card back detection - 98.85% validation
- Conclusion: Training metrics don't transfer to real-world poker UIs

### Architecture Decisions
- Chose Tauri over Electron (performance, bundle size)
- Chose Supabase for auth (simplicity, generous free tier)
- Chose Stripe for payments (industry standard)

---

## How to Update This File
After completing any task, add an entry with:
- Date/week
- What was done
- Files involved (if significant)
- Any decisions made
