<!-- GATE-META
milestone: TD-241
audited_repo: a3ka/hft-platform
audited_base: 32574deddfd273a925a9ae9ca146a4417af6490e
audited_head: f2fa757dbfa632e65b3d264d166760cef738261f
verdict: APPROVE
-->

# R-229 — `TD-241`, часть engine-dev: атрибуция тишины выдачи `OPS-I-8` → `OPS-I-11` в `watchdog.rs`

**Роль:** reviewer (PR-гейт). **Дата:** 2026-10-04.
**Предмет — ВЕТКА** `origin/fix/TD-241-watchdog-comments`; вершина взята командой
(`docs/04-workflow.md` §2 «Предмет гейта — ВЕТКА»):

```
$ git fetch origin && git rev-parse origin/fix/TD-241-watchdog-comments origin/main
f2fa757dbfa632e65b3d264d166760cef738261f
32574deddfd273a925a9ae9ca146a4417af6490e
```

SHA из мандата (`f2fa757`) совпал с вершиной. База = `origin/main` = `32574de`
(`git merge-base` → `32574de…`), ветка свежая, отставания нет.

## Вердикт: **APPROVE**

Диф — ровно три строки `///` doc-комментариев в `crates/ops/src/watchdog.rs`; ноль строк
исполняемого кода, ноль тестов. Правка приводит код к FA и закрывает engine-dev-часть `TD-241`.
Sacred-часть `TD-241` (шапки оракулов) в предмет не входит и остаётся открытой за architect'ом.

## Block-FA — живой инвариант тронутого модуля (`crates/ops` → `docs/fa/ops.md`)

Открыт `docs/fa/ops.md` §6 на проверяемой ревизии (`git show origin/fix/TD-241-watchdog-comments:docs/fa/ops.md`):

- **`OPS-I-11`** (строка 479) — «Тишина ВЫДАЧИ — алерт … **Потребитель** — `ops-watchdog`, правило
  на ДЕЛЬТАХ между тактами (`check_serving_silence`, `crates/ops/src/watchdog.rs`) … Отсутствие или
  устаревание самого сердцебиения — отдельный алерт (`check_serving_heartbeat_missing` /
  `ServingHeartbeatStale`, пороги `serving_heartbeat_warn_ms` / `serving_heartbeat_crit_ms`)».
  Все три исправленные строки — ровно эти символы: `Incident::ServingSilence` (`:54`),
  `Thresholds::serving_heartbeat_*` (`:214`), `check_serving_silence` (`:624`). Атрибуция верна.
- **`OPS-I-8`** (строка 476) — «Предмет — поток РЫНОЧНЫХ ДАННЫХ; тишина ВЫДАЧИ клиентам —
  `OPS-I-11`, а не этот инвариант». Прежняя атрибуция FA прямо противоречила.

Оставшиеся `OPS-I-8` в `src/` проверены поштучно — все про MD-поток, корректны и не тронуты:

```
$ grep -rn 'OPS-I-8' crates/*/src/
crates/ops/src/alerts.rs:126:  summary: "md_event_age_ms > 5min — поток молчит, процесс жив (OPS-I-8, OPS-SILENCE)",
crates/ops/src/alerts.rs:228:  // Gauge с label `venue`: > 5 минут = поток молчит (OPS-I-8; §7 P1 = `> 5min`).
crates/ops/src/silence.rs:1:   //! OPS-I-8 — тишина в потоке: «жив, но не работает» (класс TD-011/TD-014).
crates/recorder/src/metric_emit.rs:85: /// **OPS-I-8 silence:** при тишине потока значение РАСТЁТ …
```

Ни одной ссылки «`OPS-I-8` на выдачу» в `crates/*/src/**` после правки не осталось — engine-dev-часть
карточки закрыта полностью, а не на названных строках (проверен КЛАСС, не место).

## Block-scope

