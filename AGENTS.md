# AGENTS.md — web-reflex Guidelines

## Invariants
1. **Dual-Doc Sync**: 웹 자동화 액션/CLI 변경 시 `README.md` 및 `~/.hermes/skills/software-development/deterministic-browser-automation/SKILL.md`를 함께 갱신한다.
2. **Binary Sync**: 빌드 후 `~/.local/bin/web-reflex`에 설치한다.
3. **Deterministic First**: 불안정한 Sleep 기반 대기를 금지하고 DOM/CDP 이벤트 기반 래칫 액션을 보장한다.
