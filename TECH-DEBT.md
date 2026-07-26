# TECH-DEBT — открытый долг

> **Reviewer-owned.** Открытые долги/риски, замеченные при работе. Закрытые переносятся вниз.

## OPEN
- **TD-038** `gateway-snapshot-crc-mismatch-on-live-compacted-journal` (найдено reviewer'ом на §8 M-28,
  2026-07-26). **Класс TD-011/M-08: зелёные юниты + Deploy-success + healthy TCP-healthcheck ≠ рабочий
  прод.** M-28 gateway-serve — ПЕРВЫЙ раз, когда код `crates/gateway` (M-22/23, read-side reducer/replay
  над журналом) реально запущен против ЖИВОГО прод-журнала (до M-28 образ нёс только
  `recorder`+`journal-retention`, gateway был инертен). §8 E2E вскрыл: JWT-verify корректен
  (`ws auth ok`; wrong-key/expired → `Error`/reject), но авторизованная сессия падает на снапшоте —
  `gateway-serve conn ended with error error=frame crc mismatch`, снапшот клиенту НЕ уходит.
  **ДЕТЕРМИНИРОВАННО (3/3 прогона FAIL — НЕ torn-tail race).** Дефект НЕ в транспорте M-28 (хендшейк/
  auth/passthrough корректны, gates GREEN), а ниже — в `gateway::snapshot` → `journal::stream`
  (`EpochFilter::OwnCaptureOnly`, `Cursor::LATEST`) на РЕАЛЬНОЙ раскладке прод-журнала:
  1 задекларированный legacy raw-сегмент `segment-00000000.jrnl` (15.19 GB, `journal.legacy.json`,
  OwnCapture) + **88 компактированных `segment-*.jrnl.zst`** (D-COMP-3) + 5 активных raw-сегментов
  (89–93). Юнит-фикстуры используют ТОЛЬКО свежий несжатый журнал (`Journal::open_with` → flush → read),
  поэтому ни `.zst`-компакция, ни legacy-раскладка в тестах не покрыты. Рабочая гипотеза (диагностика —
  зона architect, НЕ reviewer): stream парсит байты `.zst`-сегмента (или legacy) как raw postcard+crc
  фреймы → детерминированный crc mismatch; STRICT-чтение (DET-I-1, Err на первом mismatch) корректно
  фейлит, но снапшот-путь обязан УМЕТЬ читать компактированную/многоформатную раскладку. **Прод НЕ
  повреждён:** gateway-serve read-only (mount `/journal` `rw=false mode=ro`), recorder healthy/`restarts=0`/
  heartbeat свежий/`writable=true` — сбор данных не задет; revert НЕ требуется (сервис безвреден-но-нефункционален,
  loopback-only, без ports-publish). **Нужно (architect → RED-first, gates §4 reviewer описывает / architect
  проектирует):** RED-оракул — `gateway::snapshot`/`stream` над журналом, СОДЕРЖАЩИМ компактированные `.zst`
  + задекларированный legacy + активный сегмент, обязан вернуть `Ok(Snapshot)` (анти-плацебо: падает на
  текущей реализации с crc mismatch); прод-масштаб дисциплина `.claude/rules/testing.md`. Затем fix в
  крейте-владельце (`journal` stream-reader для `.zst` ЛИБО `gateway` snapshot read-path) → engine-dev impl →
  reviewer повторный §8 E2E. Severity: **MAJOR** (продуктовая цель M-28 — «фронт получает снапшот+live» —
  на проде НЕ работает; milestone НЕ закрывается до фикса + §8-GREEN E2E).
- **TD-037** `github-actions-billing-block-halts-ci-cd` — **✅ CLOSED 2026-07-25 (founder восстановил
  билинг; доказано СКВОЗНЫМ прогоном, а не глазами).** Founder исправил Billing & plans после эскалации;
  подтверждение: re-run CI на `841d7d3` → success (13:45); затем push M-34 merge `211e452` → CI run
  30163501141 **success** + Deploy 30163501124 **success** (оба jobs СТАРТОВАЛИ и прошли, не «not started»)
  → VPS обновлён, recorder healthy. Пайплайн (ci.yml fmt/clippy/test/audit + deploy.yml gated-on-CI)
  функционирует. Единственный оставшийся артефакт — стей­л failure-run на `1732f91` (billing-окно, не
  перезапускался; главная давно ушла вперёд на зелёный `211e452`). Ниже — исходное описание.
  **ВЕСЬ CI/CD пайплайн ОСТАНОВЛЕН на уровне аккаунта GitHub, НЕ регрессия кода.** Push M-35 close-out
  (`841d7d3`, docs-only) → CI run `30159262236` **failure за 12s**: ВСЕ 5 jobs (`fmt+clippy+test`,
  `cargo audit`, `Protected artifacts`, `Delivery gate`, `All checks passed`) со статусом «not started»,
  аннотация GitHub: *«The job was not started because recent account payments have failed or your spending
  limit needs to be increased. Please check the 'Billing & plans' section.»* Ни один job не запустился —
  это billing-блок аккаунта, НЕ падение теста/линта. Диф был docs-only (1 markdown, 0 LOC кода, локально
  fmt exit=0) — код ни при чём. Онсет: между `2026-07-24T23:35Z` (последний зелёный CI, M-33) и
  `2026-07-25T13:11Z` (первый билинг-фейл). **Следствие (КРИТИЧНО):** (1) `ci.yml` не может гонять
  fmt/clippy/test/audit → нет автоматического workspace-гейта; (2) `deploy.yml` gated-on-CI (TD-017/018
  fail-closed) → **автодеплой на VPS НЕВОЗМОЖЕН** (Deploy ждёт CI success, которого не будет) → §8 eyes-on
  для любого milestone, тронувшего `crates/**`, **выполнить нельзя** до восстановления билинга. M-35
  (docs-only) деплой не требовал, поэтому смержен, но СЛЕДУЮЩИЙ code-milestone (M-34 TPP и т.д.) упрётся в
  этот блокер на §8. **Revert НЕ помогает** — billing вне репозитория. **Зона: FOUNDER** (только владелец
  аккаунта чинит Billing & plans / spending limit; ни один агент не имеет доступа). Severity: **MAJOR**
  (весь gate-3/§8 контур не функционирует; прод не обновляется и не верифицируется через пайплайн —
  recorder на VPS продолжает работать на старом образе, но любой фикс/milestone застрянет). До восстановления:
  code-milestone'ы можно верифицировать ТОЛЬКО локально (reviewer worktree fmt/clippy/test + прямой ssh на
  VPS вручную), но auto-deploy и CI-гейт недоступны — это НЕ эквивалент §8 (нет CI-подтверждения на merge-SHA).
- **TD-036** `chain-bootstrap-from-worktree-not-origin-feat` (RN-18 process gap, замечено reviewer'ом
  на PR-гейте M-30, 2026-07-24) — engine-dev НЕ запушил свои GREEN-коммиты (`4fd09d0`/`a896cb8`) на
  `origin/feat/M-30-book-gap-detection` (оставался на architect-RED baseline `ff62333` = compile-RED
  до самого merge); tester забутстрапился из ЛОКАЛЬНОГО worktree `/tmp/hft-engine-m30`, а не из origin
  (milestone §Handoff явно требовал «бутстрап ТОЛЬКО с origin/feat/M-30 — RN-18; push GREEN перед
  handoff»). **Класс TD-014 (chain-break):** незапушенный GREEN живёт только в чужом worktree → любой
  следующий агент, честно бутстрапящийся с origin, увидел бы compile-RED, а не готовый импл; аудит-трейл
  «что тестировал tester» не воспроизводим с origin. **НЕ дефект кода и НЕ блокер M-30** — reviewer
  независимо создал СВОЙ worktree из точного `a896cb8`, прогнал все гейты с нуля (fmt/clippy/358-0/
  verify PASS), ревью валидно; на merge reviewer ff-запушил `a896cb8` → origin/feat/M-30 (GREEN
  восстановлен на origin) перед мержем в main. **Действие (зона architect/founder):** dispatch-mandate
  + tester-profile для M-NN обязаны ЯВНО предписывать intra-chain push GREEN на shared `feat/M-NN`
  ПЕРЕД handoff'ом следующему агенту (gates.md §8 «intra-chain push» уже это требует — правило есть,
  соблюдение в цепочке M-30 не выполнено). Иначе следующий milestone рискует тем же расхождением.
