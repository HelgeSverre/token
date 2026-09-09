# Handoff

Updated 2026-09-09. Temporary continuation note, not a roadmap.

## Where we stopped

Pixel scrolling and wheel easing are complete and merged into `main`
(`c4db196`); the feature branch is deleted. Tests, lint and native macOS
synthetic-input/restart checks passed. The plan is archived.

## Unfinished user report

The editor I-beam sometimes disappears on hover. It has not been reliably
reproduced or fixed. Hit testing requests `CursorIcon::Text`; next investigate
native cursor restoration if the report still reproduces. See the
[cursor findings](docs/dev/refactoring-audit-2026-09-06.md#font-roles-and-context-menu-layout--2026-09-08).

## Notes for the next agent

- Build and keep verification artifacts in the repository's normal `target/`.
- Intermittent test-process exit warnings/startup delays remain unexplained;
  clean reruns are not a root-cause fix. Evidence is in the
  [verification report](docs/benchmark/2026-09-09-pixel-scrolling.md#verification-boundary).
- Future features, optional providers and broader platform checks belong in
  their existing plans/audit records, not this handoff. IME remains deferred.

Delete this file once the cursor follow-up is resolved or explicitly parked.
Do not keep it alive for the broader roadmap.
