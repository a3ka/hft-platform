# R-008 — PR-гейт `feat/alerting`, второй круг (после R-005)

- **Дата (UTC):** 2026-08-01
- **Ветка:** `feat/alerting`, HEAD `e67d1fd`
- **База сравнения:** `origin/main` (`0307035`)
- **Предыдущий вердикт:** `research/reviews/R-005-alerting.md` — REJECTED (7 находок, 2 блокера)
- **Роль:** reviewer (PR-time гейт, `.claude/rules/gates.md` §4)

## ВЕРДИКТ: **REJECTED**

**Один блокер остаётся: F-2 закрыт частично.** Основной путь утечки
(`reqwest::Error` → `TransportError`) действительно вылечен и подтверждён моими собственными
прогонами. Но `TELEGRAM_BOT_TOKEN` по-прежнему попадает в cron-лог по ВТОРОЙ ветке того же
`send()` — ответ с не-2xx статусом печатается телом наружу дословно. Эта ветка не покрыта
НИ ОДНИМ оракулом.

Шесть остальных находок (F-1, F-3, F-4, F-5, F-6, F-7) — **устранены**, проверено по
существу, а не по факту зелёного теста: чтением логики, мутационным контролем и сквозным
прогоном реального бинаря.

Отдельно — **новая находка F-8** (MAJOR, НЕ блокер): регрессия `next_seq` даёт вечную
ложную тревогу `WD-SEQ-STALLED`.

---

## Block-scope

`git diff --name-only origin/main...HEAD` — 19 файлов, все в разрешённой зоне:
`crates/ops/**`, `scripts/`, `docs/runbooks/`, `research/reviews/`, `Cargo.lock`.

```
$ git diff --name-only origin/main...HEAD | grep -E 'crates/(journal|contracts|risk|killswitch|oms|venue-)'
NONE - clean
```

`crates/journal/**`, `crates/contracts/**` (T1), `crates/risk/**`, `crates/killswitch/**`,
`crates/oms/**`, `crates/venue-*/**` — **не тронуты**.

- **Block-C (contract governance):** N/A — `crates/contracts/**` в дифе отсутствует.
- **Block-risk (RISK-BLOCK, `gates.md` §5):** N/A — safety-путь не тронут. `ops` —
  read-only наблюдатель (читает `recorder.heartbeat`, маркеры, `docker ps`), order-egress
  отсутствует. risk-critic не требуется.

`Cargo.lock` + `crates/ops/Cargo.toml` — только добавления (`reqwest`, `anyhow`, `tempfile`),
чужие зависимости не тронуты (`.claude/rules/scope-guard.md`, shared-access правило).

## Block-DoneBlock (мои собственные прогоны, сырой агрегированный вывод)

```
$ df -h /home | tail -1
/dev/md2  437G  381G   34G  92% /            ← места хватает; ложных exit=101 из прошлого инцидента нет

$ cargo test -p ops 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" (блоков: "NR")"}'
passed=146 failed=0 (блоков: 17)

$ cargo test --workspace 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" (блоков: "NR")"}'
WORKSPACE passed=644 failed=0 (блоков: 168)

$ cargo clippy -p ops --all-targets -- -D warnings 2>&1 | tail -3; echo clippy_exit=$?
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
clippy_exit=0

$ cargo fmt --check --all; echo fmt_exit=$?
fmt_exit=0

$ bash scripts/verify_alerting.sh 2>&1 | grep -E "^(PASS|FAIL|VERDICT)"; echo exit=$?
PASS  cargo fmt --check (весь workspace)
PASS  cargo clippy -p ops --all-targets -D warnings
PASS  бинарь ops-watchdog собирается
PASS  существует crates/ops/tests/red_ops_watchdog_cycle.rs
PASS  существует crates/ops/tests/red_ops_transport_redaction.rs
PASS  оракул интервал-независимости присутствует
PASS  оракулы F-1 зелёные (6)
PASS  склейка переехала в библиотеку: бинарь зовёт run_cycle
PASS  старой склейки в бинаре нет (run_heartbeat_checks)
PASS  старой склейки в бинаре нет (push_or_clear)
PASS  оракулы редакции секрета зелёные (включая сквозной прогон бинаря)
PASS  сырой reqwest-error больше не кладётся в TransportError
PASS  хардкоженых токенов в crates/ops/src, scripts, deploy нет
PASS  оракулы F-3 зелёные (5)
PASS  оракулы F-5 зелёные (3)
PASS  оракулы F-6 зелёные (4)
PASS  код инцидента WD-CRON-FAILED объявлен в крейте
PASS  оракулы F-7 зелёные (3)
PASS  здоровый прод не шумит; состояние ограничено за неделю (2)
PASS  весь крейт ops зелёный
PASS  workspace зелёный
VERDICT: PASS
exit=0
```

