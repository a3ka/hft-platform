# PROJECT-STATE — что реализовано

> **Reviewer-owned.** Обновляется ТОЛЬКО reviewer'ом после merge (scope-guard). Отражает
> фактическое состояние `main`, не планы (планы — `docs/DESIGN.md §10` роадмап).

## Инфраструктура (готово, проверено 2026-07-10)
- Репо `a3ka/hft-platform` (private, ветка `main`).
- VPS cpx32 (`167.233.192.131`): Ubuntu 26.04, Docker 29.6 + Compose, Rust 1.97; репо
  склонирован в `/root/hft-platform` (read-only deploy-key).
- CI/CD: `.github/workflows/ci.yml` (fmt+clippy+test+audit) + `deploy.yml` (build-on-VPS,
  `git push main` → SSH → `docker compose up --build` → healthcheck → rollback). **Проверено
  сквозным деплоем: recorder-заглушка Up/healthy, journal-том persistent.**

## Код (M-00 — скелет)
- `crates/contracts` — минимальный T1 `Event`/`EventKind` (SCHEMA_VERSION=0). Тесты: 2 GREEN.
- `crates/recorder` — STUB (доказывает pipeline; рыночного не пишет).
- Процессный слой `.claude/` + `CLAUDE.md` — в работе.

## Пока НЕ реализовано
- Реальный даталеер/поток (Binance + Hyperliquid) — следующий блок.
- Крейты `journal` (DET-I-1), `book`, `venues`, `risk`, `killswitch`, `oms`, `sim`,
  `signals`, `alpha`, `portfolio`, `strategy`, `runner`, `research-cli` — пофазно per DESIGN §10.
