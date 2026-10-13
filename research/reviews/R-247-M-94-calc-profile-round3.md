<!-- GATE-META
milestone: M-94
audited_repo: a3ka/hft-platform
audited_base: 045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0
audited_head: e05566f42006e8a7ef51accd5f037912024b8d83
verdict: APPROVE
-->

# R-247 — M-94 calc-profile, PR-гейт круг 3 (после `R-246`, `A-052`, `C-289`) — APPROVE

**Предмет:** ветка `origin/feat/M-94-calc-profile`. Вершина взята командой
(`git fetch origin && git rev-parse origin/feat/M-94-calc-profile`) =
`e05566f42006e8a7ef51accd5f037912024b8d83`, совпала со справкой мандата architect'а. Повторный
`fetch` перед записью вердикта — та же вершина. База — `045fef9` (merge-base с `origin/main`);
`main` = `6ee3f1a`, слияние чистое (локальный `141a336`, не публикуется). Работа — в detached
worktree `/tmp/hft-reviewer-m94` (ветка) и `/tmp/hft-reviewer-m94-merge` (дерево слияния).
Новое с `R-246`: `A-052` (DECISION), `0f30ec6`/`611b8fe`/`9537edb` (architect: оракул согласия
двух бинарей, режим «ослабитель», текстовый запрет копий, спека), `C-289` (NOTE, суженный круг),
`2b395b7` (engine-dev, задача 12), `e05566f` (журнал §16, круг 11).

**Живые инварианты тронутых модулей** (диф трогает `crates/gateway/src/**`,
`crates/gateway-serve/src/**`; `crates/ops/**` — только `tests/`):
- `VB-I-2` (`docs/fa/viz-backend.md:285`, live == replay) — предмет круга: правило величины,
  жившее в прогревателе отдельной копией, было вторым определением расчёта. Снято (см. §1).
- `VB-I-9` (`docs/fa/viz-backend.md:292`, граница плоскостей) — держится:
  `grep -rnE 'postgres|sqlx|diesel' crates/gateway-serve/src crates/gateway/src | wc -l` → `0`.
- `OPS-I-11` (`docs/fa/ops.md:479`, тишина выдачи) — сердцебиение выдачи пополнено полем
  `calc_profile`, разбор сторожем совместим (`o1` в составе `cargo test --all`, зелёный); снимается
  на проде задачей 11 (§8 ниже).
`crates/gateway` / `crates/gateway-serve` собственной FA не имеют (`reading-map.md` §2); барьер
`check_review_fa.sh` числит их за `docs/fa/viz-backend.md` (`:190-198`).

**Греп яруса C** (`origin/main`): `TECH-DEBT.md` — по `M-94`, `calc-profile`, `calc_profile`,
`deploy_catchup`, `compose_deploy_form` (ноль попаданий; кандидаты `R-245`/`R-246`/`A-052` не
заведены — это close-out); `scripts/next_artifact_id.sh TD` → `TD-244`. `PROJECT-STATE.md` — по
`M-94` (ноль), `MS-STATE: M-9` (`M-90…M-93` CLOSED).

## Вердикт

**APPROVE.** `R-246` B-1 закрыт исправлением кода С оракулом на находку — ровно условие
APPROVED `R-246`. Свою мутацию круга 2 я повторил: теперь она роняет оракул согласия двух
бинарей. Риск-блок не нужен, Block-C не тронут, scope dev-коммита — только удаления в одном файле
зоны §10. Мержу; задача 11 (`.env` прода) — в окне после merge.

## 1. `R-246` B-1 — закрыт, проверено исполнением

| что проверено | команда / наблюдение | итог |
|---|---|---|
| копий в прогревателе нет | `grep -nE '86_400_000 %\|< 1000\|is_nan' crates/gateway/src/bin/gateway-checkpoint.rs` → 0 строк, exit=1; шаг `task12` гейта — PASS | закрыт |
| удалено ровно то, что велел `A-052` §3 (а) | `git show 2b395b7`: `:433-461` (профиль: таймфрейм, каденция, NaN + комментарии «6.»/«7.»), `:587-596` (legacy: таймфрейм + `let timeframe_ms`), `:631-635` (legacy: NaN). Вызовы `calc_profile::parse_*` в обоих режимах на месте | соответствует |
| `validate_selector` не тронут | тело функции `045fef9` ≡ `e05566f` (`diff` по `awk '/^pub fn validate_selector/,/^}/'`, 125 строк) → идентично; `git log 045fef9..e05566f -- crates/gateway/src/lib.rs` → только `dcc15ef` (`pub mod`) | соблюдено |
| **моя мутация `R-246` M3** — общий `parse_timeframe_ms` без «делит сутки» (`calc_profile.rs:339` → `if false {`) | `bash scripts/tests/red_m94_two_binary_agreement.sh` → `расхождений: 2` (legacy и profile, `GATEWAY_TIMEFRAME_MS=604800000`: выдача ACCEPT, прогреватель REFUSE), `VERDICT: FAIL`, exit=1 | **ловит** (в `R-246` — ни один оракул) |
| та же мутация, крейты целиком | `cargo test -p gateway -p gateway-serve --no-fail-fast` → 3 FAILED (`red_timeframe_guard_startup` ×2, `red_window_guard_startup` ×1), exit=101 | как в `R-246` + теперь согласие |
| без мутации | откат `git checkout --`, `git status --porcelain` пуст; согласие `сравнений: 70, расхождений: 0`, exit=0 | зелёный |
| «ослабитель» (RED на находку) | `red_m94_shared_grammar_probe.sh` → 6/6 PASS, включая «ослабитель NaN: приём общей parse_bands дошёл до ОБОИХ бинарей» и негативный контроль таймфрейма, exit=0 | зелёный после удаления (у `C-289` на `9537edb` был красным) |

