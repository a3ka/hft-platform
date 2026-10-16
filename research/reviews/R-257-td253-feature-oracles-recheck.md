<!-- GATE-META
milestone: TD-253
audited_repo: a3ka/hft-platform
audited_base: 24ee5c271213a4bcdee2e160b608e3eceb566194
audited_head: 3bff5e3aa6fa5ab2c83c3375f0aa27a82ae5edd1
verdict: REJECT
-->

# R-257 — перепроверка `gates.md` §9 (харнесс-трек, адверсарий): `TD-253` — оракулы под `--features testing` в CI

**Вердикт: REJECT** — одна блокирующая находка (Б-1), семь неблокирующих (Н-1…Н-7).

Предмет — ветка `harness/td253-feature-oracles`. Вершина взята командой
`git fetch -q origin && git rev-parse origin/harness/td253-feature-oracles` →
`3bff5e3aa6fa5ab2c83c3375f0aa27a82ae5edd1`; база — `origin/main` =
`24ee5c271213a4bcdee2e160b608e3eceb566194`; диапазон — два коммита автора (`24976e79`, `3bff5e3a`).
Роль — architect-клон со свежим контекстом, НЕ автор правки; дерево — собственное detached
`/tmp/hft-arch-recheck-td253`. Код, CI, барьер и проба не правились — только этот файл.

**FA тронутого модуля.** Диф трогает `crates/gateway-serve/tests/red_m95_provenance_fresh.rs`
(только `//!`-комментарий). FA крейта — `docs/fa/viz-backend.md` (карта
`scripts/check_review_fa.sh:196`); живой инвариант, который сторожат f0…f6 и ради которого
всё заведено, — **`VB-I-11`** (`docs/fa/viz-backend.md:294`, провенанс истории
`history_start_seq`/`history_truncated`). `check_review_fa.sh` на диапазоне даёт `SKIP`
(только не-прод пути) — требование здесь когнитивное, и оно выполнено названием ID.

## Инвариант, который правка обязана держать (формулировка автора)

> ЛЮБОЙ тест в `crates/*/tests/` под `cfg(feature = "<f>")` исполняется в CI-джобе, входящем в
> агрегат `All checks passed`, и его провал краснит этот джоб; новый файл под фичей покрывается
> без правки CI.

Что держит, что нет — ниже по осям мандата.

## Что ПОДТВЕРЖДЕНО (ось б — мутационный контроль, повторён лично)

Оба мутанта воспроизведены на `3bff5e3a`, код возвращён (`git status --porcelain` пуст — Done Block).

| мутант | строка | `cargo test -p gateway-serve` (прежний CI) | `… --features testing` (новый шаг) |
|---|---|---|---|
| MV2 — v1: `refresh → Err` ⇒ `Some(провенанс по каталогу)` | `crates/gateway-serve/src/lib.rs:1611` | passed=211 failed=0 **rc=0** — слеп | `f5_v1_refresh_failure_after_stale_is_fail_closed` FAILED (`:723`), **rc=101** |
| ML2 — legacy: та же подмена | `crates/gateway-serve/src/lib.rs:2451` | passed=211 failed=0 **rc=0** — слеп | `f6_legacy_refresh_failure_after_stale_is_fail_closed` FAILED (`:761`), **rc=101** |

Честное дерево под фичей: passed=221 failed=0 rc=0 (50 блоков); `--test red_m95_provenance_fresh`
— 7 passed (f0…f6 все зелёные). Утверждение автора «MV2: 211/0 без фичи, f5 FAILED с фичей»
воспроизведено точно. Шаг CI действительно превращает слепоту прежнего CI в красный джоб —
по существу предмет `TD-253` закрыт.

**Проводка в агрегат.** `ci.yml:29-30` — шаг `run: cargo test -p gateway-serve --features testing`
внутри `build-test`; `ci.yml:668` — `build-test` в `needs` агрегата; `ci.yml:672` — его
`result != success` роняет условие. `bash scripts/tests/red_ci_aggregate.sh` → `VERDICT: PASS —
сценариев: 8, расхождений: 0`, rc=0. Шаг стоит ПОСЛЕ `cargo test --all` без `continue-on-error`:
при красном предыдущем шаге он не исполняется, но джоб уже красен — инвариант не нарушен.