| проверка | результат |
|---|---|
| тронутые пути | `crates/ops/src/watchdog.rs` — один файл |
| зона исполнителя | `crates/ops/src/**` — engine-dev (`scope-guard.md`, таблица владения) ✅ |
| `*/tests/**`, `scripts/**`, `docs/**`, `milestones/**`, `Cargo.toml` | не тронуты ✅ |
| `crates/contracts/**` (Block-C) | не тронут — RFC не требуется ✅ |
| `risk`/`killswitch`/`oms`/`venue-*` (Block-risk) | не тронуты — risk-critic не требуется ✅ |
| процессный слой (`gates.md` §11) | не тронут — `FOUNDER-APPROVED` не требуется ✅ |
| `.claude/rules/**` и др. зона §9 | не тронута — перепроверка Fable не требуется ✅ |

```
$ git diff --stat origin/main...origin/fix/TD-241-watchdog-comments
 crates/ops/src/watchdog.rs | 6 +++---
 1 file changed, 3 insertions(+), 3 deletions(-)
```

Диф целиком просмотрен построчно: каждая из трёх пар −/+ отличается только подстрокой
`OPS-I-8` → `OPS-I-11`.

## Block-RED / атомарность

- Тесты не тронуты (`git diff origin/main...HEAD -- '*/tests/*'` пусто) — RED-first не задет.
- Один коммит `f2fa757 fix(TD-241): … [engine-dev]` — conventional, ссылка на задачу, метка роли,
  без `Co-Authored-By` ✅.
- Новых механизмов несущего пути нет — DoD «механизм на пути» не применим.

## Block-DoneBlock — перепрогон на вершине ветки (свой worktree, `/tmp/hft-reviewer-td241`)

```
$ git log -1 --format='%H %s'
f2fa757dbfa632e65b3d264d166760cef738261f fix(TD-241): атрибуция правила тишины выдачи — OPS-I-11 [engine-dev]

$ cargo fmt --all -- --check
fmt-exit=0

$ cargo clippy -p ops --all-targets --all-features -- -D warnings 2>&1 | tail -1
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.98s
clippy-exit=0

$ cargo test -p ops 2>&1 | grep -E '^test result' | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" (блоков: "NR")"}'
passed=172 failed=0 (блоков: 19)
test-exit=0
```

Узкий скоуп `-p ops` достаточен: правка комментарная, символы и сигнатуры не менялись, соседи
сломаться не могут. Полную тройку CI исполнит PR-прогон (`All checks passed`).

Барьер `review-fa` ДО этого вердикта — подтверждаю заявление dev'а исполнением:

```
$ EVENT_NAME=pull_request PR_BASE_SHA=32574de… bash scripts/check_review_fa.sh
FAIL  ни один review-файл не введён диапазоном и не назван полным путём в %B (S=∅) — механизм D
exit=1
```

Результат ПОСЛЕ коммита этого вердикта — в §Пост-проверка ниже.

## Находки (не блокируют)

- **N-1. Псевдо-waiver в теле коммита.** `f2fa757` несёт строку `FA-WAIVER: n/a — …`. Барьер waiver
  в `%B` не читает (`check_review_fa.sh` ищет `^FA-WAIVER: crates/<name> — …` в review-ФАЙЛЕ;
  `.claude/agents/reviewer.md`, поправка `TD-165`), а форма `n/a` норме не соответствует вовсе.
  Строка инертна и вреда не несёт; waiver здесь и не нужен — FA у `ops` есть, живой инвариант
  назван выше. Для dev'а: эта строка в коммите не нужна.
- **N-2. Остаток `TD-241` — sacred, за architect'ом.** Четыре места в двух оракулах:
  `crates/ops/tests/red_m89_serving_silence.rs:2`, `crates/gateway-serve/tests/red_m91_legacy_counters.rs:12,242,341`.
  Карточка `TD-241` остаётся **OPEN** с сужённым охватом (обновлена в `TECH-DEBT.md` этим же PR).

## Что искал грепом в ярусе C (`reading-map.md` §2)

`TECH-DEBT.md` — `TD-241` (строка таблицы `:217` и карточка `:5661`), `TD-237`;
`PROJECT-STATE.md` — `TD-241` (`:3074`, упоминание при заведении). Новой реализованной
возможности предмет не несёт — запись в `PROJECT-STATE.md` не требуется.

## Пост-проверка

(заполняется после коммита вердикта — см. коммит-сообщение и PR)
