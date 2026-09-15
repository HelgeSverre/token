# Files

- [Rendering, Layout, Hit Testing, and Damage](rendering-layout.md) - Explains how Token turns the current model into a shared layout snapshot, paints text and chrome, maps input coordinates, and limits redraw work with damage-aware fast paths.
- [Runtime, Event Loop, and Message Updates](runtime.md) - Explains how Token prepares application state, translates winit events into messages, applies updates on the main thread, schedules redraw damage, and delivers results from background workers. Use this page when tracing startup, input, asynchronous work, stale-result handling, or shutdown.
