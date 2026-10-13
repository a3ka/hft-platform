# R-010 — PR-гейт `feat/alerting` rev4 (круг 4)

- **Дата (UTC):** 2026-08-01T03:30Z
- **Ветка:** `feat/alerting`, HEAD `abae7f4` — `fix(ops): R-009 F-11 — блокировать HTTP-редиректы в TelegramTransport`
- **База:** `origin/main` (19 коммитов в диффе, +6248 строк)
- **Предыдущие круги:** `R-005` (7 находок), `R-008` (F-2 частично + F-8), `R-009` (F-9/F-10/F-11)
- **Роль:** reviewer, чистый worktree `/tmp/hft-rev-alerting4` (detached `origin/feat/alerting`)

## ВЕРДИКТ: **APPROVED** — merge в `main`

Три блокера круга 3 закрыты и **закреплены обратной мутацией** (главный вопрос этого круга).
Четвёртого пути утечки токена не найдено — прокси-путь проверен эмпирически и закрыт TLS.
Остаточные находки (F-12/F-13/F-14) — MAJOR/MINOR, не блокеры, заведены в `TECH-DEBT.md`.

---

## 1. Обратная мутация — ГЛАВНОЕ этого круга

Оракул, зелёный с первого запуска против уже сделанного фикса, не доказывает НИЧЕГО.
`R-009` возник ровно из этого: фиксы F-2 rev2 и F-8 были верны по существу, но их откат
проходил все гейты (`passed=146 failed=0`, `VERDICT: PASS`). Требование к rev4 —
оракулы F-9/F-10 обязаны **КРАСНЕТЬ** на откате. Проверено фактически, три мутации:

| Мутация | Что откачено | Ожидание | Факт |
|---|---|---|---|
| **A** | `transport.rs`: возврат `resp.text()` в `TransportError` (`"Telegram API вернул {status}: {body}"`) — откат фикса R-008 F-2 rev2 | F-9 краснеет | **КРАСНЫЙ: 3 падения** (`passed=74 failed=3`) |
| **B** | `watchdog_cycle.rs`: ветка `check_seq_regressed` удалена целиком (26 строк) — откат фикса R-008 F-8 | F-10 краснеет | **КРАСНЫЙ: 4 падения** (`passed=144 failed=4`) |
| **B2** | `watchdog_cycle.rs`: ветка на месте, убраны ровно ДВЕ строки сброса якоря — **ядро** фикса F-8, та самая мутация, что в R-009 прошла все гейты | F-10 краснеет | **КРАСНЫЙ: 2 падения** (`passed=146 failed=2`) |
| **C** | `transport.rs`: убрана `.redirect(Policy::none())` — откат фикса F-11 | F-11 краснеет | **КРАСНЫЙ: 6 падений** (`passed=79 failed=6`) |

Имена упавших тестов:

```
Мутация A (F-9):
  f9_non_2xx_response_body_never_reaches_the_transport_error
  f9_huge_hostile_body_is_not_read_into_the_error_even_truncated
  f9_watchdog_binary_never_logs_the_non_2xx_response_body

Мутация B (F-10):
  f10_unreadable_heartbeat_tick_neither_fakes_nor_hides_a_regression
  f10_next_seq_regression_fires_its_own_critical_incident_not_a_stall
  f10_two_independent_regressions_within_one_dedup_window_are_both_delivered
  f10_after_regression_normal_growth_from_the_new_baseline_stays_silent_for_a_day

Мутация B2 (F-10, ядро фикса):
  f10_two_independent_regressions_within_one_dedup_window_are_both_delivered
  f10_after_regression_normal_growth_from_the_new_baseline_stays_silent_for_a_day

Мутация C (F-11):
  f11_same_host_relative_redirect_is_not_followed_and_sends_no_referer
  f11_cross_host_redirect_never_delivers_the_token_to_the_other_host
  f11_every_redirect_status_is_blocked_not_just_307
  f11_redirect_chain_stops_at_the_configured_host
  f11_scheme_relative_location_does_not_smuggle_the_token_to_another_host
  f11_watchdog_binary_never_sends_the_token_to_a_redirect_target
```

**Ключевое.** B2 — это в точности мутация, которая в круге 3 дала `146 passed / 0 failed` и
`VERDICT: PASS`, то есть переоткрывала блокер незаметно для ВСЕХ гейтов. Теперь она валит
2 поведенческих оракула (не clippy, не линтер). Закрепление настоящее, **F-9 и F-10 ЗАКРЫТЫ**.

