# Roadmap

## Now (Sprint to Jan 1 Launch)
- [ ] Fix calibration overlay (fullscreen mode over external windows)
- [ ] GPT-4o-mini vision OCR integration
- [ ] o1-mini strategy engine integration
- [ ] End-to-end game state flow (capture → detect → analyze → display)
- [ ] Real-time UI showing detected cards + recommendations

## Next (Post-Launch Polish)
- [ ] Optimize capture interval (<2 second latency)
- [ ] Error handling and retry logic for API failures
- [ ] Usage analytics and cost monitoring
- [ ] Onboarding flow improvements
- [ ] Settings persistence

## Later (Growth Features)
- [ ] Hand history logging
- [ ] Session statistics
- [ ] Multi-table support
- [ ] Advanced strategy modes (tournament vs cash)
- [ ] Offline mode with local models

## Completed
- [x] Tauri + React project scaffolding
- [x] Supabase authentication (signup/login)
- [x] Stripe subscription integration ($29/$59/$99 tiers)
- [x] Checkout flow with Stripe
- [x] Landing page
- [x] Basic screen capture functionality
- [x] YOLO model training (V1: 95.7% → V2: 99.07% → V3 with card backs)
- [x] Gemini→Claude cascade approach (deprecated)
- [x] Decision: Pivot from local YOLO to cloud GPT-4o-mini (YOLO failed on real poker UIs)

## Blockers / Known Issues
- Calibration overlay renders inside app window instead of fullscreen over desktop
- Need to test GPT-4o-mini accuracy on various poker client skins
