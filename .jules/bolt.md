## 2024-10-25 - [Domain Matching Micro-Optimization]
**Learning:** Checking domain suffixes (`ends_with`) combined with string formatting (`format!`) inside loops in policy code incurs unnecessary heap allocation and slowdown.
**Action:** Replace `host.ends_with(&format!(".{}", t))` with a zero-allocation byte slice check: `host.ends_with(t) && host.len() > t.len() && host.as_bytes()[host.len() - t.len() - 1] == b'.'`.