Все мутации откачены; `git status --porcelain` пуст, дифф `origin/feat/alerting..worktree` пуст.

---

## 2. F-11 по существу — блокировка редиректов

### (а) Клиент фактически не следует за 3xx

Проверено на живом зонде (мутация C выше плюс ручной прогон настоящего бинаря против
локального сервера, отвечающего `301` с `Location: http://evil.example/x`):

- запрос уходит **только** на сконфигурированный эндпоинт (`host_a.hits() == 1`);
- на хост-цель редиректа не приходит **ничего** (`assert_untouched` — 0 запросов);
- `Referer` не отправляется ни разу, включая относительный `Location` на тот же хост;
- цепочка A→B→C обрывается на первом шаге;
- схемо-относительный `Location: //host:port/path` (выглядит относительным, уводит на чужой
  хост) — та же блокировка.

Класс решения **правильный**. Альтернатива «следовать, если тот же хост» опирается на данные
удалённой стороны (`Location` бывает относительным, схемо-относительным, цепочечным) — надёжной
такая проверка не бывает; Telegram Bot API штатно не редиректит вовсе, так что цена запрета
нулевая. Фикс стоит в `with_credentials_and_endpoint` — ЕДИНСТВЕННОЙ точке конструирования
клиента (`from_env`/`with_credentials` идут через неё), обойти его неоткуда.

### (б) Что видит оператор при легитимном 3xx — НЕ молчаливая потеря

Прогон настоящего `ops-watchdog` против редиректящего сервера, сырой операторский вид
(stdout+stderr, как их сливает cron в `/var/log/hft/watchdog.log`):

```
[CRITICAL] WD-HB-STALE — recorder heartbeat не обновлялся ... | host: probe | runbook: docs/runbooks/alerting.md#wd-hb-stale
[ops-watchdog] TelegramTransport::send failed: transport http error: Telegram API вернул неуспешный статус 301 Moved Permanently (тело ответа намеренно не печатается — R-008 F-2, может нести секрет из URL запроса)
...
[ops-watchdog] 1785554683096 — обнаружено 4 алертов (4 отправлено, 0 подавлено дедупликацией)
EXIT=0
```

- **текст алерта сохранён** — `StdoutTransport` отрабатывает ДО телеграма и безусловно
  (`bin/ops-watchdog.rs:86`), канал «лог cron'а» остаётся рабочим;
- **отказ доставки назван явно**, отдельной строкой на КАЖДЫЙ алерт, с редакцией по F-2/F-9;
- **токен в выводе: 0 вхождений** (grep по маркеру `AAG-OPERATOR-PROBE-SECRET`).

Сообщение не теряется молча. **F-11 ЗАКРЫТА.** Остаточное — см. F-12 ниже: `EXIT=0`, то есть
эскалация обёртки не срабатывает.

---

## 3. Четвёртый путь утечки — искал, НЕ НАШЁЛ

Три круга дали три разных класса (содержимое ошибки → тело ответа → заголовок при редиректе).
Проверены все названные поверхности:

| Поверхность | Метод | Результат |
|---|---|---|
| **Прокси из env** (`HTTPS_PROXY`/`ALL_PROXY`) | эмпирический зонд: локальный raw-сервер как прокси, продовый эндпоинт `https://api.telegram.org` | **ЗАКРЫТО.** Прокси получает ровно `CONNECT api.telegram.org:443` + `Host:` — путь URL внутри TLS-туннеля. `token present on proxy: false` |
| **DNS-резолвинг** | по конструкции | имя хоста, токен в пути URL — не резолвится |
| **TLS SNI** | по конструкции | имя хоста |
| **Логирование URL до отправки** | `grep -rn "url\|token\|TELEGRAM" crates/ops/src/` | ни одного печатающего сайта; `url` — локальная переменная `send()`, нигде не логируется |
| **`Debug` на структурах с токеном** | grep по `derive(Debug` | `TelegramTransport` Debug **не выводит** (нет derive). Единственный Debug в модуле — `TransportError`, держит только уже редактированные строки |
| **Метрики/трейсы** | `transport.rs` не импортирует `metrics`/трейсинг | путь отсутствует |
| **Файл состояния** (`watchdog.state.json`) | оракул F-9 `f9_watchdog_binary_never_logs...` проверяет файл на диске | чисто |
| **Паника при сборке клиента** | `.expect("...TLS backend недоступен")` | статическая строка |
| **`scripts/watchdog_cron.sh`** | чтение целиком | нет `set -x`, нет дампа env, нет `env`/`printenv`; в лог идёт только вывод бинаря |

