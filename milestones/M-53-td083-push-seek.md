<!-- ВНИМАНИЕ: номер M-47 БЫЛ ЗАНЯТ (GW-I-10/TD-046, verify_M-47.sh существует с 02.08).
     Переименовано в M-53 — первый свободный, проверено замером по milestones/ и scripts/. -->
# M-53 — `TD-083` (P0): push-цикл читает журнал с головы и заклинивает прод-gateway

**Статус:** СПЕКА ГОТОВА · **Дата:** 2026-08-03 · **Приоритет: P0** — прод-read-path
функционально мёртв при зелёном healthcheck.
**Ветка:** `feat/TD-083` (оракул compile-RED, в `main` не пушится — `gates.md` §8).
**Гейты:** critic → engine-dev → tester → reviewer → §8 + **повторный sidecar-прогон**.
**RISK-BLOCK не применяется** (read-only путь, order-egress нет). **Граница C не затрагивается.**

---

## 0. Как найдено

`R-025` (reviewer, sidecar-прогон против ЖИВОГО прода — задача 5 M-46). Симптом
`frames_received=0` развёрнут до корневой причины. **M-46 окупился здесь целиком:** дефект
не ловится ничем, кроме прогона на реальном журнале.

## 1. Что сломано (воспроизведение 100%, `R-025`)

```
docker restart hft-gateway-serve && sleep 20   → (healthy), CPU 0.00%, CLOSE_WAIT 0
wsprobe --frames 5 --seconds 12                → latency_first_snapshot_ms=7877, frames_received=0
через 5 s после ухода клиента                  → CPU 100.30%, CLOSE_WAIT 2
через 4 мин                                    → CPU 100.26%, CLOSE_WAIT 10, /proc/1/task = 1
docker ps                                      → Up 23 minutes (healthy)   ← ЗЕЛЁНЫЙ
следующий клиент                               → connect timeout ×2, в логе сервера ТИШИНА
```

### Две сцепленные причины

**(1) `frames_since` читает с ГОЛОВЫ на каждом тике.**
`crates/gateway/src/lib.rs:1772` — `journal::stream(dir, filter)`, то есть от начала журнала,
и лишь потом отбрасывает всё до курсора. Ср. snapshot-путь, где это уже исправлено:
`snapshot_from_checkpoint:1885` — `journal::stream_from(dir, filter, ckpt_cursor.upto_seq)`.

**Фикс M-38b (`GW-I-11`) применили к snapshot-пути и не применили к live-push.** Прод-цена:
промотать ≈139M событий (`history_start_seq=16049334` → `cursor≈155000630`) при ≈190k
событий/с ⇒ **≈12 минут на ОДИН тик**, планируемый каждые 250 ms.

**(2) Однопоточный рантайм + синхронный вызов в `select!`.**
`gateway-serve/src/main.rs:17` — `#[tokio::main(flavor = "current_thread")]` (на проде
`/proc/1/task` = 1). `serve::frames_msgs(...)` — блокирующий journal-read прямо в
`tokio::select!`, без `spawn_blocking` ⇒ монополизирует единственный поток рантайма.

### Следствия (все наблюдались на проде)

- live-push молчит навсегда ⇒ панель показывает застывший снапшот;
- **accept-loop не исполняется** ⇒ следующий клиент не подключится вообще;
- уход клиента не детектируется: `stream.next()` не поллится, а `sink.send(..).is_err()`
  достижимо ТОЛЬКО когда есть кадры; кадров нет ⇒ **таск течёт вечно**, сокет в `CLOSE_WAIT`;
- **healthcheck зелёный**: `</dev/tcp/127.0.0.1/8080` (`docker-compose.yml:149`)
  удовлетворяется ядром из listen-backlog, даже когда приложение не вызывает `accept()`.

Дословно сценарий `gates.md` §8: «rollback ловит падение healthcheck, но не тихую деградацию».

## 2. Почему не поймал ни один оракул

На журнале в сотни байт чтение с головы отрабатывает за микросекунды ⇒ M-46 `O-3`
(`red_ws_frames_converge_to_latest`) зелёный и остаётся зелёным. Дефект виден ТОЛЬКО на
прод-масштабе. `.claude/rules/testing.md` требует прод-масштабный кейс для sacred-оракулов
I/O-пути (урок TD-011, `crates/journal/tests/red_open_bounded.rs`) — **у push-пути такого
оракула не было**. Это дыра в моём наборе M-46, а не только дефект `gateway`.