- **TD-035** `toolchain-drift-local-clippy-weaker-than-CI` — **✅ CLOSED 2026-07-24** (architect durable-фикс
  `94d055a`, reviewer APPROVED/merged). Пин `rust-toolchain.toml` (`channel = "1.97.0"`, components rustfmt+clippy)
  + `ci.yml` `dtolnay/rust-toolchain@stable → @1.97.0` (обе job'ы) ⇒ ЕДИНАЯ версия local==CI. gates.md §3 (RN-17)
  расширен: verify ОБЯЗАН гонять CI-точную команду (`clippy --all-targets --all-features -- -D warnings`) НА
  ТОЙ ЖЕ версии toolchain; бамп версии — ОДновременно в обоих местах. **Доказано, а не заявлено:** (1) reviewer
  в worktree — `rust-toolchain.toml` авто-разрешил local в `rustc 1.97.0 / clippy 0.1.97` (был 1.94.1), CI-точный
  clippy → exit 0; (2) CI на merge (run 30087853334) **success** с `@1.97.0` — пин работает в CI-окружении;
  (3) **`Delivery gate`** (сборка прод-образа recorder+journal-retention) green с `rust-toolchain.toml` в контексте
  ⇒ Docker-билд honors пин 1.97.0 (local==CI==Docker-build). Ниже — исходное описание.
  Локальный toolchain (reviewer/tester/verify) — **clippy/rustc 1.94.1**, CI — **rust-1.97.0**. Lint
  `clippy::unnecessary_sort_by` затянулся между версиями → локальный `cargo clippy -- -D warnings` СЛАБЕЕ CI.
  Следствие: M-23 (`8613066`) прошёл `verify_M-23.sh` + tester + reviewer локально (все на 1.94) с 2 clippy-ошибками,
  которые CI (1.97) отреджектил на merge (`gateway/src/lib.rs:784,791`). Это класс **«green local ≠ green CI»**: три
  локальных `-D warnings`-гейта дали false-green, потому что RN-17 («verify ⊇ терминальные CI-гейты») подразумевал
  ОДИНАКОВЫЙ toolchain, а он дрейфанул. Отдельно verify_M-23.sh не CI-эквивалентен и по флагам: CI гоняет
  `clippy --all-targets --all-features`, verify — `--workspace --all-targets` (без `--all-features`).
  **Обход в M-23:** engine-dev fix-forward `94230c4` (2 строки), reviewer перепроверил CI-эквивалентным
  `cargo +1.97.0 clippy --workspace --all-targets --all-features -- -D warnings` (exit 0) перед reland.
  **Durable-фикс (зона architect, процессный/CI-слой):** (а) закрепить toolchain — `rust-toolchain.toml` с CI-версией,
  чтобы локальный clippy == CI бит-в-бит; (б) унифицировать verify_*.sh clippy-инвокейшн с CI (`--all-features`).
  Пока не сделано — reviewer ОБЯЗАН на PR-гейте гонять clippy CI-версией (или не считать локальный `-D warnings`
  достаточным до зелёного CI). Severity: **MAJOR** (скрытый gate-байпас: локальные гейты не эквивалентны CI —
  прошёл 3 гейта, пойман только на merge; на будущих milestone'ах повторится с любым новым lint).
- **TD-031** `segment-provenance-constant-in-container-rollback-isolation-void` (найдено reviewer'ом на
  §8 M-18, 2026-07-21; **BLOCKING close-out M-18**).
  **✅ CLOSED 2026-07-21 (merge `7a237f7`, reviewer APPROVED; фикс — МАШИННАЯ изоляция по SCHEMA-ЭПОХЕ,
  доказано ЖИВЫМ §8, не тестами).** Фикс (не provenance-заплатка, а корень): `SCHEMA_VERSION` 2→3 +
  `decide_open_segment` reuse требует `header.schema_version == contracts::SCHEMA_VERSION` (engine-dev
  `c005c83`, +9/−1). `SCHEMA_VERSION` — compile-time константа, вкомпилённая в бинарь; не читается из
  git/env/fs/часов рантайме → НЕ деградирует в no-git контейнере (в отличие от provenance). risk-critic
  C-018 **rev4 PASS** (`9d2eefc`, prototype-verified анти-плацебо: schema-клауза снята → RED падает).
  **§8 HARD-CHECK N2 на VPS (прод HEAD `7a237f7`, CI+Deploy success):** активный сегмент — **`segment-57`
  с schema_version=3** (header byte[12]=`03`), первое живое L2Delta после fix-деплоя ушло в НЕГО, а НЕ в
  schema-2 сегмент; fix-бинарь при старте увидел активный `segment-56` (schema-2 header) → `2==3` false →
  открыл НОВЫЙ `segment-57` (schema-3) — ровно та машинная изоляция, которой provenance не дал. Метрики
  (nsenter в netns → `127.0.0.1:9101/metrics`): `md_events_total{kind=l2delta,venue=binance,BTCUSDT}=1789`
  + `{binance_futures,BTCUSDT}=1754`; **non-BTC L2Delta отсутствует** (scope (а)); recorder healthy,
  `seq_gaps=0`, `next_seq` монотонен, `writable=true`, 0 panic/ERROR/backstop; write-rate ≈ 3.8 GB/сут
  (в §8 BTC-only бюджете). **⚠ Forensic-уточнение к пунктам 2 ниже:** СМЕШАННЫХ (schema-2 + variant-6)
  сегментов ДВА — `55` И `56`: pre-fix бинарь (`ce122d1`, schema-2) продолжал капчить L2Delta и ротировал
  55→56 (закрыт 14:10), пока фикс не деплойнулся (~15:35). RFC §10 называет только 55 — фактический
  tainted-набор `{55,56}`, оба schema-2 с variant-6 в хвосте; изоляция держится с `57` вперёд.
  Provenance-константа как таковая НЕ исправлена (сегменты одной schema-эпохи по-прежнему
  неразличимы по билду) — вынесено в отдельный follow-up (см. `provenance-forensics` ниже). Ниже —
  исходное описание дефекта (сохранено для аудита). **Симптом:** после деплоя M-18 (`ce122d1`) первое
  живое `MdPayload::L2Delta` (variant-6) ушло НЕ в новый сегмент, а в **pre-M18 активный
  `segment-00000055.jrnl`** (создан 12:37 pre-M18 бинарём `fb66b52`, ДО деплоя 12:44). Это провал task 6
  acceptance («первое BTC L2Delta ушло в НОВЫЙ M-18-provenance сегмент») и **C-018 merge-condition 2**
  (условие, гейтившее risk-critic PASS). **Корень (reviewer описал; architect проектирует фикс —
  gates.md §4):** `crates/recorder/src/main.rs:448` строит provenance через `git_short_sha()` =
  `git rev-parse --short HEAD` В РАНТАЙМЕ, но runtime-контейнер НЕ содержит git/`.git` (проверено:
  `docker exec … command -v git` → `NO-GIT`; `ls /.git /app/.git` → отсутствует) → `git_short_sha()`
  возвращает `None` → provenance = КОНСТАНТА `recorder v0.0.0 (git:no-git-info)` на КАЖДОМ деплое
  (лог recorder'а на старте: `provenance=recorder v0.0.0 (git:no-git-info)`). Поэтому
  `decide_open_segment` (`segments.rs:1152`) видит `header.provenance == cfg.provenance` → **REUSE**
  активного сегмента вместо открытия нового. Провенанс НИКОГДА не меняется между деплоями → сегменты
  катятся ТОЛЬКО по размеру (1 GiB), а не на схема-forward деплое. **Следствия:**
  1. **Структурная гарантия изоляции C-018 R1 ВОИД В ПРОДЕ.** RED `red_l2delta_rollback_boundary`
     зелёный ТОЛЬКО потому, что фикстура ЗАДАЁТ разный `provenance`; прод (нет git в контейнере) даёт
     константу → precondition теста в проде НЕ выполняется. Класс TD-011/TD-014: unit-green ≠ live-поведение.
  2. **Сегмент 55 — СМЕШАННЫЙ** (pre-M18 варианты 0..5 в начале + post-M18 variant-6 в хвосте) ⇒ чистый
     quarantine «file-move целого сегмента» из runbook §5.1 больше невозможен без потери pre-M18 данных.
  3. **Latent rollback-hazard (C-018 R1) снова ЖИВОЙ:** pre-M18 бинарь против сегмента 55 → `scan_tail`
     не декодит variant-6 → риск тихого seq-reuse. (Смягчение: auto-rollback `deploy.yml` теперь целится
     в предыдущий УСПЕШНЫЙ SHA = `ce122d1`, уже M-18-aware, декодит variant-6 → немедленный авто-триггер
     низкий; но любой ручной откат за M-18 или будущий schema-forward вариант несёт тот же дефект.)
  **Прод СЕЙЧАС здоров и данные безопасны** (recorder healthy, `seq_gaps=0`, `writable=true`, капча
  L2Delta работает, BTC-only соблюдён) — это НЕ активная порча, а латентный rollback-риск + невыполненная
  структурная гарантия. **Revert НЕ сделан** (fix-forward per runbook §5.1: деплой pre-M18 бинаря против
  журнала с variant-6 в активном сегменте — ровно запрещённый hazard; revert опаснее fix-forward).
  **Нужно (architect, RED-first фикс-forward, прод-масштаб оракул per testing.md):** provenance/эпоха
  ОБЯЗАНА нести дискриминатор, реально меняющийся при schema-forward деплое НЕЗАВИСИМО от наличия git в
  рантайме — напр. git-sha вкомпилён на СБОРКЕ (`build.rs`/`vergen`/build-arg → `env!`), а не читается
  рантаймом; ЛИБО `decide_open_segment` форсит новый сегмент, когда набор декодируемых бинарём вариантов
  превосходит объявленный в активном сегменте; и/или машинный барьер TD-029 (startup schema-guard),
  который делает hazard громким независимо от provenance. Оракул обязан ПАДАТЬ на текущем прод-режиме
  (provenance-константа), а не только на фикстуре с разным provenance. Severity: **MAJOR** (sacred
  journal-integrity / rollback-safety; MD-only, путь к деньгам не тронут; блокирует close-out M-18).
- **TD-032** `provenance-constant-in-container-segments-not-build-distinguishable` (заведено reviewer'ом
  на close-out M-18 как C-018 rev4 merge-condition 2 / follow-up к TD-031, 2026-07-21). TD-031 закрыл
  СМЕШЕНИЕ ЭПОХ (schema-2 vs schema-3) машинным schema-гейтом, но НЕ исправил сам корень «provenance =
  константа в no-git контейнере»: recorder строит provenance через `git rev-parse` В РАНТАЙМЕ
  (`crates/recorder/src/main.rs`), контейнер без git → `recorder v0.0.0 (git:no-git-info)` на ВСЕХ
  деплоях. Следствие (в пределах ОДНОЙ schema-эпохи): два разных билда, эмитирующих тот же набор
  вариантов, дают ИДЕНТИЧНЫЙ provenance → сегменты неразличимы по билду, `decide_open_segment` reuse'ит
  через рестарт/деплой одной эпохи (что для no-churn желательно, но forensically сегменты одного билда и
  соседнего неотличимы). Нужно: git-sha вкомпилён на СБОРКЕ (`build.rs`/`vergen`/build-arg `GIT_SHA` →
  `env!`), а не читается рантаймом → provenance реально меняется по билду, сегменты forensically
  различимы, и defense-in-depth к schema-гейту. **Отдельный journal-hardening milestone, НЕ M-18**
  (зона: architect спека/RED — прод-масштаб оракул, ПАДАЮЩИЙ на рантайм-git режиме, per testing.md;
  engine-dev/architect impl). Severity: MINOR (schema-гейт TD-031 уже закрыл опасное смешение эпох;
  это forensic-точность + defense-in-depth, MD-only, не путь к деньгам).
- **TD-033** `emitted-variant-must-bump-schema-version-no-machine-enforcement` (заведено reviewer'ом на
  close-out M-18 из C-018 rev4 N1, 2026-07-21). Машинная изоляция сегментов по schema-эпохе (TD-031)
  держится ДИСЦИПЛИНОЙ «новый ЭМИТИРУЕМЫЙ вариант `EventKind`/`MdPayload` ⇒ bump `SCHEMA_VERSION`»
  (CT-RFC-04 §3). Это задокументированное standing-правило (как замороженные дискриминанты), но у него
  НЕТ машинного энфорсмента: будущий вариант, добавленный БЕЗ bump'а, снова откроет смешение эпох
  (schema-N сегмент получит variant, которого его эпоха не декларировала). Нужно: сделать это ЯВНЫМ
  пунктом contract-RFC гейта (reviewer Block-C проверяет: новый эмитируемый вариант в `crates/contracts`
  ⇒ соответствующий bump `SCHEMA_VERSION` в том же RFC) и/или grep-канарейка в verify, сверяющая число
  эмитируемых вариантов с `SCHEMA_VERSION`. **Зона: architect (процессный/contract-RFC слой).** Severity:
  MINOR (латентный процессный риск; активной порчи нет, срабатывает только при будущем варианте без bump).
- **TD-034** `volume-profile-bins-export-i64-truncates-i128-accumulator` (заведено reviewer'ом на merge M-24,
  2026-07-23). VP-аккумулятор в gateway держит объём в **i128** (`BTreeMap<session_id, BTreeMap<price_e8, i128>>`)
  — верно, детерминизм без f64. Но экспортное поле `VolumeProfileRow.bins: Vec<(i64, i64)>` и `va_pct_e8` кастуют
  `v as i64` в `compute_vp_row`, а `merge_volume_profile` реконструирует гистограммы ИЗ этих i64-bins → при
  патологическом переполнении (Σ size_e8 на ОДНОЙ цене за ОДНУ сессию > `i64::MAX`) `as i64` тихо усекает, и
  merge понесёт усечённое значение. POC/VA внутри считаются на i128 (точны), усечение только на границе экспорта.
  **На практике не достижимо:** `i64::MAX / 1e8 ≈ 9.2e10` единиц объёма на одной цене за сутки (для BTC — ~92 млрд
  BTC), поэтому это MVP-1-долг, явно принятый в milestone-хэндоффе. Известный класс: контракт-форма беднее
  внутреннего аккумулятора. Если появится инструмент с таким объёмом (или мелкий тик + огромный notional) —
  промоутить `bins` объём в i128 или saturating-cast с флагом. **Зона: architect (контракт-форма `VolumeProfileRow`)
  при появлении реального консюмера.** Severity: NOTE (недостижимо на текущих инструментах; POC/VA не искажены —
  усечение только в экспортном bins-объёме; MD-only, не путь к деньгам).
- **TD-029** `recorder-startup-schema-guard-missing` (заведено reviewer'ом на merge M-18 как
  merge-condition 1 из risk-critic C-018 rev3 — «TD-a»). Recorder при старте НЕ проверяет, что
  активный/хвостовой сегмент несёт ТОЛЬКО декодируемые этим бинарём варианты `EventKind`/`MdPayload`.
  Следствие (sacred data-path): при откате на бинарь СТАРШЕ schema-forward деплоя (напр. pre-M18, не
  знающий `MdPayload::L2Delta` дискриминант 6) против ТОГО ЖЕ персистентного журнала hot-путь
  `scan_tail_for_last_seq` пропускает недекодируемый фрейм → `next_seq` может недосчитаться → **тихий
  SEQ REUSE** (порча тотального порядка, DET-I-1) — ХУЖЕ громкого краха. В M-18 этот сценарий закрыт
  СТРУКТУРНО (provenance-изоляция: новый git-sha → `decide_open_segment` открывает НОВЫЙ сегмент,
  L2Delta никогда не дописывается в pre-M18 сегмент; RED `red_l2delta_rollback_boundary` GREEN,
  анти-плацебо доказан risk-critic) + ПРОЦЕДУРНО (runbook `ops.md` §5.1). TD-029 добавляет МАШИННЫЙ
  барьер: recorder ГРОМКО падает на старте, если активный сегмент несёт события, которые бинарь не
  умеет декодить (schema-version-aware старт) — превращает тихий seq-reuse в громкий отказ. **Отдельный
  journal-hardening milestone, НЕ M-18** (зона: architect — спека/RED; engine-dev — impl). Severity:
  MINOR (триггер — операторский откат ЗА schema-forward деплой, узкий и ручной; в M-18 уже закрыт
  структурно+процедурно; MD-only blast radius — не путь к деньгам).
- **TD-030** `reader-first_seq-guard-missing` (заведено reviewer'ом на merge M-18 как merge-condition 1
  из risk-critic C-018 rev3 — «TD-b»). `read_all`/`stream` сшивают сегменты по ИНДЕКСУ файла
  (`iter_segments_sorted` + blind `extend`, `segments.rs:586/846-862`) БЕЗ проверки монотонности
  `first_seq`. Следствие: ошибочный re-stitch терминального архива (quarantined post-M18 сегмент) в
  живой журнал — ТИХИЙ беспорядок seq (`[0,1,2,3,4,7,5,6]`, probe risk-critic C-018 rev2), а не отказ.
  Сейчас правило «архив не re-stitch'ится в live» — операторская дисциплина (runbook §5.1), НЕ машинный
  барьер. TD-030: fail-closed `Err` на немонотонном `first_seq` в `read_all`/`stream` → делает правило
  ПРИНУДИТЕЛЬНЫМ. **⚠ Класс TD-011:** наивный монотонный guard СПОТКНЁТСЯ на legacy-сегментах с
  `first_seq=0` (сентинел, `segments.rs:509-511`) → форсить fail-closed в ГОРЯЧИЙ прод read-path под
  таймдавлением M-18 рискует ХУДШЕЙ прод-регрессией, чем закрываемая дисциплинарная дыра. Поэтому —
  **отдельный journal-hardening milestone, НЕ M-18** (граница reviewer↔architect gates.md §4: дефект
  описан, защита проектируется RED-first в своём milestone). Severity: MINOR (defense-in-depth против
  операторской ошибки; сам триггер требует нарушения явного письменного запрета из runbook).
- **TD-028** `export-io-ram-linear-in-journal-size` (замечено reviewer'ом на PR-гейте M-17, 2026-07-21;
  флаг research-dev). `crates/research-cli/src/export_io.rs::export_to_dir` собирает trades/snapshots в
  `HashMap<(Venue, String), InstrumentBucket>` за ОДИН проход `journal::stream`, затем сериализует
  per-instrument. Стрим сам bounded-memory (per-сегмент), но БУФЕРЫ (`Vec<TradeRow>`/`Vec<(i64,OrderBook)>`)
  копятся в RAM за всю выборку → **память линейна по объёму журнала**. На прод-масштабе (>10⁷ событий,
  15+ GB журнал) экспорт всей истории может не влезть в RAM — класс TD-011 этажом выше (research-путь, не
  прод-сбор, поэтому Severity ниже). НЕ блокер M-17 (приёмка = корректность+детерминизм, оба GREEN; экспорт
  окна/инструмента текущего масштаба работает). **Нужно для прод-M-19** (фронт против полной истории):
  chunked/streaming-write (per-(venue,symbol) файл закрывать по мере прохода, или time-window партиции) —
  зона research-dev, при подключении M-19 к реальному объёму. Severity: **NOTE** (research-инструмент, не
  24/7 прод; данные не теряются — экспорт либо отработает, либо честно OOM'нет, журнал цел).
- **TD-027** `ops-metrics-declared-and-cataloged-but-not-wired-to-emission` — **✅ CLOSED task 4C
  (`ac645ac`, reviewer §8 PROD GREEN + APPROVED 2026-07-20).** Фикс: OPS-I-10 «объявлена ⟹ эмитится»
  — продюсер-сеймы `emit_post_append` (writer), `run_books_feeder`, `sample_rss`/`sample_md_age`
  (sampler в main), все вызваны в живом `main.rs`. Sacred RED `red_metrics_emission.rs` (6 critic
  re-audit'ов: label-aware value-ассерты, dead-zero, dimension/value-collapse, kind-aware, RssAnon≠VmRSS)
  прогоняет РЕАЛЬНЫЕ продюсеры и ассертит SAMPLE (не registry-only). verify-гейт OPS-I-10 (покрытие
  §3-карты + live-wiring канарейка). **§8 PROD (VPS `ac645ac`) ДОКАЗАЛ живые SAMPLE'ы** (не тестами —
  scrape /metrics через busybox-sidecar): `journal_bytes_written_total=15245` (TD-011 P0 liveness жив),
  `journal_seq_current=51923737`, `journal_segment_index=49`, `journal_disk_free_bytes=103.6G`,
  `md_events_total{venue,symbol,kind}` живой kind-aware (trade 8124/5414, l2snapshot, funding,
  open_interest — TD-014 жив), `md_event_age_ms{venue}` 83/992/77, `book_levels{venue,symbol,side}`
  живой per-серия (TD-016 жив), `recorder_rss_anon_bytes=17506304` (RssAnon, TD-016 P1 жив). Гейты
  reviewer'ом независимо: workspace **282/0**, red_metrics_emission 5/5, clippy 0, fmt clean,
  `verify_M-09.sh` **PASS (20)**; scope dev-коммита ⊂ `recorder/src/{lib,main,metric_emit}.rs`; critic
  C-014 re-audit #6 APPROVE. Алерты 3 формирующих инцидентов (TD-011/014/016) теперь ссылаются на
  ЖИВЫЕ метрики. **Остаточные NOTE:**
  (1) **✅ DONE task 4D (`f442c96`/merge `83c340c`, reviewer §8 PROD GREEN 2026-07-20):** метрика
  переименована `journal_bytes_written_total → journal_frames_written_total` (честное имя — счётчик
  КАДРОВ, +1/append; точный байт-счётчик потребовал бы правки sacred `Journal::append` — out of scope).
  Чистый rename: 5 файлов (ops/{alerts,metrics}, recorder/{lib,metric_emit}, deploy/alerts/ops.rules.yml),
  только строковый литерал + комментарии, поведение идентично; TD-011 PromQL перерендерена
  (`rate(journal_frames_written_total[1m]) == 0`, yml == renderer, drift 0). Гейты reviewer'ом: workspace
  282/0, clippy 0, verify PASS (21). **§8 PROD (`83c340c`): `journal_frames_written_total=4099` растёт,
  старое имя ОТСУТСТВУЕТ (grep пусто), healthy restarts=0.** Sacred (oracle `red_metrics_emission.rs`)
  обновлён architect'ом (`028fe08`), не dev. critic C-015 re-audit APPROVE.
  (2) **OPEN (до task 3):** `journal_seq_gaps_total` — БЕЗ writer-продюсера (в append-потоке рекордера seq монотонен по
  построению → естественного gap-триггера нет; классифицирована как event/elsewhere). Правило алерта
  **OPS-GAP** ссылается на неё → на writer-пути оно НИКОГДА не сработает. Gap реально детектируется
  ТОЛЬКО на READ/replay-пути (`read_all`/`stream` через границы сегментов) — нужен либо продюсер там,
  либо пересмотр правила OPS-GAP (зона architect). Ниже — исходное описание (для истории).
  **Класс TD-011: зелёные гейты (ops 52/52, verify
  PASS, паритет GREEN) маскируют нерабочий предохранитель.** `/metrics` живёт и отдаёт 15 объявленных
  семейств (HELP/TYPE на все), НО живые SAMPLES на проде есть ТОЛЬКО у 2: `book_divergence_bps`
  (sink, 4 серии non-zero) и `venue_http_status_total` (venue recon). Остальные **13 объявлены в
  реестре `METRICS` + зацитированы в каталоге правил `ops::alerts`, но НИКОГДА не инкрементируются**
  в рантайме (grep call-site пуст): `journal_bytes_written_total`, `journal_seq_current`,
  `journal_seq_gaps_total`, `journal_segment_index`, `journal_disk_free_bytes`,
  `journal_write_errors_total`, `md_events_total`, `md_event_age_ms`, `venue_ws_reconnects_total`,
  `book_levels`, `recorder_rss_anon_bytes`, `book_resync_total` (0 — корректно, ресинков не было),
  `backup_restore_drill_ok` (task 3).
  **СЛЕДСТВИЕ (важное — цель milestone'а):** правила алертов для ТРЁХ ФОРМИРУЮЩИХ M-09 инцидентов
  ссылаются на МЁРТВЫЕ метрики → эти алерты НИКОГДА не сработают:
  **TD-011 (P0 «recorder жив, но не пишет» — инцидент №1 milestone'а) → `journal_bytes_written_total`
  (не wired); TD-014 (P1) → `md_events_total` (не wired); TD-016 (P1) → `recorder_rss_anon_bytes`
  (не wired); OPS-GAP → `journal_seq_gaps_total` (не wired).** «Система сама сообщает о тихой
  деградации» для этих классов НЕ достигнута.
  **Почему паритет это НЕ ловит:** OPS-I-5 — РЕЕСТРОВО-СТАТИЧЕСКИЙ (имя ∈ `METRICS` const ↔ правило ↔
  §7.1). Он проверяет согласованность ИМЁН, НЕ рантайм-эмиссию. Зелёный паритет даёт ЛОЖНУЮ уверенность,
  что алерты подкреплены живыми метриками.
  **Почему НЕ в task 4 (не дефект task 4, а его граница):** task-4 carve-out ЯВНО запрещает трогать
  journal-write путь (`JR-I-1`) и recorder hot-loop; эмиссия `journal_*` требует именно этого пути,
  `recorder_rss_anon_bytes` — sampler-таск, `md_events_total`/`book_levels` — recorder-цикл. Это
  ОТДЕЛЬНАЯ работа с собственным carve-out. Task 4 (эндпоинт + каталог + паритет) СВОЮ приёмку выполнил
  (см. PROJECT-STATE) — долг НЕ блокирует APPROVED task 4, но БЛОКИРУЕТ реальную наблюдаемость.
  **Нужно (architect RED-first, следующая задача — task 4C / метрик-эмиссия):** развести каждую
  объявленную метрику до РЕАЛЬНОГО инкремента (journal append → bytes/seq/gaps/segment/disk/write_errors;
  MD-событие → md_events_total/age; venue reconnect → ws_reconnects; book-maintenance → book_levels;
  rss-sampler → recorder_rss_anon_bytes) + RED/§8, который ассертит РАНТАЙМ-эмиссию (не только реестр).
  **ДО провижининга Alertmanager (§O, founder ★) эти метрики ОБЯЗАНЫ быть живыми — иначе алерты театр.**
  Severity: **MAJOR** (цель milestone'а для 3 формирующих инцидентов не достигнута; регрессии нет —
  эндпоинт+каталог net-new и корректны).
- **TD-025** `recon-runtime-floods-ReconDivergence-on-healthy-prod` (замечено reviewer'ом на §8
  eyes-on M-09 task 2, прод `b1adec0`, 2026-07-18). **Класс TD-011: юнит-гейты ЗЕЛЁНЫЕ (ops 33/33,
  workspace 256/0, verify PASS), а прод пишет ложь под healthy-статусом.** Recon-runtime смержен
  в main (`b1adec0`) и задеплоен; на ЗДОРОВОМ рынке recon **флудит** `Sys(ReconDivergence)` в
  durable-журнал ~1/мин. Замер (декодер `journal::read_all` активного сегмента, все post-deploy Sys):
  - **(A) стартовый транзиент — ✅ RESOLVED (merge `e9fc258`, §8 re-run PASS 2026-07-18).** Был: 4
    события `best_diverged=true div_bps=10000 Resynced` (по одному на venue×symbol) — orchestrator
    сравнивал REST-reference с ПУСТОЙ local-книгой (fetcher делает ПЕРВЫЙ fetch немедленно на старте,
    ДО первого L2Snapshot feeder'а). Фикс: self-seeding `ReconDetector` (`seeded: bool`, early-return
    ДО reconcile и ДО push в окно; `seeded=true` на первой непустой local) — `crates/ops/src/recon.rs`.
    Sacred RED `red_recon_window.rs` 9a/9b/9c, critic C-012 re-audit `5ec8094` APPROVE. **§8 LIVE
    (reviewer): healthy ~8 мин → 0 `Sys(ReconDivergence)` (стартовый флуд УСТРАНЁН); injection
    (спот-WS заморожен, REST жив) → 6× `best_diverged=true` эмит (gate НЕ over-suppress'ит порчу).**
  - **(B) оконный флуд:** 12+ событий `best_diverged=false div_bps=41..1129 Resynced` по всем 4
    символам, ~1/мин. Оконное знаковое среднее near-touch ОБЪЁМА НЕ сходится к 0 на живом рынке:
    остаточный churn 41..1129 bps ≫ ε_prod=5, часть ≫ ε_max=50. **Третий §8-провал того же класса**
    (near-touch объём: local WS-книга момента T1 vs async REST момента T2), уже ПОСЛЕ windowed-
    редизайна `ops.md §4.3`. Гипотезы (диагностика — architect/founder): (1) реальная cadence ~1/мин
    (budget-gated), а дизайн полагал 5 мин → K=12 покрывает КОРРЕЛИРОВАННЫЕ выборки → mean не гасится;
    (2) K=12 в принципе мал для гашения prod-churn (дизайн: ПЕРВИЧНЫЙ рычаг — ДЛИНА окна, не порог);
    (3) систематический bias между WS-реконструкцией и REST-снапшотом near-touch объёма (не zero-mean
    churn — тогда никакое K не поможет, и near-touch объёмный recon через REST нежизнеспособен).
    values 860/1129 ≫ ε_max=50 → порогом (fail-closed ≤50) не подавляются в принципе.
  **Severity: MAJOR — ✅ CLOSED 2026-07-18 (обе ветки).** **(A) seed-gate — RESOLVED** (`e9fc258`,
  §8 re-run PASS). **(B) оконный объёмный флуд — ✅ CLOSED B2** (`4939d8f`, founder ★ §4.3.2, reviewer
  §8 PROD GREEN + APPROVED). Диагноз B подтверждён: систематический WS(T1)-vs-REST(T2) near-touch
  объёмный bias (НЕ zero-mean churn → усреднение окна гасит дисперсию, но не bias; часть ≫ ε_max=50 →
  порогом непобедимы), в т.ч. на нетронутом BinanceFutures. B1 (калибровка K/cadence) отклонён (не
  лечит bias, упирается в fail-closed ε_max=50). **★ РЕШЕНИЕ B2:** рантайм-эмиссию объёмной сверки
  УБРАТЬ (единственный флудивший путь удалён, а не подавлен порогом); рантайм-alert ⟺
  `best_price_diverged` + seed-gate; объём → офлайн-трек research-dev (BACKLOG «M-09 хвост», НЕ блокер).
  T1 `ReconAudit` НЕ меняется → CT-RFC НЕ нужен. **§8 PROD ДОКАЗАЛ УДАЛЕНИЕ ФЛУДА** (не тестами —
  живым замером журнала на VPS, декодер `journal::stream` bounded-memory):
  - BASELINE (window-impl `e3491d9`, тот же журнал ДО деплоя): `RECON_DIVERGENCE=1414 best_true=0
    best_false=1414 div=[5..816]` — флуд B живой.
  - post-B2 healthy ~9 мин (~13k событий): **`RECON_DIVERGENCE=0`** — флуд удалён; panic/ERROR=0.
  - injection (спот-WS 9443 заморожен, спот-REST жив, ETH дрейф 8.5 bps>5, книга доказано stale):
    **4× `best_diverged=true` div 725..2597 Resynced** — best-путь §8-жив (не always-silent). Инъекция
    обратима, спот-WS восстановлен, контейнер healthy весь прогон.
  Анти-плацебо reviewer'ом независимо: против pre-B2 src (`a418968`) 3 B2-silent оракула ПАДАЮТ, против
  B2 — ops 33/33. Оконные объёмные оракулы (`red_recon_window` 1–7) СНЯТЫ (файл → `red_recon_runtime.rs`).
  **Остаточная задача — НЕ долг рантайма, а офлайн-follow-up:** объёмная near-touch сверка над записанной
  книгой (research-dev, `research/data-quality/`), необязательная. **Прод B2 инертен по объёму
  (0 ложных audit), best-путь работает.**
  **⚠ NUMBERING COLLISION:** этот номер `TD-025` (recon) КОНФЛИКТУЕТ со вторым `TD-025`
  (`red-prod-migration-fails-on-full-host-disk`, M-08 task 20, ниже в этом файле) — коллизия предшествует
  B2, требует отдельного renumber-прохода (не тронуто здесь во избежание рассинхрона кросс-ссылок).
- **TD-016** `recorder-memory-drift-under-healthy-status` (замечено reviewer'ом на §8 eyes-on
  M-07, 2026-07-13). **Класс TD-011: тихая деградация под ЗЕЛЁНЫМ статусом** — контейнер
  `healthy`, heartbeat свежий, журнал растёт, `restarts=0`, и при этом RSS монотонно ползёт.
  Замеры по ОДНОМУ И ТОМУ ЖЕ контейнеру (`started=2026-07-13T15:48:58Z`, `restarts=0`, т.е.
  без перезапуска, рост не объясняется рестартом):
  | аптайм | MEM |
  |---|---|
  | ~1 мин | 8.40 MiB |
  | ~5 мин | 8.86 MiB |
  | ~2 часа | 21.63 MiB |
  | ~5 часов (пред-деплойный контейнер) | 48.27 MiB |
  Историческая норма (PROJECT-STATE, M-05/M-06 eyes-on) — **5–9 MiB**. Тренд ≈ **+6.5 MiB/час**,
  пред-деплойная точка (48 MiB @ ~5ч) ложится на ту же кривую → это не шум и не разовый выброс.
  **НЕ вызвано M-07** (мозг стратегии структурно недостижим из recorder'а: его дерево
  зависимостей не содержит `alpha`/`portfolio`/`strategy`/`sim`) — долг предшествует M-07 и
  замечен только потому, что я снял baseline ДО деплоя. Гипотезы (не проверены, диагностика —
  не зона reviewer'а): рост, привязанный к размеру сегмента журнала (сейчас **8.8 GiB**);
  накопление в буфере писателя/`mpsc`; удержание в venue-адаптерах (depth-книги futures,
  ср. TD-012 `limit=1000` reconcile). Хост — 7.5 GiB RAM, немедленного OOM нет, но траектория
  ведёт туда, а healthcheck этого НЕ поймает (ровно урок TD-011: «зелёные юниты + Deploy-success
  ≠ рабочий прод»). **Нужно (architect → RED-first):** оракул границы РЕСУРСА, а не только
  корректности — прогон writer'а на прод-масштабе с counting-allocator/RSS-бюджетом, падающий
  на текущей реализации (паттерн `crates/journal/tests/red_open_bounded.rs`, TD-011). Плюс
  наблюдаемость: писать RSS в heartbeat/лог, чтобы дрейф был виден без ssh. Severity: **MAJOR**
  (24/7 прод-сбор данных; отказ recorder'а = потеря рыночных данных, которые не восстановить
  задним числом — данные невоспроизводимы, в отличие от кода).
  **СТАТУС 2026-07-14 (M-08 задачи 9/9b, merge `1123b13`): фикс НА `main`, на проде НЕ ПОДТВЕРЖДЁН.**
  Корень найден по коду (`venue-binance::apply_diff_to_book`: уровень, из которого цена ушла,
  `size=0` больше не получает и живёт вечно). **v1-эвикция (кап 5000 + side-filter по mid ДИФФА)
  отреджекчена reviewer'ом на PR-гейте:** на асимметричном диффе (лучший bid в окне не менялся —
  штатная ситуация) она стирала ЖИВЫЕ уровни, включая best bid → тихая порча `L2Snapshot` в
  журнале при зелёных RSS/health (класс TD-011). v2 (`421d5b6`): side-filter снят (уровень удаляет
  ТОЛЬКО `size==0`); эвикция — по расстоянию от mid КНИГИ за пределами окна эмиссии
  (`MAX_REL_DIST` ±60%), т.е. режется ровно то, что НИКОГДА не эмитится и ни в один расчёт не
  входит; `BACKSTOP_LEVELS_PER_SIDE = 50_000` — аварийный кап от OOM (эвиктит самое дальнее,
  `tracing::warn`); наблюдаемость D — `tracing::info!(symbol, bids, asks, "book levels")` ≥1/мин.
  Анти-плацебо доказан reviewer'ом независимо: `c1_asymmetric_diff_must_not_delete_live_levels` и
  `td016_evicts_only_levels_outside_emission_window` **FAIL против v1-impl**, GREEN против v2.
  **§8 ВЫПОЛНЕН 2026-07-14 (прод `b7721d1`, 4.2 ч наблюдения, метрика D). ДОЛГ ОСТАЁТСЯ OPEN —
  фикс НЕ удерживает книгу; попутно опровергнута исходная метрика:**
  1. **Прежняя метрика («+8 MiB/час», по `docker stats`) БЫЛА ЗАГРЯЗНЕНА.** `docker stats`/
     `memory.current` cgroup включают **page cache** файла журнала, а recorder пишет ~30 MB/мин.
     Замер cgroup `memory.stat` на проде: `anon 13 492 224` (куча) против `file 163 508 224`
     (кэш журнала, reclaimable). Настоящий рост кучи — **≈ +1 MiB/час** (`RssAnon` 13 176 →
     16 228 KB за 4 ч), а не +8. Историческая точка «139 MiB @17ч» и оракульная «48 MiB @5ч»
     мерились ТЕМ ЖЕ загрязнённым способом → величина TD-016 была завышена. **Мерить `RssAnon`
     (`/proc/<pid>/status`), НЕ `docker stats`** — см. TD-021.
  2. **Но лик книги РЕАЛЕН и фиксом v2 НЕ УСТРАНЁН.** Метрика D (`book levels`) за 4.2 ч:
     BTC `5072/5064 → 13840/3000`, ETH `5203/5095 → 12927/5344` — монотонный рост, ~+2000
     уровней/час на растущей стороне. Механика видна в самих числах: цена идёт вверх → bids, из-под
     которых цена ушла, `size=0` больше НЕ получают и остаются навсегда; asks тают. Причина
     неэффективности v2: окно эвикции `MAX_REL_DIST` = **±60 % от mid** при BTC ~118k$ = ±71 000 $
     — оно не эвиктит практически НИЧЕГО. Единственный реальный потолок — `BACKSTOP_LEVELS_PER_SIDE
     = 50 000` (при текущем темпе достижим за ~18 ч; на §8-окне не сработал, `backstop=0`).
  3. **Следствие для ДАННЫХ (важнее памяти):** мёртвые уровни лежат ВНУТРИ окна эмиссии ⇒ попадают
     в `L2Snapshot` и в полосы OBI 6–60 %. Дальние полосы сигнала содержат ФАНТОМНУЮ ликвидность,
     которой на бирже нет. Это не регрессия M-08 (так было и до него), но это ровно тот дефект,
     ради которого писался E7. Нужен контракт, который ограничивает книгу ТАМ, где живёт сигнал
     (кап уровней с сохранением топа / окно по дистанции, СОРАЗМЕРНОЕ полосам OBI / bucket-агрегаты
     вместо сырой книги — зона architect, RED-first).
  4. Что фикс v2 всё же дал: он **безопасен для данных** (v1 стирал живые уровни, включая best bid
     — порча журнала) и §8 подтвердил, что глубина НЕ деградировала: полосы на прогретой книге
     (4.2 ч) — `avg bid/ask buckets 1154/969` против baseline `1316/1452`, полоса 600–6000 bps
     `1115/873` против `975/845`, >6000 bps — ноль в обоих. Плюс метрика D, без которой пункты
     1–3 были бы догадками.
  Прод на §8-окне здоров: `restarts=0`, `panic/ERROR=0`, `backstop=0`, heartbeat свежий,
  `seq_gaps=0`. Revert НЕ требуется. Severity: **MAJOR** (data-quality сигнала + неограниченный
  рост до 50k/сторона).
  **ПЕРЕСПЕКА 2026-07-14 (M-08 rev6 задача 13, architect, merge `8882c1e`): приоритет РАЗВЁРНУТ —
  точность данных > экономия памяти.** После TD-021 (метрика вранула: page cache вместо кучи)
  память проблемой не является (`RssAnon` на проде после деплоя = **11 376 kB** при 7.5 GiB RAM),
  а эвикция режет уровни ВНУТРИ полос OBI 6–60 % — портит незаменимый актив ради дешёвого.
  Поэтому `BACKSTOP_LEVELS_PER_SIDE` поднят 50k → **200k** и остаётся ТОЛЬКО аварийным потолком от
  OOM (не рабочим инструментом); in-band уровни не режутся. Рабочая гипотеза architect'а: рост
  5k → 13.8k за 4 ч — вероятно **СХОДИМОСТЬ** (бутстрап REST даёт top-5000, дальние уровни книга
  узнаёт из diff-потока), а не лик; отличить может только **асимптота** метрики `book levels` и
  recon с биржей (P2.5). Baseline после деплоя `8882c1e` (книга сброшена рестартом):
  BTC `4909/5124`, ETH `5026/5064`. **Долг ОСТАЁТСЯ OPEN как НАБЛЮДЕНИЕ** (не как код-дефект):
  следующий §8 обязан снять `book levels` через 4–8 ч и сказать, вышла ли кривая на плато.
  Плато → закрыть; продолжение линейного роста → новый контракт (кап с сохранением топа /
  окно, соразмерное полосам OBI / bucket-агрегаты) — зона architect, RED-first.
  **Новый baseline после деплоя `cf53e81` (M-09 task 1 CT-RFC-03, рестарт 2026-07-16T10:01:59Z,
  книга сброшена):** BTC `5030/5027`, ETH `5033/5028`, `RssAnon 11 828 kB` — точка отсчёта для
  следующего замера асимптоты (uptime ~1 мин на момент §8, не показатель плато). CT-RFC-03 —
  контракт-only, инертен, лика книги не касается; наблюдение TD-016 переносится на M-09 §8-хвост.
  **ЭЛЕВАЦИЯ 2026-07-22 (пивот P-COCKPIT, charter `cce11a5`): TD-016 повышен из «наблюдения» в
  ПРЕДУСЛОВИЕ TPP.** Виз-бэкенд (`docs/fa/viz-backend.md` §4, VB-I-5) строит TPP Bid/Ask/Delta по полосам
  3-30% на diff-книге; мёртвые/несходящиеся дальние уровни ⇒ ФАНТОМНАЯ ликвидность в band-sums. depth_probe
  (`research/data-quality/depth-probe-staleness.md`) уточнил: чистый фантом НЕ подтверждён, но `dd=100%`
  конфаундится resync-обнулением ⇒ достоверность дальних полос НЕ доказана. Планка перенесена на
  **корректность поддержки книги** (эвикция мёртвых уровней + resync-целостность), а не «вендор для глубины»
  (валидированного эталона глубже ~1.3% нет ни у кого, включая Tardis). До фикса — TPP-полосы несут
  `depth_band_provenance: diff-reconstructed, validated≤1.3%` (не выдаются за биржевой факт). Блокирует
  включение TPP-полос в export-контракт; RED-спека на venue-book (resync не роняет восстановимые дальние
  полосы в 0) — зона architect, предусловие Трека C (TPP COIN).
  **M-23 (heatmap, `94230c4`) ПОТРЕБЛЯЕТ TD-016 честно, НЕ закрывает.** Heatmap/COB оконны (`[mid·(1±W)]`,
  W=max(bands)) — дальние мёртвые уровни за окном в дисплей не эмитятся; ячейки глубже 1.3 % несут
  `depth_band_provenance="diff-reconstructed"`. Достоверность дальних ячеек внутри окна повышает ТОЛЬКО
  фикс TD-016 (эвикция + resync-целостность) — аддитивно, провенанс уже на месте (как Bookmap: показываем
  реконструкцию честно). TD-016 остаётся **OPEN**.
  **M-32 (depth-verification, merge `bb00915`, 2026-07-24) — ФАНТОМ-ПОДОЗРЕНИЕ ЭМПИРИЧЕСКИ СНЯТО для полос
  1.5-30%; TD-016 остаётся OPEN по КОРРЕКТНОСТИ/эвикции.** Прямой замер на СЫРОМ `L2Delta` (BTCUSDT, gap-free
  segment 78, 3.4 ч, 121k дельт) развёл churn от resync через sequence-gap (DV-I-3, `gaps=0/censored=0`):
  дальние уровни РЕАЛЬНО отменяются биржей — `cancel_fraction` FAR (3-30%) = **0.805** vs NEAR = **0.981**,
  order-flow `consistency_rate` = **0.950**. ⇒ дальние полосы 1.5-30% — ЖИВАЯ ликвидность, а НЕ мёртвые
  «растут-вечно» фантомы (то, что shell-notional depth_probe доказать НЕ мог). **Провенанс-путь ПРИНЯТ
  founder'ом (граница C, 2026-07-24, `research/data-quality/depth-verdict.md`):** TPP-полосы строятся на
  diff-реконструкции с `depth_band_provenance: "diff-reconstructed, validated<=1.3%"` (VB-I-5), диапазон
  1.5-60% (30-60% — provenance + follow-up live-замер). **Что M-32 НЕ закрыл (TD-016 по-прежнему OPEN):**
  верификация «измеритель не врёт» ≠ «книга поддержана корректно и bounded» — эвикция мёртвых уровней и
  resync-целостность (Track A 3б/3в) остаются за **M-31**; неограниченный рост уровней (backstop 200k)
  наблюдается отдельно. M-32 = валидация ИЗМЕРЕНИЯ, M-31 = корректность ПОДДЕРЖКИ; ортогональны. TD-016 **OPEN**.
- **TD-019** `storage-status-not-published-in-heartbeat` — **✅ CLOSED 2026-07-14** (M-08 task 12,
  `24d8e83`, merge `8882c1e`). heartbeat = JSON с состоянием; **доказано на проде**, а не тестами:
  `cat recorder.heartbeat` → `{"events":2736,"free_bytes":119134494720,"min_free_bytes":10737418240,
  "next_seq":18733828,"segment_index":2,"ts_wall_ms":1784038790063,"writable":true}`. Деградация
  диска теперь видна БЕЗ ssh. Healthcheck compose'а смотрит на mtime, не на содержимое → смена
  формата прод-безопасна (контейнер healthy, restarts=0). Анти-плацебо: `red_heartbeat_status`
  FAIL против пред-фиксного дерева (голый таймстамп не парсится как JSON). Ниже — исходное описание.
  Обещание M-08 E4: «состояние наблюдаемо через `storage_status().writable == false` (recorder
  публикует его в heartbeat → **видно без ssh**)». По факту `crates/recorder` **ни разу не зовёт**
  `journal::storage_status` (grep пуст), а `recorder.heartbeat` = **13 байт** (только ms epoch).
  Проверено на проде: `/root/jctl guard` → `writable=true free_bytes=121230888960`, т.е. API
  работает — но наружу не выведен. Сам disk-guard fail-closed РАБОТАЕТ и деградация не тихая:
  `run_writer` делает `journal.append(kind)?` → при storage-guard `Err` writer возвращает `Err` →
  recorder падает громко (контейнер unhealthy/restart-loop), а не молча теряет события. То есть
  это дыра в НАБЛЮДАЕМОСТИ, не в safety. Зона: engine-dev (recorder) по RED от architect'а.
  Severity: **MINOR** (safety не нарушен; но «увидим переполнение диска без ssh» — не выполнено).
- **TD-020** `retention-implemented-but-never-invoked` (найдено reviewer'ом на §8 M-08, 2026-07-14).
  Ретеншен есть как БИБЛИОТЕКА (`prune_segment` + `ColdCopyProof` + cold-выгрузка, RED зелёный),
  но его **никто не вызывает**: grep `prune_segment|ColdCopyProof|retention` по `crates/recorder/src`
  и `crates/*/src/bin` — **пусто**. Нет ни оператора, ни CLI, ни cron, ни конфигурации Storage Box.
  Следствие: цель M-08 «сбор не останавливается НИКОГДА» **не достигнута** — ротация лишь дробит
  диск на сегменты по 1 GiB, а суммарный рост остаётся ~2.8 GB/сут при 113 GB свободных ⇒ **~40 дней
  до disk-guard**, после чего recorder встанет (fail-closed, но встанет). §8-пункт «ретеншен —
  dry-run» выполнить было НЕЧЕМ: механизма в проде не существует. Нужен операторский путь
  (CLI/крон + конфиг Storage Box) — зона architect (спека) → engine-dev (impl).
  Severity: **MAJOR** (это ГЛАВНАЯ цель milestone'а; таймер ~40 дней уже тикает).
  **СТАТУС 2026-07-14 (M-08 rev6 task 11, merge `8882c1e`): ОПЕРАТОР НАПИСАН, НО В ПРОДЕ ЕГО НЕТ.
  ДОЛГ ОСТАЁТСЯ OPEN.** Что сделано: `journal::retention_plan/retention_execute` + бинарь
  `crates/journal/src/bin/journal-retention.rs` (DryRun по дефолту; Apply только через
  `verify_cold_copy` → `ColdCopyProof` → `prune_segment`; активный сегмент/`keep_min`/
  незадекларированный legacy не трогаются; `disk_pressure` → exit=3; сбой сверки → exit=2, сегмент
  остаётся горячим). 7 оракулов R1–R7 GREEN, анти-плацебо доказан reviewer'ом (все 7 FAIL против
  `4475bfa`). **Чего НЕ сделано — доставка, и без неё ретеншен по-прежнему НИКЕМ НЕ ВЫЗЫВАЕТСЯ
  (факты с прода `8882c1e`, проверено reviewer'ом на §8):**
  1. `Dockerfile` собирает `cargo build --release **--bin recorder**` и копирует в runtime-образ
     ТОЛЬКО `recorder` → `docker exec hft-recorder ls /usr/local/bin/` = один `recorder`.
     Бинаря `journal-retention` в проде НЕТ.
  2. На VPS нет Rust toolchain (`which cargo rustc` → пусто) — собрать на месте нельзя.
  3. Холодного хранилища нет: `/mnt/*` пуст, Hetzner Storage Box не заведён/не смонтирован.
     `--cold` указывать некуда, `verify_cold_copy` физически не на что сослаться.
  4. Cron'а нет: `/etc/cron.d/` → только `e2scrub_all`. Алерта на exit≠0 (`disk_pressure`) нет.
  ⇒ §8-пункты «первый dry-run ретеншена на проде» и «cron» **невыполнимы**; журнал (16.8 GB в трёх
  сегментах, 111 GB свободно, ~2.8 GB/сут) продолжает расти к disk-guard. **Это тот же класс
  дефекта, что исходный TD-020, этажом выше: была библиотека без оператора — стал оператор без
  доставки.** Урок процессный: «код на main» ≠ «функция существует в проде» — для ОПЕРАТОРСКОГО
  пути доказательством является запуск на VPS, а не зелёный `cargo test`.
  **Нужна задача 14** (зона: architect — спека/RED доставки; engine-dev/architect — impl):
  сборка `journal-retention` в образ (или отдельный образ/бинарь на хосте) + монтирование холодного
  хранилища + cron + алерт на exit≠0 (2 = сверка не прошла, 3 = disk_pressure). Первый прогон на
  проде — ОБЯЗАТЕЛЬНО `--mode dry-run` (дефолт уже такой), Apply — отдельной командой после
  визуального подтверждения отчёта. M-08 не закрывается до этого.
  **СТАТУС 2026-07-14 (M-08 tasks 14/15 PR-gate, stack `d43d923..91f11aa`, rollback `b43044d`):
  доставка REJECTED и откатана.** Локально delivery-оракулы были зелёные, включая deep build/run,
  но §8 на VPS доказал, что реальное cron-задание не работает: лог
  `journal-retention: неизвестный флаг --dir=/journal`, exit=1. Скрипт/compose передают
  `--flag=value`, CLI принимает только `--flag value`. D5 всё ещё недостаточен: он доказал
  устанавливаемость cron и поведение alert-обёртки со стабом `docker`, но не прогнал реальный
  binary+compose argv path. Следующая задача 14 должна либо поддержать оба синтаксиса в CLI, либо
  передавать split-аргументы в compose/cron, и обязана иметь оракул, который запускает реальное
  задание до `retention_plan`, а не только stub.
  **СТАТУС 2026-07-15 (reland `8a2e377`, reviewer §8): КОМПАКЦИОННЫЙ оператор ДОСТАВЛЕН и РЕАЛЬНО
  ВЫПОЛНЕН на проде — часть цели достигнута, но ДОЛГ ОСТАЁТСЯ OPEN.** Что сделано и доказано:
  reviewer выполнил §8-B — реальную компакцию боевого `/journal` через доставленный cron-скрипт
  `deploy/bin/journal-compaction-cron.sh` (exit=0, legacy-0 байт-в-байт цел, 5 сегментов сжаты,
  **диск +4.69 GB**, recorder healthy). Т.е. компакция «двигает диск фактом», как требовал гейт.
  **Чего ещё НЕТ (почему OPEN):** (1) **cron НЕ УСТАНОВЛЕН** на проде (`/etc/cron.d` без hft) —
  reviewer запустил скрипт ВРУЧНУЮ; без установленного cron компакция разовая, диск снова растёт
  (для durable-сдвига дедлайна нужна установка `deploy/cron.d/journal-retention` + маркер/алерт);
  (2) **TD-024** — задокументированные ad-hoc команды compose (`docker compose run --rm
  journal-compaction`) СЛОМАНЫ equals-form'ом; работает только точный cron-argv (хрупко);
  (3) **РЕТЕНШЕН** (`--mode apply`, освобождение через cold-выгрузку) по-прежнему не запускался —
  нет Storage Box (`/mnt/*` пуст, founder ★). Компакция снижает темп роста (~9× на закрытых), но
  БЕЗ ретеншена диск всё равно растёт (медленнее). Установка cron + Storage Box + фикс TD-024 —
  условие полного закрытия TD-020 и M-08.
  **СТАТУС 2026-07-15 (task 20 / rev12, `d3e7db2`, reviewer §8): CRON АКТИВИРОВАН — компакция теперь
  DURABLE (авто), но ДОЛГ OPEN до Storage Box.** Из трёх хвостов выше закрыты (1) и (2):
  • **(2) TD-024 CLOSED** (task 19) — compose-путь чинён, парсер принимает equals-форму.
  • **(1) cron УСТАНОВЛЕН и АВТО-прогон подтверждён eyes-on** (task 20): `/etc/cron.d/hft-journal-
    retention` (компакция `50 3 * * *`, ретеншен dry-run `7 4 * * *`), crond active; reviewer
    доказал АВТО-запуск (temp every-minute) — cron сам отработал: **свежий `compaction.last-success`
    2026-07-15T18:56:02Z** (позитивный heartbeat пишется, silent-absence детектируется), alert не
    взведён, legacy-0 байт-в-байт цел (`234583c8…`), recorder healthy. Тот прогон компактил 0
    (keep_raw=2 берёг единственные 2 закрытых; legacy skipped) — штатно; реальный 03:50 сожмёт по
    мере накопления закрытых сегментов (recorder закрывает ~9/сут). Disk-moving компакция через ЭТОТ
    же код-путь доказана в §8-B дважды (+4.69, +1.94 GB). Мониторинг: `*.alert` (сбой) + `*.last-success`
    freshness (>26ч = молчит) — оба маркера, D9-гейт.
  • **(3) РЕТЕНШЕН apply — по-прежнему OPEN:** нет Storage Box (`/mnt/*` пуст, founder ★); cron стоит
    на `--mode=dry-run` (правильно — apply без cold-выгрузки не должен удалять). Компакция снижает ТЕМП
    роста, но durable-СНИЖЕНИЕ = ретеншен с cold. **TD-020 закрывается вместе с Storage Box + первым
    успешным apply.**
- **TD-026** `red-prod-migration-fails-on-full-host-disk` (найдено reviewer'ом на §8 M-08 task 20,
  2026-07-15; **перенумерован из TD-025 → TD-026 reviewer'ом 2026-07-19** — коллизия номера с recon-flood
  TD-025, `docs(M-09) task 4` close-out).
  2026-07-15). SACRED-тест `crates/journal/tests/red_prod_migration.rs` использует
  `WriterConfig::own_capture(...)`, у которого `min_free_bytes` = ПРОД-дефолт **10 GiB**. На чекауте с
  <10 GiB свободного (у reviewer'а локально: 8.9 GiB / 437G, 98%) оба теста падают с `error:
  StorageGuard` (disk-guard срабатывает на реальный порог) — `verify_M-08.sh` → VERDICT: FAIL.
  **Это НЕ логика:** подтверждено независимо — CI (adequate-disk runner) на `d3e7db2` `cargo test`
  GREEN, и ветка task 20 не трогает `crates/journal`. Пре-existing env-чувствительность: пороговый
  тест зависит от АБСОЛЮТНОГО свободного места ХОСТА. Соседние фикстуры (`red_rotation`,
  `red_segments_epochs`) задают `min_free_bytes: 0` явно; `red_prod_migration` унаследовал прод-дефолт.
  **Следствие:** блокирует task 7 (tester `verify_M-08` PASS на чистом чекауте) на full-disk хосте —
  тестовое окружение обязано иметь ≥10 GiB, ЛИБО тест задаёт `min_free_bytes: 0` (как соседи; disk-guard
  проверяется отдельно в `red_retention`). Зона: architect (SACRED-тест, не reviewer). Патч
  однострочный (min_free_bytes: 0 в own_capture-вызовах теста) ИЛИ явное требование к test-env.
  Severity: **MINOR** (тест-качество/окружение; прод не затронут, CI зелёный).
  **✅ CLOSED 2026-07-15 (`7937b59`, architect).** Оба вызова `WriterConfig::own_capture(...)` в
  `red_prod_migration.rs` обёрнуты в `WriterConfig { min_free_bytes: 0, ..own_capture(...) }` —
  меняется ТОЛЬКО disk-guard порог (проверяется отдельно в `red_retention`), ассерты миграции
  (legacy байт-в-байт цел, seq, новый сегмент, stream-blocked-until-declared) НЕ тронуты. Reviewer
  перепрогнал на full-disk-освобождённом хосте (61G): `red_prod_migration` **2/2** (был StorageGuard-
  FAIL), workspace **185/0**, **`verify_M-08.sh` VERDICT: PASS** — tester task 7 разблокирован
  независимо от свободного места хоста. Test-only, §8 не требуется (recorder от теста не зависит).
- **TD-021** `memory-metric-includes-page-cache` (найдено reviewer'ом на §8 M-08, 2026-07-14).
  Все прежние замеры памяти recorder'а (мои в TD-016: 8.4 → 48 → 139 MiB; оракульная мотивация
  «+6.5 MiB/час») снимались через `docker stats`, который показывает cgroup `memory.current` —
  **включая page cache** файла журнала. Recorder пишет ~30 MB/мин ⇒ кэш растёт линейно и выглядит
  как утечка. Факт с прода (`memory.stat`): `anon 13 492 224` / `file 163 508 224`. Правильная
  метрика — `RssAnon` из `/proc/<pid>/status` (или `memory.stat anon`), она дала **+1 MiB/час**
  вместо +8. **Урок процессный:** метрика ресурса, по которой заводится долг и пишется RED-оракул,
  сама обязана быть провалидирована — иначе оракул «границы ресурса» гейтит призрак. Внести в
  `.claude/rules/testing.md` (прод-масштаб для sacred I/O-путей) и в чек-лист §8
  (`.claude/rules/gates.md`): **RSS мерить как anon, не как cgroup-total**. Зона: architect
  (процессный слой). Severity: **MAJOR** (метрика вводила в заблуждение два milestone'а подряд).
- **TD-022** `closed-segment-compaction-not-delivered` (M-08 task 15, reviewer §8, 2026-07-14).
  Компакция закрытых сегментов была реализована в стековой ветке (`d131519`, oracle fix `e4f23d1`)
  и локально выглядела не-плацебо: C1-C6 FAIL против `76aadb2`, GREEN на `91f11aa`, C5 валит
  наивную реализацию "распаковать .zst целиком в RAM". Но стек был откатан из-за красного task 14
  на prod dry-run, поэтому **компакции нет ни на main, ни на проде**, закрытый сегмент не сжат,
  свободное место и deadline disk-guard не отодвинуты. Следующий виток должен доставить
  операторский/CLI-путь компакции или явную §8-команду, реально сжать закрытый сегмент на VPS,
  доказать чтение `journal::stream` после сжатия и отсутствие потери событий. Severity: **MAJOR**.
  - **rev 9 (reviewer §8, 2026-07-14) — REJECTED + REVERTED (`82b33db`).** Стек rev9
    (`cb46e34`+`9cf5acf`+`1ff1b55`, задачи 15 crash-window self-heal + 16 оператор компакции)
    прошёл ВСЕ локальные гейты на merge-коммите (`fmt`/`clippy`/**181 passed**/`verify_M-08` PASS/
    `verify_delivery` PASS вкл. D5a+D7/`crontab -n` 0). Оба rev8-блокера reviewer'а закрыты и
    подтверждены фактом: (1) D-COMP-1 — `segments()` (прод-путь `stream`) дедуплицирует raw+.zst
    через общий `dedup_indexed_paths` (raw побеждает); репро крах-окна: было 3172 события → стало
    3000; (2) D-COMP-2 — self-heal ветки `dst.exists()` со sha256-сверкой, битый `.zst` НЕ удаляет
    оригинал. Анти-плацебо C7/C8/C9 падают против `cb46e34`, GREEN на HEAD.
    **НО §8 eyes-on на VPS вскрыл НОВЫЙ, БОЛЕЕ ОПАСНЫЙ дефект (data-loss):** оператор
    `--mode compact --keep-raw N` жмёт **старейшие** закрытые сегменты первыми, а `segment-0` на
    проде — **15 GB LEGACY** (без v2-магии `HFTJRN02`, задекларирован в `journal.legacy.json`,
    невосполнимая история 2026-07-10..14). `compact_segment` его СЖИМАЕТ (sha сырых == sha
    распакованных → верификация проходит → оригинал УДАЛЯЕТСЯ), но обратное чтение `.zst` идёт
    через `skip_v2_header_forward`, который ТРЕБУЕТ v2-магию → `CorruptHeader` → `segments()`/
    `list_segments`/`stream` падают на первом же классифае ⇒ **ВЕСЬ ЖУРНАЛ НЕЧИТАЕМ, а оригинал
    legacy стёрт.** Доказано фактом в песочнице (не на проде — prod-каталог НЕ тронут): раскладка
    legacy-0(declared)+v2-1(closed)+v2-2(active), реальная `compact_closed_segments(keep_raw=1)` →
    после неё `list_segments`/`stream` = `corrupt SegmentHeader`. Cron на VPS НЕ установлен
    (`/etc/cron.d` пуст) ⇒ авто-порчи нет, prod цел (5 сырых сегментов); но код+runbook на main
    инструктировали оператора установить cron, который уничтожил бы историю при первом запуске.
    **Следующий виток (architect): компакция ОБЯЗАНА не трогать legacy-сегменты** (либо
    `compact_segment` возвращает `Err` на сегмент без v2-магии — конструктивный барьер, RED-оракул;
    либо `.zst` несёт восстановимый v2-заголовок и legacy читается после сжатия). RED-набор обязан
    включать legacy-сегмент в каталоге (дефект фикстуры: C1-C9 строят ТОЛЬКО v2 через
    `Journal::open_with` — прод-раскладка с legacy не покрыта). Severity: **CRITICAL** (потеря
    невосполнимых первичных данных).
  - **✅ CLOSED 2026-07-15 (reland `8a2e377`, reviewer §8 GREEN на РЕАЛЬНОМ проде).** Цепочка:
    architect `4d92373` (чистый revert-of-revert `82b33db` — восстановил rev9-стек 1:1, reviewer
    сверил tree(4d92373)==tree(2b2311f) побайтово) + `7754308` C10 RED + `0c7bef4` TD-023 +
    `0cd4eca` D-COMP-4 (`compact_segment` → `Err` на сегменте без `SEGMENT_MAGIC` в первых байтах,
    ДО любой мутации; `compact_closed_segments` тихо пропускает legacy) + `8a2e377` §8-план.
    **D-COMP-4:** конструктивный барьер по первым 8 байтам файла (как «активный не сжимаем»).
    **Анти-плацебо доказан reviewer'ом независимо:** C10 FAIL против `7754308` (без барьера,
    `red_compaction.rs:562` «legacy стёрты»), GREEN на HEAD. Гейты (перепрогнаны на чистом
    worktree): fmt/clippy clean, **workspace 182/0**, `red_compaction` **10/10** (C1-C10),
    `red_book_bounded` 7/7, `verify_M-08.sh` PASS, `verify_delivery_M-08.sh` PASS (D1-D7 + deep
    D1-deep/D2-deep). **§8 два шага (`--mode compact --dry-run` НЕ существует):**
    (A) sandbox с доставленным бинарём (образ `hft-platform-recorder:local`) на faithful прод-
    раскладке (no-magic declared legacy-0 + 35 v2): legacy байт-в-байт цел, `.zst` для legacy НЕ
    создан, 32 v2 сжаты (14.5×), `journal::stream` = 3500 событий ДО и ПОСЛЕ (потерь нет).
    (B) **РЕАЛЬНАЯ компакция боевого `/journal` через доставленный cron-скрипт** (`exit=0`, alert
    не взведён): **боевой legacy-0 БАЙТ-В-БАЙТ ЦЕЛ** (полный sha256 `234583c8…bdbdc72` == эталон,
    size=15188347171, mtime=1784018822 не изменились — барьер сработал на живом 15 GB legacy);
    сегменты 1-5 → `.jrnl.zst`, `zstd -t` каждого = исходный raw-размер (данные целы, +D-COMP-2
    sha-roundtrip до удаления raw); **свободно 111.20 → 115.88 GB (+4.69 GB) — диск двинулся**;
    recorder healthy, restarts=0, next_seq растёт, heartbeat свежий (конкурентная компакция
    закрытых не задела живого писателя). Legacy-безопасность компакции доказана на РЕАЛЬНОМ активе.
- **TD-023** `book-memory-oracle-flaky-under-parallel-tests` (найдено reviewer'ом на §8 M-08 rev9,
  2026-07-14). `crates/venue-binance/tests/red_book_bounded.rs::td016_memory_bounded_when_price_
  drifts_out_of_band` меряет прирост памяти через **процесс-глобальный** `static CUR` +
  `#[global_allocator]` (`CUR.load` до/после `pump()`). Под `cargo test --all` 7 тестов бинаря
  бегут параллельно и соседние потоки загрязняют глобальный счётчик → `growth` ловит чужие живые
  аллокации. На 2-ядерном CI-раннере (run `29375951711` @`2b2311f`): `growth=6 559 969 B > 4 MiB` →
  FAILED, exit 101; **тот же коммит на re-run — GREEN** (флак подтверждён: red→green без изменений
  кода). Локально: 5/5 в изоляции, 6/6 полным бинарём (больше ядер/памяти — окно уже). Тест sacred
  (architect-only), не вызван изменением M-08 (0 строк дифа по `venue-binance`/`book`, идентичен
  последнему зелёному main `76d9560`). Тот же класс, что C5/TD-021: **метрика на глобальном
  аллокаторе не изолирована от параллельного прогона.** Фикс — зона architect: сериализовать замер
  (`--test-threads=1` для бинаря / `serial_test`), либо мерить размер книги напрямую (не глобальный
  аллокатор), либо поднять допуск с запасом. Severity: **MAJOR** (флак на критическом CI-пути:
  `main` перестаёт быть детерминированно зелёным; «Deploy failure на зелёном коде» легко принять за
  инфраструктурный флак и обойти руками — маскирует настоящие регрессии).
  **✅ CLOSED 2026-07-15 (reland `8a2e377`, `0c7bef4`, architect).** Оракул переписан
  (`td016_book_saturates_at_backstop_not_grows_with_updates`): глобальный аллокатор-счётчик
  удалён целиком (он и был источником гонки), память книги = O(числа уровней) `BTreeMap`,
  поэтому меряем число уровней напрямую против контрактного потолка `2·BACKSTOP_LEVELS_PER_SIDE`
  (детерминированно, без гонки). Порог 4 MiB был вдобавок ФИКЦИЕЙ (остался с rev1 при backstop
  5000; rev6 поднял до 200k/сторону). reviewer перепрогнал: 7/7 стабильно, GREEN под полным
  `cargo test --workspace` (77 блоков, 182/0) — флак устранён. Sacred-тест, правка architect'а.

- **TD-024** `compose-service-command-uses-equals-form-binary-rejects` (найдено reviewer'ом на §8
  M-08 rev10, 2026-07-15). §8 Step B: попытка запустить компакцию через ЗАДОКУМЕНТИРОВАННУЮ
  операторскую команду `docker compose run --rm journal-compaction` упала:
  `journal-retention: неизвестный флаг` — сервисы `journal-compaction` И `journal-retention` в
  `docker-compose.yml` держат `command:` в форме `--dir=/journal`/`--mode=compact`/`--keep-raw=2`
  (equals-form), а ручной arg-парсер бинаря (`match arg { "--dir" => next() }`) `=`-форму НЕ
  разбирает → «неизвестный флаг», exit 1. Хуже того, любая документированная команда вида
  `docker compose run --rm journal-retention --mode apply` (README D6) APPEND'ит аргумент, а в
  `docker compose run SERVICE ARG` этот ARG **ЗАМЕНЯЕТ** весь `command:`-блок ⇒ теряется
  `--dir=/journal` ⇒ бинарь берёт `DEFAULT_DIR=./journal-data` (пустой каталог в контейнере), а
  НЕ боевой `/journal`. **Работает ТОЛЬКО потому, что cron-скрипты** (`journal-{retention,
  compaction}-cron.sh`) передают ПОЛНЫЙ раздельный argv (`--dir /journal … --mode compact`),
  который заменяет блок целиком корректным набором — именно через cron-скрипт reviewer выполнил
  §8-B успешно. Т.е. прод-путь (cron) работает, но задокументированные ad-hoc операторские команды
  и bare-запуск сервиса СЛОМАНЫ (та же серия «текст в репо ≠ работает в проде», что весь TD-020;
  гейт `verify_delivery` D5a/D7 гонял ТОЛЬКО cron-argv, а `command:`-блок compose против живого
  бинаря не проверял). **Фикс (architect/engine-dev):** либо `command:` в раздельной форме
  (`- --dir` / `- /journal` отдельными элементами списка), либо парсер принимает `--flag=value`
  (`split_once('=')`); README-команды привести к форме, которая НЕ теряет `--dir`. Плюс гейт
  `verify_delivery`: прогнать РЕАЛЬНЫЙ бинарь именно через `command:`-блок compose (bare service
  run), а не только через cron-argv. Данные НЕ пострадали (бинарь падал на arg-парсинге ДО мутаций;
  legacy цел). Severity: **MAJOR** (операторский интерфейс хрупкий/сломан вне точных cron-скриптов;
  apply-команда из README целится в неверный каталог).
  **✅ CLOSED 2026-07-15 (task 19 / rev11, `e31e23e`, §8 PROD GREEN).** Цепочка: architect
  `475bbd5` RED `red_cli_argv.rs` (гоняет НАСТОЯЩИЙ бинарь `CARGO_BIN_EXE_journal-retention`:
  equals-форма dry-run + compact + регресс раздельной формы) + гейт **D8** (`verify_delivery`
  извлекает `command:`-блок ОБОИХ сервисов из compose, подставляет `${VAR:-default}`+sandbox
  СОХРАНЯЯ форму флага, прогоняет реальным бинарём — прямо закрывает слепое пятно D5a/D7) →
  engine-dev `935bc9b` фикс парсера (нормализация argv ДО цикла: `--flag=value` →
  `split_once('=')` → `[--flag, value]`; раздельная форма без изменений — регрессии нет) +
  `e31e23e` README §4 (Apply = ПОЛНЫЙ повтор argv, не «короткая» `--mode apply`, что теряла `--dir`).
  **Анти-плацебо доказан reviewer'ом независимо:** оба equals-теста FAIL против `475bbd5` (без
  фикса, exit=1 «equals-форма отвергнута»), раздельный проходит; GREEN на HEAD. Гейты: workspace
  **185/0**, verify_M-08 PASS, verify_delivery PASS вкл. **D8 обоих сервисов**, crontab -n 0.
  **§8 PROD (VPS `e31e23e`):** `docker compose --profile ops run --rm journal-compaction` (ровно
  та equals-form команда, что падала до фикса) → **exit=0**, сжаты сегменты 6,7 (10.43×), диск
  +1.94 GB; `docker compose --profile ops run --rm journal-retention` (dry-run) → **exit=0**,
  0 prune, legacy/active/young корректно skipped, disk_pressure нет; **legacy-0 байт-в-байт цел**
  (sha256 `234583c8…bdbdc72`, size+mtime не изменились), recorder healthy, restarts=0. Задокументи-
  рованный операторский путь через compose теперь работает end-to-end. Data не пострадали.
- **TD-018** `deploy-ci-gate-cannot-read-ci-status` (найдено reviewer'ом на §8 M-08, 2026-07-14).
  Гейт TD-017 (`deploy.yml` job `ci`, «Wait for CI success on this commit») **не работает**:
  `gh api repos/$REPO/actions/runs?head_sha=$SHA` возвращает **`HTTP 403 Resource not accessible
  by integration`** — у дефолтного `${{ github.token }}` в этом репо нет права `actions: read`, а
  блока `permissions:` в `deploy.yml` НЕТ. Гейт fail-closed отработал «правильно» (Deploy не пошёл,
  прод не тронут), но по НЕВЕРНОЙ причине: он не может прочитать статус CI **никогда** →
  **автодеплой полностью заблокирован** (run `29318076908` на `1123b13`: CI success, Deploy failure
  на шаге гейта). Следствие: код M-08 лежит на зелёном `main`, но НЕ в проде; §8 eyes-on
  невыполним, milestone не закрывается. **Фикс — мелкий, зона architect** (CI/процессный слой):
  добавить в `deploy.yml` `permissions: { actions: read, contents: read }` (либо перевести гейт на
  `workflow_run` с фильтром `conclusion == success`), затем re-run Deploy на `1123b13`.
  Severity: **MAJOR** (гейт-байпас наоборот: пайплайн не может выкатить прод; при этом «Deploy
  failure» на зелёном main легко принять за флаки и обойти руками — обход = возврат TD-017).
  **✅ CLOSED 2026-07-14 (`b7721d1`, architect): `permissions: actions:read` добавлены. Доказано
  СКВОЗНЫМ прогоном, а не глазами: run 29318076908 @`1123b13` → Deploy FAILURE на 403 (гейт не
  пустил), run 29318836147 @`b7721d1` → CI success → Deploy SUCCESS. Прод обновлён.**
- **TD-017** `deploy-not-gated-on-ci` (замечено reviewer'ом на §8 M-07, 2026-07-13).
  `.github/workflows/deploy.yml` — САМОСТОЯТЕЛЬНЫЙ workflow на `push: branches: [main]`
  (paths: `crates/**`, `Cargo.toml`, `Cargo.lock`, `Dockerfile`, `docker-compose.yml`),
  без `needs: ci` и без ожидания `ci.yml`. Наблюдение на merge M-07: **«Deploy to VPS» дошёл до
  `success`, пока `CI` был ещё `in_progress`** — т.е. прод пересобрался и перезапустился ДО того,
  как fmt/clippy/тесты завершились. Следствие: **красный CI не останавливает деплой** — на VPS
  может уехать код, проваливший тесты; страховка `deploy.yml` (healthcheck+rollback) ловит только
  падение контейнера, но НЕ логическую регрессию с живым healthcheck'ом (ровно TD-011/TD-013:
  контейнер жив, данные испорчены). Сейчас риск ограничен (recorder/testnet, денег на пути нет),
  но на P4 (live-торговля) это дыра на пути к деньгам. **Фикс — мелкий, эффект высокий:**
  `needs: ci` (или `workflow_run` с фильтром `conclusion == success`). Зона — architect
  (процессный/CI-слой), не reviewer. Severity: **MAJOR** (гейт-байпас: gates §8 требует
  «дождаться CI+Deploy success», но пайплайн допускает Deploy success ПРИ красном CI).
  **✅ CLOSED 2026-07-14 (M-08 задача 6 + TD-018 фикс `b7721d1`).** Гейт `deploy.yml` job `ci`
  («Wait for CI success on this commit») + `deploy: needs: ci`, fail-closed на красный CI / таймаут /
  отмену; verify-гейт T12 проверяет связь структурно. Работоспособность доказана СКВОЗНЫМ прогоном:
  красный/непрочитанный CI Deploy НЕ пустил (`1123b13`), зелёный — пустил (`b7721d1` → Deploy
  success → VPS HEAD `b7721d1`). Deploy success при красном CI более невозможен.
- **TD-001** recorder Docker-образ работает root'ом (M-00 заглушка). Hardening (non-root +
  права journal-тома) — при реальном recorder (M-01). Severity: MINOR.
- **TD-002** `hetzner-server` приватный ключ был вставлен в чат (скомпрометирован). Пересоздать
  на лэптопе + заменить на VPS при случае. Severity: MINOR (доступ и так только founder+ключи).
- **TD-003** `[verify-at-impl]` по Hyperliquid и Binance (rate-лимиты, подпись действий для
  ордеров) — уточнить при реализации order-стороны. Severity: NOTE.
- **TD-004** Binance L2 сейчас `@depth20@100ms` (частичный снапшот, топ-20). Для OBI-сигнала
  (полосы 3%/8%) нужна бОльшая глубина → полноценный snapshot+diff-sync (recon §A/§D). Severity: NOTE (следующая фаза).
- **TD-005** HL `l2Book` даёт снапшоты по изменению книги (наблюдалось ~реже Binance). Проверить
  полноту cadence; при нужде добавить `bbo`. Funding/liquidations пока не подписаны. Severity: NOTE.
- **TD-006** Журнал — один сегмент без ротации/ретеншена/cold-выгрузки (docs/06). Severity:
  повышен до **MAJOR** (2026-07-13: 15.0 GB в одном сегменте, ~2.8 GB/сутки, 114 GB свободно).
  **СТАТУС 2026-07-14 (M-08 задачи 2/3, merge `1123b13`): реализовано на `main`, в проде НЕ
  ПОДТВЕРЖДЕНО** — ротация `segment-NNNNNNNN.jrnl` (1 GiB, seq сквозной), retention с
  `ColdCopyProof` (удалить невыгруженный сегмент нельзя ВЫРАЗИТЬ в API), disk-guard fail-closed
  (`min_free_bytes` → `append` → `Err`, ни байта, ни seq). Деплой заблокирован TD-018; на проде
  по-прежнему один растущий сегмент `segment-00000000.jrnl` (15.0 GB). Закрывается после §8.
  **СТАТУС 2026-07-14 (прод `8882c1e`): ротация — ✅ ПОДТВЕРЖДЕНА В ПРОДЕ** (три сегмента:
  legacy 15 188 347 171 B заморожен байт-в-байт (полный sha256 до/после деплоя совпал), 
  `segment-00000001` закрылся ровно на 1 073 741 818 B, пишется `segment-00000002` с магией
  `HFTJRN02`; `seq` сквозной, `restarts=0`). **Ретеншен и cold-выгрузка — ❌ В ПРОДЕ НЕ РАБОТАЮТ**
  (бинарь не доставлен, холодного хранилища нет — см. TD-020). ⇒ **TD-006 остаётся OPEN**: диск
  по-прежнему монотонно растёт, просто кусками по 1 GiB. Закрывается вместе с TD-020 (задача 14).
  **СТАТУС 2026-07-15 (reland `8a2e377`, reviewer §8-B): компакция РЕАЛЬНО сжала боевые сегменты
  (диск +4.69 GB, ~5-9× на закрытых), но ДОЛГ OPEN.** Разовая ручная компакция место освободила;
  для durable-сдвига дедлайна нужен УСТАНОВЛЕННЫЙ cron (сейчас не установлен) + фикс TD-024, а для
  реального СНИЖЕНИЯ (не только замедления роста) — ретеншен с cold-выгрузкой (Storage Box, ★).
  **СТАТУС 2026-07-15 (task 20, `d3e7db2`): CRON АКТИВИРОВАН — компакция теперь durable, TD-024
  CLOSED. Диск durable-замедлен, но НЕ снижается без ретеншена.** cron установлен (`50 3` компакция,
  `7 4` ретеншен dry-run), АВТО-прогон подтверждён eyes-on (см. TD-020). Темп роста будет durable
  сбит компакцией (~9×), но абсолютное СНИЖЕНИЕ диска требует ретеншена apply + Storage Box (★).
  **TD-006 закрывается вместе с TD-020** (Storage Box + первый успешный retention apply).
  **СТАТУС 2026-07-14 (tasks 14/15 rollback `b43044d`): всё ещё OPEN.** Реальная prod-компакция
  закрытого сегмента не была выполнена: стек с `compact_closed_segments` откатан из-за красного
  dry-run ретеншена, свободное место не увеличено, deadline disk-guard не отодвинут.
- **TD-007** DET-I-1 (бит-идентичный replay + state_hash) реализован частично (seq+read_all).
  Полный snapshot/state_hash — следующая фаза journal. Severity: NOTE.
- **TD-008** `t1-report-forms-promotion` (M-04). Rust-типы T1-форм `TrialRecord`/
  `ValidationReport` временно живут в `crates/research-cli/src/types.rs` со статусом
  «T1-designate» (per docs/fa/research-cli.md §N amendment 2026-07-10 + critic C-001 M1).
  Единственный продюсер/консюмер сейчас — research-cli; JSON несёт `report_schema_version`.
  Промоушен в `crates/contracts` + генерация JSON Schema (CT-I-4) — отдельным contract-RFC
  при появлении первого кросс-языкового консюмера (Python-тулинг). Severity: NOTE.
- **TD-009** `obi-track-a-report-pending` (M-04 задача 8, ОТКРЫТА). Прогон OBI Трек A/B →
  `research/reports/R-001*` гейтится накоплением данных полной книги (VPS пишет с 2026-07-10),
  вердиктом risk-critic (анти-оверфит чек-лист gates.md §6) и подписью founder ★. Merge
  M-04-кода risk/oms/venues/contracts не трогал — risk-critic обязателен на ОТЧЁТЕ, не на
  этом merge. Также см. TD-004 (Binance @depth20 недостаточен для полос 3%/8% — нужен
  full-book snapshot+diff). Severity: NOTE (гейт пути к деньгам, не долг кода).
- **TD-010** `binance-rest-depth-limit-5000-undercount` (M-05 task 5 / B1, venue-dev, ОТКРЫТА).
  Заведено по флагу founder'а от venue-dev: REST-resnapshot глубины Binance ограничен
  `limit=5000` уровнями на один вызов — дальние полосы книги за пределами топ-5000 одним
  снапшотом не покрываются, а reconcile против diff-книги ограничен этим потолком. Прямое
  следствие для anti-phantom eviction (B1): в самых дальних полосах устаревшие лимитки могут
  не эвиктиться из-за неполноты reference-снапшота. Точный масштаб undercount + стратегия
  (пагинация vs принятие потолка с явной границей достоверности полос) — за venue-dev при
  посадке task 5/B1; на этом merge (engine-dev part) код venue не трогался. Связано с TD-004.
  Severity: NOTE (граница достоверности данных дальних полос, не риск ордер-пути).
- **TD-012** `binance-futures-rest-depth-limit-1000-undercount` (M-06 venue-dev, ОТКРЫТА).
  Аналог TD-010/TD-004 для USDT-M перп: REST depth-снапшот `/fapi/v1/depth` futures ограничен
  `limit=1000` уровнями на вызов — дальние полосы книги за топ-1000 одним снапшотом не покрываются,
  reconcile diff-книги ограничен этим потолком. `FuturesDepthBook.apply_snapshot` (REPLACE, INV-N2)
  корректно эвиктит stale в пределах снапшота, но за границей top-1000 reference неполон. Также
  открытый вопрос `!markPrice@arr` update-rate (согласовать с research-dev, если важна cadence
  funding-breadth). Точный масштаб undercount + стратегия (пагинация vs явная граница достоверности
  полос) — за venue-dev/architect при углублении deep-book. Класс TD-004. Severity: NOTE (граница
  достоверности данных, не риск ордер-пути; MD-only).

- **TD-015** `trials-ledger-cross-epoch-metrics-not-comparable` (M-07, ОТКРЫТА). Ledger
  append-only (`INTG-I-6`) — записи НЕ переписываются, поэтому несопоставимость лечится
  ФИЛЬТРАЦИЕЙ на чтении, а не правкой файла. Reviewer проверил фактическое состояние на merge
  M-07: `research/trials-ledger.jsonl` = **4 записи, все пре-M-07** (`code_hash f7f4761…`,
  `cell-0..3`, `ts_wall_ms 1783724134640` ≈ 2026-07-10, Sharpe ≈ −1.73…−2.21), и цепочка M-07
  ledger НЕ трогала (`git log b0be701..5141fd9 -- research/trials-ledger.jsonl` пуст).
  Две несопоставимые эпохи:
  (1) **пре-M-07** (эти 4 записи) — получены СТАРЫМ ad-hoc harness'ем (`qty=1.0`, taker-in по
      `SignalOut`, taker-out по `horizon_ms`), т.е. меряют ЛОГИКУ, КОТОРОЙ БОЛЬШЕ НЕТ. Их числа
      несопоставимы с post-M-07 не «немного», а принципиально;
  (2) **окно equity-бага** — любой грид-прогон, сделанный на коде между `37753a6` и `5141fd9`
      (в т.ч. в dev-worktree'ах, не в репо), несёт **ЗАВЫШЕННЫЙ Sharpe** (фантомные equity-точки
      занижали σ). В репозиторном ledger таких записей НЕТ, но если они всплывут из чьего-то
      локального прогона — они невалидны.
  **Правило (обязательное для M-04 task 8 / TD-009 и любого отчёта `research/reports/R-*`):**
  в метрики и в deflated-Sharpe брать ТОЛЬКО записи, произведённые кодом **≥ `5141fd9`**
  (новая семантика D7, держится оракулами ST-I-8g/8h); пре-M-07 записи — историческая справка,
  не база сравнения. Дискриминатор — `code_hash` записи (у пре-M-07 эпохи он `f7f4761…`).
  ⚠ Отдельно: формального отчёта `research/reports/R-001*` в репо ПОКА НЕТ (директория
  `research/reports/` отсутствует; черновик лежит вне git-зоны в `tmp/pilot/`) — при его
  появлении правило выше применяется к нему, и это обязан проверить risk-critic
  (анти-оверфит чек-лист gates.md §6). Severity: MAJOR (путь к деньгам — искажённый Sharpe
  ведёт к подписи founder'а на промоушен; кода-долга нет, долг — дисциплина чтения ledger'а).

- **TD-013** `binance-futures-rest-resnapshot-no-backoff-418-ban` (M-06 venue-binance-futures,
  **ФИКС MERGED inert 2026-07-12, closes при §8 реленда #4**). **Прод-регрессия, поймана §8 eyes-on** (класс TD-011:
  зелёные юниты + Deploy-success замаскировали). При wire BinanceFutures в recorder (#4,
  `2eee4bf`) futures-адаптер на ЖИВОМ прод-трафике попал в hot-loop REST-ресинка: депт-книга не
  бутстрапится, `fetch_snapshot` (`/fapi/v1/depth?limit=1000`) отдаёт **HTTP 418 "I'm a teapot"**
  (Binance IP rate-limit ban), и код НЕМЕДЛЕННО (без backoff) реквестит снова. Замер на VPS:
  **133 × 418 за 25s (~5 req/s), 0 успешных снапшотов, книга не собралась**. Петля
  само-поддерживает бан (продолжающиеся реквесты во время бана сбрасывают его таймер) и
  абьюзит биржу с IP, ОБЩЕГО со спот-пайплайном (`venue-binance`) — риск коллатерального бана
  рабочего спот-сбора. **Корень (reviewer описал, architect проектирует фикс — gates.md §4):**
  `crates/venue-binance-futures/src/lib.rs:596-600` (snapshot fetch failed → `pending_snapshots.push(make_snapshot_future(...))`
  без задержки) и `:613-620` (snapshot stale → тот же немедленный refetch). Нет exp-backoff,
  нет honoring `Retry-After`/429/418, нет cap на частоту REST. **Нужен RED-оракул (architect):**
  ресинк-путь при повторной ошибке снапшота ОБЯЗАН backoff'ить (exp + jitter, honor 418/429
  cooldown), не hot-loop'ить; анти-плацебо на наивной немедленной-retry реализации.
  Затем venue-dev impl → engine-dev релендит #4 (тривиально: re-apply `2eee4bf`). **#4 РЕВЕРТНУТ**
  (`6ddf810`+`6de58e8`), main = tree(`3f38ab0`) inert, прод inert-safe re-verified (418=0,
  CPU 0.99%, seg растёт, hb свежий). Связано с TD-012 (тот же limit=1000, но это completeness;
  TD-013 — корректность/rate ресинка). Severity: MAJOR (была live прод-регрессия + exchange-abuse;
  сейчас блокирует реленд #4).
  **ФИКС (MERGED inert `cc4f529`, reviewer APPROVED 2026-07-12):** architect RED `449bb38`
  (`tests/red_backoff.rs`) → venue-dev `cc4f529` — чистая политика `pub struct Backoff`
  (`next_delay(Option<Retry-After>)`: BASE 100ms, exp ×2, cap 5мин, honor cooldown; `reset()` на
  success), wire'нута в `handle_snapshot`: на fail/stale → `next_delay` → **реальный
  `tokio::time::sleep(delay).await` внутри `make_snapshot_future` ПЕРЕД `fetch_snapshot`**; на
  success → `reset()`. `fetch_snapshot` мапит 418→120s/429→10s cooldown (или `Retry-After` header)
  ДО `error_for_status` → hot-loop рвётся на первом 418. **Reviewer-верификация анти-плацебо (RED
  тестит только политику, НЕ await):** код-рид подтвердил РЕАЛЬНЫЙ sleep в I/O-future (не
  сконструированный-и-забытый Backoff); sleep суспендит только futures символа, не runner. Все
  тесты + workspace GREEN, fmt/clippy clean. **ОСТАЁТСЯ:** (1) реленд #4 (engine-dev, re-apply
  `2eee4bf`) → ПОЛНЫЙ §8 eyes-on LIVE-проверка (418-backoff реально работает: cooldown-sleeps,
  книга бутстрапится, futures L2Snapshot в журнал) — ТОЛЬКО тогда TD-013 CLOSED; (2) **RN-10
  (jitter, NOTE):** джиттер decorrel'ации hammering'а НЕ добавлен — спека RED-оракула его не требует
  (политика детерминирована, джиттер = забота I/O-caller). При 2 символах + 418→120s cooldown
  риск синхронного hammering'а низкий. Если нужен ±jitter — потребует rand/fastrand в
  venue-binance-futures `[dependencies]` (own-crate, formally allowed) + покрытие; отдельная
  мелкая задача, не блокер реленда.
  **LIVE RELAND RESULT (`8b26d6c`, 2026-07-12):** anti-hot-loop часть TD-013 прошла §8: при 418
  recorder логировал cooldown/retry-after sleeps с интервалами ~50-60s на BTCUSDT/ETHUSDT, а не
  прошлые 133×418/25s; CPU/MEM нормальные, restarts=0. Но полный #4 §8 NOT GREEN из-за нового
  blocker'а TD-014 (нет live L2Snapshot/Funding), поэтому milestone close-out не достигнут.

- **TD-014** `binance-futures-live-depth-funding-not-emitted-after-backoff` (M-06 #4 reland,
  **CLOSED 2026-07-13 by `c123bbd` + §8 live GREEN**). После фикса TD-013 reland `8b26d6c` прошёл code-review, локальные
  gates (`red_futures_wired`, fmt, clippy, workspace tests, `verify_M-06.sh` PASS) и GitHub
  CI+Deploy, но §8 eyes-on на VPS НЕ прошёл продуктовый критерий recorder wire. Наблюдения:
  3 `venue connect` строки были (`binance`, `hyperliquid`, `binance_futures`), journal рос, seq
  непрерывен (`seq_gaps=0` на tail-inspection), heartbeat свежий, restarts=0, TD-013 backoff
  live-работал. Однако в 20 MiB / 115 MiB live journal tails были только `BinanceFutures`
  OpenInterest + ConnUp; **0 `BinanceFutures` L2Snapshot и 0 Funding**, при частых
  `venue-binance-futures: depth continuity gap detected, resyncing book` и `snapshot stale vs
  buffered diffs, refetching with backoff`. Liquidation может быть редким событием, но Funding из
  `!markPrice@arr` rare-event'ом не является, поэтому отсутствие Funding блокирует reland.
  Реверт выполнен (`e6b4a75` + `d819cc3`); prod inert-safe re-verified (VPS HEAD `d819cc3`,
  spot+HL only, futures/418=0, hb age 8s, segment +60KB/5s, CPU/MEM нормальные, restarts=0).
  **Нужен architect RED/live-equivalent oracle:** futures runner обязан, при mock/controlled fstream
  depth + markPrice + REST snapshot/backoff сценарии, стабильно эмитить L2Snapshot и Funding после
  resync/backoff, без hot-loop и без starvation markPrice path. Затем venue-dev fix → engine-dev
  reland #4 → reviewer full §8. Severity: MAJOR (prod behavior blocker, no order-path impact).
  **LIVE RELAND-2 RESULT (`af7725f` over `595fc24`, 2026-07-12):** TD-014 fix attempt added
  `FuturesSession` seam + `run()` delegation and local `red_live_emit` passed; reviewer static check
  confirmed live path delegates WS text / snapshot result / tick through the seam (no obvious parallel
  untested runner path). Local gates all GREEN: `red_futures_wired`, `venue-binance-futures` 7/7,
  workspace tests, fmt/clippy, `verify_M-06.sh` PASS exit=0. Pre-merge §8 on VPS still NOT GREEN:
  journal tail since deploy had `BinanceFutures` ConnUp and OpenInterest=16 with `seq_gaps=0`, but
  **0 `BinanceFutures.L2Snapshot` and 0 `BinanceFutures.Funding`**; logs showed repeated
  `depth continuity gap detected`, `snapshot stale vs buffered diffs`, and 429 backoff. Candidate was
  not merged; VPS restored to `origin/main` `2bbcbd7` and rechecked healthy, spot+HL only. Current
  RED/live oracle missed this production mode; TD-014 remains OPEN/BLOCKING.
  **LIVE TD-014 v2 RESULT (`fac7c07` over `71255c5`, 2026-07-12):** stronger local lifecycle
  oracle passed and reviewer confirmed the code path is still MD-only and recorder wiring is real.
  Local gates all GREEN: `red_futures_wired`, `venue-binance-futures` 7/7, workspace tests,
  fmt/clippy, `verify_M-06.sh` PASS exit=0. Pre-merge §8 on VPS still NOT GREEN: journal tail since
  deploy had `BinanceFutures.L2Snapshot=16`, `OpenInterest=16`, `seq_gaps=0`, but
  **`BinanceFutures.Funding=0`**; L2 was sparse rather than expected ~1/s/symbol. Logs during the
  live window showed ongoing churn (`depth continuity gap` 311, `snapshot stale` 44, `429` 18);
  initial CPU reached 6.99% before settling near 1.2%. Candidate was not merged; VPS restored to
  `origin/main` `3eff0db` and rechecked healthy, spot+HL only. TD-014 remains OPEN/BLOCKING.
  **LIVE TD-014 T2 RESULT (`669ce40` over `38c3175`, 2026-07-12):** futures-continuity oracle
  (`pu`, not spot `U == last+1`) passed locally and reviewer static check confirmed the dual-rule
  implementation is MD-only: strict `pu == last_update_id` in steady-state, Binance snapshot-bridge
  rule in reconcile-loop, mandatory `pu` fail-closed. Local gates all GREEN: `red_futures_wired`,
  `venue-binance-futures` 8/8, workspace tests, fmt/clippy, `verify_M-06.sh` PASS exit=0. Pre-merge
  §8 on VPS showed T2 materially improved live depth: `BinanceFutures.L2Snapshot=470`,
  `OpenInterest=54`, `seq_gaps=0`; after startup, last 3m had `gap=0`, `stale=0`, `429=0`, CPU
  ~1.1%, restarts=0, and only one non-looping 418. However, the acceptance criterion still failed:
  **`BinanceFutures.Funding=0`** in the 48 MiB journal tail since deploy. Candidate was not merged;
  VPS restored to `origin/main` `4012c55` and rechecked healthy, spot+HL only. TD-014 remains
  OPEN/BLOCKING, now narrowed to persisted live Funding emission under the real fstream path.
  **LIVE TD-014 T3 RESULT (`99b1329` over `c747a97`, 2026-07-13):** per-symbol markPrice oracle
  passed locally and reviewer static check confirmed `run()` subscribes to
  `<sym>@markPrice@1s`, while `FuturesSession::on_ws_text` emits Funding from both per-symbol
  single-object markPrice and legacy `!markPrice@arr`. Local gates all GREEN: `red_futures_wired`,
  `venue-binance-futures` 9/9, workspace tests, fmt/clippy, `verify_M-06.sh` PASS exit=0.
  Pre-merge §8 on VPS still NOT GREEN: journal tails since deploy had
  `BinanceFutures.L2Snapshot=637`, `OpenInterest=66`, `seq_gaps=0`; later log window was clean
  (`gap=0`, `stale=0`, `429=0`, CPU ~1.2%, restarts=0), but **`BinanceFutures.Funding=0`**
  persisted and logs had `markPrice/Funding=0`. Candidate was not merged; VPS restored to
  `origin/main` `1d5ecfa` and rechecked healthy, spot+HL only. TD-014 remains OPEN/BLOCKING.
  The next fix must instrument or reproduce raw WS delivery/stream-name/parse-drop behavior:
  current unit coverage proves parser/session handling, but not that Binance actually delivers
  a usable markPrice message through this combined session.
  **LIVE TD-014 T4 RESULT (`c123bbd` over `d9b3b1c`, 2026-07-13):** venue-dev pivoted Funding
  from dead WS markPrice delivery to REST `/fapi/v1/premiumIndex` all-perps polling, matching the
  live-proven OI REST path. Local gates all GREEN: `red_futures_wired`, `venue-binance-futures`
  10/10, workspace tests, fmt/clippy, `verify_M-06.sh` PASS exit=0. Remote Docker verify on VPS
  also GREEN (`VERDICT: PASS exit=0`; host has no Rust toolchain, so reviewer ran it in
  `rust:1-slim` with rustfmt/clippy components installed). Pre-merge §8 on VPS GREEN:
  `BinanceFutures.L2Snapshot=465`, `OpenInterest=48`, **`Funding=48`**, `seq_gaps=0`;
  late logs `418=0`, `429=0`, `gap=0`, `stale=0`, CPU/MEM normal, restarts=0. Candidate was
  merged via `1504d8b`; TD-014 CLOSED.

## Замечания reviewer'а M-35 (margin-inventory / CT-RFC-05, 2026-07-25)
- **RN-18 ✅ CLOSED** (architect `a6d178f`). `journal/examples/dump.rs` арм нового MdPayload-варианта был
  вне литеральных Allowed-paths task 2b, но необходим для `clippy --all-targets`; architect формально
  расширил Allowed-paths 2b + gates правило (exhaustive-match ревизия покрывает src+examples+bins).
- **RN-20 (epoch-tripwire регрессия, ✅ пойман+исправлен, урок процессный).** CT-RFC-05 bump
  `SCHEMA_VERSION` 3→4 уронил намеренный tripwire `contracts/tests/red_rfc04.rs::
  ct_rfc04_rev2_schema_epoch_is_three` (`assert_eq!(SCHEMA_VERSION,3)` — by-design ловит эпоху-bump без
  осознанного апдейта). Пойман **CI `cargo test --all` ПОСЛЕ merge в main** (`ba61c62`) — НЕ локальным
  `verify_M-35.sh`, т.к. тот гонял подмножество contracts-suite (ct_rfc05/red_schema/ct_rfc01/red_rfc02,
  БЕЗ red_rfc04) — **RN-8-класс** (acceptance тестит меньше, чем CI). Deploy-гейт fail-closed удержал прод
  (VPS не тронут, откатывать нечего). Fix-forward architect `b3a5a95`: tripwire 3→4 (L2Delta-эпоха
  историческая, текущая=MarginInventory) + `verify_M-35.sh` += ПОЛНЫЙ contracts-suite + gates правило
  (SCHEMA_VERSION-bump ⇒ verify гоняет ВЕСЬ `cargo test -p contracts`, не подмножество). Урок закреплён:
  milestone, бампающий эпоху схемы, обязан прогонять все epoch-tripwire'ы локально.
- **RN-21 (TD-020-класс: коллектор дремлет, ✅ пойман §8 + исправлен task 2e).** `run_margin_inventory`
  существовал и был GREEN по unit-тестам, но `recorder/main.rs` его не спавнил → первый §8 после
  `b3a5a95` показал **0 MarginInventory** на проде при живом ключе (0 auth-ошибок — просто некому было
  поллить). Fix task 2e (`bc42e73`, engine-dev): явный `tokio::spawn(run_margin_inventory)` под
  `Venue::Binance`. Урок (тот же, что TD-020): «код на main + зелёный cargo test» ≠ «функция работает в
  проде» — для активируемого через spawn/wiring кода доказательством является §8 decode журнала, а не
  unit-suite. Оракул task 2e — законно §8 (spawn в `main()` не unit-тестируем; паттерн M-06/M-09 wiring).
- **C-025 IP-restrict условие ✅ ВЫПОЛНЕНО.** risk-critic C-025 PASS был обусловлен flip `ipRestrict:false→
  true` на `167.233.192.131` до §8. Подтверждено фактически: signed REST-запрос С VPS (IP совпадает) →
  HTTP 200 с данными; ключ read-only-функционален и IP-заперт (запрос с другого IP был бы отклонён).
- **Recorder-owner (scope-guard `966e45e`) — разрешён.** Кто армирует exhaustive-match recorder'а при новом
  T1-варианте было неоднозначно (пропущено в `45ec491`); `966e45e` закрепил recorder→engine-dev в
  scope-guard + gates (энумерация грепом, не по памяти). Больше не долг.

## Замечания reviewer'а M-24 (2026-07-23)
- **RN-19 ✅ CLOSED** (merged `2a36c9f`, reviewer APPROVED 2026-07-23). architect добавил обе тай-брейк фикстуры в
  `red_volume_profile.rs`: `vp_poc_tie_goes_to_lowest_price` (равный макс-объём на 100 и 102 → POC=100) и
  `vp_value_area_tie_expands_upward` (`above==below` → VAH=103/VAL=102). Оба GREEN + анти-плацебо (обратный выбор
  стороны падает); `verify_M-24.sh` 6/6 PASS, `cargo test --workspace` 340/0 (reviewer перепрогнал независимо).
  Инвариант тай-брейков §Design теперь держится ТЕСТОМ. Ниже — исходное описание.
- **RN-19** (оракул-полнота: тай-брейки POC/VA реализованы верно, но не запинены фикстурой — класс «happy-path
  фикстуры», testing.md чек-лист #4 «Границы»). VP-I-1 требует «POC тай → низшая цена», VP-I-2 требует «VA тай
  above==below → верхний», и impl оба обрабатывает КОРРЕКТНО (reviewer проверил построчно: POC-компаратор
  `v1.cmp(v2).then(p2.cmp(p1))` даёт низшую цену при равном объёме; VA-ветка `above >= below` берёт верх при тае).
  Но НИ ОДНА sacred-фикстура не даёт РАВНЫХ объёмов: `vp_poc` (1/2/6/1) — уникальный max; `vp_value_area`
  (1/3/6/2/1) — на каждом шаге above≠below. Значит если бы impl выбрал НЕ ту сторону тая, оракулы всё равно
  прошли бы → инвариант держится реализацией, а не тестом. Не блокер (impl верен, поведенческий
  `red_gateway_live_eq_replay` покрывает accumulate/merge; MD-only низкий риск, critic по doc-гейту §9 не
  требовался). **Зона architect (RED-first):** добавить в `red_volume_profile.rs` фикстуру с РАВНЫМИ объёмами на
  двух ценах (POC-тай) и VA-шаг с `above==below` (VA-тай) — анти-плацебо на выбор неверной стороны. Тот же класс,
  что M-07/M-08 «идеальная фикстура» (закреплён в testing.md) — здесь пойман до вреда, но повтор под наблюдением.
- **RN-20** (seed/apply-асимметрия для кумулятивного-по-гистограмме индикатора — reviewer проверил вручную из-за
  отсутствия critic'а). VP, как и VWAP (RN-контекст M-20), session-накопителен, но БЕЗ time-bucket эмита — поэтому
  engine-dev сделал ПРОТИВОПОЛОЖНЫЙ VWAP выбор: `apply_vp` НЕ вызывается в seed (VWAP seed'ит аккумулятор без
  эмита). Это КОРРЕКТНО именно потому, что `merge_volume_profile` восстанавливает полную гистограмму из `bins` и
  складывает (ассоциативно), а не хранит только производные POC/VA. Урок для будущих gateway-индикаторов: правило
  seed-или-нет зависит от того, несёт ли delta ПОЛНОЕ состояние для пересчёта (VP: bins = полное → seed не нужен) или
  ИНКРЕМЕНТ, требующий прогрева (VWAP: sum_pv/sum_v кумулятивны в эмитируемом бакете → seed нужен). Оба держат
  live==replay, но по разным причинам — задокументировано, чтобы следующий индикатор не скопировал не тот паттерн.

## Замечания reviewer'а M-20 (2026-07-23)
- **RN-18** (intra-chain push gap — GREEN-коммиты в отдельном клоне, не на `origin/feat/M-NN`) engine-dev
  реализовал M-20 в ОТДЕЛЬНОМ клоне `/tmp/hft-engine-m20` (не worktree общего чекаута — `git worktree list`
  его не видит) и НЕ запушил GREEN на `origin/feat/M-20` (там остался только RED `d15e7a8`). tester был
  вынужден гонять гейты в engine-клоне; reviewer — фетчить объекты из `/tmp/hft-engine-m20` (`git fetch
  <path> feat/M-20`), проверять и лишь затем досводить `origin/feat/M-20` до GREEN + merge. Тот же класс, что
  M-22 N1 (коммиты висели на `engine-dev/M-22`, не на shared feat). Работает, но хрупко: коммиты живут ТОЛЬКО
  в локальном /tmp-клоне до reviewer'а — упади машина, цепочка теряется (ровно риск `branch-hygiene.md` +
  gates §8 intra-chain). **Правило (durable, зона architect — процессный слой):** мандат dev-агенту обязан
  явно требовать `git push origin HEAD:feat/M-NN` СВОИХ GREEN-коммитов ПЕРЕД handoff'ом на tester (не работа
  в приватном клоне); tester/reviewer бутстрапятся с `origin/feat/M-NN`, а не из чужого worktree. Внести в
  `.claude/rules/gates.md` §8 / handoff-мандат dev как обязательный пункт. Не блокер (M-20 смержен зелёным,
  аудит-трейл восстановлен), повтор класса — под наблюдением.

## Замечания reviewer'а M-22 (2026-07-22)
- **RN-17** (verify обязан зеркалить CI — 3-й инстанс класса RN-8) M-22 прошёл tester'ом с
  `verify_M-22.sh` VERDICT: PASS, но reviewer заблокировал merge: `cargo fmt --all -- --check`
  (ТОЧНЫЙ CI-гейт `ci.yml:20` build-test) = exit 1 на ВСЕХ 5 sacred `crates/gateway/tests/*.rs`
  (impl fmt-clean), а `verify_M-22.sh` fmt-гейта НЕ содержал вовсе → false-green. Merge дал бы
  красный CI на main + блок §8 (deploy `needs: ci`). Это ТРЕТИЙ инстанс одного класса: RN-8
  (M-05, fmt-гейт покрывал journal+book, не recorder) и clippy-gap M-18/TD-031 (verify без
  clippy-гейта → `assertions_on_constants` уехал бы на main). **Правило (durable): каждый
  `verify_M-NN.sh` обязан ЗЕРКАЛИТЬ терминальные гейты CI — `cargo fmt --all -- --check` И
  `cargo clippy --workspace --all-targets -D warnings` — теми же командами, что `ci.yml`.** Иначе
  acceptance даёт зелёный там, где CI красный, и дефект ловится только reviewer'ом/на main. Fix
  architect: `200e3ef` (fmt sacred тестов, whitespace-only, `git diff -w` пуст) + `49a03a6`
  (fmt-гейт добавлен ПЕРВЫМ в verify). Зона систематизации — architect (процессный слой): свести в
  `.claude/rules/gates.md` §3 требование «verify ⊇ CI-гейты» как обязательный пункт шаблона
  acceptance-скрипта. Не блокер (M-22 смержен зелёным), но повтор класса — под наблюдением.

## Замечания reviewer'а M-06 #4 (2026-07-11)
- **RN-9** (§8 eyes-on поймал то, что все зелёные гейты пропустили — снова) Code-review A+B
  #4 PASS: wiring engine-dev'а КОРРЕКТЕН (default_venues loop, `Box<dyn Fn>` type-erasure,
  supervise() неизменён, MD-only, boundary чист, fmt/clippy/workspace-test/verify_M-06 все
  GREEN на worktree). Дефект НЕ в #4-wiring, а в уже-смерженном (инертном) `venue-binance-futures`
  (venue-dev), который #4 сделал LIVE. Урок TD-011 подтверждён третий раз: «Deploy success» ≠
  «прод работает»; юнит-тесты futures-адаптера (фикстуры, offline) не могли поймать реакцию на
  реальный Binance rate-limit. **Wiring #4 сам по себе безупречен** — при фиксе TD-013 реленд
  тривиален. Для architect: RED-оракул фьючерс-адаптера должен включать симуляцию 418/429-ответа
  REST (прод-масштаб дисциплина `.claude/rules/testing.md`), не только happy-path парсинг.

## Замечания reviewer'а M-06 #4 reland (2026-07-12)
- **RN-11** (§8 split-result) Reland `8b26d6c` доказал, что TD-013 backoff больше не hot-loop'ит
  Binance 418, но одновременно показал новый live blocker: после backoff futures depth/funding
  не доходят до journal. Урок: RED #4 `default_venues` wiring достаточен для engine-dev contract,
  но не покрывает venue-runner liveness. Следующий RED должен быть не только "recorder wires
  BinanceFutures", а "futures runner under resync/backoff emits depth+funding".
- **RN-12** (§8 reland-2 oracle miss) `red_live_emit` + `FuturesSession` seam closed an obvious
  anti-placebo gap, but still did not model the live Binance sequence that keeps the adapter in
  gap/stale/backoff with 429 and no L2/Funding emission. Static delegation proof is necessary but
  insufficient; next chain must make the liveness oracle reproduce this prod failure mode before
  another #4 reland.
- **RN-13** (§8 TD-014 v2 miss) `71255c5` strengthened the lifecycle oracle enough to make L2
  nonzero in live, but not enough to satisfy product behavior: Funding stayed at 0, L2 cadence was
  sparse, and the runner continued gap/stale/429 churn. Next oracle must cover the actual persisted
  cadence and funding path under this churn, not only a deterministic recovery-snapshot unit path.
- **RN-14** (§8 TD-014 T2 split-result) `669ce40` fixed the futures continuity failure enough for
  dense live L2 and quiet post-startup logs, but the product gate still failed because no
  `BinanceFutures.Funding` reached the persisted journal. Next oracle must assert the real
  `!markPrice@arr` live path all the way to `MdPayload::Funding` in journal-recovered output, with
  observable parse/drop counters or equivalent diagnostics; parser-only fixtures are no longer
  sufficient.
- **RN-15** (§8 TD-014 T3 miss) `99b1329` added per-symbol `<sym>@markPrice@1s` and passed the
  new RED, but live still persisted zero Funding while L2/OI were healthy and churn was gone.
  This rules out recorder wiring and most depth-sync starvation theories. Next cycle should add
  raw WS markPrice observability before another reland: count received stream names, parse failures,
  symbol-filter drops, and emitted Funding, or capture a short live fstream sample under the exact
  combined URL. Without those counters, local RED can keep proving paths for messages that prod
  never receives or silently drops.
- **RN-16** (§8 TD-014 T4 closure) REST `/fapi/v1/premiumIndex` polling closed the live Funding
  gap immediately. Lesson: after repeated WS-delivery misses, prefer the already live-proven REST
  ingestion path over more parser/session oracles for a stream the exchange/network is not
  delivering in this deployment. Keep WS markPrice parser tests as regression coverage, but do not
  depend on WS markPrice for production funding-breadth until a separate raw-capture task proves
  delivery.

## Замечания reviewer'а M-05 (не блокирующие, 2026-07-11)
- **RN-8** (fmt-гейт под-покрытие) `verify_M-05.sh` fmt-гейт проверяет только `journal+book`, не
  `recorder` — из-за чего v2 recorder-файлы без trailing newline (`cargo fmt --all --check` FAIL)
  прошли verify зелёным. Поймано reviewer'ом вручную (`cargo fmt --all`), engine-dev пофиксил
  (`7db4479`). → architect: расширить fmt-гейт verify_M-05.sh на recorder. Также урок: verify-скрипт
  milestone'а обязан fmt-check ВСЕ тронутые крейты, не подмножество.
- **RN-4** (AUDIT sacred-файла) engine-dev правил `scripts/verify_M-05.sh` (architect/tester-owned,
  SACRED per scope-guard) в коммите `2a21b8c` (task #4). Правка УЗКАЯ: замена placeholder
  `echo PENDING J1 + FAIL++` на реальный прогон `run "J1 …" cargo test -p recorder --test
  red_shutdown_j1` — оракул J1 стал доступен после task #2. Reviewer подтверждает допустимость:
  (а) явная авторизация founder'а на эту J1-строку; (б) правка НЕ ослабляет гейт — конвертирует
  форсированный FAIL в честный тест-прогон; (в) сверено построчно — J2/J3/B1/fmt/clippy-строки и
  FAIL-агрегатор не тронуты. РЕВЕРТ НЕ ТРЕБУЕТСЯ. На будущее: wiring sacred-скрипта — отдельный
  коммит tester/architect (паттерн M-06 task 6), не бандл в feature-коммит dev'а.
- **RN-5** (partial-merge, founder-authorized) engine-dev part M-05 (tasks 2/3/4) смержен в `main`
  ДО полного close-out milestone'а. `verify_M-05.sh` → `VERDICT: FAIL (1)`, и единственный FAIL —
  `B1 resnapshot anti-phantom` (venue-dev task 5) PENDING, ортогональный к journal/recorder-фиксу.
  Push разрешён явным founder-override правила auto-push-only-on-exit-0 (B1 не в зоне engine-dev,
  фикс journal-integrity прод-критичен). Milestone остаётся IN_PROGRESS до B1 (task 5) + wiring
  task 6 (verify exit 0). НЕ close-out. **⚠ ОТКАЧЕНО через ~4 мин — прод-регрессия, см. TD-011.**
  Урок: eyes-on §8 ssh-проверка ОБЯЗАТЕЛЬНА и поймала то, что зелёный CI/юнит-тесты/Deploy-success
  пропустили; «Deploy success» ≠ «прод пишет данные».
- **RN-6** (DET-I-1 подтверждение) `read_all` остался STRICT (Err на первом CRC-mismatch +
  postcard-decode→Err — сверено на `b22583c`); resync-толерантность вынесена в ОТДЕЛЬНУЮ
  `recover()` (честный побайтовый ресинк, без rand/wall-clock). DET-I-1 exact-replay НЕ ослаблен.
  `next_seq = meta.max(seg-scan)` — источник истины сегмент, reuse исключён (мета-lag не даёт
  отката; мета-ahead даёт gap, не reuse — оба безопасны для монотонности).

## Замечания reviewer'а M-04 (не блокирующие, 2026-07-10)
- **RN-1** (NOTE) `verify_M-04.sh` T6 объединяет `contracts+journal+book` в один `check` —
  провал любого из трёх не различается по строке. Приемлемо для регресс-гейта (все GREEN),
  но при росте числа крейтов стоит разнести на per-crate строки для точной диагностики.
- **RN-2** (NOTE) Латентность δ_md — эмпирика из журнала, но δ_submit/δ_cancel — measured WS
  RTT ×2 (пессимизм-прокси, НЕ реальный order-path замер: P1 order-path ещё нет, D7 это
  честно фиксирует в provenance). Честность δ_submit/δ_cancel обязана быть предметом
  risk-critic на отчёте R-001 (чувствительность ×2 латентности per gates.md §6.4) — уже
  учтено дизайном стресс-вариантов, отмечаю для явной проверки на задаче 8.

## Замечания reviewer'а (фикс ts_exch_ms=0 у L2Snapshot, 2026-07-11)
- **RN-3** (NOTE) В фикс-коммите `1477bca` sacred inline-модуль `ts_exch_tests`
  (architect-owned) получил rustfmt-переносы (multi-line `assert_eq!`/let-else/let-binding).
  Сверено построчно: семантика тестов идентична (те же литералы 1_752_000_000_123 / 777_000 /
  1_600_000_000_000, те же ассерты и сообщения, та же структура). Переформатирование
  ВЫНУЖДЕНО гейтом `verify_M-04.sh` T1a (`cargo fmt --check`) — architect закоммитил RED-тесты
  с строками >100 col (допустимо: compile-RED всё равно не собирается), а GREEN обязан пройти
  fmt-гейт. Приемлемо (whitespace-only, semantics-preserving); отмечено для аудита касания
  sacred-файла dev-агентом.

## CLOSED
- **TD-011** `scan_next_seq-full-segment-read-oom` (M-05 task#3) — **RESOLVED 2026-07-11**.
  Инцидент: v1 `Journal::open()` делал `read_to_end` ВСЕГО сегмента (прод 2.65 GiB) в RAM на каждом
  старте → recorder не писал (101% CPU, 2.48 GiB RAM, OOM-риск); юнит-RED на крошечных фикстурах не
  поймал; healthcheck обманут; поймано eyes-on §8. Откачено (`c2ad02c`/`ffdc410`/`e190356`).
  ФИКС (v2, `a356c81`): `scan_tail_for_last_seq` — читает последние ≤4 MiB (seek+read_exact),
  `next_seq = max(meta, tail+1)`, O(1) память. Верификация: (а) architect RED-оракул
  `red_open_bounded.rs` (64 MiB + counting-allocator, бюджет 8 MiB) GREEN; (б) reviewer НЕЗАВИСИМЫЙ
  прод-масштаб харнес (2.94 GiB): open()=4 ms, max RSS 6 MiB, next_seq корректен; (в) eyes-on §8 на
  VPS после merge/deploy: новый recorder пишет (CPU 0.53%, MEM 5.41 MiB, tail-scan реального 2.71 GiB
  прод-сегмента → `next_seq=3467845`, сегмент растёт). Урок закреплён в `.claude/rules/testing.md`
  (прод-масштаб RED для sacred I/O) + RN-8 (fmt-гейт под-покрытие).
