<!-- GATE-META
milestone: TD-253
audited_repo: a3ka/hft-platform
audited_base: 24ee5c271213a4bcdee2e160b608e3eceb566194
audited_head: 1a49d3ded55fef4e7dfdaa92ceec18f162a0981b
verdict: APPROVE
-->

# R-258 — перепроверка `gates.md` §9, КРУГ 2 (харнесс-трек, адверсарий): `TD-253` — оракулы под `--features testing` в CI

**Вердикт: APPROVE** — условие снятия REJECT `R-257` выполнено целиком; блокирующих находок нет,
три неблокирующих (Н-1…Н-3 круга 2).

Предмет — ветка `harness/td253-feature-oracles`. Вершина взята командой
`git fetch -q origin && git rev-parse origin/harness/td253-feature-oracles` →
`1a49d3ded55fef4e7dfdaa92ceec18f162a0981b`; база — `origin/main` =
`24ee5c271213a4bcdee2e160b608e3eceb566194`. Круг 2 — два коммита поверх вердикта круга 1
`8975b0d1`: `4c4395c0` (барьер + проба) и `1a49d3de` (шапка теста). Роль — architect-клон со
свежим контекстом, НЕ автор; дерево — собственное detached `/tmp/hft-arch-recheck2-td253`.
Правился только этот файл.

**Код и CI в круге 2 не менялись** — `git diff --stat 3bff5e3a origin/harness/td253-feature-oracles`:
четыре файла (`red_m95_provenance_fresh.rs` 6 строк комментария, `R-257`, `check_feature_oracles.sh`,
`red_feature_oracles.sh`); `lib.rs`/`ci.yml` в списке отсутствуют. Мутанты `MV2`/`ML2` круга 1
не повторялись — им нечего было бы опровергать.

**FA тронутого модуля.** Диф трогает `crates/gateway-serve/tests/red_m95_provenance_fresh.rs`
(`//!`-шапка). Живой инвариант, который сторожат f0…f6, — **`VB-I-11`** (`docs/fa/viz-backend.md:294`,
провенанс истории `history_start_seq`/`history_truncated`). `check_review_fa.sh` на диапазоне —
`SKIP` (только не-прод пути); требование когнитивное, выполнено.

## 1. Условие снятия REJECT `R-257` — по пунктам