## 3. §Tasks

| # | Задача | Зона | Статус | Оракул |
|---|---|---|---|---|
| 1 | `gateway::frames_since_with_stats(dir, filter, sel, after, max) -> io::Result<(Vec<Frame>, Cursor, ReadStats)>` — аддитивно, симметрично `snapshot_from_checkpoint`; `frames_since` остаётся тонкой обёрткой (совместимость) | engine-dev | ⚠️ DONE (rev2, см. report §2) | компиляция O-1/O-2 |
| 2 | **SEEK вместо чтения с головы:** `journal::stream_from(dir, filter, after.upto_seq)` в обеих функциях | engine-dev | ⚠️ ЧАСТИЧНО (см. report §2 — VWAP-регрессия, `frames_since` НЕ seek-based) | **O-1, O-2** |
| 3 | Блокирующий journal-read вынести из async-таска: `tokio::task::spawn_blocking` вокруг `frames_msgs` в push-цикле (`gateway-serve/src/lib.rs:424-431`) | engine-dev | ✅ DONE | O-3 |
| 4 | Таск обязан завершаться при уходе клиента ДАЖЕ когда кадров нет (сейчас единственная ветка выхода достижима только при наличии кадров) | engine-dev | ✅ DONE | O-3 |
| 5 | RED-оракулы O-1/O-2 | **architect** (sacred) | ✅ DONE | — |
| 6 | Оракул O-3 на утечку таска/accept-loop | **architect** (sacred) | ⏳ OPEN | — |
| 7 | `scripts/verify_M-53.sh` | **architect** (sacred) | ⏳ OPEN | — |

## 4. RED-оракулы

`crates/gateway/tests/red_push_seek_bounded.rs` (готов, **compile-RED**):

- **O-1 `td083_push_tick_seeks_instead_of_reading_from_head`** — тик у хвоста открывает
  ≤3 сегментов; на чтении с головы открывает ВСЕ ⇒ падает;
- **O-2 `td083_tick_cost_is_independent_of_journal_length`** — стоимость тика одинакова на
  коротком и длинном журнале; растущая с историей цена ⇒ падает.

**Меряется РАБОТА (`ReadStats.segments_opened`), а не время** — сознательно, урок TD-078:
оракул с потолком wall-clock превращается в измеритель CI-машины. Число открытых сегментов
от скорости раннера не зависит.

**O-3 (задача 6, писать):** клиент подключился и ушёл ⇒ таск завершился, сокет не остался в
`CLOSE_WAIT`, следующий клиент подключается успешно. Деградированный вход: уход клиента в
момент, когда кадров НЕТ (именно этот случай не имел ветки выхода).

## 5. Allowed paths

**Разрешено:** `crates/gateway/src/**`, `crates/gateway-serve/src/**` (engine-dev) ·
`crates/gateway/tests/**`, `crates/gateway-serve/tests/**`, `scripts/verify_M-53.sh`
(**architect only**) · `research/reports/M-53-*.md`.

**Запрещено:** `crates/contracts/**` · `crates/journal/src/**` (используем существующий
`stream_from`, менять журнал не требуется) · прод-конфиг (`docker-compose.yml`, env на VPS).

## 6. Acceptance — `scripts/verify_M-53.sh`

Паритет с CI-job `fmt+clippy+test` (`gates.md` §3) + O-1/O-2/O-3 GREEN + весь набор M-46
остаётся GREEN (регресс-защита: фикс push-пути не имеет права сломать сверку с реплеем).

## 7. Закрытие требует ПОВТОРНОГО sidecar-прогона против прода

Гейт на фикстурах необходим, но НЕ достаточен — дефект родом с прода. Close-out обязан
содержать: `frames_received > 0`, CPU `gateway-serve` в норме через 5 минут после ухода
клиента, `CLOSE_WAIT = 0`, успешное подключение ВТОРОГО клиента после первого.

## 8. Отдельно — `TD-084`: healthcheck подтверждает жизнь мёртвого сервиса