Заметка о смысле: после удаления прогреватель по-прежнему отвергает `604800000` — через
`validate_selector` (`A-052` §2.3). Расхождение с выдачей теперь не прячется: его видит оракул
согласия, а `validate_selector` спекой §3.2 назван инвариантом библиотеки, не грамматикой. Мой
довод `R-246` «изменение общего правила не доходит до прогревателя» в части NaN снят удалением,
в части таймфрейма/каденции — наблюдаемостью расхождения. Это то, что решил `A-052`.

## 2. Scope и дисциплина

- **`2b395b7`** — `git show --numstat`: `0 43 crates/gateway/src/bin/gateway-checkpoint.rs`;
  добавленных строк ноль. Файл в §10 (engine-dev). Subject несёт задачу и находку.
- **Sacred:** ни один коммит `[engine-dev]` в `045fef9..e05566f` не трогает `*/tests/**`,
  `scripts/verify_*`, `scripts/check_*`, `scripts/lib/**`, `config/calc-profile/**` (перебор
  `git show --name-only` по всем dev-коммитам — пусто).
- **Правки оракулов круга** (architect, `003a26d..e05566f`) — только дополнения:
  `red_m94_single_grammar.rs` +37/−0, `red_m94_single_grammar_warmer.rs` +35/−0, проба +49/−1,
  новый `red_m94_two_binary_agreement.sh` и корпус, `verify_M-94.sh` +10/−0. Ни одна прежняя
  проверка не ослаблена.
- **Risk-блок:** `risk`/`killswitch`/`oms`/`venue-*` не тронуты — risk-critic не требуется (§13 спеки).
- **Block-C:** `crates/contracts/**` не тронут.
- **Атомарность:** задача 12 — `6c9e0fc` + `2b395b7`, задача 13 — `df2fe01`; бандлов нет.
- Остальной диф ветки судился `R-245` (scope п.1) и `R-246`; с тех пор изменён только наборами
  architect'а под plan-time гейтом (`A-052` → `C-289`).

## 3. Не блокирующие

- **N-1 — `R-245` N-5 / `R-246` N-1 по-прежнему не сделан.** `deploy/bin/calc-profile-gate.sh:78`
  — `compose run … --print-ckpt-name 2>/dev/null`: при отказе гейта оператор не видит причины.
  Заводится в долг при close-out.
- **N-2 — нумерация комментариев в `run_profile_mode`:** после удаления блоков «6.»/«7.»
  комментарии идут «5.» → «8.» (`gateway-checkpoint.rs:431`, `:436`). Косметика, зона engine-dev.
- **N-3 — колонка Status §11 спеки** у задач 1–6, 12, 13 по-прежнему `⏳ OPEN` при сданных и
  принятых задачах. Зона architect'а (переезд спеки — его шаг close-out'а).
- **N-4 — кандидаты прошлых кругов** заводятся в `TECH-DEBT.md` при close-out (мандат п.5):
  `R-245` N-2+N-3 (= K-1: W7 судит текст; исполняемые пробы деплоя вне CI), K-2 (холостая
  `GATEWAY_BANDS` в `scripts/lib/compose_deploy_form.sh:7,28`), разные таблицы отсутствия у двух
  бинарей без профиля (спека §3.2 (а) — названо, пиннится, не выравнивается), `A-052` N-1
  (полосу `≥ 1` принимают на старте оба бинаря, но подписаться на неё нельзя —
  `session.rs:80`).

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-94-calc-profile origin/main
e05566f42006e8a7ef51accd5f037912024b8d83
6ee3f1af60fd1a482842d267d84fc625505c2db4
$ git merge-base --is-ancestor e05566f origin/feat/M-94-calc-profile && echo ancestor_ok
ancestor_ok

