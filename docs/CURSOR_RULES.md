# Cursor Rules for pkr.ai

## Before ANY Task

Always read these files first:
1. `/docs/PRODUCT_BRIEF.md` - Understand what we're building
2. `/docs/ROADMAP.md` - Know current priorities and blockers
3. Relevant `/docs/FEATURES/*.md` - Get implementation details

## After EVERY Task

1. Update relevant documentation to reflect changes
2. Update `/docs/ROADMAP.md` if scope changed
3. Add entry to `/docs/CHANGELOG.md`

## Golden Prompt Template

```
Before doing anything:
- Read /docs/PRODUCT_BRIEF.md
- Read /docs/ROADMAP.md
- Read /docs/FEATURES/[relevant-feature].md

Then:
- Implement the requested change
- Update or create documentation
- Update the roadmap if scope changes
- Add an entry to CHANGELOG.md
```

## Feature Implementation Checklist

- [ ] Read existing feature doc (or create one first)
- [ ] Implement the feature
- [ ] Update feature doc with implementation details
- [ ] Update ARCHITECTURE.md if new patterns introduced
- [ ] Move roadmap item to Completed
- [ ] Add CHANGELOG entry

## Code Standards

- TypeScript strict mode
- React functional components with hooks
- Rust for Tauri backend (screen capture, API calls)
- Tailwind for styling
- No placeholder code - paste-ready implementations only

## Current Priority (Jan 1 Launch)

1. Fix calibration overlay (fullscreen over desktop)
2. GPT-4o-mini vision integration
3. o1-mini strategy integration
4. End-to-end flow working