TCP-connect удовлетворяется listen-backlog'ом ядра. Это НЕ входит в периметр M-53 (чиним
причину, а не индикатор), но обязано быть заведено: индикатор, который зелен при мёртвом
сервисе, хуже отсутствующего — он гасит тревогу. Зона правки — `docker-compose.yml`
(engine-dev/deploy), кандидат: healthcheck, выполняющий реальный WS-handshake.

---

## 9. ⚠️ УТОЧНЁННЫЙ ДИАГНОЗ (architect, после отчёта engine-dev и личного замера)

Первая редакция §3 предписывала «seek в `frames_since`». **Это было неверно, и dev это
доказал:** VWAP — all-time cumulative (since genesis), поэтому наивный seek даёт БЫСТРЫЙ, но
НЕЧЕСТНЫЙ VWAP — хуже, чем текущее «молчит». Dev откатил (rev2) и передал вопрос мне, не
решая единолично на correctness-пути. Это правильное поведение.

### Решение УЖЕ НАПИСАНО и НЕ ПОДКЛЮЧЕНО

`gateway::LiveReducer` (M-38b задача #6) — резюмируемое состояние, живущее МЕЖДУ тиками:
`resume()` поднимает состояние из чекпоинта, `pump()` докармливает только новые события через
`stream_from`. Ровно то, что нужно: VWAP корректен (состояние накоплено с genesis), цена тика
— по приращению.

**Замер:** `grep -rn "LiveReducer" crates/gateway-serve/src/` → **ни одного использования**,
только упоминание в комментарии (`lib.rs:436`). Push-цикл зовёт `serve::frames_msgs` →
`frames_since` → чтение с головы. Тот же класс, что M-45: механизм построен, покрыт оракулами,
но **не стоит на пути**.

### Оба оракула `LiveReducer` ТАВТОЛОГИЧНЫ — не давят ни на что

`crates/gateway/tests/red_frames_seek_bound.rs` зелёный целиком, и это ложное спокойствие:

1. **`pumped_frames_identical_to_frames_since`** сравнивает кадры `pump` с кадрами
   `frames_since`. Но `pump` (`gateway/src/lib.rs:2941`) буквально ВОЗВРАЩАЕТ результат
   `frames_since` — комментарий признаётся прямо: «Используем frames_since для byte-identity
   с эталоном… Это компромисс: byte-identity важнее накапливающего self.reducer».
   ⇒ оракул сравнивает `frames_since` с `frames_since` и зелен всегда.
2. **`pump_at_tail_is_bounded`** проверяет `events_decoded <= 64`. Но `stats` берутся из
   `read_stats_from_stream(&stream)` — потока `stream_from`, то есть ТОЛЬКО второй половины
   работы. Работа, потраченная внутри `frames_since` (чтение с головы), в `stats` не попадает.
   ⇒ оракул меряет половину и подтверждает ограниченность, которой НЕТ.

**Это дефект моих оракулов, а не только кода.** Ровно тот класс, который critic ловил у меня
в `C-055` §2 («сравнение технически присутствует, но математически сравнивает `[] == []`»).

### Переформулированные задачи

| # | Задача | Зона |
|---|---|---|
| 2a | **`pump` строит кадр ИЗ СВОЕГО состояния**, не вызывая `frames_since`. Byte-identity доказывается ОРАКУЛОМ, а не тем, что pump возвращает эталон дословно | engine-dev |
| 2b | **`ReadStats` из `pump` учитывает ВСЮ работу тика** (обе фазы), иначе измерение снова окажется половинным | engine-dev |
| 2c | **Push-цикл `gateway-serve` использует `LiveReducer::resume` при подключении и `pump` на тике** вместо `frames_msgs` | engine-dev |
| 5b | **Переписать оба оракула `red_frames_seek_bound.rs`** так, чтобы они падали против нынешней конструкции: эталон для byte-identity строится НЕЗАВИСИМО (полный реплей), а не вызовом того же `frames_since` | **architect** (sacred) |

**`frames_since` остаётся как есть** — честный эталон полного реплея, на нём стоят оракулы
M-46. Менять его семантику не требуется: исправляется ПУТЬ, а не эталон.