Единственная поверхность, где токен уходит открытым текстом, — **`TELEGRAM_API_BASE` со схемой
`http://`** (зонд подтвердил: прокси видит `POST http://api.telegram.org/bot<TOKEN>/sendMessage`
целиком). Прод-дефолт — `https`, он захардкожен и запиннен оракулом
`f2_default_endpoint_is_production_telegram`, а правка env требует root, у которого токен и так
есть. Не блокер, заведено как F-13/TD.

---

## 4. Regress-контроль F-1..F-8

`bash scripts/verify_alerting.sh` — 25 проверок, каждая находка прошлых кругов имеет свою секцию
и счётчик зелёных оракулов: F-1 (6), редакция F-2 (включая сквозной прогон бинаря), F-3 (5),
F-5 (3), F-6 (4), F-7 (3), F-9 (5), F-10 (6), F-11 (8), «здоровый прод не шумит» (29).
`VERDICT: PASS`, `exit=0`. Ни одна старая находка не отвалилась.

---

## 5. Scope, sacred, дисциплина коммитов

- **Block-scope: PASS.** Дифф трогает только `crates/ops/{src,tests}`, `scripts/`,
  `docs/runbooks/`, `research/reviews/`, `Cargo.lock`/`crates/ops/Cargo.toml`. Ни `risk`, ни
  `killswitch`, ни `oms`, ни `venue-*`, ни `crates/contracts/**`.
- **Block-C (контракты): N/A** — `crates/contracts/**` не тронут, contract-RFC не требуется.
- **Block-risk: N/A** — safety-путь не тронут, `risk-critic` по `gates.md` §5 не требуется
  (`ops` — операторский периметр, не путь к деньгам; order-egress отсутствует).
- **Sacred целы:** `git diff 42a43e7..HEAD -- crates/ops/tests scripts/verify_alerting.sh` —
  **пусто**. Dev (`abae7f4`) не тронул ни один тест и ни одну строку acceptance-скрипта, реализуя
  под них. RED-first соблюдён: оракул F-11 (`3948b28`, architect) коммитнут ДО фикса
  (`abae7f4`, engine-dev) и на своём коммите был КРАСНЫМ (6/8) — мутация C воспроизвела ровно
  ту же базовую линию.
- **Атомарность:** 19 коммитов, у каждого conventional-subject со ссылкой на находку/круг,
  идентичности ролей корректны (architect — тесты/verify, engine-dev — impl, reviewer — вердикты),
  co-author трейлеров нет. Бандлов нет.

---

## 6. Остаточные находки — НЕ блокеры, в TECH-DEBT

### F-12 (MAJOR) — устойчивый отказ доставки не эскалируется: `exit=0`

`main()` возвращает `Ok(())` даже когда КАЖДЫЙ `telegram_transport.send()` вернул `Err`
(`bin/ops-watchdog.rs:87-89` — только `eprintln!`). Следствия:

1. `watchdog_cron.sh` видит `rc=0` ⇒ **удаляет** `watchdog.alert` и пишет `watchdog.last-success`,
   то есть штатный путь эскалации (ALERT-маркер + `logger -p user.err` в syslog) НЕ срабатывает;
2. итоговая строка врёт по формулировке: «4 **отправлено**» при нуле реально доставленных —
   `delivered` в `CycleOutcome` значит «прошло дедуп», а не «доставлено»;
3. сценарий укуса: founder вписал токен с опечаткой → `401` на каждом алерте → сторож работает,
   алерты формируются, в Telegram не приходит НИЧЕГО, и единственный след — строка в
   `/var/log/hft/watchdog.log`, который никто не читает (для того телеграм и вводили).

**Почему не блокер:** текст алерта не теряется (stdout), отказ печатается явно и редактировано,
а до этой ветки алертинга не было вовсе — регресса нет. На день ноль (токена нет, транспорт =
no-op) не кусается вообще.
**Граница `gates.md` §4:** дефект ОПИСАН; выбор защиты (ненулевой exit при полном отказе
доставки / отдельный маркер / переформулировка «отправлено») — зона **architect**, RED-first.

### F-13 (MINOR) — `TELEGRAM_API_BASE` без ограничения схемы