Гейты зелёные. **Вердикт REJECTED вынесен НЕ по красному гейту, а по дефекту, который
зелёные гейты не видят** — ровно тот класс, ради которого существует PR-гейт.

---

## Статус семи находок R-005

| # | Находка | Статус |
|---|---|---|
| F-1 | `WD-SEQ-STALLED` выключается интервалом cron | ✅ **УСТРАНЕНА** |
| F-2 | `TELEGRAM_BOT_TOKEN` течёт в лог | ⚠️ **ЧАСТИЧНО — БЛОКЕР ОСТАЁТСЯ** |
| F-3 | Ложная тревога по диску в окно компакции | ✅ **УСТРАНЕНА** |
| F-4 | RED-first вывернут (тесты писал автор impl) | ✅ **УСТРАНЕНА** (процессно) |
| F-5 | «не смог оценить» стирает дедуп-память | ✅ **УСТРАНЕНА** |
| F-6 | Маркеры `*.alert` не читаются | ✅ **УСТРАНЕНА** |
| F-7 | Рестарт-петля подавляется после первого сообщения | ✅ **УСТРАНЕНА** |
| F-8 | *(новая)* Регрессия `next_seq` → вечная ложная тревога | 🆕 **MAJOR, не блокер** |

---

### F-1 — УСТРАНЕНА (проверено по существу)

Прошлый круг провалился именно здесь, поэтому проверял логику, а не зелёный тест.

**Механизм фикса корректен.** `state::WatchdogState` получил отдельный якорь
`seq_progress_heartbeat` / `seq_progress_check_ms`, который в
`watchdog_cycle::run_seq_stalled_check` двигается **только** при реальном росте `next_seq`:

```rust
(Some(anchor_hb), Some(_)) if hb.next_seq > anchor_hb.next_seq => { /* якорь переезжает */ }
(Some(anchor_hb), Some(anchor_ms)) => {
    // Прогресса нет — якорь НЕ двигаем
    match check_seq_stalled(&anchor_hb, anchor_ms, hb, now_ms, thr) { ... }
}
```

Возраст застоя = `now_ms − anchor_ms` растёт с **реальным** временем; порог
`seq_stall_min_gap_ms` (60 с) преодолевается при любом интервале cron. Старый баг
(`prev_heartbeat`/`prev_check_ms` двигались каждый цикл → расстояние навсегда равно интервалу
cron) технически невозможен: `prev_*` в диагностике больше не участвуют, они оставлены только
ради roundtrip-совместимости состояния.

**Персистентность проверена** — это второе место, где фикс мог быть иллюзорным: watchdog
запускается cron'ом, то есть каждый такт — НОВЫЙ процесс. Оба поля якоря помечены
`#[serde(default)]` и входят в сериализуемый `WatchdogState`; харнесс оракула `CronSim::tick`
гоняет состояние **через JSON на каждом такте** (`load_or_default` → `run_cycle` → `save`),
то есть оракул падает и на реализации «история живёт только в памяти процесса».

**Мутационный контроль (мой собственный, M1):** вернул старое поведение — якорь двигается
каждый такт:

```
===== M1 (F-1: якорь двигается каждый цикл — старый баг) =====
  test f1_stall_after_normal_progress_is_measured_from_last_progress ... FAILED
  test f1_seq_stall_is_detected_at_every_realistic_cron_interval ... FAILED
  test result: FAILED. 21 passed; 2 failed
```

Падают ИМЕННО F-1-оракулы. Главный из них проверяет 4 реальных интервала
(30 с / 1 мин / 2 мин / 5 мин) на 30 минутах вставшего сбора и зажимает срабатывание с ОБЕИХ
сторон (`>= seq_stall_min_gap_ms` — анти-флап сохранён, `<= min_gap + interval` — не позже
первого такта за порогом). «Фикс, который алертит всегда» этим оракулом не проходит, а парный
vantage `f1_healthy_growth_never_fires_seq_stalled_at_any_interval` ловит его отдельно.

### F-2 — ЧАСТИЧНО. **БЛОКЕР ОСТАЁТСЯ**

**Что реально починено.** `TransportError` больше не несёт `reqwest::Error`.
`redact_reqwest_error` построен целиком из статических строк и вообще не читает содержимое
ошибки (ни `Display`, ни `Debug`, ни `source()`) — секрет технически не может пройти через
классификатор. Это правильное решение правильного класса.

