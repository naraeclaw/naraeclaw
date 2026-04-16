# naraeclaw-runtime — Transitional Holding Crate

This crate is a **temporary holding area**, not a permanent home. It contains 126K LOC of subsystems extracted from the original monolith that have not yet been decomposed into their final crate structure.

Do not add new functionality here without checking the active roadmap. Keep runtime changes small and focused around agent loop, gateway, channels orchestrator, daemon, cron, security, observability, TUI, skills, and doctor boundaries.

**Stability tier:** Experimental — no stability guarantee. Decomposition begins at v0.8.0.
