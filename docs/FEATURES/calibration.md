# Calibration Overlay

## Purpose
Allow users to define screen capture regions by drawing rectangles over their poker client window.

## Current Status
🚧 **BLOCKED** - Overlay renders inside app window instead of fullscreen over desktop

## Requirements
1. Overlay must render **fullscreen over entire desktop** (not just app window)
2. User can see their poker client through semi-transparent overlay
3. User draws rectangles to define capture regions:
   - Hole cards (2 regions for 2 cards)
   - Community cards (5 regions or 1 region)
   - Pot size (OCR region)
4. Regions persist to local storage / config file
5. ESC key cancels calibration mode

## Technical Challenge
Tauri windows by default render within their own bounds. To overlay the entire desktop:

### Option A: Tauri Fullscreen Transparent Window
```rust
// In tauri.conf.json or via Rust
WindowBuilder::new()
  .fullscreen(true)
  .transparent(true)
  .decorations(false)
  .always_on_top(true)
  .skip_taskbar(true)
```

### Option B: Separate Overlay Window
Create a second Tauri window specifically for calibration that spawns fullscreen.

### Option C: Native Screen Coordinates
Use Tauri's `window.set_position()` and `window.set_size()` to manually cover all monitors.

## Target Flow
1. User clicks "Calibrate" button
2. App enters fullscreen transparent overlay mode
3. Poker client visible underneath
4. User draws rectangles with mouse
5. Coordinates saved relative to screen (not app window)
6. User clicks "Done" or presses Enter
7. Overlay closes, app returns to normal mode

## Data Structure
```typescript
interface CalibrationRegion {
  id: string;
  label: 'hole_card_1' | 'hole_card_2' | 'community' | 'pot';
  x: number;      // Screen coordinate
  y: number;      // Screen coordinate
  width: number;
  height: number;
}

interface CalibrationConfig {
  regions: CalibrationRegion[];
  createdAt: string;
  pokerClient: string; // e.g., "PokerStars", "GGPoker"
}
```

## Files Involved
- `src/components/CalibrationOverlay.tsx` - React component
- `src-tauri/src/capture.rs` - Uses saved coordinates for capture
- `src-tauri/tauri.conf.json` - Window configuration

## Blockers
- [ ] Research Tauri fullscreen transparent window behavior
- [ ] Test on multiple monitors
- [ ] Handle DPI scaling

## Status
🚧 In Progress - core issue is fullscreen overlay rendering