# дерево слияния: detached origin/main + merge --no-ff origin/feat/M-94-calc-profile → 141a336 (6ee3f1a e05566f)
$ cargo fmt --all -- --check; echo exit=$?
fmt exit=0
$ cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -1; echo exit=$?
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.20s
clippy exit=0
$ cargo test --all 2>&1 | grep -E '^test result' | awk '{p+=$4; f+=$6; i+=$8} END {...}'
passed=1233 failed=0 ignored=5 (блоков: 276)
test exit=0
$ TMPDIR=/tmp bash scripts/verify_M-94.sh; echo exit=$?
PASS ×73
SKIP ×7 — 6 ci-parity по карте (cargo-audit install, pip jsonschema, «база события», refs/salvage, pyyaml, агрегат) + task11 (§8, после merge)
VERDICT: PASS
exit=0
$ git status --porcelain      # после verify
(пусто)
$ bash scripts/verify_design_claims.sh --merge-preview origin/main; echo exit=$?
VERDICT: PASS (0 нарушений)
exit=0

# ветка (/tmp/hft-reviewer-m94), e05566f
$ sed -i '339s/if 86_400_000 % v != 0 {/if false {/' crates/gateway/src/calc_profile.rs   # мутация R-246 M3
$ bash scripts/tests/red_m94_two_binary_agreement.sh; echo exit=$?
сравнений: 70, расхождений: 2
FAIL  согласие двух бинарей (A-052 §3 (б)): правило величины судит по-разному выдача и прогреватель
  legacy   GATEWAY_TIMEFRAME_MS=604800000: выдача=ACCEPT прогреватель=REFUSE
  profile  GATEWAY_TIMEFRAME_MS=604800000: выдача=ACCEPT прогреватель=REFUSE
VERDICT: FAIL
exit=1
$ cargo test -p gateway -p gateway-serve --no-fail-fast
test misaligned_timeframe_env_blocks_startup ... FAILED
test weekly_timeframe_env_blocks_startup ... FAILED
test timeframe_guard_untouched_by_window_guard ... FAILED
passed=503 failed=3   exit=101
$ git checkout -- crates/gateway/src/calc_profile.rs; git status --porcelain
(пусто)
$ bash scripts/tests/red_m94_two_binary_agreement.sh; echo exit=$?
сравнений: 70, расхождений: 0
PASS  согласие двух бинарей: выдача и прогреватель одинаково принимают/отвергают каждый случай корпуса в обоих режимах
VERDICT: PASS
exit=0
$ bash scripts/tests/red_m94_shared_grammar_probe.sh | grep -E '^(PASS|FAIL|VERDICT|сценариев)'
PASS  setup: сторож вживлён в 6 из 6 общих функций
PASS  g4: выдача — сторожевой отказ общей функции доходит до результата без профиля и в профиле
PASS  w4: прогреватель — сторожевой отказ доходит в окружении, флагах и профиле
PASS  setup: ослабитель вживлён (parse_bands → NaN, parse_timeframe_ms → 604800000)
PASS  ослабитель NaN: приём общей parse_bands дошёл до ОБОИХ бинарей — своей проверки NaN поверх общей функции нет
PASS  негативный контроль: ослабленный таймфрейм расходится (держит инвариант validate_selector) — ось согласия чувствительна
сценариев: 6, провалов: 0
VERDICT: PASS
exit=0
$ git show --numstat --format='' 2b395b7
0	43	crates/gateway/src/bin/gateway-checkpoint.rs

# VB-I-9
$ grep -rnE 'postgres|sqlx|diesel' crates/gateway-serve/src crates/gateway/src | wc -l
0

# прод ДО merge (задача 11 ещё не исполнялась)
$ ssh … 'cd /root/hft-platform && git log -1 --format="%h %s"; grep -c "^GATEWAY_BANDS=" .env; docker ps --format "{{.Names}} {{.Status}}"'
c789e573 Merge pull request #326 from a3ka/fix/compose-workdir-comment
1
hft-gateway-serve Up 2 days (healthy)
hft-recorder Up 2 days (healthy)
```

## Условие APPROVED — выполнено

`R-246` B-1 снят исправлением кода с оракулом на находку (`red_m94_two_binary_agreement.sh`,
режим «ослабитель»), tester на `2b395b7` — PASS, мутационный контроль исполнен architect'ом и
повторён мной. Дальше: PR → зелёные чеки → merge → задача 11 в окне до конца Deploy → §8.

## Handoff §D

Следующий шаг — мой (merge через PR и §8-гейт); затем architect (переезд спеки и гейта по `Р-2`,
строка `TD-229` в `ROADMAP`, M-95).
