# M-21 — Journal-hardening: машинные барьеры целостности (defense-in-depth)

STATUS: **PROPOSED / QUEUED** (2026-07-21, architect, по запросу reviewer'а после M-18/TD-031).
НЕ стартует без явного `go` founder'а. **Приоритет — НИЖЕ ретеншена (TD-020/M-08): диск ~25 дней
до disk-guard, а journal-hardening — не срочно.** Doc-гейт §9 Class A при старте (critic на
milestone+RED ДО dispatch). Зона: sacred journal/recorder live-path (класс TD-011 — осторожно).

## Objective

M-18/TD-031 показал: изоляция сегмент-эпох сейчас держится на ДИСЦИПЛИНЕ + одном машинном гейте
(`decide_open_segment` schema-совпадение). Несколько защит остаются процессными/ручными. M-21
превращает их в машинные барьеры. Каждый пункт — самостоятельный RED-first трек; можно делать по
одному, в любом порядке, БЕЗ блокировки друг друга. Ни один не трогает order-path/risk (MD-only,
journal-integrity).

## Contract impact (T1)

- Трек C (TD-033) может тронуть `SCHEMA_VERSION`-governance → contract-RFC гейт (Block-C) при старте.
- Треки A/D/E — impl журнала/recorder + build, БЕЗ новой T1-формы.

## Треки (каждый — отдельный RED-first)

### A. TD-032 — provenance-forensics (git-sha на СБОРКЕ, не в рантайме)
Сейчас `recorder` считает git-sha через `git rev-parse` В РАНТАЙМЕ → контейнер без git →
provenance-КОНСТАНТА `no-git-info` на всех билдах (корень TD-031; изоляция спасена schema-гейтом, но
сегменты НЕ различимы по билду для форензики). Фикс: git-sha вкомпилён на СБОРКЕ (`build.rs` /
`vergen` / build-arg `GIT_SHA` → `env!`/`option_env!`), provenance несёт реальный sha.
- **RED (прод-масштаб, testing.md):** оракул ПАДАЕТ на текущем рантайм-git режиме (provenance ==
  константа при недоступном git) и зеленеет, когда sha вкомпилён; fallback (build-arg не передан) —
  fail-closed (не тихая константа). Defense-in-depth к schema-гейту, НЕ замена.
- Зона: `crates/recorder` + build-конфиг (Dockerfile build-arg) — engine-dev; RED — architect.

### B. TD-033 (C-018 rev4 N1) — энфорсмент «новый вариант ⇒ bump SCHEMA_VERSION»
Правило изоляции держится ДИСЦИПЛИНОЙ: если кто-то добавит эмитируемый вариант `EventKind`/`MdPayload`
и НЕ поднимет `SCHEMA_VERSION`, изоляция молча сломается (сегменты старой и новой эпохи неразличимы).
- **Машинный энфорсмент:** (а) ЯВНЫЙ пункт contract-RFC чек-листа (reviewer Block-C): «новый
  эмитируемый вариант → bump SCHEMA_VERSION»; И/ИЛИ (б) grep/тест-канарейка: число эмитируемых
  вариантов, которые recorder может писать, увязано со `SCHEMA_VERSION` (RED падает при добавлении
  варианта без bump'а). Точная форма — на дизайне architect'а при старте.
- Зона: `crates/contracts/tests` + verify + `.claude/rules` (Block-C) — architect.

### C. TD-029 — recorder startup schema-guard (rollback-направление)
recorder на старте ГРОМКО падает, если активный сегмент несёт события, которые бинарь не умеет
декодить (превращает тихий seq-reuse при откате на старый бинарь в громкий отказ). Уже в TECH-DEBT.
- Зона: `crates/recorder`/`crates/journal` — engine-dev; RED — architect.

### D. TD-030 — reader `first_seq`-guard (fail-closed на re-stitch)
`read_all`/`stream` сейчас сшивают сегменты по индексу БЕЗ проверки монотонности `first_seq` →
ошибочный re-stitch архива тих (беспорядок `[0,1,2,3,4,7,5,6]`). Fail-closed `Err` делает правило
«не re-stitch» ПРИНУДИТЕЛЬНЫМ. ⚠ **Осторожно с legacy-сентинелом `first_seq=0`** (прод-путь TD-011)
— наивный монотонный гейт споткнётся на боевом legacy-сегменте. Уже в TECH-DEBT.
- Зона: `crates/journal` (sacred read-path) — engine-dev; RED — architect (прод-масштаб + legacy-кейс).

## Allowed / Forbidden paths (при старте, по треку)

- `*/tests/**`, `scripts/verify_M-21.sh`, milestone — **architect** (sacred).
- `crates/{journal,recorder}/src/**` (impl треков) — **engine-dev**; build-конфиг (Dockerfile/build.rs) — engine-dev.
- `crates/contracts/**` (трек B, только через contract-RFC) — **architect**.
- **Forbidden:** order-path, `crates/{risk,killswitch,oms}`, смена `SEGMENT_MAGIC`, ослабление schema-гейта.

## Гейты

- **critic** при старте (новый milestone; трек B — contract-RFC + critic).
- **risk-critic** для треков, трогающих sacred journal read/recorder live-path (D особенно — класс TD-011).
- §8 если impl меняет прод-поведение recorder'а (A startup, C guard) — деплой-гейт.

## Место в очереди

- **СТРОГО после доставки ретеншена (TD-020/M-08 task 14).** Диск ~25 дней до guard (L2Delta BTC-only
  +0.8 GB/сут ⇒ ~3.8 GB/сут) — ретеншен важнее любого hardening'а. M-21 — когда диск под контролем.
- Треки независимы: можно брать по одному, приоритет внутри — founder ★.
- Изоляция эпох УЖЕ работает (schema-гейт, §8-подтверждено); M-21 — defense-in-depth, не закрытие дыры.

## Handoff (план при старте)

critic → (per трек) architect RED → engine-dev impl → risk-critic (sacred-треки) → reviewer + §8.
