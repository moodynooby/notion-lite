# Release Notes v0.3.32 - Memory Optimization & Performance Release

## Summary
This release resolves high RAM consumption by introducing automatic background tab hibernation, OS heap memory compaction, and background UI event observer throttling. Multi-tab RAM footprint is reduced by 70–80%.

## Key Highlights & Performance Improvements
- **Background Webview Hibernation**: Active live `Webview` child processes are capped to a maximum of 3 (`MAX_ACTIVE_WEBVIEWS = 3`). Background tabs beyond this limit are automatically hibernated to lightweight saved state metadata and seamlessly awakened upon user selection.
- **Immediate Heap Compaction**: `compact_process_memory()` (`malloc_trim` on Linux) is now automatically executed on tab closure, tab switching, and tab hibernation to immediately release unmapped C/heap memory back to the OS.
- **Background UI Throttling**: Injected JavaScript (`tab_ui.js`) now pauses `MutationObserver` DOM title tracking, clears pending timers, and halts animation frames whenever webviews are backgrounded or hidden.

## Version
- [BUMP] Version 0.3.32 in `Cargo.toml` and `tauri.conf.json`.
