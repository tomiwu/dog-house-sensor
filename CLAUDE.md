# Instructions for Claude Code

Read `AGENTS.md` first. It has all the rules.

- Language: English, ASD-STE100 Simplified Technical English, in code comments, docs, commits and PRs.
- Before each commit, run `scripts/precommit.sh` (format, lint, test). All must pass.
- Rust toolchain: `rust-toolchain.toml` (Rust 1.100.0, edition 2024). Workspace members: `protocol` (no_std), `receiver` (Raspberry Pi bridge).
- The design is in `docs/PRD.md`. Read only the sections that the issue names.
- Radio defaults must stay legal in Poland and the EU (PRD 7.1a): channel 0 to 83, max 10 mW e.i.r.p. Other values only with `allow_out_of_band = true` and a clear warning.
- No hardware in CI. Use fakes for the radio, the sensor and MQTT.