Проверил сам, подставив маркер-токен и прогнав РЕАЛЬНЫЙ бинарь по четырём сетевым сценариям
(маркер `8899001122:REVIEWERxSECRETxMARKERxZZZ`, счётчик вхождений в stdout и stderr):

```
== connect refused (http://127.0.0.1:1) ==       stdout: 0   stderr: 0
== DNS unresolvable (.invalid) ==                stdout: 0   stderr: 0
== malformed base URL ==                         stdout: 0   stderr: 0
== https на закрытый порт (TLS/connect) ==       stdout: 0   stderr: 0

stderr (типичная строка):
[ops-watchdog] TelegramTransport::send failed: transport http error: telegram transport
error (connect): не удалось доставить сообщение в Telegram Bot API (детали редактированы
— R-005 F-2, секрет живёт в URL значения ошибки)

$ grep -c "$MARKER" watchdog.state.json
0
```

**Что НЕ починено.** У `TelegramTransport::send` ДВЕ ветки отказа. Отредактирована одна.
Вторая — `crates/ops/src/transport.rs:135-141`:

```rust
if !resp.status().is_success() {
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    return Err(TransportError::Http(format!(
        "Telegram API вернул {status}: {body}"
    )));
}
```

`body` — это **данные, контролируемые удалённой стороной**, а не собственная строка процесса.
Токен вшит в путь запроса (`/bot<token>/sendMessage` — требование самого Bot API), поэтому
любой ответ, эхом возвращающий URI (страница ошибки прокси/CDN/captive-portal, неверно
настроенный `TELEGRAM_API_BASE`, MITM), кладёт секрет в cron-лог.

**Воспроизведение (мой прогон, реальный бинарь, локальный сервер отдаёт 404 с эхом URI):**

```
$ head -1 stderr
[ops-watchdog] TelegramTransport::send failed: transport http error: Telegram API вернул
404 Not Found: <html>404 Not Found: the requested URI
/bot8899001122:REVIEWERxSECRETxMARKERxZZZ/sendMessage was not found on this proxy</html>

$ grep -c "$MARKER" stderr.txt
5          ← по одному на каждый доставляемый алерт
```

Симптом: `scripts/watchdog_cron.sh` делает `>>"${LOG}" 2>&1` — секрет оседает в файле на VPS
навсегда и молча.

**Почему это блокер, а не заметка.**

1. Это ТА ЖЕ находка F-2, в ТОЙ ЖЕ функции. Собственное обоснование фикса гласит: «НИКАКОЙ
   путь печати не безопасен — секрет живёт в значении», после чего вторая ветка исключается
   из этого правила предположением о поведении УДАЛЁННОГО сервера («`Telegram API вернул
   {status}: {body}` не содержит URL» — doc-comment `transport.rs:157`). Инвариант «секрет
   технически не может достичь лога» здесь не выполняется.
2. **Дефект оракула — фикстура счастливого пути** (`.claude/rules/testing.md`, правило,
   закреплённое 2026-07-14 после двух milestone'ов подряд). Ни один F-2-оракул не подаёт
   не-2xx ответ вообще: `f2_watchdog_binary_never_prints_the_token_into_the_cron_log` бьёт по
   мёртвому эндпоинту (ветка `connect`), `f2_successful_delivery_still_hits_telegram_bot_api_path`
   отдаёт `HTTP/1.1 200 OK {"ok":true}`. Ветка `!status.is_success()` не исполняется ни разу:

```
$ grep -n "404\|401\|is_success\|HTTP/1.1" crates/ops/tests/red_ops_transport_redaction.rs
149: sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\n\r\n{\"ok\":true}")
```

Пропущены пункты чек-листа деградированного входа: **множественность** (у отказа две ветки,
покрыта одна) и **отсутствие/враждебность** (ответ сервера — не доверенная строка).

Дизайн фикса и RED-оракул на регресс — зона architect (`gates.md` §4, граница
reviewer↔architect). Я фиксирую дефект и непокрытую ветку, решение не проектирую.

### F-3 — УСТРАНЕНА

Тренд считается не по соседней паре сэмплов, а по истории `state::disk_history` с горизонтом
`DISK_TREND_HORIZON_MS = 2ч` (окно обслуживания 03:50→04:07 UTC покрыто целиком) и
минимальным накопленным спаном `DISK_TREND_MIN_SPAN_MS = 30 мин` — холодный старт на двух
сэмплах не читается как достоверный тренд. Абсолютные backstop'ы (`free ≤ min_free`,
`< 3×min_free` без истории) не тронуты — они и не были подвержены этому классу.

Мутационный контроль (M5 — вернул горизонт 400 с и min-span 0, т.е. «тренд по соседней паре»):

