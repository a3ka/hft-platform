<!-- GATE-META
milestone: M-94
audited_repo: a3ka/hft-platform
audited_base: 045fef9a4345ffc55b2779e0fa5e87f9b72c6aa0
audited_head: 2209a05ac269528dd07489ea5234d448d6fefda9
verdict: REJECT
-->

# R-245 — M-94 calc-profile, PR-гейт — REJECT

**Предмет:** ветка `origin/feat/M-94-calc-profile`. Вершина взята командой
(`git fetch origin && git rev-parse origin/feat/M-94-calc-profile`) =
`2209a05ac269528dd07489ea5234d448d6fefda9`, совпала со справкой мандата. База —
`045fef9` (merge-base с `origin/main`); `main` впереди на 2 коммита (`783261e`, `6ee3f1a` —
только `docs/ROADMAP.md`), слияние чистое. Работа — в detached worktree
`/tmp/hft-reviewer-m94` (ветка) и `/tmp/hft-reviewer-m94-merge` (дерево слияния с `origin/main`).

**Живые инварианты тронутых модулей** (диф трогает `crates/gateway/**`, `crates/gateway-serve/**`,
`crates/ops/**` — только `tests/`):
- `VB-I-2` (`docs/fa/viz-backend.md:285`, live == replay: одно определение расчёта у писателя и
  читателя слепка) — держится: в режиме профиля оба процесса берут оси из одного загрузчика,
  `p1`/`p2` зелёны;
- `VB-I-11` (`docs/fa/viz-backend.md:294`, провенанс истории) — профиль версии/`sha256` в
  сердцебиении и `.profile` у слепка — тот же класс честности;
- `OPS-I-11` (`docs/fa/ops.md:479`, тишина ВЫДАЧИ — «жив, но не обслуживает») — читатель
  сердцебиения не ломается аддитивным полем (`o1`), сторож `M-89/M-93` не затронут кроме пути
  установки при откате (см. B-2);
- `OPS-I-8` (`ops.md:476`), названный в спеке §14, — про тишину ПОТОКА MD, к предмету не
  относится (как и отметил `A-049`).
`crates/gateway` / `crates/gateway-serve` собственной FA не имеют — долг уже назван (`reading-map.md`
§2, дом — `M-81`); опора — `viz-backend.md`.

**Греп ярусa C:** `TECH-DEBT.md` (origin/main) — по `TD-227`, `TD-229`, `TD-232`,
`TD-238`, `TD-243`, `runbook_markers`/`S17b`, `compose_deploy_form`/`D10`; последний номер `TD-243`.

## Вердикт

**REJECT.** Две блокирующие находки — обе нарушают явные строки спеки, одна из них —
запретный список §5, введённый обязательным решением арбитра `A-049` Р-6. Ни одна не
ломает прод-инварианты `I-1…I-9` сегодня (все оракулы зелёны), но гейт не вправе
снимать запрет арбитра за architect'а. Маршрут по профилю — **architect**: он либо
диспетчеризует dev на правку, либо меняет спеку (§5/§3.7) сам, назвав это.

## Блокирующие

### B-1 — второй экземпляр разборщиков и разбор значений в загрузчике (спека §3.2, §5 строка 4; `A-049` Р-6)

§5: «ЗАПРЕЩЕНО — дублировать разбор значений в загрузчике; держать второй экземпляр
разборщиков окна heatmap / шага VP / `ALLOWED_PROFILES` / связи тройки». §3.2:
«`serve_config_from_env` и `admission_policy_from_env` ЗОВУТ их … а не держат копию»;
«Проверку ЗНАЧЕНИЙ [загрузчик] не дублирует».

Факт на `2209a05`:
1. **Окно heatmap и шаг VP — две копии.** `crates/gateway-serve/src/lib.rs:3016-3034`
   (`GATEWAY_HEATMAP_WINDOW`) и `:3288-3313` (`GATEWAY_VP_BIN_WIDTH_E8`) — прежние
   разборщики оставлены в ветке без профиля; `gateway::calc_profile::parse_heatmap_window_frac`
   и `parse_vp_bin_width_e8` из `gateway-serve` не зовутся ни разу:
   `grep -c 'parse_heatmap_window_frac\|parse_vp_bin_width_e8' crates/gateway-serve/src/lib.rs` → `0`.
   (`ALLOWED_PROFILES` сделан верно: `:3420` зовёт `calc_profile::parse_allowed_profiles`.)
