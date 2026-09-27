# M-89 — остаток S0: работа одной выдачи ограничена СТРУКТУРНО и видна СНАРУЖИ процесса

**Статус:** PROPOSED (набор коммитится ДО диспетчеризации dev — `04-workflow.md` §2).
**Ревизия, на которой сняты утверждения о коде:** `origin/main` = `dcb435f`.
**Предмет роадмапа:** строка `M-89` (`docs/ROADMAP.md`), остаток S0 программы SCALE
(`docs/plans/scale-program-2026-09-21.md` §4 п. 2, п. 4; §15.1).
**Подписи founder'а НЕ требует** — инженерная работа в действующем мандате (план §13, §15.8);
изменения прода идут существующей процедурой допуска и отката (§8 `gates.md`).
**Замер-основание:** `docs/plans/m89-warm-resume-measure-2026-09-27.md` (архитектор-клон, прод
2026-09-27 12:11Z + локальный эксперимент на `939d1b9`).

---

## 1. Objective

`M-87` поставил S0 частично: ограничитель ПАРАЛЛЕЛИЗМА работает, ограничитель ОБЪЁМА одной
выдачи — нет (`TD-219`), показания предохранителя не выходят из процесса (`TD-220`), счётчик
прочитанных байт занижает и стоит `read_dir` на горячем пути (задача 15 `M-87`), а сам
счётчик процессный и заставляет оракулы идти по очереди (`TD-224`/`TD-225`).

Поставка закрывает остаток ДВУМЯ вещами, и обе — структурные, а не счётные:

1. **Работа одной выдачи ограничена САМИМ ЧТЕНИЕМ.** Первый `pump` после тёплого `resume`
   стоит `O(хвост) + O(log)` на поиск позиции, а не `O(префикс активного сегмента)`. Тогда
   порог свежести `max_tail_events` (уже стоит на точке входа, `M-87` §4.0bis) — И ЕСТЬ
   бюджет объёма; отдельный `CallBudget` не нужен и удаляется как плацебо.
2. **Показания видны снаружи процесса.** `gateway-serve` пишет JSON-сердцебиение на
   записываемый том; `ops-watchdog` читает его на хосте и применяет правило молчания и
   правило свежести (`PL-I-8`: алерт живёт вне наблюдаемой машины/процесса).

## 2. Вред — ЗАМЕР, а не гипотеза

Отчёт `docs/plans/m89-warm-resume-measure-2026-09-27.md` (сырые выводы там):

| N активного | хвост | `events_scanned` первого pump | `rchar` первого pump | файл сегмента | второй pump |
|---|---|---|---|---|---|
| 2 000 | 500 | 2 500 | 188 753 | 123 053 | 65 893 |
| 200 000 | 500 | 200 500 | 10 273 252 | 10 207 543 | 65 909 |
| 400 000 | 500 | 400 500 | 20 620 325 | 20 554 613 | 65 913 |

`events_scanned = N + хвост` РОВНО; `rchar ≈ размер файла + 65.7 КБ`; ×2.007 по `rchar` при ×2
длине; ×109 при ×200. Второй/третий тик — константа ≈ 66 КБ (hint работает со второго тика).
**Проекция на прод** (слепок 12:00Z, замер 12:11Z): курсор слепка `712 075 015`, `first_seq`
активного сегмента 839 = `711 770 132` ⇒ **304 883 события / ≈ 412 МБ** перед курсором
читаются первым pump'ом КАЖДОЙ новой подписки; к ротации (1 ГиБ) — ≈ 800 тыс. / 1 ГБ; сразу
после ротации — почти 0. Хвост после слепка при каденции `*/15` — 30–90 тыс. событий. Лишняя
работа первого pump'а — ×4…×30 к самому хвосту, ×10⁴ к одному тику.