```
  test f3_cold_start_spike_without_history_does_not_project_exhaustion ... FAILED
  test f3_nightly_maintenance_spike_does_not_raise_disk_alert ... FAILED
  test result: FAILED. 21 passed; 2 failed
```

Парный vantage `f3_sustained_real_decline_still_raises_critical` подтверждает, что сглаживание
не превратилось в «молчать всегда», а `f3_absolute_floor_alerts_immediately_without_history` —
что пол срабатывает мгновенно.

### F-4 — УСТРАНЕНА (процессно)

```
$ git log --format='%h %an <%ae> %s' af135b3~2..HEAD
e67d1fd engine-dev  docs(alerting): runbook ...
d129120 engine-dev  fix(ops): R-005 F-1/F-3/F-5/F-6/F-7 — слой склейки watchdog_cycle::run_cycle
9747538 engine-dev  fix(ops): R-005 F-2 — TELEGRAM_BOT_TOKEN больше не течёт в TransportError
af135b3 architect   test(ops): acceptance-гейт scripts/verify_alerting.sh
f726fd8 architect   test(ops): RED-оракулы на находки R-005 F-1..F-7 (сценарные, слой склейки)

$ git diff --stat af135b3..HEAD -- crates/ops/tests
(пусто)
```

Оракулы (`f726fd8`) и acceptance-скрипт (`af135b3`) написаны architect'ом ДО реализации
(`d129120`); dev тесты не трогал ни разу. Порядок RED→GREEN соблюдён, авторство разделено.
Дополнительно: оракулы стали **сценарными** (последовательность запусков cron с состоянием
через JSON), а не «вызов детектора с подставленными аргументами» — это и есть тот слой, в
котором жили оба блокера.

### F-5 — УСТРАНЕНА

Введён трёхвариантный `Verdict`: `Healthy` (снять подавление), `Alert`, `Unknown` (дедуп-память
не трогать ни в какую сторону). `Unknown` возвращается там, где источник не дал оснований
судить: гэп с якоря меньше анти-флап-порога; `free_bytes`/`min_free_bytes` отсутствуют
(recorder не смог `statvfs`); первый в жизни сэмпл. Нечитаемый heartbeat приводит к раннему
`return` до остальных проверок — якоря не сбрасываются.

Мутационный контроль (M2 — `Verdict::Unknown => self.state.clear(&key)`):

```
  test f5_unknown_disk_reading_does_not_reset_dedup_of_disk_low ... FAILED
  test result: FAILED. 22 passed; 1 failed
```

Парный vantage `f5_genuine_recovery_does_reset_dedup_so_the_next_incident_is_delivered`
подтверждает, что настоящее выздоровление подавление всё-таки снимает.

*Заметка (не находка):* мутация M2 убила только диск-оракул; `f5_unknown_verdict_does_not_
reset_dedup_of_seq_stalled` её пережил — в его сценарии ручной прогон происходит через
5 мин 5 с после якоря, то есть вердикт там `Alert`, подавленный окном, а не `Unknown`.
Инвариант F-5 покрыт (диск-оракул ловит), но конкретно seq-ветка `Unknown` проверяется
слабее, чем обещает имя теста.

### F-6 — УСТРАНЕНА

Добавлены `CronJobObservation::failure` / `CronFailureMarker` и отдельный код инцидента
`WD-CRON-FAILED` (`Incident::CronFailed`, CRITICAL), проверяемый **независимо** от
`CronMarkerStale` (та молчит ещё 26 ч, ориентируясь только на позитивный `.last-success`).
Бинарь читает пару `<job>.last-success` + `<job>.alert` для всех трёх задач
(`compaction`/`gateway-checkpoint`/`retention`). Fail-closed: сам факт наличия файла алертит,
даже если таймстамп в первой строке не распарсился.

Мутационный контроль (M4 — маркер сбоя игнорируется):

```
  test f6_failed_cron_run_is_detected_immediately_from_alert_marker ... FAILED
  test f6_failure_marker_with_unparseable_timestamp_still_alerts ... FAILED
  test f6_two_failed_jobs_produce_two_distinct_delivered_alerts ... FAILED
  test result: FAILED. 20 passed; 3 failed
```

### F-7 — УСТРАНЕНА

`ContainerRestarted` идёт через `Cycle::record_always_delivered` — обходит дедуп-окно целиком.
Обоснование корректно: `check_container_restarted` по построению срабатывает ТОЛЬКО когда
`RestartCount` вырос с прошлого такта, то есть каждое срабатывание уже является новым фактом;
окно здесь не защищало ни от чего, а глушило текущие рестарты. Рестарт-петля продолжает
сообщать о себе на каждом такте, а не замолкает после первого алерта.