| пункт `R-257` | требование | факт на `1a49d3de` |
|---|---|---|
| Б-1 | барьер краснеет на `if:` у шага (любая форма) | `check_feature_oracles.sh:53-55` — любой `^\s*(-\s+)?if:` в теле `build-test` → FAIL. e1 (`if: false`) и e2 (`if: github.event_name == 'push'`) из `R-257`, воспроизведённые заново: **оба `exit=1`** (были `exit=0`). Job-level `if:` (e14) и инлайновый `- if:` (e18) — тоже `exit=1` |
| Б-1 (а) | сценарий пробы | s13 `if: false`, s14 `if: push` — оба с setup-стражем workflow (`must`) |
| Б-1 (б) | мутант, роняющий ровно его | `mif` (`:53` → `if false`) → FAIL **ровно s13, s14**; остальные 15 pass |
| Б-1 | «сценариев: 14+» | проба печатает `сценариев: 17, pass=17, FAIL=0` |
| Н-2 | шапка теста тем же PR | `1a49d3de`: «Сегодня — красен» → «Заведён красным … Зелёный с `M-95` (PR #333): f0…f6 — 7 passed». Оба утверждения проверены: `gh pr view 333` → MERGED 2026-10-10T12:21Z (`b3e9d7b1`); `cargo test -p gateway-serve --features testing --test red_m95_provenance_fresh` → `7 passed; 0 failed`, rc=0 |
| Н-1 | `cfg(all/any(…feature…))` | `ATTR` (`:59`) видит `all(`/`any(` в одной строке; предел (макрос, `cfg_attr`, разнос по строкам) назван `:24-26`. s15/s16 + мутант `mattr` роняет ровно их |
| Н-3 | число m1 в теле коммита | тело `4c4395c0` заявляет 8 (s1 s3 s4 s5 s6 s8 s9 s15) и честно называет расхождение круга 1; мой прогон — **ровно эти 8** |
| Н-5 | консервативные отказы в шапке | `:27-30` |
| Н-6 | setup-страж на фикстуру workflow | `run_case` 4-й аргумент `must` (`:51`); проставлен у s3, s4, s5, s7, s13, s14 |

## 2. Мутационный контроль — повторён, числа тела коммита совпали ВСЕ

```
$ bash scripts/tests/red_feature_oracles.sh | tail -2
сценариев: 17, pass=17, FAIL=0
VERDICT: PASS                                       rc=0
$ FO_BARRIER=mif   (:53 → if false)   → FAIL s13 s14;                        pass=15 FAIL=2  rc=1
$ FO_BARRIER=mattr (:59 → старый ATTR без all/any) → FAIL s15 s16;            pass=15 FAIL=2  rc=1
$ FO_BARRIER=m1    (:73 → if true)    → FAIL s1 s3 s4 s5 s6 s8 s9 s15;        pass=9  FAIL=8  rc=1
$ FO_BARRIER=m2    (:50 → if false)   → FAIL s7;                              pass=16 FAIL=1  rc=1
$ FO_BARRIER=m3    (:83 → if false)   → FAIL s11;                             pass=16 FAIL=1  rc=1
```

Заявлено в теле `4c4395c0`: mif → s13 s14; mattr → s15 s16; m1 → s1 s3 s4 s5 s6 s8 s9 (+s15);
m2 → s7; m3 → s11. Расхождений — ноль. Класс `R-257` Н-3 (число без прогона) в этом коммите не
повторился.

## 3. Новый `ATTR`-регексп на живом дереве — ложных срабатываний нет

```
$ grep -rnE "$ATTR" crates/*/tests/        → 12 строк, все 4 файла gateway-serve, все — реальные атрибуты
$ bash scripts/check_feature_oracles.sh    → PASS gateway-serve … (4 файл(ов)) …; VERDICT: PASS  rc=0
$ grep -rnE 'cfg\((not|all|any)\(.*feature' crates/*/tests/   → 3 строки, все начинаются с `//` (комментарии),
                                             ATTR требует `#` в начале строки — не матчатся
$ sed -n 196p crates/gateway-serve/tests/red_m87_registry.rs  → литерал внутри contract_violations("…") — не матчится
$ grep -rnE '^\s*/\*' crates/*/tests/ | wc -l                 → 0 (блочных комментариев, где `#[cfg(` мог бы стоять
                                             в начале строки, в tests/ нет)
$ grep -nE '^\s*(-\s+)?["'"'"']?if["'"'"']?:' .github/workflows/ci.yml → только :669 (агрегат, вне build-test)
```

Счёт `n` = 4 совпадает со списком файлов, несущих литерал `feature = "testing"`
(red_m87_entrypoint, red_m91_legacy_counters, red_m95_provenance_fresh, red_ws_session).

## 4. Новые обходы (адверсарий, изолированные фикстуры `FO_ROOT`/`FO_CI`)

```
e1_if_false            exit=1  ← Б-1 закрыт
e2_if_push             exit=1  ← Б-1 закрыт
e14_joblevel_if        exit=1  (ловит)
e18_dash_if_inline     exit=1  (ловит)
e15_quoted_if  "if": false     exit=0  PASS  ← Н-1 (круг 2)
e16_single_quoted_if 'if': false exit=0 PASS ← Н-1 (круг 2)
e17_anchor_merge  <<: *skip (якорь вне джоба, if: false) exit=0 PASS ← Н-1 (круг 2)
e20_shell_custom  shell: bash -c 'true' {0}   exit=0  PASS  ← Н-1 (круг 2)
e22_not  второй крейт ТОЛЬКО под cfg(not(feature = "testing")) → FAIL «jr … под feature "testing"» ← Н-2 (круг 2)
```

## Находки

### Н-1 (MINOR, круг 2) — три формы саботажного класса проходят барьер и не названы в пределе

Шапка (`:17-20`) говорит «ключ `if:` где угодно в теле `build-test` — FAIL». Три YAML-эквивалента
барьер не видит: (а) ключ в кавычках `"if": false` / `'if': false` (e15/e16) — лечится одним
фрагментом регекспа `["']?if["']?:` на `:53`; (б) merge-key `<<: *skip` с якорем, определённым
ВНЕ тела джоба (e17) — строка `if:` живёт вне `JOB`; (в) собственный шаблон `shell: bash -c 'true' {0}`
(e20) — шаг завершается `0`, не исполнив `cargo`. Почему не блокирует: модель угрозы барьера —
ДРЕЙФ (шаг потерян, сужен, заглушен при правке CI), а не саботаж; автор, желающий обойти, правит
сам барьер в том же PR. Честность требует другого: назвать (б)/(в) в «пределе» рядом с `cfg_attr`,
а (а) закрыть — цена один фрагмент регекспа и один сценарий. Поддержку якорей GitHub Actions'ом
я не проверял — e17 назван как форма, прошедшая барьер, не как подтверждённый обход CI.

### Н-2 (MINOR, круг 2) — `cfg(not(feature = "f"))` атрибутируется как «под фичей f»

`ATTR` (`:59`) допускает `not(` через `.*[(,]`. Крейт, чьи тесты стоят ТОЛЬКО под
`#![cfg(not(feature = "testing"))]`, получает требование шага `--features testing` (e22, `exit=1`),
хотя эти тесты исполняет обычный `cargo test --all`. Направление — fail-closed (барьер требует
ЛИШНЕЕ, а не молчит), на живом дереве `not(feature` в `tests/` нет (грэп §3). Одна строка в шапке
либо `(?<!not\()` — но ERE lookbehind не умеет; проще исключить `not(` отдельным `grep -v`.

### Н-3 (MINOR, круг 2) — остаток `R-257` Н-6: у s6 нет workflow-стража

`red_feature_oracles.sh:90-92` — сценарий «шаг в чужом джобе» строит фикстуру без `must`:
если `TAIL` перестанет содержать джоб `other`, s6 будет судить шаг, стоящий в `build-test`, и
ожидать FAIL от PASS-состояния. Низкий риск (константа в том же файле); симметрия
`testing.md` «страж на КАЖДЫЙ сценарий» не доведена. `must="  other:"` закрыл бы.

## Полномочия, связность, замки

- Зона круга 2: `scripts/check_*.sh`, `scripts/tests/*.sh`, `crates/*/tests/**` — харнесс/RED,
  зона architect'а; граница C не задета. Замок §11 — `docs_freeze` rc=0.
- Утверждения о состоянии в шапке теста (`:38-39`) проверены командами (§1, строка Н-2).
- Фикстуры за собой убраны: `/tmp/tmp.*` до/после прогона пробы — 4925/4925.

## Done Block

```
$ git fetch -q origin && git rev-parse origin/harness/td253-feature-oracles origin/main
1a49d3ded55fef4e7dfdaa92ceec18f162a0981b
24ee5c271213a4bcdee2e160b608e3eceb566194
$ git worktree add --detach /tmp/hft-arch-recheck2-td253 origin/harness/td253-feature-oracles
HEAD is now at 1a49d3de test(TD-253): red_m95_provenance_fresh — «Сегодня — красен» приведено к факту …
$ git diff --stat 3bff5e3a origin/harness/td253-feature-oracles
 .../tests/red_m95_provenance_fresh.rs | 6 +-   research/reviews/R-257-… | 273 +
 scripts/check_feature_oracles.sh | 27 +-       scripts/tests/red_feature_oracles.sh | 37 +-
 4 files changed                                  (lib.rs, ci.yml — не тронуты)

$ cargo test -p gateway-serve --features testing --test red_m95_provenance_fresh
test result: ok. 7 passed; 0 failed; 0 ignored; … finished in 5.79s        rc=0
$ gh pr view 333 --json title,state,mergedAt,mergeCommit
M-95: одна подписка обходит каталог журнала один раз (TD-229) | MERGED | 2026-10-10T12:21:38Z | b3e9d7b1

$ bash scripts/tests/red_feature_oracles.sh | tail -2
сценариев: 17, pass=17, FAIL=0
VERDICT: PASS                                                               rc=0
# мутанты — §2 выше (mif 2, mattr 2, m1 8, m2 1, m3 1 — все совпали с телом 4c4395c0)

# барьеры в CI-форме (EVENT_NAME=pull_request PR_BASE_SHA=24ee5c27…), до добавления этого файла
archived_refs        VERDICT: PASS                                          rc=0
protected_artifacts                                                         rc=0
artifact_ids                                                                rc=0
gate_meta            VERDICT: PASS — вердиктов проверено: 1 (R-257) …      rc=0
docs_freeze                                                                 rc=0
review_fa            SKIP (диапазон трогает ТОЛЬКО не-прод пути …)          rc=0
feature_oracles      VERDICT: PASS                                          rc=0
$ bash scripts/tests/red_ci_aggregate.sh | tail -1
VERDICT: PASS — сценариев: 8, расхождений: 0                                rc=0
$ bash scripts/verify_design_claims.sh --merge-preview origin/main | tail -1
VERDICT: PASS (0 нарушений)                                                 rc=0

$ b=$(ls -1d /tmp/tmp.* | wc -l); bash scripts/tests/red_feature_oracles.sh >/dev/null; a=$(ls -1d /tmp/tmp.* | wc -l); echo $b $a
4925 4925
$ bash scripts/reserve_artifact_id.sh R
reserve: резерв R-258 взят                                                  rc=0
```

## Что дальше

Предмет готов к PR автора (`harness-track.md` §3/§5): проба зелёная против честного барьера и
красная против пяти мутантов — предъявлено; вердикт адверсария — этот файл на ветке. Н-1…Н-3 —
по усмотрению автора тем же PR (одна строка регекспа + одна-две строки предела + `must` у s6)
или отдельной записью долга; ни одна не удерживает merge.