## Находки

### Б-1 (БЛОКИРУЕТ) — барьер слеп к `if:` на шаге: шаг «есть», но не исполняется, барьер PASS

`scripts/check_feature_oracles.sh:17-18` формулирует: «Шаг не смеет глушить код возврата
(`|| true`, `continue-on-error`)». Третья — и самая идиоматичная в GitHub Actions — форма
глушения, **условие `if:` на шаге**, не проверяется и в «пределе, названном честно»
(`:20-23`) не названа. Воспроизведение на изолированной фикстуре (крейт `gs`, inner-атрибут,
шаг в `build-test`):

```
$ FO_ROOT=<fixture> FO_CI=.github/workflows/ci.yml bash scripts/check_feature_oracles.sh
# фикстура e1:  - name: fo / if: false / run: cargo test -p gs --features testing
PASS  gs: оракулы под feature "testing" (1 файл(ов)) исполняет build-test — «cargo test -p gs --features testing»
VERDICT: PASS            exit=0      ← шаг НИКОГДА не исполняется, барьер зелёный
# фикстура e2:  if: github.event_name == 'push'
VERDICT: PASS            exit=0      ← на pull_request (единственный производитель зелёного
                                        чека для merge'а, gates.md §8) шаг пропущен
```

Почему это блокирует, а не «предел»: (1) инвариант автора — «провал краснит этот джоб» —
нарушается состоянием, на котором барьер PASS; (2) класс НЕ экзотический для этого корпуса —
соседний оракул агрегата уже считает его глушением наравне с `continue-on-error`:
`scripts/tests/red_ci_aggregate.sh:57-58` — `if step.get("continue-on-error") or
str(step.get("if","")).strip() in ("false", "${{ false }}"): out.append("шаг-условие обезврежен
(continue-on-error / if: false)")`; (3) проба `red_feature_oracles.sh` сценарии s4/s5/s7 показывают,
что автор этот класс (шаг формально есть, но не красит) СУДИЛ — и пропустил одну из его форм.
Дешёвая развязка в духе остальных проверок барьера (без YAML-парсера): любой ключ
`^[[:space:]]*if:` в теле `build-test` → FAIL (сегодня в джобе ни одного `if:` — `grep -n '^\s*if:'
.github/workflows/ci.yml` даёт только `:669` у агрегата); либо шаговая привязка — `if:` в блоке
того же `- name:`, что и искомый `run:`. Фикс обязан прийти с (а) сценарием пробы
«`if:` на шаге → FAIL» и (б) мутантом «проверка `if:` → `false`», роняющим ровно его
(`testing.md` «Исправление по вердикту тоже требует оракула»; `harness-track.md` §5 п.2).

### Н-1 (MINOR) — форма `cfg(all(feature = …))` / `cfg(any(…, feature = …))` невидима, предел не назван

Регексп поиска (`check_feature_oracles.sh:51`) принимает только `#[cfg(feature` / `#![cfg(feature`.
Шапка (`:21-23`) называет невидимыми макрос и `cfg_attr` и утверждает «форма в дереве одна».
На живом дереве это так (`grep -rnE 'cfg\((any|all)\(.*feature' crates/*/tests/` — только три
строки в комментариях). Но ВТОРОЙ крейт, у которого единственная форма — `all(...)`, проходит
молча:

```
# фикстура e13: gs — inner-атрибут + шаг; jr — только `#![cfg(all(feature = "testing", unix))]`, шага нет
PASS  gs: … исполняет build-test …
VERDICT: PASS            exit=0      ← jr под фичей, CI его не исполняет, барьер молчит
```

Изолированно (e3, один крейт в форме `all`) барьер падает по «ноль найденных» — но на живом дереве
`FOUND>0` всегда (gateway-serve), и этот страж не сработает. Либо расширить регексп до
`cfg\((all|any)\(.*feature`, либо назвать `all/any` в пределе рядом с `cfg_attr`. Не блокирует:
нарушитель должен появиться в будущем и в нестандартной форме.

### Н-2 (MINOR, тот же класс, что `R-255` Н-1) — в только что отредактированной шапке теста осталось протухшее «Сегодня — красен»

`crates/gateway-serve/tests/red_m95_provenance_fresh.rs:36-37`: «Сегодня — красен: точки
`m95-catalog:*` нет, остановка не наступает». На дереве слияния файл зелёный целиком:
`cargo test -p gateway-serve --features testing --test red_m95_provenance_fresh` → `7 passed; 0
failed`, rc=0 (Done Block). Коммит `3bff5e3a` правил строки `:31-34` этой же шапки по `R-255`
Н-1 («устаревшая ссылка в своей зоне») и оставил ложное утверждение двумя строками ниже. Поправить
тем же PR — раз файл уже тронут; это комментарий, поведения не меняет.

### Н-3 (MINOR) — число в теле коммита не воспроизводится: m1 роняет 7 сценариев, а не 8

`24976e79` (тело): «мутанты барьера m1 (проверка шага → true) … роняют её (8, 1, 1 сценарий)».
Повтор m1 (`if grep -qE "…${want}…" <<<"$JOB"` → `if true`):

```
$ FO_BARRIER=<m1> bash scripts/tests/red_feature_oracles.sh
FAIL  s1 … FAIL  s3 … FAIL  s4 … FAIL  s5 … FAIL  s6 … FAIL  s8 … FAIL  s9
сценариев: 13, pass=6, FAIL=7          rc=1
```

m2 (`continue-on-error` → `if false`) роняет ровно s7, m3 (пустота → `if false`) — ровно s11:
1 и 1 сходятся. Мой четвёртый мутант m6 (`JOB=$(cat "$CI")`, потеря привязки к джобу) роняет
ровно s6 — привязка к `build-test` пиннится. Расхождение 7≠8 — класс «утверждение о состоянии
без команды в том же сообщении» (профиль architect'а); не блокирует (все три мутанта ловятся),
но число в теле коммита — часть аудит-трейла и должно быть снято прогоном.

### Н-4 (ось г, принято без действия) — цена шага: весь крейт гоняется ВТОРОЙ раз, и это неизбежно

Замер на этом хосте, кэш тёплый по зависимостям, пересборка только gateway-serve:
`cargo test -p gateway-serve` wall=157 s; `… --features testing` wall=141 s (с остановкой на
первом красном бинаре; на честном дереве — все 50 блоков). В шаг попадают и тесты БЕЗ фичи
(например `red_m89_structural_bound`, не под `cfg(feature)`) — то есть крейт тестируется дважды.
Отказаться от прогона без фичи нельзя: `lib.rs:575-577` несёт `#[cfg(feature = "testing")]` /
`#[cfg(not(feature = "testing"))]` — прод-форма бинаря существует только без фичи и обязана
тестироваться. Сужение `--test` отвергнуто самим инвариантом (новый файл без правки CI).
Конфликта с `check_resource_oracles.sh` нет: барьер структурный (счётчики потоковые, `:16-26`),
второй параллельный прогон того же бинаря не добавляет конфаундинга сверх уже существующего
в `cargo test --all`. Цена ≈ +2–3 мин на джоб `build-test` принимается.

### Н-5 (MINOR) — консервативные отказы и одна слепая зона у формы шага; не документированы

Барьер FAIL'ит формы, которые CI исполнил бы: `run: |` многострочный (e5), `run: … # комментарий`
(e9), ключ `build-test: # x` (e10 — «нет джоба build-test»). Это fail-closed и приемлемо, но автор
следующей правки ci.yml наткнётся на это без подсказки — одна строка в шапке барьера.
`working-directory:` на шаге (e6) барьер не видит, но в этом репозитории чужой каталог роняет
сам `cargo` — слепота не тихая. Подкаталог `tests/sub/mod.rs` (e4), `feature="x"` без пробелов
(e7), job-level `continue-on-error` (e8), два крейта/две фичи (s8, e11) — все ловятся.

### Н-6 (MINOR) — setup-страж пробы проверяет фикстуру теста, но не фикстуру workflow

`red_feature_oracles.sh:44-48` сверяет, что `tests/t.rs` несёт заявленный атрибут; что `ci.yml`
фикстуры несёт заявленную форму (закомментированный шаг в s5, `|| true` в s4, `continue-on-error`
в s7), страж не проверяет. `testing.md` «Целостность гейта» п.3 требует стража на КАЖДЫЙ
сценарий. Риск низкий (`printf` в песочницу), но симметрии нет.

### Н-7 (информационно) — `branch-build.yml` фичу не гоняет, и этого от него не требуется

`.github/workflows/branch-build.yml:107` — `cargo test --all` без фичи. Джоб не входит в агрегат
(`grep -c branch-build ci.yml` → 0), инвариант его не касается; но зелёный `branch-build` на
ветке НЕ означает, что оракулы под фичей прошли — узнаётся только из PR-чека.

## Полномочия, связность, замки (ось д)

- Зона: `.github/workflows/ci.yml`, `scripts/check_*.sh`, `scripts/tests/*.sh`,
  `crates/*/tests/**` — харнесс/RED, зона architect'а (`scope-guard.md`). Граница C не задета
  (ни данных, ни промоушенов, ни параметров). Замок §11: `check_docs_freeze.sh` в CI-форме rc=0 —
  зона `.claude/**`/`CLAUDE.md`/`04-workflow.md` не тронута.
- Утверждения о коде в комментариях: `ci.yml:23-28` («без фичи не компилируются… `VB-I-11`…
  норма Р-2») — `VB-I-11` есть (`viz-backend.md:294`), Р-2 есть (`04-workflow.md:131`), слепота
  без фичи показана MV2/ML2. Шапка барьера `:7-11` — состав файлов под фичей на дереве совпадает
  с грепом (red_m87_entrypoint, red_m91_legacy_counters, red_m95_provenance_fresh, red_ws_session;
  `red_m87_registry.rs:196` — литерал, исключён верно). Комментарий теста `:31-34` — шаг и барьер
  существуют на ветке; `docs/archive/verify_M-95.sh` существует и зовёт отсутствующий
  `scripts/tests/red_verify_M-95_ci_map.sh` (`:42`) — «не исполняется» верно.
- `verify_design_claims.sh --merge-preview origin/main` → `VERDICT: PASS (0 нарушений)`, rc=0.
- Харнесс-трек §5 п.5 (уборка фикстур): `trap 'rm -rf "$SANDBOX"' EXIT` есть; замер
  `/tmp/tmp.*` до/после прогона пробы — 4925/4925.

## Done Block

```
$ git fetch -q origin && git rev-parse origin/harness/td253-feature-oracles origin/main
3bff5e3aa6fa5ab2c83c3375f0aa27a82ae5edd1
24ee5c271213a4bcdee2e160b608e3eceb566194
$ git worktree add --detach /tmp/hft-arch-recheck-td253 origin/harness/td253-feature-oracles
HEAD is now at 3bff5e3a test(TD-253): red_m95_provenance_fresh — …
$ git log --oneline origin/main..HEAD
3bff5e3a test(TD-253): red_m95_provenance_fresh — … (R-255 Н-1) [architect]
24976e79 ci(TD-253): оракулы под --features testing исполняются в build-test; барьер формы шага + проба [architect]

# честное дерево
$ cargo test -p gateway-serve --features testing 2>&1 | grep -E '^test result' | awk '{p+=$4; f+=$6} END{print "passed="p" failed="f}'
passed=221 failed=0            rc=0  (блоков: 50)
$ cargo test -p gateway-serve --features testing --test red_m95_provenance_fresh
test result: ok. 7 passed; 0 failed; …      rc=0   wall=7.25s

# MV2: lib.rs:1611  Err(_) => None  →  Err(_) => Some(current_history_provenance_with_catalog(&cat,&filter_for_history))
$ cargo test -p gateway-serve                 → passed=211 failed=0   rc=0
$ cargo test -p gateway-serve --features testing
test f5_v1_refresh_failure_after_stale_is_fail_closed ... FAILED
thread 'f5_…' panicked at crates/gateway-serve/tests/red_m95_provenance_fresh.rs:723:5
test result: FAILED. 6 passed; 1 failed; …    rc=101
# ML2: lib.rs:2451  (legacy) та же подмена через crate::_gw::…(&cat,&cfg1.filter)
$ cargo test -p gateway-serve                 → passed=211 failed=0   rc=0   wall=157.14s
$ cargo test -p gateway-serve --features testing
test f6_legacy_refresh_failure_after_stale_is_fail_closed ... FAILED
thread 'f6_…' panicked at crates/gateway-serve/tests/red_m95_provenance_fresh.rs:761:5
test result: FAILED. 6 passed; 1 failed; …    rc=101 wall=140.86s
# код возвращён
$ cp <orig> crates/gateway-serve/src/lib.rs && git status --porcelain
<пусто>

# проба и мутанты барьера
$ bash scripts/tests/red_feature_oracles.sh | tail -3
сценариев: 13, pass=13, FAIL=0
VERDICT: PASS                                  rc=0
$ FO_BARRIER=m1 (шаг→true)        → FAIL s1,s3,s4,s5,s6,s8,s9; сценариев: 13, pass=6, FAIL=7   rc=1
$ FO_BARRIER=m4 (continue-on-error→false) → FAIL s7;  pass=12, FAIL=1   rc=1
$ FO_BARRIER=m5 (пустота→false)   → FAIL s11; pass=12, FAIL=1   rc=1
$ FO_BARRIER=m6 (JOB=cat $CI)     → FAIL s6;  pass=12, FAIL=1   rc=1

# обходы против НАСТОЯЩЕГО барьера (изолированные фикстуры, FO_ROOT/FO_CI)
e1_if_false            exit=0  PASS   ← Б-1
e2_if_push             exit=0  PASS   ← Б-1
e3_cfg_all (один крейт) exit=1 FAIL «не найдено ни одного крейта…»
e13_second_crate_cfg_all exit=0 PASS  ← Н-1
e4_subdir              exit=1  FAIL (ловит)
e5_multiline run: |    exit=1  FAIL (консервативно, Н-5)
e6_workdir             exit=0  PASS (слеп, но cargo упадёт сам, Н-5)
e7_nospace             exit=1  FAIL (ловит)
e8_joblevel_coe        exit=1  FAIL (ловит)
e9_trailing_comment    exit=1  FAIL (консервативно, Н-5)
e10_jobkey_comment     exit=1  FAIL «нет джоба build-test» (консервативно, Н-5)
e11_two_feats          exit=1  FAIL (ловит вторую фичу)

# барьеры в CI-форме на ветке (EVENT_NAME=pull_request PR_BASE_SHA=24ee5c27)
archived_refs        VERDICT: PASS   rc=0
protected_artifacts                  rc=0
artifact_ids                         rc=0
gate_meta            VERDICT: PASS — вердиктов проверено: 0 …   rc=0   (до этого файла)
docs_freeze                          rc=0
review_fa            SKIP (диапазон трогает ТОЛЬКО не-прод пути крейтов …)   rc=0
feature_oracles      VERDICT: PASS   rc=0
$ bash scripts/tests/red_ci_aggregate.sh | tail -1
VERDICT: PASS — сценариев: 8, расхождений: 0   rc=0
$ bash scripts/verify_design_claims.sh --merge-preview origin/main | tail -1
VERDICT: PASS (0 нарушений)                    rc=0

$ grep -n '^\s*if:' .github/workflows/ci.yml
669:    if: always()
$ grep -rnE 'cfg\((any|all)\(.*feature' crates/*/tests/ | wc -l
3        (все три — в комментариях)
$ b=$(ls -1d /tmp/tmp.* | wc -l); bash scripts/tests/red_feature_oracles.sh >/dev/null; a=$(ls -1d /tmp/tmp.* | wc -l); echo $b $a
4925 4925
$ bash scripts/reserve_artifact_id.sh R
reserve: резерв R-257 взят                     rc=0
```

## Условие снятия REJECT

1. Б-1: барьер краснеет на `if:` у шага (любая форма, не только `false`) — плюс сценарий пробы
   и мутант, роняющий ровно его; `red_feature_oracles.sh` печатает «сценариев: 14+».
2. Н-2 — тем же PR (файл уже тронут). Н-1, Н-3, Н-5, Н-6 — по усмотрению автора в том же PR или
   строкой предела в шапке барьера; Н-4, Н-7 — без действия.
3. Перепроверка круга 2 — по §9 (свежий контекст); если правка ограничится барьером/пробой/
   комментарием — зона та же, критик по триггерам §1 не срабатывает (новых крейтов, T1, risk-путей нет).