2. **Загрузчик разбирает значения сам.** `crates/gateway/src/calc_profile.rs:239-290` — свой
   `parse_bands` (`:320`) и свои правила диапазонов каденции/таймфрейма/окна; в режиме профиля
   выдача и прогреватель берут готовые `AppliedProfile.*`, минуя прежние разборщики. Правила
   РАЗОШЛИСЬ по режимам уже сейчас: окно `0` — в профиле отказ (`:285`), без профиля — offline
   `None` (`gateway-serve/src/lib.rs`, ветка `Ok(0) => None`); полосы `≤ 0` — в профиле отказ
   (`:334`), без профиля разборщик их принимает.

Почему блокирует, хотя прод цел: это ровно та форма «второй разбор = второе правило», которую
арбитр запретил после трёх кругов, и пункт стоит в запретном списке, а не в рекомендациях.
Тестер этого не видел и не мог — оракулы `p*`/`s*` судят поведение режима профиля, где копий
нет. Дизайн исправления — за architect'ом (`gates.md` §4, граница reviewer↔architect).

### B-2 — откат по здоровью не переустанавливает сторожа (спека §3.7 таблица, шаг 4)

Спека, шаг 4, колонка «при отказе»: «…`up` на PREV, **сторож `|| true`**, выход `≠ 0`».
`deploy/bin/deploy-apply.sh:79-97` (`full_rollback_prev`) — build, cron, `up` на PREV; вызова
`${DEPLOY_WATCHDOG_INSTALL}` нет ни там, ни в ветке `health_failed` (`grep -n -i watchdog
deploy/bin/deploy-apply.sh` — только шаг 5 успеха). Прежний `deploy.yml` (`045fef9`, ветка
`DEPLOY FAILED`) это делал: `bash deploy/bin/install-watchdog.sh || true`. Регресс против базы и
против спеки. Проба `red_m94_deploy_apply.sh` этот пункт не пиннит (миры `a4*` не смотрят на
`watchdog@PREV`) — значит, у находки два адресата: реализация и оракул (architect).
Практический вред узкий (на откате по здоровью хостовый сторож обычно уже от PREV), но случай
«прошлый деплой упал на установке сторожа (`a5`), этот откатился по здоровью» оставляет прод
без сторожа — ровно то, что `M-93` объявил недопустимым.

## Не блокирующие (в работу architect'у тем же кругом, где дёшево)

- **N-1 — FA станет ложной на merge.** `docs/fa/viz-backend.md:42`: «`GATEWAY_BANDS=…` —
  строкой в host `.env`, дефолт compose `:147` остался `0.001`». После M-94 носитель —
  `config/calc-profile/active.env`, дефолта в compose нет, строка в `.env` удаляется задачей 11.
  §10 давал architect'у этот файл ровно для этой правки — она не сделана. `verify_design_claims`
  прозу не ловит (PASS). Класс `TD-138`.
- **N-2 — W7 (`scripts/deploy_catchup.py`, `af11322`) проверяет текст, и обход есть.**
  Адверсарно (мандат п.1), все четыре прошли `check-wiring` exit=0: (а) вызов
  `deploy-apply.sh "$PREV"` закомментирован в `deploy.yml`; (б) вызов с `|| true`;
  (в) откат в скрипте обёрнут `if false; then …; fi`; (г) `echo git reset --hard -q "$PREV"`.
  НЕ регресс: прежний W7 на `045fef9` так же слеп (откат закомментирован в старом `deploy.yml` →
  exit=0, проверено). Правка W7 сама по себе корректна и строже прежней на стороне скрипта
  (комментарий в скрипте ловится, W13c). → кандидат в долг K-1 (заводится в `TECH-DEBT.md` при close-out этого милестоуна).
