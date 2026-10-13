<!-- GATE-META
milestone: M-100
audited_repo: a3ka/hft-platform
audited_base: 28cae3136102a55e6841ea372f7a84cff95f607b
audited_head: 17258aa6e538fcc574eb181b10250dcb24396d4f
verdict: APPROVE
-->

# R-265 — M-100 «три контракта выдачи» (S1a-K), PR-гейт круг 2: **APPROVE**

PR #351, ветка `origin/feat/M-100-three-serving-contracts`. Вершина взята командой
(`git fetch origin && git rev-parse …` → `17258aa6…`), совпала со справкой тестера; повторный
`fetch` перед записью вердикта — та же `17258aa6`. По условию `R-264` судится ТОЛЬКО диф после
`28cae313` (база аудита) плюс повтор гейта; принятое в `R-264` не переоткрывается.

Диф круга: `d0a04f91` (RED, architect) · `401ddbf6` (§14 круг 6, architect) · `7b9d1750`
(task #5, Б-1) · `4da264c4` (Н-1) · `17258aa6` (Н-3) — engine-dev. Плюс сам `R-264` (`553626f2`).

## Предъявление FA

`crates/gateway/**`, `crates/gateway-serve/**` ⇒ `docs/fa/viz-backend.md` (живые на вершине,
`grep -nE '^\| VB-I-(2|4|12) '` → строки 285/287/295):
- **`VB-I-12`** — допуск теперь не пропускает подписку, которую слепок не обслужит: тройка без окна
  получает `unsupported`, а не `not_ready` (тот же класс «объявлено ≠ обслужено», что `VB-I-12`
  закрывает со стороны кадра);
- **`VB-I-4`** — поля `Selector`/`Snapshot` не тронуты; меняется только текст отказа (Н-1);
- **`VB-I-2`** — диф круга его не касается; гейт `h1`/`h2` зелёный.
Плюс инвариант поставки `I-2` (`П-030`) спеки §5 — ради него и круг.

Грепом искал: `TECH-DEBT.md` — `TD-197`, `TD-242`, `M-100`; `PROJECT-STATE.md` — `M-100`,
`MS-STATE: M-9`.

## Блоки гейта

| блок | итог |
|---|---|
| Scope | PASS — dev тронул только `crates/gateway-serve/src/{admission,lib}.rs`, `crates/gateway/src/lib.rs` (§6); тесты и спека — коммиты architect'а в его зоне |
| Contract Block-C | PASS — T1 не тронут; `CT-RFC-09` §2.10.1 не менялся (решение architect'а — вариант (а) `R-264`: RFC в силе) |
| RISK-BLOCK | не применяется — нет order-пути |
| RED-first | PASS — RED `d0a04f91` (15:01:33) раньше фикса `7b9d1750` (15:13:52); dev-коммиты `*/tests/**` не трогают (`git show --numstat`) |
| Атомарность | PASS — Н-2 `R-264` исполнен: по коммиту на находку, subject называет задачу |
| Done Block / acceptance | PASS — сырой вывод ниже |
| Механизм на пути | PASS — `admit` зовётся из транспорта (`lib.rs:1231`) под `bind_with_policy` |

## Б-1 `R-264` — ЗАКРЫТ

`profile_allowed` (`crates/gateway-serve/src/admission.rs`): `if sel.window_ms != Some(p.window_ms)
{ continue; }` — окно сверяется всегда, `None` не совпадает ни с одним `LiveProfile.window_ms: i64`.
Оракул M-87 (`sel_with`) несёт окно профиля; RED на находку — юнит
`form_profile_triple_without_window_is_unsupported` (с setup-стражем «с окном — допуск») и провод
`w5b_triple_without_window_is_unsupported` (страж: каденция профиля есть, окна нет).

Прод-клиентов, шлющих v1-подписку без окна, нет: `wsprobe` идёт legacy-путём (`bind` без
политики, спека §2: «прод-потребителя v1 сегодня нет»). Поведенческий риск для действующих
потребителей — нулевой.

**Мутация reviewer'а (иная, чем у тестера: сверка окна удалена целиком):**
```
red_m100_three_contracts_wire: FAILED. 8 passed; 2 failed
    w5_window_outside_allowed_profiles_is_unsupported
    w5b_triple_without_window_is_unsupported
red_m87_admission:             FAILED. 18 passed; 1 failed
    form_profile_triple_without_window_is_unsupported
откат (git checkout), git status --porcelain → пусто
red_m100_three_contracts_wire: ok. 10 passed; 0 failed
red_m87_admission:             ok. 19 passed; 0 failed
```
Тестер (круг 6) — мутация «вернуть `if let Some(w)`» роняет оба новых оракула, `w5` при этом
зелёный: значит новая пара судит именно ось `None`, которую `w5` не покрывал.

## Н-1, Н-3 `R-264` — ЗАКРЫТЫ

- Н-1: текст отказа — «(bands/symbol/timeframe_ms/window_ms/depth_cadence_ms)» = оси `admit()`.
- Н-3: комментарий писателя слепка приведён к факту (`u2_*` зелёные со стражем на `CALC_STATE_VERSION`).

## Замечания (не блокеры)

- **N-1 — текст отказа по-прежнему без оракула.** Расхождение текста с осями `admit()` случилось
  второй раз (`R-235` Н-2 → `R-264` Н-1); оба раза ловил человек. Третье повторение по
  `binding-requires-mechanism` запрещает лечить прозой — решать architect'у, нужна ли проверка
  строки на проводе. Сейчас — названо, не заведено.
- **N-2 — косметика:** `crates/gateway/src/lib.rs` ~4868 в `//`-комментарии стоят
  экранированные кавычки `(\"гейтить …\")` — артефакт генерации, на поведение не влияет.
- FA: строка `VB-I-12` («гейтить на `schema_version == 11`»), `VB-I-13` и `OPS-I-11`
  (`docs/fa/ops.md`, «три проверки `admit()`») — задача 7, зона architect'а.

## Дерево слияния

Ветка отстаёт от `main` на 6 коммитов (`f43c1191`); пересечение с предметом — только
`crates/gateway-serve/tests/red_ws_session.rs` (TD-257). Проверено на локальном merge
(`dffac598`, родители `17258aa6 f43c1191`, затем `reset --hard` назад):
```
$ git merge-tree --write-tree origin/main 17258aa6 → rc=0 (конфликтов нет)
$ cargo test -p gateway-serve --features testing --test red_ws_session --test red_m87_admission --test red_m100_three_contracts_wire
test result: ok. 10 passed; 0 failed
test result: ok. 19 passed; 0 failed
test result: ok. 13 passed; 0 failed
$ bash scripts/verify_design_claims.sh --merge-preview origin/main
VERDICT: PASS (0 нарушений)   exit=0
```

## Done Block

```
$ git fetch origin && git rev-parse origin/feat/M-100-three-serving-contracts
17258aa6e538fcc574eb181b10250dcb24396d4f
$ git worktree add --detach /tmp/hft-reviewer-M-100-r2 origin/feat/M-100-three-serving-contracts
HEAD is now at 17258aa6

$ bash scripts/verify_M-100.sh 2>&1 | grep -E '^(PASS|FAIL|SKIP|VERDICT)' (ci-parity сокращены); echo exit
PASS  task1: red_m100_heatmap_frame_close (h1, h2)
PASS  task2: red_m100_view_projection (p1…p4)
PASS  task2/4/5: red_m100_three_contracts_wire (w1…w9)
PASS  task3: red_m100_calc_key_id (k1…k5)
PASS  task4: red_m100_state_version (sv1…sv3)
PASS  I-reg: gateway red_m88_update_contract
PASS  I-reg: gateway red_m88_golden_vectors
PASS  I-reg: gateway-serve red_m87_admission
PASS  I-reg: gateway-serve red_m95_provenance_fresh (f0…f6)
PASS  ci-map: проба карты CI-паритета
PASS  ci-parity: cargo fmt --all -- --check
PASS  ci-parity: cargo clippy --all-targets --all-features -- -D warnings
PASS  ci-parity: cargo test --all
…  (PASS-строк ci-parity: 59; FAIL-строк в логе: 0)
PASS  ci-parity: учтено шагов 64 из 64 (исполнено 58, исключено по карте 6)
SKIP  task6: §8 — снимок wsprobe v12 …; снимает reviewer
VERDICT: PASS
exit=0

$ gh run view 38437258606 --json headSha,event,conclusion
{"conclusion":"success","event":"pull_request","headSha":"17258aa6e538fcc574eb181b10250dcb24396d4f"}
$ gh run view 38437252961 --json headSha,event,conclusion
{"conclusion":"success","event":"push","headSha":"17258aa6e538fcc574eb181b10250dcb24396d4f"}
$ gh pr checks 351 → 22 джоба pass, включая «All checks passed» и «Review FA»
```

## Задача 6 (§8) — базовая линия прода ДО деплоя

```
прод-чекаут dc03af35; hft-gateway-serve Up 20 hours (healthy), CPU=0.00% до прогона
ckpt-8f69809dd707e8c9.bin  od -t u4 -N16 → 1129596488 825249867 3 11   (байты 12..16 = 11)
sha256 cd608ff4b69f848c056a7d963d20926d881e89b75fa9d773e08f6cb80cf9e2fd, mtime 19:30:04
wsprobe --frames 2 → schema_version 11, latency_first_snapshot_ms=1003, frames_received=2,
                     snapshot 483227 Б, полей calc_key_id/calc_profile/heatmap_window_e8 НЕТ
```
Замер «после» — в close-out этого же вердикта (продолжение ниже, после merge).

## Условие APPROVE — выполнено

Б-1 закрыт с RED на находку; Н-1/Н-3 той же правкой; атомарно. Merge — через PR #351.

## Handoff §D — кому

Merge и §8 — reviewer (я). Затем **`architect`** — задача 7 (`VB-I-13`, строка `VB-I-12`,
`OPS-I-11`), N-1, переезд спеки/гейта M-100 в `docs/archive/`.
