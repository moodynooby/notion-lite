## 2026-10-24 - Efficient Serialization of AppState
**Learning:** `serde_json::to_string` introduces unnecessary allocations and overhead compared to `serde_json::to_vec` when serializing data for encryption.
**Action:** Used `serde_json::to_vec` for serializing `AppState` in `save_to_disk` to reduce string allocations.
