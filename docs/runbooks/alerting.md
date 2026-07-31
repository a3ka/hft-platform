# Runbook — ops-watchdog алертинг

Источник кода: `crates/ops/src/watchdog.rs` (детекторы), `crates/ops/src/format.rs`
(форматирование), `crates/ops/src/transport.rs` (доставка), `crates/ops/src/state.rs`
(дедупликация/состояние), `crates/ops/src/bin/ops-watchdog.rs` (cron-бинарь, всё I/O).

Мотив (не формальность): предыдущий проект founder'а потерял 14 дней данных и 335 млн
тиков, потому что сбор встал, а никто не узнал — здоровье было видно только по ssh.
`ops-watchdog` — cron-процесс, который читает состояние recorder'а (heartbeat, cron-маркеры,
docker) каждые несколько минут и алертит человека, если что-то из перечисленного ниже
сломалось, ДО того как это станет невосполнимой потерей.

## Что проверяется и почему именно так (пороги — `watchdog::Thresholds::default()`)

| Код | Условие | WARNING | CRITICAL | Обоснование порога |
|---|---|---|---|---|
| `WD-HB-MISSING` | `recorder.heartbeat` отсутствует/не парсится | — | сразу | нет файла → recorder либо не стартовал, либо том пропал |
| `WD-HB-STALE` | heartbeat не обновлялся `now − ts_wall_ms` | 60с | 180с | recorder тикает heartbeat каждые 10с (код: `crates/recorder/src/lib.rs`, `Duration::from_secs(10)`); Docker healthcheck (`docker-compose.yml`) уже считает heartbeat протухшим на 60с — WARNING синхронизирован с этим; CRITICAL — втрое дальше (18 пропущенных тиков) |
| `WD-NOT-WRITABLE` | heartbeat несёт `writable:false` | — | сразу | журнал перестал принимать записи (disk-guard/fs ошибка) |
| `WD-SEQ-STALLED` | `next_seq` не растёт между двумя проверками (≥60с друг от друга) | — | сразу | САМЫЙ опасный класс: процесс жив (heartbeat свежий), а данные не идут — healthcheck этого не видит вообще |
| `WD-DISK-LOW` | `free_bytes` низко / прогноз по тренду убыли | < 3×min_free ИЛИ <72ч до min_free | ≤min_free ИЛИ <24ч до min_free | замер на проде 2026-07-31: ~117 КБ/с убыли без ретеншена при free≈77.6 GiB, min_free=10 GiB; 72ч даёт запас поверх суточного ретеншен-цикла (04:07 UTC), 24ч — меньше одного цикла |
| `WD-CONTAINER-MISSING` | контейнер не виден в `docker ps` | — | сразу | не запущен/упал |
| `WD-CONTAINER-UNHEALTHY` | контейнер виден, но не healthy | — | сразу | unhealthy/restarting/exited |
| `WD-CONTAINER-RESTARTED` | `RestartCount` вырос с прошлой проверки | сразу | — | контейнер падал и сам поднялся — human должен знать о факте, даже если сейчас healthy |
| `WD-CRON-MISSING` | маркер cron-задачи (`compaction`/`gateway-checkpoint`/`retention`.last-success) отсутствует | сразу | — | ни одного успешного прогона не зафиксировано |
| `WD-CRON-STALE` | маркер старше порога | 26ч | 48ч | задачи ежесуточные; 26ч — конвенция, уже задокументированная в `deploy/bin/journal-retention-cron.sh`; 48ч — двое суток пропущено подряд |

Пороги живут в коде (`Thresholds::default()`), не только здесь — при расхождении код есть
источник правды, этот файл — объяснение "почему".

## Что делать при срабатывании

1. **`WD-HB-STALE` / `WD-SEQ-STALLED` / `WD-NOT-WRITABLE` / `WD-HB-MISSING`** (сбор данных
   под угрозой): зайти на VPS, проверить `docker ps` и `docker logs hft-recorder --tail 200`.
   Если контейнер не healthy/упал — см. п.3. Если контейнер healthy, но `next_seq` не
   растёт — venue-соединение могло замолчать без разрыва TCP; смотреть логи на предмет
   реконнектов/ошибок парсинга.
