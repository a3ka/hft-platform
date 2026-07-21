# M-20 — VWAP (session-anchored + rolling) — производный индикатор + кандидат-сигнал

STATUS: **PROPOSED / QUEUED** (2026-07-21, architect, по запросу founder'а). НЕ стартует без явного
`go` founder'а И до готовности инфраструктуры (см. «Место в очереди»). Doc-гейт §9 Class A при
старте (critic на milestone+RED ДО dispatch). Блок-2/3 roadmap (индикаторы/сигналы после инфры).

## Objective

VWAP = Σ(price × size) / Σ(size) по окну — вычислим **напрямую из уже собираемого `MdPayload::Trade`**
(price+size+side+ts_exch_ms): новый захват и T1 НЕ нужны. Это чистый детерминированный редьюсер над
потоком `Event` (журнал-принцип, «сигнал не видит будущего»). M-20 даёт:
- **Дисплей-VWAP** (session-anchored + rolling + per-bar) как `LineData` серию для cockpit'а (M-19),
  через тот же экспорт-контракт `code2alpha`, что M-17 (без новой инфраструктуры);
- **кандидат-сигнал** «отклонение цены от VWAP» (mean-reversion) — семья S-002+, идёт через
  анти-оверфит §6 + kill-screen (M-10), как любой сигнал; **вычислимость ≠ альфа**.

Устанавливает паттерн «производный индикатор» (детерминированный редьюсер над `Trade`), которому
следуют будущие индикаторы (TWAP, MA-семейство) — их не нужно каждый раз спекать с нуля.

## Contract impact (T1) — НЕТ

VWAP читает существующий `MdPayload::Trade`. Дисплей — производная `LineData` серия (экспорт M-17).
Новых T1-форм нет → CT-RFC не требуется.

## Инварианты (RED, sacred)

| ID | Инвариант |
|---|---|
| VW-I-1 | **Корректность + детерминизм:** VWAP = Σ(price·size)/Σ(size) на фикстуре с известными сделками → ТОЧНОЕ значение (проверяется вручную посчитанным); два прогона одного потока → идентичны. RED: рукописный набор сделок → ожидаемый VWAP до последнего знака |
| VW-I-2 | **Прод-масштаб без переполнения:** Σ(price·size) на fixed-point ×1e8 копится в **i128** (`i64`-цена × `i64`-размер переполняет i64 на одном произведении, не то что на сумме); НЕТ f64 в аккумуляции (детерминизм). RED: большой поток high-price×large-size — результат корректен, нет overflow/паники |
| VW-I-3 | **Окна корректны:** (а) session-anchored — сброс аккумулятора на границе якоря (UTC-день или заданный anchor); (б) rolling-N — старые сделки ВЫПАДАЮТ из окна; (в) per-bar — агрегация в бакет таймфрейма детерминирована. RED: последовательность через границу якоря/окна → правильный сброс/выпадение |
| VW-I-4 | **Per (venue, symbol) — не смешивать площадки:** VWAP считается отдельно на venue; кросс-venue VWAP (если делаем) — ЯВНЫЙ отдельный агрегат с пометкой, не молчаливое суммирование разной ликвидности. RED: сделки двух venue → раздельные VWAP, не слитый |
| VW-I-5 | **Честность по дырам ленты:** VWAP окна, где лента `Trade` имеет gap (реконнект recorder'а), помечается/отчёт ССЫЛАЕТСЯ на data-quality (`research/data-quality/gaps-*`, M-08). Неполная лента ≠ «точный VWAP» (та же дисциплина, что эпохи ledger'а). RED/гейт: окно с инъецированным gap → VWAP несёт флаг неполноты |
| VW-I-6 | **(если делаем сигнал) VWAP-deviation — чистый редьюсер:** отклонение `(price − vwap)/vwap` без доступа к будущему, детерминизм-тест обязателен; пре-регистрация H/S-карточки с критериями фальсификации ДО касания test-данных (Граница A) |

## Allowed / Forbidden paths

- `crates/research-cli/src/**` (VWAP-редьюсер session/rolling/per-bar + экспорт `LineData` серии) — **research-dev**.
- `crates/signals/src/**` (VWAP-deviation сигнал, если делаем — семья S-002+) — **signal-engineer** (Граница A).
- `research/specs/S-0NN-vwap-*.md`, `research/hypotheses/H-*.md` (пре-регистрация сигнала) — signal-engineer.
- `research/exports/format.md` (добавить VWAP-серию в контракт, `export_schema_version` bump) — research-dev.
- `*/tests/**` (VW-I-* RED), `scripts/verify_M-20.sh`, milestone — **architect** (sacred).
- **Forbidden:** `crates/{risk,killswitch,oms,journal,recorder,venue-*,contracts}`; любой order-path; промоушен сигнала (Граница B/C — founder-подпись); f64 в аккумуляции VWAP.

## §Tasks (RED-first)

| # | Статус | Задача | Кто | Acceptance |
|---|---|---|---|---|
| 1 | ⏳ | VW-I-* RED (`research-cli/tests/red_vwap.rs`: корректность/детерминизм/i128-масштаб/окна/per-venue/gap-честность) | architect | RED падает без impl; достижим; прод-масштаб (VW-I-2) обязателен |
| 2 | ⏳ | `verify_M-20.sh` | architect | exit=0 на GREEN |
| 3 | ⏳ | VWAP-редьюсер: session-anchored + rolling-N + per-bar (чистый, i128, детерминированный) | research-dev | VW-I-1..4 GREEN |
| 4 | ⏳ | Экспорт VWAP как `LineData` серии под code2alpha + `format.md` (bump `export_schema_version`) | research-dev | серия корректна; format.md обновлён |
| 5 | ⏳ | Gap-честность: VWAP окна ссылаются на data-quality; неполнота помечена | research-dev | VW-I-5 GREEN |
| 6 | ⏳ | (опц.) VWAP-deviation сигнал + пре-регистрация H/S-карточки | signal-engineer | VW-I-6 GREEN; карточка+spec |
| 7 | ⏳ | (опц.) прогон VWAP-deviation через M-10 kill-screen → отчёт `research/reports/R-*` | research-dev | вердикт по пре-рег. критериям |

## Гейты

- **critic** (новый milestone §9 Class A при старте). Сигнал (task 6) = Граница A, детерминизм-тест обязателен.
- **risk-critic N/A для ИНДИКАТОРА** (дисплей-VWAP: MD-only, нет safety/order-path). НО **бэктест-ОТЧЁТ**
  VWAP-deviation сигнала (task 7) — анти-оверфит §6 + risk-critic (как M-10/M-17); эпоха ledger'а (TD-015).
- §8 не применим (research-only compute, не деплой-путь; прод-recorder не трогается).

## Место в очереди (зависимости)

- **Не блокирует и не блокируется инфраструктурой безопасности** (M-11 risk/oms не нужен — это compute
  над записанными данными). Но по приоритету founder'а — ПОСЛЕ закрытия инфры (M-18 захват + M-09
  data-safety), в фазе «индикаторы/сигналы» (блок-2/3 BACKLOG).
- Independent от M-16/M-17/M-19; переиспользует экспорт-контракт M-17 (`format.md`) и, для отчёта,
  расширенное окно данных M-16.
- **Дисплей-VWAP** (tasks 3-5) — дёшево, самостоятельная ценность (линия на графике). **Сигнал**
  (tasks 6-7) — квант-десковая работа с полным анти-оверфит гейтом, отдельно и позже.

## Handoff (план при старте)

critic → research-dev (редьюсер+экспорт, tasks 3-5) → (опц.) signal-engineer (пре-рег+сигнал) +
research-dev (прогон) → risk-critic на ОТЧЁТ. Architect: VW-I-* RED + verify.