Причина (по коду `dcb435f`): обе ветки `LiveReducer::resume` ставят `tail_hint: None`
(`crates/gateway/src/lib.rs`, комментарий «hint обязан быть заполнен ПЕРВЫМ ЖЕ pump'ом»),
`resolve_active_start_offset` без hint возвращает `header_end`
(`crates/journal/src/segments.rs`), а `CkptHeader` несёт только `cursor` (seq) — ни `pos`, ни
hint. `seq→offset` индекса нет.

## 3. Состояние сегодня — ЗАМЕР (каждая строка снята командой на `dcb435f`)

| # | факт | команда / место |
|---|---|---|
| 1 | бюджет вызова НЕ подключён к пути выдачи | `grep -c 'feed_tail_within(' crates/gateway-serve/src/lib.rs` → 0 (только комментарии); определение `crates/gateway-serve/src/admission.rs:318` |
| 2 | `pump_one` — плацебо по построению: читает ВСЕ сегменты каталога с начала, без курсора и селектора | `crates/gateway-serve/src/admission.rs:380-420` (`journal::list_segments` + чтение каждого файла порциями 64 КБ) |
| 3 | периодические `pump`'ы ОТБРАСЫВАЮТ `ReadStats` | `Ok((frames, _new_cursor, _stats))` в `crates/gateway-serve/src/lib.rs` (v1-путь и legacy-путь push-цикла); `add_journal_payload_bytes_pub` зовётся ТРИ раза и только со `stats` `resume` (`:1190`, `:1337`, `:2092`) |
| 4 | warm-`resume` считает байты ОПИСЬЮ без активного сегмента | `payload_bytes_after_cursor` (`crates/gateway/src/lib.rs:3881-3895`): `first_seq > after_seq` — сегмент, содержащий курсор, исключён; цена — `journal::list_segments` (`read_dir` + заголовки) на каждую выдачу |
| 5 | счётчики выдачи — процессные атомики | `crates/gateway-serve/src/metrics.rs:36-41` (`*_GLOBAL`); `serving_counters()` читает их |
| 6 | у `gateway-serve` на проде ДВА монтирования и оба `rw=false` | `docker inspect -f '{{range .Mounts}}…' hft-gateway-serve` → `/journal rw=false`, `/ckpt rw=false` (ssh 2026-09-27) |
| 7 | `ops-watchdog` на проде НЕ установлен и НЕ собран | `ls /etc/cron.d` → нет `hft-watchdog`; `ls target/release/ops-watchdog` → нет; `/var/lib/hft/watchdog.state.json` → нет (ssh 2026-09-27). В репозитории есть `scripts/watchdog_cron.sh`, но НЕТ фрагмента `deploy/cron.d/watchdog` |
| 8 | `recorder::write_heartbeat` пишет НЕ атомарно | `crates/recorder/src/lib.rs:310-335`: `std::fs::write(hb_path, body)` — без `.tmp` + `rename`. Мандат этой поставки называл его образцом атомарной записи — **это неверно**, образцом он служит только по форме JSON и периоду |
| 9 | шаг `task-status` `verify_M-87.sh` не видит строку `2bis` и молча отбрасывает `🟡` | `scripts/verify_M-87.sh:447,454` (`[0-9]+б?`, предфильтр `(⏳ OPEN\|✅ DONE\|🟡)` + `case` только на `⏳ OPEN`) — `TD-221` |
| 10 | примитивы сдвига по байтам в `journal` ЕСТЬ, ни один не публичен и ни один не используется для первого pump'а | `scan_tail_for_last_seq` (`crates/journal/src/lib.rs:363`, `fn`, не `pub`), `probe_frame_boundary` (`segments.rs:1717`, метод `EventStream`), `PositionedBufReader::open` (`segments.rs:1490`, приватная структура) |

**Находка сверх мандата (гипотеза, не замер — названа, чтобы её проверил гейт, а не забыли):**
`resolve_active_start_offset` при `hint.pos < file_len` зовёт `probe_frame_boundary`; та на
НЕПОЛНОМ кадре (`read_frame_payload` → `Ok(None)` по `UnexpectedEof`) возвращает `false`, и
чтение откатывается в `header_end` (`segments.rs:1706-1716`). `BufWriter` recorder'а сбрасывает
буфер по заполнению, то есть МЕЖДУ байтами одного кадра; читатель, попавший в это окно на
УСТАНОВИВШЕМСЯ тике, перечитывает активный сегмент целиком (до 1 ГиБ). В замере §2 писателя
рядом не было, поэтому второй тик и был константой. Сценарий `d6` этой поставки требует от
пути сдвига обратного поведения (§5.4); распространяется ли это на установившийся тик —
решение dev'а в рамках задачи 1, и reviewer заводит карточку долга, если нет.

## 4. Инварианты поставки

| ID | инвариант | зона | оракул |
|---|---|---|---|
| `I-1` | работа первого `pump` после тёплого `resume` ∝ хвосту после курсора слепка + `O(log)` на поиск позиции; НЕ ∝ префиксу сегмента, содержащего курсор | `journal` (аддитивно) + `gateway` | `w1`, `d2`, `d3`, `d6`, `d8`, `d9` |
| `I-2` | снимок после тёплого `resume` со сдвигом бит-идентичен независимой свёртке (`VB-I-2`, `VB-I-11`) на всех вырожденных входах; порча рядом с точкой сдвига ⇒ `Err`, не пропуск (`JR-I-2`: разрыв при чтении → abort, не «пропустить») | `journal` + `gateway` | `d1`…`d9` |
| `I-3` | допущенный запрос (`readiness = Ready` ⇔ хвост ≤ `max_tail_events`) стоит `f(max_tail_events)` байт чтения на РЕАЛЬНОМ WS-пути; мёртвый бюджет удалён | `gateway-serve` | `s1` |
| `I-4` | `journal_payload_bytes_read` — от ЧИТАТЕЛЯ, включая каждый `pump` и сегмент с курсором; `payload_bytes_after_cursor` с горячего пути ушёл | `journal` (счётчик) + `gateway` + `gateway-serve` | `p1`, `v` |
| `I-5` | счётчики выдачи — на ЭКЗЕМПЛЯР сервера (`counters_handle`) | `gateway-serve` | `i1`, `i2` |
| `I-6` | счётчики + свежесть наружу процесса: сердцебиение на записываемом томе; `ops-watchdog` читает и алертит на молчание и несвежесть; композиция путей проверена | `gateway-serve` + `ops` + `deploy` | `h0`, `h1`, `g1`…`g5`, шаг `verify` «композиция» |
| `I-7` | шаг `task-status` переехал в `verify_M-89.sh` с исправлениями `TD-221` | architect | шаг `verify` `task-status` + проба |

### 4.1. Форма — задана ДОСЛОВНО там, где RED-набор компилируется против имён

```rust
// crates/journal/src/segments.rs — АДДИТИВНО (RAW-гейт критика, gates.md §1: путь чтения журнала)
/// Найти позицию ПЕРВОГО кадра с `seq > after_seq` в СЫРОМ сегменте `seg_path`, не читая
/// сегмент целиком. Возвращает `Some(TailHint { seg_idx, last_seq: after_seq, pos })` ТОЛЬКО
/// при выполнении гарда точности (§5.2); иначе `None` (вызыватель откатывается к чтению с
/// `header_end`, и откат НАБЛЮДАЕМ — см. `ReadStats::seek_fallbacks`). `.zst` ⇒ `None`.
pub fn locate_after_seq(seg_path: &Path, after_seq: u64) -> io::Result<Option<TailHint>>;

/// Байты, ПРОЧИТАННЫЕ этим стримом: кадры `[len][payload][crc]` + заголовки + пробы границ.
/// Ведётся тем же приёмом, что `events_scanned` (M-57): инкремент в месте чтения, не в
/// вызывателе. Аддитивен к остальным счётчикам.
impl EventStream { pub fn payload_bytes_read(&self) -> u64; }

// crates/gateway/src/lib.rs — АДДИТИВНО
pub struct ReadStats {
    /* существующие поля без изменений */
    /// M-89: число откатов поиска позиции к `header_end` за вызов (наблюдаемость fail-closed).
    pub seek_fallbacks: u64,
}
// `payload_bytes_read` заполняется ИЗ `EventStream::payload_bytes_read()` на ВСЕХ путях
// (`pump`, `snapshot_from_checkpoint`, cold-`resume`); `payload_bytes_after_cursor` УДАЛЯЕТСЯ.

// crates/gateway-serve/src/metrics.rs — АДДИТИВНО
pub struct ServingCountersHandle { /* атомики ЭКЗЕМПЛЯРА */ }
impl ServingCountersHandle { pub fn snapshot(&self) -> ServingCounters; }
// crates/gateway-serve/src/server.rs
impl Server {
    /// По образцу `slots_handle` (задача 13 M-87). `None` ⇒ сервер без политики.
    pub fn counters_handle(&self) -> Option<std::sync::Arc<ServingCountersHandle>>;
    /// Сердцебиение — АДДИТИВНЫЙ билдер, не поле `ServeConfig` (одиннадцать sacred-литералов).
    pub fn with_heartbeat(self, cfg: HeartbeatConfig) -> Server;
}
pub struct HeartbeatConfig { pub path: PathBuf, pub period: std::time::Duration }
/// Чистая функция над env: `GATEWAY_HEARTBEAT_PATH` (обязательна для сердцебиения; отсутствие
/// ⇒ `Ok(None)` — сердцебиения нет, и это ЛОВИТ watchdog кодом MISSING, а не бинарь),
/// `GATEWAY_HEARTBEAT_PERIOD_MS` (дефолт 10 000; невалидное ⇒ `Err`, отказ старта — GW-I-14).
pub fn heartbeat_config_from_env(get: impl Fn(&str) -> Option<String>) -> Result<Option<HeartbeatConfig>, String>;

// УДАЛЯЮТСЯ из crates/gateway-serve/src/admission.rs (вместе с тестами `u5_*`, §16):
//   CallBudget · BudgetStop · Cancel · feed_tail_within · pump_one · PumpStep
```

Что НЕ фиксируется формой, а формулируется ТРЕБОВАНИЯМИ (§5): где именно `gateway` зовёт
`locate_after_seq` (в `resume` или в первом `pump`), как устроена бисекция, как хранится
`seek_fallbacks` внутри стрима. Гард точности (§5.2) — не деталь реализации, а инвариант.

### 4.2. Форма сердцебиения выдачи (задана ДОСЛОВНО — её читают ДВА крейта и гейт)

Файл `GATEWAY_HEARTBEAT_PATH`, запись `<path>.tmp` → `rename` (атомарно: читатель никогда не
видит частичный JSON — `h1` наблюдает это опросом), период `GATEWAY_HEARTBEAT_PERIOD_MS`
(дефолт 10 000, как у recorder'а), значения — из счётчиков ЭКЗЕМПЛЯРА сервера (`I-5`):

```json
{"schema":1,"ts_wall_ms":…,"attempts":…,"successes":…,"refusals_supported":…,
 "refusals_unsupported":…,"journal_payload_bytes_read":…,"slots_in_flight":…,
 "freshness":{"source_ms":…,"projection_ms":…,"published_ms":…,"snapshot_ms":…}}
```

**Ошибки записи ГЛОТАЮТСЯ и логируются** (как у recorder'а): сердцебиение — наблюдаемость,
сбой файла не роняет выдачу; его отсутствие видит watchdog (`WD-SERVING-HB-MISSING`).
**В журнал не пишется** (`OPS-I-6`).

### 4.3. Форма читателя — `crates/ops` (задана ДОСЛОВНО, аддитивно)

```rust
// crates/ops/src/watchdog.rs
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub struct ServingHeartbeatSample { pub ts_wall_ms: i64, pub attempts: u64, pub successes: u64,
    pub refusals_supported: u64, pub refusals_unsupported: u64,
    pub journal_payload_bytes_read: u64, pub slots_in_flight: u64 }
pub enum Incident { /* существующие */ ServingHeartbeatMissing, ServingHeartbeatStale, ServingSilence }
//   коды: WD-SERVING-HB-MISSING (CRITICAL) · WD-SERVING-HB-STALE (WARNING/CRITICAL по порогам)
//         · WD-SERVING-SILENCE (CRITICAL)
pub struct Thresholds { /* существующие */ pub serving_heartbeat_warn_ms: i64, pub serving_heartbeat_crit_ms: i64 }
//   дефолты: warn 60 000 / crit 180 000 — те же, что у recorder'а (период 10 с, cron */5)
pub fn check_serving_heartbeat_missing(hb: Option<&ServingHeartbeatSample>) -> Option<Alert>;
pub fn check_serving_heartbeat_stale(now_ms: i64, hb: &ServingHeartbeatSample, thr: &Thresholds) -> Option<Alert>;
/// По ДЕЛЬТАМ prev→cur: Δattempts > 0 ∧ Δrefusals_supported > 0 ∧ Δsuccesses == 0 ⇒ Some.
/// Отрицательная дельта (рестарт процесса) ⇒ None (новый якорь, не голодание).
pub fn check_serving_silence(prev: &ServingHeartbeatSample, cur: &ServingHeartbeatSample) -> Option<Alert>;

// crates/ops/src/watchdog_cycle.rs — `CycleInputs` НЕ МЕНЯЕТСЯ (12 литералов в sacred-тестах)
#[derive(Debug, Clone, Default)]
pub struct ServingInputs { pub heartbeat: Option<ServingHeartbeatSample> }
pub fn run_cycle_full(inputs: &CycleInputs, serving: &ServingInputs, now_ms: i64,
    thr: &Thresholds, dedup_window_ms: i64, state: &mut WatchdogState) -> CycleOutcome;
// `run_cycle(..)` ≡ `run_cycle_full(.., &ServingInputs::default(), ..)` — поведение прежнее.
// crates/ops/src/state.rs
pub struct WatchdogState { /* существующие */ #[serde(default)] pub prev_serving_heartbeat: Option<ServingHeartbeatSample> }
// crates/ops/src/bin/ops-watchdog.rs — env `WATCHDOG_SERVING_HEARTBEAT_PATH`
//   (default `/var/lib/docker/volumes/hft-platform_gateway-state/_data/gateway-serve.heartbeat`);
//   бинарь зовёт ТОЛЬКО `run_cycle_full`.
```

Дедуп, якорь и нечитаемый такт — по той же дисциплине, что у recorder-сердцебиения
(`R-005` F-1/F-5: нечитаемый такт не сбрасывает якорь; дедуп-окно применяется).

## 5. `I-1`/`I-2` — сдвиг к позиции курсора БЕЗ смены формата слепка

### 5.1. Почему не смена формата слепка

Положить `pos` в `CkptHeader` — смена формата ⇒ bump `ckpt_schema_version` ⇒ все прогретые
слепки невалидны ⇒ холодная пересборка ≈ 23 мин на проде (`R-187`), и у каждого читателя
своё представление о позиции, которое протухает при компакции. Позиция — свойство СЕССИИ
(M-57 круг 2: `TailHint` в памяти сессии, sidecar отвергнут `R-035`), и находить её надо по
`seq`, который слепок уже несёт. Это то же решение, что M-57 принял для установившегося тика,
распространённое на первый.

### 5.2. Требования к поиску позиции (свойства, не код)

1. **Сложность.** ≤ `2·⌈log2(размер_файла / 64 КиБ)⌉ + 4` проб, каждая читает ≤ 64 КиБ
   подряд (`TAIL_SCAN_CHUNK`-класс, `crates/journal/src/lib.rs:53`); итого на 1 ГиБ ≈ 14 проб
   ≈ 1 МиБ. Оракул: `events_scanned ≤ хвост + 64`, `rchar ≤ хвост_байт + 2 МиБ`, отношение
   `rchar(200k)/rchar(2k) ≤ 2.0` (`w1`).
2. **Ресинк по границе кадра.** Проба с произвольного смещения ищет валидный кадр
   `[len][payload][crc32]` + декодируемый `Event` байт-ресинком (образец —
   `scan_tail_for_last_seq`). `len > FRAME_LEN_SANITY_CAP` и CRC-промах — не граница.
3. **ГАРД ТОЧНОСТИ (инвариант `I-2`, не оптимизация).** Результат принимается ТОЛЬКО если
   кадр по найденному `pos` декодируется в `Event` с `seq == after_seq + 1` — ЛИБО хвост пуст:
   последний валидный кадр имеет `seq == after_seq` и `pos` = его конец (`pos == len`
   легитимен по условию 6 `resolve_active_start_offset`). Любое иное ⇒ `None` ⇒ откат к
   `header_end` (`seek_fallbacks += 1`). Основание — `JR-I-2` (`seq` без дыр: следующий кадр
   ОБЯЗАН нести `after+1`) — гард делает ложное совпадение CRC (2⁻³²) или ошибку бисекции
   НЕВЫПОЛНИМОЙ молча: «перепрыгнуть» валидные события нельзя, потому что позиция —
   не «где-то после», а РОВНО `after+1`.
4. **Порча.** Кадр `after+1` испорчен ⇒ гард не проходит ⇒ откат ⇒ чтение с `header_end`
   упирается в порчу и даёт `Err`, как независимый путь (`d7`). Порча дальше по хвосту ⇒
   `Err` из стрима, курсор доставки не двигается (`d7b`). Ресинк ВНУТРИ прода-пути чтения
   (`journal::stream*`) по-прежнему запрещён — он живёт только в офлайн-`recover` (M-05 J3).
5. **Сегмент, содержащий курсор, — любой СЫРОЙ**: активный ИЛИ закрытый (ротация после
   прогрева, ≈ 11 % времени на проде — `d3`). Для закрытого сырого сегмента чтение начинается
   с найденной позиции так же (сегодня — `Passive` с начала, `open_next_segment`). `.zst` —
   НАЗВАННЫЙ ПРЕДЕЛ: `locate_after_seq` ⇒ `None`, читается один сегмент целиком, корректность
   обязательна (`d4`).
6. **Только чтение.** Никаких файлов рядом с журналом (`:ro`-том прода, `d9`), никакого
   общего состояния между сессиями (`d8`).
7. **Наблюдаемость отката.** `ReadStats::seek_fallbacks` — чтобы «сдвиг не сработал и всё
   перечиталось» было видно в статистике, а не только в `rchar`.

### 5.3. Где зовётся — требование, не предписание

`resume` журнал не читает (замер: `events_scanned=0`; sacred `red_frames_seek_bound`). Значит
позиция ищется в ПЕРВОМ `pump`, когда `self.tail_hint.is_none() && self.cursor.upto_seq.is_some()`:
по каталогу (`SegmentCatalog::open` уже строится там же) найти сегмент с `first_seq ≤ cursor+1 <
next.first_seq`, позвать `locate_after_seq`, передать результат как `hint` в
`stream_from_at_with_catalog`. Найденная позиция проходит СУЩЕСТВУЮЩУЮ валидацию hint'а
(`resolve_active_start_offset`, условия 1–6) — второй контур, не первый. Реализация внутри
`journal` (в `stream_from_at_with_catalog` при `hint == None`) допустима, если сохраняет
семантику `stream`/`stream_from` (`after_seq = None` ⇒ полный проход) и все sacred-оракулы
`crates/journal/tests/**`, `crates/gateway/tests/**` зелены.

### 5.4. Рваный кадр (`d6`)

Если по найденной позиции лежит НЕПОЛНЫЙ кадр (`UnexpectedEof` внутри `[len][payload][crc]`),
это «писатель дописывает», а не «позиция негодна»: тик пустой, откат в `header_end` ЗАПРЕЩЁН,
следующий тик читает дописанный кадр ровно один раз. Различение честное: неполный кадр
нельзя валидировать CRC, поэтому события с такой позиции НЕ ВЫДАЮТСЯ (fail-closed), а
позиция сохраняется; если на следующем тике CRC не сходится — обычный откат по условию 6.

## 6. `I-3` — граница структурная, бюджет-плацебо удаляется

`CallBudget`/`BudgetStop`/`Cancel`/`feed_tail_within`/`pump_one` УДАЛЯЮТСЯ (`admission.rs`)
вместе с оракулами `u5_budget_exhausted_by_events_is_named`,
`u5_budget_exhausted_by_payload_bytes_is_named`, `u5_cooperative_cancel_stops_between_chunks`,
`NeverCancel` (`red_m87_admission.rs`; §16). `red_m87_registry`/`red_m87_entrypoint` эти имена
не используют (`grep -l 'feed_tail_within\|CallBudget' crates/gateway-serve/tests/*.rs` →
только `red_m87_admission.rs` и два комментария в `r196_conditions`/`read_volume_truth`).

Взамен — `s1` (`red_m89_structural_bound.rs`): допущенный запрос на прод-форме (один сегмент
1 ГиБ-конфига, префикс 200 тыс., хвост 500 ≤ `max_tail_events = 1 000`) через настоящий
`subscribe` v1 до `snapshot` И первого `frame` стоит `≤ ckpt_bytes + tail_bytes·4 + 2 МиБ` по
`rchar`. Это и есть предохранитель по объёму: `readiness` не пропускает хвост сверх порога,
а пропущенный хвост стоит ровно себя.

**Почему это закрывает `TD-219`, а не переименовывает его.** `TD-219` требовал «вызов
бюджета на пути выдачи + оракул против его отсутствия». Оракул против ОТСУТСТВИЯ вызова
проверял бы имя; `s1` проверяет РАБОТУ — то, ради чего вызов был нужен. Reviewer закрывает
карточку по `s1`, а не по грепу.

## 7. `I-4` — счётчик от читателя

`EventStream::payload_bytes_read()` (§4.1) кормит `ReadStats.payload_bytes_read` на всех
путях; в транспорте КАЖДЫЙ `pump` push-цикла (v1 и legacy) прибавляет свои байты к счётчику
экземпляра. `payload_bytes_after_cursor` удаляется — с ним уходит `read_dir` + заголовки на
каждую выдачу (задача 15 `M-87`, `R-202` Н-1). Оракулы: `p1` (библиотека, нижняя граница —
байты хвоста на диске, верхняя — `rchar`), `v` (WS: `≥ ckpt + tail`, `≤ rchar`).

## 8. `I-5` — счётчики на экземпляр

`counters_handle()` (§4.1). Процессный `serving_counters()` может остаться АГРЕГАТОМ по всем
экземплярам — не запрещается и не судится. Оракулы `red_m87_entrypoint.rs` переводятся на
ручку экземпляра architect'ом (задача 7, §16). **`SERIAL` снимается ТОЛЬКО по мутации:** если
после перевода возврат процессного счётчика в утверждение `c6`/`u1` РОНЯЕТ `i1`
(`red_m89_counters_instance.rs` — B видит прирост A), блокировка снята обоснованно; иначе
остаётся вместе со сторожем `red_m89_serial_guard.rs` (`TD-225`), который тогда продолжает
требовать равенства.

## 9. `I-6` — наружу процесса: три артефакта и одна композиция

| артефакт | зона | что |
|---|---|---|
| писатель | `crates/gateway-serve/src/**`, `main.rs` | `with_heartbeat` + `heartbeat_config_from_env` (§4.1), запись §4.2 |
| описание прода | `docker-compose.yml` (engine-dev) | сервис `gateway-serve`: том `gateway-state:/state` (**rw**), `GATEWAY_HEARTBEAT_PATH: /state/gateway-serve.heartbeat`, `GATEWAY_HEARTBEAT_PERIOD_MS: ${…:-10000}`; объявление `volumes: gateway-state:` |
| читатель | `crates/ops/src/**` | §4.3 |
| планировщик | `deploy/cron.d/watchdog` (НОВЫЙ, engine-dev) | по образцу `deploy/cron.d/journal-retention`: `WATCHDOG_SERVING_HEARTBEAT_PATH=/var/lib/docker/volumes/hft-platform_gateway-state/_data/gateway-serve.heartbeat`, `*/5 * * * * root /root/hft-platform/scripts/watchdog_cron.sh` |

**Композиция (шаг `verify`, `testing.md` §«канарейка», п. 2):** путь, куда пишет
`gateway-serve` (`compose`: том + путь в контейнере ⇒ `/var/lib/docker/volumes/hft-platform_<том>/_data/<файл>`),
обязан СОВПАСТЬ с путём, откуда читает `ops-watchdog` (`deploy/cron.d/watchdog`
`WATCHDOG_SERVING_HEARTBEAT_PATH`). Две строки в двух файлах — ровно тот тихий no-op, который
канарейка ловит.

**Предел, названный честно:** установка `deploy/cron.d/watchdog` в `/etc/cron.d` и сборка
`ops-watchdog` на VPS — РУЧНОЙ шаг с подписью founder ★ (`deploy/README.md` §Планировщик:
«`install /etc/cron.d/...` — осознанный ручной шаг»). Поставка доставляет артефакты и
доказывает их композицию; §8-гейт при close-out предъявляет `ls /etc/cron.d/hft-watchdog` и
свежий `/var/lib/hft/watchdog.last-success` ЛИБО явно записывает, что алерт по-прежнему не
установлен (`TD-220` тогда закрывается ЧАСТИЧНО, не полностью).

## 10. ЗАПРЕЩЕНО (`testing.md`: спека правки существующего кода несёт запретный список)

| запрещено | почему |
|---|---|
| менять форму слепка (`CkptHeader`, `ckpt_schema_version`) ради позиции | холодная пересборка ≈ 23 мин на проде (`R-187`); позиция — свойство сессии (§5.1). Допустимо ТОЛЬКО с замером, показывающим, что §5.2 недостижим без этого |
| файлы/sidecar рядом с журналом, общее состояние между сессиями | `:ro`-том прода; `R-035` F-035-1/2; `d8`/`d9` |
| ресинк по байтам ВНУТРИ прод-пути чтения (`journal::stream*`), молчаливый пропуск кадров | `JR-I-2`, `JR-I-11`; ресинк — только в офлайн-`recover`. Поиск позиции пробует, но выдача идёт со строгим CRC от точной позиции (§5.2 п. 3) |
| менять формат журнала на диске, `SCHEMA_VERSION`, семантику `stream`/`stream_from`/`read_all` | вне предмета; `after_seq = None` ⇒ полный проход остаётся |
| менять `gateway::validate_selector`, семантику пяти исходов `M-87`, порядок `admit → try_acquire → readiness` | `A-033`/`A-037`/`A-038` |
| менять состав полос, `GATEWAY_BANDS`, `GATEWAY_CANONICAL_BANDS`, `MAX_REL_DIST` | граница C, `П-014`/`П-029` |
| менять предел объёма ответа 2 000 000 Б | `П-020` |
| менять авторизацию, `key_material`, форму ошибок WS | `VB-I-9`, `CT-RFC-09` §2.7 |
| поле в `ServeConfig` вместо билдера `with_heartbeat` | одиннадцать sacred-файлов с литералом `ServeConfig { … }` (`A-037` У-3) |
| менять `CycleInputs`, сигнатуру `run_cycle`, `Thresholds` иначе как аддитивно | 12 литералов `CycleInputs { … }` в `red_ops_watchdog_cycle.rs` (sacred) |
| писать сердцебиение в `/journal` или `/ckpt` | оба `:ro` по инварианту (GS-I-3; писатель слепка — только прогреватель) |
| писать сердцебиение в журнал | `OPS-I-6` |
| ослаблять сдвиг до «читать с начала, но считать честно» | предмет — РАБОТА, не счётчик: `w1`/`s1` меряют `rchar` |
| трогать `crates/{risk,killswitch,oms,contracts,venue-*,book}/**`, `research/**` | вне зоны; `contracts` — sacred |
| править `*/tests/**` | sacred, зона architect'а; странный тест ⇒ `!!! SCOPE VIOLATION REQUEST !!!` |

## 11. Пределы, названные честно

1. **`.zst`-сегмент, содержащий курсор, читается целиком** (§5.2 п. 5). На проде это
   недостижимо при `Ready`: компакция (cron 03:50, `COMPACTION_KEEP_RAW=2` —
   `deploy/cron.d/journal-retention`, `docker-compose.yml`) не трогает два последних сырых
   сегмента, а порог свежести 255 600 событий ≈ 30 мин при 142 соб/с (`R-196`) — курсор
   слепка при `Ready` всегда лежит в них; оракул `d4` пиннит корректность и «не хуже одного
   сегмента».
2. **Размер события в фикстурах ≈ 51 Б против ≈ 1 351 Б на проде** — число проб растёт как
   `log2(размер файла)`, на проде их на ~5 больше; пороги это покрывают, а прод-замер до/после
   (§15, SKIP-шаг) снимает настоящие числа.
3. **`h1` наблюдает атомарность ОПРОСОМ** (≈ 30 чтений за 3 периода) — неатомарная запись
   ловится вероятностно, а не доказательно; структурного оракула на `rename` нет, и он не
   изображается.
4. **Молчание выдачи судится по ДЕЛЬТАМ между тактами cron'а (5 мин)**: тревога появляется не
   раньше второго такта после начала голодания. Активная проба (`M-87` §7 п. 3, `TD-207`)
   этим не заменяется и здесь не строится.
5. **Установка cron на VPS — вне поставки** (§9). Без неё «алерт живёт вне процесса»
   доказан артефактами и композицией, но не наблюдением на проде.
6. **`freshness` в сердцебиении — то, что отдаёт процесс** (`metrics::freshness()`); его
   правдивость — предмет `C9` `M-87`, не этой поставки.
7. **Прод-замер до/после (§15) снимает reviewer** на §8-гейте: сам прод в наборе не
   участвует.
8. **Структурная граница — НЕ «дёшево», и число названо.** После поставки первый `pump`
   стоит ∝ хвосту, а хвост ограничен порогом свежести `GATEWAY_MAX_TAIL_EVENTS = 255 600`
   (`docker-compose.yml:226`; `docker inspect hft-gateway-serve` на проде 2026-09-27 → то же).
   При ≈ 1 351 Б/событие (замер §2) это ≈ **345 МБ в худшем случае** на одну новую подписку,
   **≈ 40–120 МБ в типичном** (хвост 30–90 тыс. за 15 мин между прогревами). Поставка
   срезает префикс активного сегмента (до ≈ 1 ГБ сверх хвоста), но стоимость остаётся
   ПЕР-ПОДПИСОЧНОЙ. Дешёвой одну подписку делает только общий расчёт на многих — S2 программы
   (план §6), не эта поставка. Сужать порог ради стоимости — решение оператора (частота
   прогрева и порог — одна пара, `M-87` §14.1quinquies), не предмет этой спеки.
9. **Отмены работы при отключении клиента НЕТ и не появляется.** Инцидент 2026-09-20, из-за
   которого заведён `M-87` («отключение клиента работу не прекратило»), закрыт здесь ОБЪЁМОМ, а
   не отменой: работа одной выдачи ограничена п. 8 и доводится до конца. Кооперативная отмена
   жила только в удаляемом плацебо (`feed_tail_within` не звался с прод-пути), то есть потери
   функции нет — есть честное название её отсутствия.

## 12. Allowed paths

**engine-dev:** `crates/journal/src/**` (АДДИТИВНО: `locate_after_seq`, счётчик байт стрима,
сдвиг в закрытом сыром сегменте, рваный кадр — §5) · `crates/gateway/src/lib.rs` (первый
`pump`, `ReadStats::seek_fallbacks`, `payload_bytes_read` от стрима, удаление
`payload_bytes_after_cursor`) · `crates/gateway-serve/src/**` и `main.rs` (счётчики
экземпляра, сердцебиение, удаление бюджета-плацебо, байты каждого `pump`) ·
`crates/ops/src/**` и `src/bin/ops-watchdog.rs` (§4.3) · `docker-compose.yml` (том, две
переменные) · `deploy/cron.d/watchdog` (новый).

**architect (этот набор и задача 7):** `milestones/M-89-*.md` · `crates/gateway/tests/**` ·
`crates/gateway-serve/tests/**` · `crates/ops/tests/**` · `scripts/verify_M-89.sh` ·
`scripts/tests/red_verify_M-89_task_status.sh` · `docs/plans/m89-warm-resume-measure-2026-09-27.md`.

**Вне зоны:** `crates/{risk,killswitch,oms,contracts,venue-*,book,recorder}/**`, секреты,
`scripts/watchdog_cron.sh` (не требует правок: путь приходит из env фрагмента cron'а).

## 13. §Tasks

| # | Status | задача | зона | проверка |
|---|---|---|---|---|
| 1 | ⏳ OPEN | **`journal::locate_after_seq` + счётчик байт стрима + сдвиг в закрытом сыром сегменте + рваный кадр** (§4.1, §5.2, §5.4). Аддитивно; sacred `crates/journal/tests/**` зелены. ОСТАТОК: всё — ещё не начата | engine-dev | `w1`, `d3`, `d6`, `d7`, `d7b`; `cargo test -p journal` |
| 2 | ⏳ OPEN | **Первый `pump` ищет позицию курсора** (§5.3): `tail_hint` заполняется найденной позицией, откат наблюдаем (`seek_fallbacks`). ОСТАТОК: всё — ещё не начата | engine-dev | `w1`, `d1`, `d2`, `d8`, `d9`, `red_frames_seek_bound`, `red_tick_read_cost` зелены |
| 3 | ⏳ OPEN | **`payload_bytes_read` от читателя на всех путях; `payload_bytes_after_cursor` удалён** (§7). ОСТАТОК: всё — ещё не начата | engine-dev | `p1`; `grep -c payload_bytes_after_cursor crates/gateway/src/lib.rs` → 0 |
| 4 | ⏳ OPEN | **Каждый `pump` push-цикла кормит счётчик байт** (v1 и legacy). ОСТАТОК: всё — ещё не начата | engine-dev | `v` (`red_m89_read_volume_truth`), `red_m87_read_volume_truth::q2` зелен |
| 5 | ⏳ OPEN | **Бюджет-плацебо удалён** из `admission.rs` (§6); комментарии, ссылающиеся на него, приведены к коду. ОСТАТОК: всё — ещё не начата | engine-dev | `verify` шаг task5 (грепы ОТСУТСТВИЯ), `s1` |
| 6 | ⏳ OPEN | **Счётчики на экземпляр**: `ServingCountersHandle`, `Server::counters_handle`, продюсеры пути выдачи пишут в экземпляр (§8). ОСТАТОК: всё — ещё не начата | engine-dev | `i1`, `i2` |
| 7 | ⏳ OPEN | **Перевод оракулов `red_m87_entrypoint.rs` на ручку экземпляра; `SERIAL` — по мутации** (§8, §16). ОСТАТОК: всё — ждёт задачи 6 | architect | `red_m87_entrypoint` под `--features testing` зелен; `red_m89_serial_guard` зелен; `red_m87_registry::r1` зелен |
| 8 | ⏳ OPEN | **Сердцебиение выдачи**: `with_heartbeat`, `heartbeat_config_from_env`, запись §4.2, `main.rs` зовёт билдер. ОСТАТОК: всё — ещё не начата | engine-dev | `h1` |
| 9 | ⏳ OPEN | **compose**: том `gateway-state` (rw), `GATEWAY_HEARTBEAT_PATH`, `GATEWAY_HEARTBEAT_PERIOD_MS` у `gateway-serve`. ОСТАТОК: всё — ещё не начата | engine-dev | `h0`, `red_m87_prod_entrypoint_argv` зелен |
| 10 | ⏳ OPEN | **`ops`: сэмпл, три инцидента, пороги, `run_cycle_full`, якорь в состоянии, env бинаря** (§4.3). ОСТАТОК: всё — ещё не начата | engine-dev | `g1`…`g5`, `red_ops_watchdog*` зелены, `scripts/verify_alerting.sh` (если зовётся) |
| 11 | ⏳ OPEN | **`deploy/cron.d/watchdog`** с `WATCHDOG_SERVING_HEARTBEAT_PATH`, равным хост-пути тома из compose (§9). ОСТАТОК: всё — ещё не начата | engine-dev | `verify` шаг «композиция» |
| 12 | ⏳ OPEN | **Прод-замер ДО/ПОСЛЕ** (§15, SKIP-шаг): `rchar`/время первой выдачи новой подписки на проде при курсоре слепка глубоко в активном сегменте; сырые строки — в вердикт. ОСТАТОК: снимается reviewer'ом на §8-гейте | reviewer | `verify` SKIP-шаг + `R-NNN` |

Задачи 1–2 ЗАВИСИМЫ и трогают `crates/journal` ⇒ **RAW-гейт критика на сильной модели**
(`gates.md` §1: раскладка/чтение журнала). Задача 7 — architect после задачи 6.

## 14. RED-оракулы

| ID | файл | что доказывает | какая мутация роняет | чего НЕ ловит | сегодня |
|---|---|---|---|---|---|
| `w1` | `crates/gateway/tests/red_m89_warm_resume_seek.rs` | первый pump после тёплого resume: `events_scanned ≤ хвост+64`, `rchar ≤ хвост+2 МиБ`, отношение 200k/2k ≤ 2.0; снимок = независимая свёртка; второй тик дёшев | «читать с header_end» (сегодня); «hint без гарда» — ловит `d7`; «считать честно, читать всё» — `rchar` | транспорт; время | RUNTIME-RED (`200 500` при хвосте 500) |
| `p1` | там же | `payload_bytes_read` pump'а ∈ `[tail_bytes, rchar]` | литерал / 0 / опись | транспорт | RUNTIME-RED (`0`) |
| `d1` | там же | курсор = первое событие сегмента: позиция «после первого кадра» находится | особый случай ⇒ откат | — | зелен (страж границы; префикс 1 событие) |
| `d2` | там же | пустой хвост: `events_scanned ≤ 64`, `rchar ≤ 2 МиБ`, снимок = эталон | «нет хвоста ⇒ прочитать всё» | — | RUNTIME-RED |
| `d3` | там же | курсор в ЗАКРЫТОМ сыром сегменте, хвост через ротацию | сдвиг только для активного | `.zst` | RUNTIME-RED |
| `d4` | там же | `.zst`: корректность и «не хуже одного сегмента» | пропуск событий на `.zst` | стоимость `.zst` (предел) | зелен (страж) |
| `d6` | там же | рваный кадр: тик = хвост, без перескана; дописанный кадр — ровно один раз | откат в `header_end` на рваном; дубль | — | RUNTIME-RED |
| `d7`/`d7b` | там же | порча за курсором / через 5 кадров ⇒ `Err`, курсор стоит | ресинк, перепрыгивающий порчу | — | зелены (стражи fail-closed) |
| `d8` | там же | две сессии из одного слепка: обе ограничены и идентичны | общее состояние сессий | — | RUNTIME-RED |
| `d9` | там же | `:ro`-каталог: работает, ничего не пишет | sidecar/индекс | root-окружение (страж setup) | RUNTIME-RED |
| `s1` | `crates/gateway-serve/tests/red_m89_structural_bound.rs` | допущенный WS-запрос на прод-форме ≤ `ckpt + tail·4 + 2 МиБ` по `rchar` | любая работа ∝ префиксу | время; параллелизм | RUNTIME-RED |
| `v` | `crates/gateway-serve/tests/red_m89_read_volume_truth.rs` | счётчик выдачи ≥ `ckpt + tail`, ≤ `rchar` | отброшенные `ReadStats` pump'а; опись без активного | — | RUNTIME-RED |
| `i1`/`i2` | `crates/gateway-serve/tests/red_m89_counters_instance.rs` | изоляция экземпляров; ручка кормится реальным путём | ручка = процессный счётчик; ручка-нули | — | COMPILE-RED (`counters_handle`, `ServingCountersHandle`; два E0277 — следствие неизвестного типа, не дефект набора) |
| сторож | `crates/gateway-serve/tests/red_m89_serial_guard.rs` | `#[tokio::test]` == `SERIAL.lock().await` в `red_m87_entrypoint.rs`, пока `SERIAL` объявлен | снять одну блокировку | смысл требования | зелен (14 == 14) |
| `h0` | `crates/gateway-serve/tests/red_m89_heartbeat_entrypoint.rs` | compose объявляет путь на rw-монтировании ≠ журнал/слепки | переменной нет; `:ro`; `/journal` | установка на VPS | RUNTIME-RED |
| `h1` | там же | прод-бинарь на окружении из compose пишет файл за период; JSON целый при каждом чтении; после выдачи значения растут | нет файла; `fs::write` (вероятностно); константы | правдивость `freshness` | RUNTIME-RED |
| `g1`…`g5` | `crates/ops/tests/red_m89_serving_silence.rs` | отсутствие/несвежесть/молчание по дельтам; якорь и дедуп в состоянии; бинарь читает env-путь | детектор-молчун; детектор-крикун; якорь в памяти процесса | установка cron | COMPILE-RED (типов нет); `g5` — RUNTIME-RED и без них |

**Стражи setup на каждом сценарии:** один сегмент (прод-форма), префикс ≥ 100× хвоста, хвост
непуст, `resume` не читал журнал, кадры пришли, `/proc/self/io` доступен, ротация/компакция
состоялись, uid ≠ 0 для `d9`, бинарь собран для `h1`/`g5`.

**Зависимого эталона нет:** `gateway::snapshot` через `journal::stream` не делит с предметом
ни слепок, ни hint, ни позиционирование; `rchar` ведёт ядро.

**Цена корпуса:** `red_m89_warm_resume_seek` — ≈ 137 с в debug (`w1` строит слепок на 200 тыс.
событий); `s1` — того же порядка. Это осознанная цена прод-масштаба, названная в мандате
(«префикс ≥ 200 тыс., как в замере»); уменьшать масштаб ради секунд запрещено — оракул
меряет ту величину, на которой дефект и виден.

## 15. Acceptance — `scripts/verify_M-89.sh`

Агрегатор с FAIL-счётчиком, решение по коду возврата, финальная `VERDICT: PASS|FAIL`, ≥ 1
проверка на задачу. Паритет с CI (`gates.md` §3): `cargo fmt --all -- --check` ·
`cargo clippy --all-targets --all-features -- -D warnings` · `cargo test --all` — плюс
`cargo test` В ФОРМЕ CI (без `--features`) на файлах, у которых под флагом другой состав.
Шаг `task-status` перенесён из `verify_M-87.sh` с исправлениями `TD-221` (регэксп
`[0-9]+(б|bis)?`, строки `🟡` проверяются наравне с `⏳ OPEN`); проба —
`scripts/tests/red_verify_M-89_task_status.sh`. Шаг «композиция» сверяет путь compose ↔ cron.
**SKIP-шаг прод-замера** (задача 12) печатается явно и не зеленеет сам.

**Базовая линия снимается КРАСНОЙ** и печатается в Handoff.

## 16. Ревизия тест-корпуса — решение по каждому изменяемому ожиданию ДО merge

| файл / тест | что защищал | решение |
|---|---|---|
| `red_m87_admission.rs::u5_budget_exhausted_by_events_is_named` · `u5_budget_exhausted_by_payload_bytes_is_named` · `u5_cooperative_cancel_stops_between_chunks` · `NeverCancel` | форму бюджета-плацебо (`A-037` У-5: «каждый объявленный тип имеет оракул») | **УДАЛЕНЫ architect'ом В ЭТОМ НАБОРЕ** (не dev'ом по задаче 5): они закрепляли реализацию, признанную плацебо `R-201`; продуктовый инвариант (объём одной выдачи ограничен) ПЕРЕНЕСЁН в `s1`. Удаление ДО задачи 5 обязательно — иначе dev, удаляя типы, упёрся бы в sacred-тесты, которые не вправе править. Остальные 18 тестов файла зелены |
| `red_m87_admission.rs` — остальные | форма исходов, `readiness`, слоты, секрет | НЕ ЗАТРАГИВАЮТСЯ |
| `red_m87_entrypoint.rs` (14 async) | точка входа | **ПЕРЕНОСЯТСЯ** на `counters_handle()` (задача 7, architect); `SERIAL` — по мутации §8 |
| `red_m87_registry.rs`, `m87_registry/mod.rs` | биекция сценариев | НЕ ЗАТРАГИВАЮТСЯ (имена сценариев не меняются) |
| `red_m87_read_volume_truth.rs::q2` | `counter ≤ rchar`, `≥ tail` | НЕ ЗАТРАГИВАЕТСЯ — остаётся зелёным при честном счётчике (граница слабее `v`) |
| `red_m87_r196_conditions.rs` | пять условий по работе | НЕ ЗАТРАГИВАЕТСЯ (комментарии о `feed_tail_within` — история круга) |
| `red_frames_seek_bound.rs`, `red_tick_read_cost.rs`, `red_tail_cursor_prod_form.rs`, `red_hint_pos_guard.rs`, `red_segment_meta_bound.rs` | hint, тик, каталог | НЕ ЗАТРАГИВАЮТСЯ; обязаны остаться зелёными — это и есть D-1(б) `A-037` для библиотеки |
| `crates/journal/tests/**` | формат, пол, монотонность | НЕ ЗАТРАГИВАЮТСЯ; зелены |
| `red_ops_watchdog_cycle.rs` (12 литералов `CycleInputs`) | склейка recorder-heartbeat | НЕ ЗАТРАГИВАЮТСЯ — форма §4.3 аддитивна намеренно |

## 17. Предъявление FA — живые инварианты тронутых модулей

- `crates/journal/**` — `docs/fa/journal.md`: **`JR-I-2`** («`seq` строго монотонен без дыр;
  разрыв при чтении → abort (не „пропустить“)») — основание гарда точности §5.2 п. 3;
  **`JR-I-11`** («ни один путь чтения журнала не имеет права молча сшить немонотонный
  каталог») — сдвиг в закрытом сегменте не обходит выбор сегментов по `first_seq`.
- `crates/gateway/**`, `crates/gateway-serve/**` — своей FA нет (долг, `M-87` §17);
  опора — `docs/fa/viz-backend.md` §5: `VB-I-2` (live == replay — `d1`…`d9`), `VB-I-10`
  (память окном, не историей), `VB-I-11` (провенанс истории не меняется сдвигом).
- `crates/ops/**` — `docs/fa/ops.md`: **`OPS-I-8`** («тишина в потоке — алерт P1: „жив, но
  не работает“») распространяется на выдачу; **`OPS-I-10`** («объявлена ⟹ эмитится») —
  `h1` судит продюсера сердцебиения, `g5` — продюсера тревоги.
- `DESIGN` §22: `PL-I-4` (запрос не читает event-store сверх проекции), `PL-I-5`
  (лимит enforce'ится сервером), `PL-I-8` (алерт вне наблюдаемой машины).

## 18. Handoff

После коммита набора — **critic обязателен**: новая milestone-спека (`gates.md` §9), оценка
≥ 5 коммитов (§1 п. 3), **и RAW-гейт на СИЛЬНОЙ модели** (§1: задачи 1–2 меняют путь чтения
журнала — `crates/journal/src/segments.rs`). Мандат критику называет повышение явно.

**`risk-critic` НЕ требуется:** `crates/risk`, `crates/killswitch`, `crates/oms`,
`crates/venue-*` и ордерный путь не затронуты (§12).

Порядок: critic → правки architect'а → engine-dev (задачи 1–6, 8–11; 1–2 первыми) →
architect (задача 7) → tester → reviewer (§8-гейт с прод-замером, задача 12; карточки
`TD-219`/`TD-220`/`TD-221`/`TD-224`/`TD-225` и находка §3 о рваном кадре).

## 19. Cross-references

- `docs/plans/scale-program-2026-09-21.md` §4 п. 2/п. 4, §13, §15.1, §15.8
- `docs/plans/m89-warm-resume-measure-2026-09-27.md` — замер-основание
- `milestones/M-87-serving-circuit-breaker.md` §4.0bis, §11 п. 5, §14.1ter, §16; задачи 15/16/24
- `TECH-DEBT.md` `TD-219`, `TD-220`, `TD-221`, `TD-224`, `TD-225`; `R-035` (sidecar отвергнут),
  `R-187` (23 мин пересборки), `R-201`/`R-202`/`R-205`/`R-206`
- `docs/fa/journal.md` `JR-I-2`, `JR-I-11` · `docs/fa/viz-backend.md` `VB-I-2/10/11` ·
  `docs/fa/ops.md` `OPS-I-6/8/10` · `docs/DESIGN.md` §22 `PL-I-4/5/8`
- `docs/PENDING-SIGNATURE.md` `П-020` (предел ответа), `П-029` (семь полос)