Мутационный контроль (M3 — вернул рестарт в обычный дедуп):

```
  test f7_multi_restart_jump_within_dedup_window_is_delivered_with_actual_counts ... FAILED
  test f7_restart_loop_keeps_reporting_across_dedup_windows ... FAILED
  test result: FAILED. 21 passed; 2 failed
```

Парный vantage `f7_stable_container_after_restart_burst_stops_reporting` подтверждает, что
обход дедупа не превратился в «шуметь вечно»: стабилизировавшийся контейнер замолкает.

### F-8 (НОВАЯ, MAJOR, не блокер) — регрессия `next_seq` даёт вечную ложную тревогу

**Где:** `crates/ops/src/watchdog_cycle.rs::run_seq_stalled_check`.

**Симптом.** Якорь переезжает только по условию `hb.next_seq > anchor_hb.next_seq`. Если
`next_seq` УМЕНЬШИЛСЯ (том журнала пересоздан / восстановление из бэкапа / чистый деплой на
новый volume), условие не выполнится, пока сбор не догонит прежнее значение. Всё это время
`check_seq_stalled` видит `cur.next_seq <= prev.next_seq` и выдаёт CRITICAL — при том что
данные РЕАЛЬНО идут. Якорь при этом не двигается никогда — состояние самовосстановиться не
может.

**Воспроизведение (мой прогон, временный проб-тест, удалён после замера):** 5 тактов
нормального роста, затем том пересоздан (`next_seq` с нуля, растёт прод-темпом 96 ев/с):

```
PROBE: за 24 тиков (2 часа) ПОСЛЕ пересоздания тома при РЕАЛЬНО растущем seq
       WD-SEQ-STALLED сработал на 24 тиках
assertion `left == right` failed: ложная тревога: сбор идёт, а детектор кричит о застое
  left: 24
 right: 0
```

При прод-значении `next_seq ≈ 1.4e8` и темпе 96 ев/с догон занял бы ~17 суток непрерывного
ложного CRITICAL.

**Почему MAJOR, а не блокер:** это ложная тревога (fail-loud), а не пропуск (fail-silent), и
требует потери/пересоздания тома — события, при котором и без того сработают
`WD-HB-MISSING`/`WD-CONTAINER-*`. Но застрявший CRITICAL маскирует настоящий застой и ведёт
ровно к тому, что runbook сам называет причиной отключения алертинга. Класс — тот же, что
F-3/F-5: «не смог оценить» / «сменилась система отсчёта» ≠ «беда».

Проектирование защиты и RED-оракул — зона architect.

---

## Прочие наблюдения (не блокируют, в долг)

- **Тело ответа печатается без ограничения длины** (`transport.rs:137`) — HTML-страница
  ошибки от прокси уходит в cron-лог целиком, ×N алертов. Смежно с F-2.
- **`disk_history` не ограничена по числу элементов**, только по горизонту (2 ч). При
  штатном cron (5 мин) это 24 записи — оракул
  `state_file_stays_bounded_over_a_week_of_cron_runs` это фиксирует. При аномально частом
  запуске рост линеен по частоте. Практического риска нет, отмечаю для полноты.
- **У watchdog'а нет watchdog'а** — если crond умер, о застое не узнает никто. Это явно
  названо в doc-комментарии бинаря и в runbook'е как вне scope; фиксирую как известное
  ограничение включения.

## Что требуется для повторного круга

1. Закрыть F-2 полностью: ветка `!resp.status().is_success()` не должна быть способна
   вынести содержимое URL наружу. Дизайн — architect.
2. RED-оракул на не-2xx ответ с враждебным телом (эхо URI) — сейчас ветка не исполняется
   ни одним тестом.
3. F-8 — решение по регрессии `next_seq` (либо защита, либо явно принятое ограничение в
   runbook + TECH-DEBT).

Следующий агент — **architect** (дизайн защиты + RED-оракулы), затем engine-dev на GREEN.

## Что НЕ сделано вследствие REJECT

- Merge в `main` не выполнен.
- `PROJECT-STATE.md` / `TECH-DEBT.md` не обновлены (обновляются после APPROVED).
- Деплой-гейт (`gates.md` §8) не запускался — деплоить нечего.
- `docs/PENDING-SIGNATURE.md` (П-003) не трогал: пока код не принят, отметка «готово к
  включению» была бы преждевременной.
- Cron на VPS не устанавливался, `TELEGRAM_BOT_TOKEN` нигде не прописывался — действия
  founder'а.
