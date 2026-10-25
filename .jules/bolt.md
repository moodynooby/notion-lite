## 2024-10-25 - [Domain Matching Micro-Optimization]
**Learning:** Checking domain suffixes (`ends_with`) combined with string formatting (`format!`) inside loops in policy code incurs unnecessary heap allocation and slowdown.
**Action:** Replace `host.ends_with(&format!(".{}", t))` with a zero-allocation byte slice check: `host.ends_with(t) && host.len() > t.len() && host.as_bytes()[host.len() - t.len() - 1] == b'.'`.

## 2025-05-18 - [Webview Tab Hibernation & Memory Compaction]
**Learning:** Holding inactive child Webview instances alive indefinitely in multi-tab desktop clients consumes hundreds of megabytes per tab (~5GB for 10 tabs). Furthermore, C/C++ memory allocators like glibc malloc do not automatically return freed heap chunks to the OS without explicit compaction calls.
**Action:** Cap active live webviews in `TabManager` (e.g., MAX_ACTIVE_WEBVIEWS = 3) and auto-hibernate LRU background tabs into saved state metadata. Invoke `compact_process_memory()` (`malloc_trim(0)`) on tab closure, switch, or hibernation to immediately reclaim unmapped heap memory back to the OS. Disconnect DOM MutationObservers and background timers in injected scripts (`tab_ui.js`) when webviews are backgrounded.