2. **`WD-DISK-LOW`**: проверить `df -h` на VPS и свежесть cron-маркеров ретеншена/компакции
   (`WD-CRON-*` — если они ТОЖЕ красные, вот и причина). Если ретеншен молчит — разобраться,
   почему (см. `deploy/README.md`), прежде чем место кончится.
3. **`WD-CONTAINER-MISSING` / `WD-CONTAINER-UNHEALTHY`**: `docker ps -a`, `docker logs`,
   `docker compose up -d` при необходимости. `WD-CONTAINER-MISSING` для `hft-recorder` —
   максимальный приоритет (сбор физически остановлен).
4. **`WD-CONTAINER-RESTARTED`**: не всегда срочно (уже поднялся), но требует разбора причины
   падения — посмотреть логи вокруг момента рестарта.
5. **`WD-CRON-MISSING` / `WD-CRON-STALE`**: проверить `/var/lib/hft/*.last-success` и логи
   соответствующего cron-задания (`/var/log/hft/*.log`), см. `deploy/bin/*-cron.sh`.

## Установка (после ревью — не входит в объём задачи engine-dev)

Бинарь: `cargo build --release -p ops --bin ops-watchdog` → `target/release/ops-watchdog`.
Обёртка для cron: `scripts/watchdog_cron.sh` (конвенция — как у `deploy/bin/*-cron.sh`:
позитивный heartbeat `WATCHDOG_LAST_RUN`, ALERT-маркер на сбой самого watchdog'а). Cron-строка
(предлагается, устанавливает reviewer/founder): каждые 5 минут.

```
*/5 * * * * root /root/hft-platform/scripts/watchdog_cron.sh >> /var/log/hft/watchdog.log 2>&1
```

Env для транспорта: `TELEGRAM_BOT_TOKEN`, `TELEGRAM_CHAT_ID` — как только founder добавит их
в окружение VPS (например `/etc/environment` или systemd-юнит для cron), доставка в Telegram
включится без единой правки кода. До этого момента алерты идут в stdout (лог cron'а,
`/var/log/hft/watchdog.log`) — это уже рабочий канал: `tail -f` / grep по коду инцидента.

## Конфигурация (env, все опциональны — дефолты соответствуют прод-топологии VPS)

- `WATCHDOG_HEARTBEAT_PATH` — путь к `recorder.heartbeat`.
- `WATCHDOG_CRON_DIR` — каталог с `*.last-success` маркерами (по умолчанию `/var/lib/hft`).
- `WATCHDOG_STATE_PATH` — файл дедуп/prev-состояния (по умолчанию `/var/lib/hft/watchdog.state.json`).
- `WATCHDOG_CONTAINERS` — список контейнеров через запятую (по умолчанию `hft-recorder,hft-gateway-serve`).
- `WATCHDOG_HOST_LABEL` — метка хоста в сообщениях.
- `WATCHDOG_DEDUP_WINDOW_MS` — окно дедупликации на инцидент (по умолчанию 30 минут).
- `TELEGRAM_BOT_TOKEN` / `TELEGRAM_CHAT_ID` — см. выше.

## Известные ограничения (на утро, честно)

- Watchdog у самого watchdog'а нет: если cron перестанет запускать `ops-watchdog` вовсе
  (crond умер, cron-файл не установлен), тишина будет такой же, как и раньше — backstop
  для ЭТОГО класса вне объёма задачи (аналог `WD-CRON-STALE`, но для самого watchdog'а).
  Обсудить с founder'ом отдельно (кандидат: сторонний внешний heartbeat-пингер вроде
  healthchecks.io, не требует кода в этом репозитории).
- Absolute-порог диска (`< 3×min_free`) на первом прогоне без тренда — грубый; после
  нескольких прогонов (есть `prev_heartbeat` в состоянии) включается прогноз по тренду,
  который точнее.