Значение принимается как есть; `http://`-база отправляет токен открытым текстом (доказано
зондом). Прод-дефолт `https` захардкожен и запиннен оракулом, runbook пишет «на проде не
задавать», правка env требует root — но кода, который бы отверг не-`https` базу вне тестов,
нет. Направление ошибки — тихий downgrade.

### F-14 (MINOR, docs) — runbook отстал от кода на один круг

`docs/runbooks/alerting.md` §«Секреты в логах» описывает R-005 F-2 и R-008 F-2 rev2, но НЕ
упоминает F-11 (редиректы не следуются, `Referer` не отправляется) и не говорит оператору, что
`WD-*` алерт мог не доехать до Telegram при живом логе. §«Известные ограничения» называет
«watchdog у watchdog'а нет», но не называет «watchdog у доставки нет» (F-12).

---

## Done Block

```
$ git rev-parse --short HEAD
abae7f4

$ git status --porcelain
{пусто}

$ git diff 42a43e7..HEAD -- crates/ops/tests scripts/verify_alerting.sh
{пусто — sacred целы}

$ cargo test -p ops 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" (блоков: "NR")"}'
passed=165 failed=0 (блоков: 18)

$ bash scripts/verify_alerting.sh 2>&1 | grep -E "^(PASS|FAIL|VERDICT)"; echo exit=$?
PASS  cargo fmt --check (весь workspace)
PASS  cargo clippy -p ops --all-targets -D warnings
PASS  бинарь ops-watchdog собирается
PASS  существует crates/ops/tests/red_ops_watchdog_cycle.rs
PASS  существует crates/ops/tests/red_ops_transport_redaction.rs
PASS  существует crates/ops/tests/red_ops_transport_redirect.rs
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
PASS  здоровый прод не шумит; состояние ограничено за неделю (+F-10, всего 29)
PASS  оракулы F-9 зелёные (5, включая сквозной прогон бинаря)
PASS  оракулы F-10 зелёные (6)
PASS  код инцидента WD-SEQ-REGRESSED объявлен в крейте
PASS  оракулы F-11 зелёные (8)
PASS  весь крейт ops зелёный
PASS  workspace зелёный
VERDICT: PASS
exit=0

# --- обратная мутация (все откачены после замера) ---
$ мутация A (откат R-008 F-2 rev2)  → passed=74  failed=3   (F-9 КРАСНЕЕТ)
$ мутация B (откат R-008 F-8)       → passed=144 failed=4   (F-10 КРАСНЕЕТ)
$ мутация B2 (ядро фикса F-8)       → passed=146 failed=2   (F-10 КРАСНЕЕТ; в R-009 было 146/0 PASS)
$ мутация C (откат R-009 F-11)      → passed=79  failed=6   (F-11 КРАСНЕЕТ)

# --- зонд прокси (временный, удалён; git status чист) ---
=== PROXY RECEIVED (https endpoint, прод-дефолт) ===
CONNECT api.telegram.org:443 HTTP/1.1
Host: api.telegram.org:443
token present on proxy: false
```

## Статус находок

| Находка | Круг | Класс | Статус |
|---|---|---|---|
| F-1..F-7 | R-005 | смешанный | CLOSED (regress зелёный) |
| F-2 rev2 | R-008 | BLOCKER (утечка в тело ответа) | CLOSED |
| F-8 | R-008 | MAJOR | CLOSED |
| **F-9** | R-009 | BLOCKER (фикс без оракула) | **CLOSED — мутация A краснеет** |
| **F-10** | R-009 | BLOCKER (фикс без оракула) | **CLOSED — мутации B/B2 краснеют** |
| **F-11** | R-009 | BLOCKER (токен за пределы машины) | **CLOSED — мутация C краснеет, зонд чист** |
| F-12 | R-010 | MAJOR | OPEN → TECH-DEBT |
| F-13 | R-010 | MINOR | OPEN → TECH-DEBT |
| F-14 | R-010 | MINOR (docs) | OPEN → TECH-DEBT |

## Что делает reviewer после APPROVED

1. merge `feat/alerting` → `main` (`--no-ff`), push;
2. `PROJECT-STATE.md` — сторож появился; `TECH-DEBT.md` — F-12/F-13/F-14;
3. пост-merge деплой-гейт `gates.md` §8 (CI+Deploy success, ssh eyes-on) — пруф ниже в
   `PROJECT-STATE.md`;
4. **НЕ ставит cron и НЕ вписывает токен** — действия founder'а, `docs/PENDING-SIGNATURE.md` П-003.