- **N-3 — исполняемые пробы деплоя не в CI.** `red_m94_deploy_apply.sh` и
  `red_m94_deploy_gate.sh` зовёт только `verify_M-94.sh`; `grep red_m94 .github/workflows/*.yml` —
  пусто. После merge тело отката сторожит лишь текстовый W7 (N-2), а его комментарий
  «откат там исполняется пробой» описывает приёмку, не постоянный барьер. → кандидат K-1 (вместе с N-2).
- **N-4** — `calc_profile.rs:568-573` `applied_profile_optional_io` — мёртвая заглушка
  «на будущее» под `#[allow(dead_code)]`. Убрать.
- **N-5** — `deploy/bin/calc-profile-gate.sh`: `compose run … 2>/dev/null` глотает stderr
  прогревателя; при отказе гейта оператор не видит, почему runner упал.

## Пункты мандата

1. **Scope.**
   - `d1de9fb` (venue/symbol из env в режиме профиля) — **ПОДТВЕРЖДАЮ.** `venue`/`symbol` не
     входят в восемь ключей (§3.1), §3.3 запрещает в env только ключи профиля; без правки режим
     профиля зашивал `Binance`/`BTCUSDT` и ломал единый источник `M-90` (`w4a`/`w4b`). Файл в §10.
   - `64197cf` — **ПОДТВЕРЖДАЮ**: чистый возврат строк `M-90`; итоговый диф compose по
     `GATEWAY_VENUE`/`GATEWAY_SYMBOL` против базы — нулевой (`045fef9:docker-compose.yml:347-348`).
   - `af11322` (`deploy_catchup.py`) — вне §10, зона architect'а (харнесс), промах замера §9.3
     назван в коммите. Адверсарная проверка — N-2: обойти можно, но не хуже базы.
   - `231914a` (`.claude/rules/scope-guard.md`) — несёт `FOUNDER-APPROVED: П-032 п.3 — …`;
     сжатие исторической справки у `recorder` раскрыто в теле, зона не изменена.
   - Сверх §10, все — architect, обоснованы: `red_depth_cadence_from_env.rs` (`04dc8a0`, шестой
     оракул, пропущенный замером §9.3 — не ослаблен, перенацелен на файл профиля),
     `red_runbook_markers.sh` (`2209a05`), `Cargo.lock` (следствие `sha2`). `crates/gateway/src/lib.rs`
     — только `pub mod calc_profile` + doc. `contracts/**`, `docs/rfc/**`, `admission.rs` — не тронуты.
   - Risk-блок: `risk`/`killswitch`/`oms`/`venue-*` не тронуты — risk-critic не требуется (§13).
   - Contract Block-C: `crates/contracts/**` не тронут; поле кадра отложено (§3.6 (в)).
2. **RED-first / атомарность.** Ни один коммит `[engine-dev]` не трогает `*/tests/**`
   (`git show --stat` по `dcc15ef…d1de9fb`). Правки оракулов после dev — только architect'ом
   (`0df2cd9`, `04dc8a0`), не ослабляют. По задаче ≥1 коммит, бандлов нет; `99598e0` — fmt с
   пометкой диапазона.
3. **Done Block dev/tester** — в мандате пересказом; сырые выводы гейтов я снял сам (ниже).
4. **§9 перепроверка харнесса.** `af11322` и `2209a05` трогают зону `gates.md` §9
   (`scripts/tests/**`) после `C-283`; этот вердикт со свежим контекстом покрывает (а)–(в) для
   них исполнением (N-2, S17b ниже).
