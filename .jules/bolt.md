## 2026-10-24 - Efficient Serialization of AppState
**Learning:** `serde_json::to_string` introduces unnecessary allocations and overhead compared to `serde_json::to_vec` when serializing data for encryption.
**Action:** Used `serde_json::to_vec` for serializing `AppState` in `save_to_disk` to reduce string allocations.
## 2024-10-25 - [Domain Matching Micro-Optimization]
**Learning:** Checking domain suffixes (`ends_with`) combined with string formatting (`format!`) inside loops in policy code incurs unnecessary heap allocation and slowdown.
**Action:** Replace `host.ends_with(&format!(".{}", t))` with a zero-allocation byte slice check: `host.ends_with(t) && host.len() > t.len() && host.as_bytes()[host.len() - t.len() - 1] == b'.'`.

## 2025-05-18 - [Webview Tab Hibernation & Memory Compaction]
**Learning:** Holding inactive child Webview instances alive indefinitely in multi-tab desktop clients consumes hundreds of megabytes per tab (~5GB for 10 tabs). Furthermore, C/C++ memory allocators like glibc malloc do not automatically return freed heap chunks to the OS without explicit compaction calls.
**Action:** Cap active live webviews in `TabManager` (e.g., MAX_ACTIVE_WEBVIEWS = 3) and auto-hibernate LRU background tabs into saved state metadata. Invoke `compact_process_memory()` (`malloc_trim(0)`) on tab closure, switch, or hibernation to immediately reclaim unmapped heap memory back to the OS. Disconnect DOM MutationObservers and background timers in injected scripts (`tab_ui.js`) when webviews are backgrounded.

## 2025-05-18 - [Optimization Without Allocation]
**Learning:** `format!` inside hot loops (like iterator `.any()`) causes heap allocation overhead. In `policy.rs` URL checking paths, computing `format!(".{}", domain)` during every `ends_with` check created unnecessary overhead in an O(n) context that is evaluated on every network request.
**Action:** Replace `ends_with(&format!(".{}", domain))` with an allocation-free array bound check `(host.ends_with(domain) && host.as_bytes().get(host.len() - domain.len() - 1) == Some(&b'.'))`. This computes the same sub-domain validation check without generating a new `String` on the heap.

## 2025-05-18 - [O(n^2) scaling from vector contains]
**Learning:** Using `.contains()` on a vector inside a loop results in O(n^2) time complexity. When scaling to many tabs (e.g. hibernation features), this causes significant latency.
**Action:** Always pre-allocate the capacity when building collections, and use `contains_key()` from `HashMap` (O(1)) instead of `.contains()` (O(n)) from a `Vec` for set-like inclusion checks.
