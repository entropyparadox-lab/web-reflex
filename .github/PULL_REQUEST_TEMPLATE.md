## Summary of Changes

<!-- Describe what this PR introduces, fixes, or optimizes. -->

## Type of Change

- [ ] 🐛 Bug fix (non-breaking change which fixes an issue)
- [ ] ⚡ New feature (action cache, healing strategy, SDK addition)
- [ ] 🚀 Performance optimization (<50ms fast-path replay)
- [ ] 🛡️ Security / Zero-PII sanitization
- [ ] 📝 Documentation update
- [ ] 🧪 Tests (Playwright E2E, unit tests)

## Checklist

- [ ] Code follows formatting rules (`cargo fmt --all -- --check`).
- [ ] Clippy checks pass (`cargo clippy --workspace -- -D warnings`).
- [ ] Rust and Python tests pass (`cargo test --workspace`, `pytest`).
- [ ] Zero-PII invariant preserved (no raw sensitive values logged or stored).
- [ ] Conventional Commit format used (e.g. `feat(engine): ...`, `fix(storage): ...`).
