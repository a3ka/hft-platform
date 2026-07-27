# M-40 — retention видит .zst-сегменты (R2, ШАГ 0b)

**Статус:** PLANNED (стаб; RED-first спека при взятии в работу)
**Риск:** R2 CRITICAL (`docs/08-arch-improvement-roadmap.md`). Блокирует R1 от тихого провала.
**Приоритет:** высокий (данные/необратимость), но R1 отложен founder'ом ~2 нед → не горит сегодня.

## Objective
`journal::retention_plan()` (segments.rs:~1369) сканирует каталог своим `read_dir` с фильтром
`extension=="jrnl"` → сжатые `.jrnl.zst` БЕЗУСЛОВНО выпадают из плана. Когда включат offsite-apply (R1),
накопленные .zst останутся на NVMe навсегда (offload/prune не сработает) — противоречит docs/06 §4.

## Allowed paths
- `crates/journal/tests/` (architect RED) · `crates/journal/src/segments.rs` (engine-dev фикс) · `scripts/verify_M-40.sh` · этот файл.

## Задачи (RED-first)
1. (architect RED) `retention_plan()` над каталогом со СМЕСЬЮ raw `.jrnl` + `.jrnl.zst` разного возраста →
   assert: .zst входят в план по возрасту наравне с raw. Анти-плацебо: падает на текущем extension-фильтре.
   Прод-масштаб + композиция стадий (compaction×retention — testing.md урок, класс «идеальная фикстура»).
2. (engine-dev) `retention_plan` ходит через общий `segments()`/`dedup_indexed_paths()` (прод-путь stream), не свой read_dir.

## Гейты: critic (касание journal-ретеншена) · reviewer. risk-critic не нужен (нет order-path).
## Cross-ref: docs/08 R2, docs/06 §4, TD-020/006.