5. **Кандидаты в долг:**
   - S17b — **исправлен, подтверждаю исполнением**: прежняя проба при `TMPDIR` внутри git-дерева —
     `FAIL S17b SETUP НЕ СОСТОЯЛСЯ`, exit=1; новая там же — `VERDICT: PASS`, exit=0. Долга не завожу.
   - `GATEWAY_BANDS` в `scripts/lib/compose_deploy_form.sh:7,28` — холостая подстановка (§9.3 назвал),
     комментарий `:7` после задачи 11 станет ложным. → кандидат K-2 (MINOR), заводится при close-out.
   - `o1` пиннит только совместимость разбора — **долга не завожу**: появление поля и его значения
     (`version == 1`, `sha256` файла) пиннит `s1` на прод-бинаре; `ops` поле не потребляет и не обязан.
   - Новые: K-1 (N-2 + N-3). Номера `TD` выдаются механизмом в момент записи, не здесь.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-94-calc-profile origin/main
2209a05ac269528dd07489ea5234d448d6fefda9
6ee3f1af60fd1a482842d267d84fc625505c2db4

# дерево слияния: detached 2209a05 + git merge origin/main → f8db6a3 (Auto-merging docs/ROADMAP.md)
$ cargo fmt --all -- --check; echo exit=$?
exit=0
$ cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -1; echo exit=$?
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.49s
exit=0
$ cargo test --all 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" (блоков: "NR")"}'
passed=1231 failed=0 (блоков: 274)
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main; echo exit=$?
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/verify_M-94.sh   # ветка, 2209a05
PASS  task7: config/calc-profile/active.env — восемь ключей
PASS  task1-2,4-5: red_m94_calc_profile_warmer (c1 c2 p1…p9, p8b)
… (всего PASS=67, FAIL=0)
SKIP ×8 — разрешённые: 6 ci-parity (установка инструментов / плумбинг CI / агрегат),
           check_review_fa (вердикта ещё не было — этот файл его и даёт), task11 (§8, после merge)
VERDICT: PASS
exit=0

# B-1
$ grep -c 'parse_heatmap_window_frac\|parse_vp_bin_width_e8' crates/gateway-serve/src/lib.rs
0
# B-2
$ grep -n -i 'watchdog' deploy/bin/deploy-apply.sh | grep -v '^[0-9]*:#'
52:DEPLOY_WATCHDOG_INSTALL="${DEPLOY_WATCHDOG_INSTALL-bash deploy/bin/install-watchdog.sh}"
213:log "=== STEP 5: install watchdog ==="
214:if ! ${DEPLOY_WATCHDOG_INSTALL}; then
215:  log "=== WATCHDOG INSTALL FAILED ==="
225:log "=== DEPLOY OK: TARGET deployed, both services healthy, watchdog installed ==="

# N-2: мутанты W7 (CATCHUP_DEPLOY_YML / CATCHUP_DEPLOY_APPLY)
$ python3 scripts/deploy_catchup.py check-wiring            → baseline exit=0
  вызов закомментирован в deploy.yml                        → exit=0
  вызов с `|| true`                                         → exit=0
  откат в скрипте под `if false; then …; fi`                → exit=0
  `echo git reset --hard -q "$PREV"`                        → exit=0
  БАЗА 045fef9: откат закомментирован в старом deploy.yml   → exit=0  (слепота не новая)

# S17b
$ TMPDIR=<каталог внутри git-дерева> bash scripts/tests/red_runbook_markers.sh   # 2209a05
ok    S17b корень — не рабочее дерево git (rc=1)
VERDICT: PASS            exit=0
$ то же на 2209a05~1
FAIL  S17b SETUP НЕ СОСТОЯЛСЯ: каталог внутри git
VERDICT: FAIL            exit=1
$ bash scripts/tests/red_runbook_markers.sh   # обычный TMPDIR, 2209a05
VERDICT: PASS            exit=0
```

## Условие APPROVED

B-1 и B-2 сняты (исправлением кода с оракулом на саму находку — `testing.md` «исправление по
вердикту тоже требует оракула» — либо явной правкой спеки architect'ом с основанием), N-1
исправлен; затем tester на новой вершине и повторный PR-гейт. Задача 11 (удаление строки
`GATEWAY_BANDS` из host `.env`) **НЕ исполнялась** — merge не состоялся, текущему проду строка
нужна.

## Handoff §D

Следующий агент — **architect** (`.claude/agents/reviewer.md` §Handoff: разбор REJECT — его).
