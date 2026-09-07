# Command History Follow-ups

Status: optional, unimplemented follow-ups retained from the
[archived palette proposal](../archived/command-palette-enhancements.md).

Palette history, pins and fuzzy ranking already ship through
[OverlaySurface Phase 4](../archived/overlay-surface.md). This is not a plan to
reimplement them. Remaining ideas from the superseded proposal:

- Define history schema migration when a version change requires it; current
  loading deserializes the stored map and defaults on errors.
- Decide whether to prune obsolete command keys or old history entries; there
  is no age-based pruning today.
- Consider explicit pin/unpin confirmation feedback beyond the existing pin
  indicator and row reordering.
- Consider cross-window history merge/locking if lost updates become a problem;
  current persistence reports write errors but does not merge concurrent writers.

These are deferred ideas, not release requirements or completed functionality.
