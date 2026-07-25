# M-35 — сбор margin-утилизации (available-inventory) usdt/usdc — CT-RFC-05

STATUS: **PROPOSED** (2026-07-25, architect; rev2 после critic C-024 REJECT — переформулирован в
**proxy-collector**). Founder-решение (2026-07-25): «маржин по usdt/usdc собирать». Источник задокументирован
**фактами с read-only ключом** (`margin-source-survey.md §9`: endpoint/HTTP-200/USDT+USDC-значения +
граница интерпретации). Это **CT-RFC** (новый T1-вариант) + первый **аутентифицированный** источник в
даталеере (read-only ключ) → **critic (doc-гейт) + risk-critic (contracts §5) ОБЯЗАТЕЛЬНЫ**. НЕТ order-egress.

## Мотивация (ЧЕСТНАЯ рамка — proxy, НЕ ledger)

Founder хочет Margin-индикатор по usdt/usdc. **Публичного raw borrow/repay LEDGER «взято/вернули/нетто» у
Binance НЕТ** (survey §1-§8, стоит в силе). НО под read-only auth достижима **supply-сторона:**
`/sapi/v1/margin/available-inventory?type=MARGIN` отдаёт **market-wide СЫРОЙ доступный к займу пул per-asset**
с `updateTime` (замер §9: USDT `19932592.29`, USDC `20514052.57`, HTTP 200, 402 актива).

**Граница интерпретации (BINDING, critic C-024):** `available` = СЫРОЙ supply-пул (сколько ЕЩЁ можно занять),
**НЕ** непогашенный объём и **НЕ** borrow/repay ledger. Утилизация/флоу — **ПРОИЗВОДНАЯ ПРОКСИ downstream**
(Δ available, или `borrowLimit − available`), помечается `derived-from-available-inventory` + caveat
«ёмкость пула меняется биржей ⇒ Δ конфаундится». M-35 журналирует **СЫРОЙ supply-факт**; интерпретация —
осознанно downstream (как L2Delta-сырьё vs book-реконструкция). НЕ выдаём за ledger.

**Дисциплина хранения (design-honesty):** collector пишет СЫРОЙ `available` (источник истины). Утилизация/
флоу (Δ) — **производная, считается ИНДИКАТОРОМ downstream** (отдельный milestone), с провенансом
`derived-from-available-inventory` + caveat «пул-капасити может меняться биржей → Δ = утилизация, не
абсолютный ledger». Храним источник, деривим позже — как L2Delta (сырьё) vs book (реконструкция).

## Contract impact (T1) — ДА → CT-RFC-05 (atomic contract-RFC, `05-contract-layer.md §4`)

**Новый вариант** (аддитивно В КОНЕЦ, postcard-дискриминант **7**; старые сегменты 0..6 читаются
байт-в-байт, CT-I-3):
```rust
/// Market-wide доступный к займу пул margin per-asset (Binance `/sapi/v1/margin/available-inventory`,
/// auth read-only). `symbol` = актив ("USDT"/"USDC"). `available_e8` = доступный объём ×1e8.
/// Утилизация/флоу (Δ available) — ПРОИЗВОДНАЯ downstream (индикатор), НЕ здесь. CT-RFC-05.
MarginInventory {
    available_e8: i64,
    ts_exch_ms: i64,   // = updateTime ×1000, если в секундах
},
```
- **`SCHEMA_VERSION` 3 → 4** (новая эпоха; сегмент schema-3 не reuse'ится schema-4 бинарём — та же
  изоляция, что L2Delta/TD-031). JSON Schema перегенерировать.
- **НЕ переиспользуем `MarginRate`** — `available` это AMOUNT (пул), не rate. Смешать = семантическая
  подмена (RC-I-10). `MarginRate` остаётся для ставки; `MarginInventory` — для пула.

## Secrets / auth (НОВОЕ для даталеера — раньше был чисто public MD)

- Margin-поллер делает **аутентифицированный signed GET** (HMAC-SHA256 query, header `X-MBX-APIKEY`)
  с **read-only** ключом (`BINANCE_API_KEY`/`SECRET` из env, НЕ из журнала/git; `.env` git-ignored локально,
  env-инъекция на VPS). Ключ read-only (`enableReading` only — подтверждено `apiRestrictions`) → двигать
  деньги не может.
- **IP-restrict ключа на `167.233.192.131`** при деплое (сейчас `ipRestrict:false`).
- **Инвариант MI-I-3 (canary):** margin-поллер НЕ содержит order-egress (submit/cancel/order-sign) — grep.

## §Tasks (RED-first)

| # | Статус | Задача | Кто | Acceptance |
|---|---|---|---|---|
| 1 | ⏳ | **CT-RFC-05:** `MarginInventory` вариант в `crates/contracts/src/lib.rs` + `SCHEMA_VERSION`→4 + doc + JSON Schema regen. **RED:** MI-I-1 (roundtrip additive, старые сегменты 0..6 читаются), MI-I-2 (`parse_available_inventory` fixture→события), MI-I-4 (fixed-point). `verify_M-35.sh`. Sacred. | architect | compile-RED; roundtrip старых сегментов GREEN; parse анти-плацебо (не тот asset/scale → FAIL) |
| 2 | ⏳ | **impl:** `parse_available_inventory(json, assets: &[&str]) -> Vec<MdEvent>`; auth signed poll `/sapi/v1/margin/available-inventory?type=MARGIN` (env-ключ, HMAC) каждые ~2 мин (founder-tunable) для USDT/USDC; recorder wiring; env-инъекция ключа | venue-dev | MI-I-1..4 GREEN; poll эмитит MarginInventory usdt/usdc; НЕТ order-egress (MI-I-3 canary GREEN) |
| 2b | ⏳ | **Exhaustive-match ревизия (engine-dev):** новый вариант `MarginInventory` (дискр.7) ломает исчерпывающие `match &md.payload` БЕЗ `_=>` → `cargo build --workspace` E0004. Обновить: `crates/journal/src/segments.rs:~1523` (арм `\| MarginInventory { ts_exch_ms, .. } => *ts_exch_ms`, как Funding/OI/MarginRate/L2Delta), `crates/sim/src/exchange.rs:~223` (арм-`{}` как прочие md-not-relevant). Оракул = `cargo build --workspace` GREEN. | engine-dev | workspace компилируется; арм семантически как соседние (ts извлекается / no-op) |
| 2c | ⏳ | **Exhaustive-match ревизия (research-dev):** `crates/research-cli/src/bin/latency_probe.rs:~120` — арм `MarginInventory { .. } => continue` (как L2Delta/OI/MarginRate: не latency-релевантно). | research-dev | workspace компилируется; `MarginInventory` пропущен как прочие не-Trade/L2 |
| 2d | ⏳ | **Exhaustive-match ревизия (engine-dev):** `crates/recorder/src/lib.rs::md_kind_label` (~75) — арм `MdPayload::MarginInventory { .. } => "margin_inventory"` (snake_case варианта, как `margin_rate`/`open_interest`/`l2delta`; метрика `md_events_total`, guard `red_metrics_emission`). `recon_loop.rs` НЕ трогать (есть `_=>`). **Пропущен в 45ec491 (энумерация по памяти вместо грепа — урок).** | engine-dev | `cargo build --workspace` GREEN; label по конвенции md_kind_label |
| 3 | ⏳ | **§8 eyes-on** (CI green ✓): деплой (ключ на VPS, IP-restrict) → журнал несёт `MarginInventory` usdt/usdc, recorder healthy, hb свежий; sanity `available` ≈ REST-значению | reviewer | свежий сегмент: MarginInventory для USDT+USDC; прод healthy; ключ read-only на VPS |

## §Инварианты (RED-оракулы; sacred, architect-only)

| ID | Инвариант | Оракул |
|---|---|---|
| **MI-I-1** | **Roundtrip аддитивен.** `MarginInventory` postcard-roundtrip; сегмент из старых вариантов (0..6) читается байт-в-байт (CT-I-3); `SCHEMA_VERSION==4`. | `crates/contracts/tests/ct_rfc05.rs`. **Анти-плацебо:** дискриминант вставлен НЕ в конец → старый сегмент ломается → FAIL |
| **MI-I-2** | **Parse.** `parse_available_inventory({"assets":{"USDT":"19932592.28",...},"updateTime":...}, &["USDT","USDC"])` → 2 события MarginInventory с верным `available_e8` и symbol=asset. | `crates/venue-binance/tests/red_margin_inventory.rs`. **Анти-плацебо:** фильтр не тот asset / без scale → FAIL |
| **MI-I-3** (canary) | **Read-only.** Margin-поллер НЕ содержит order-egress. grep: нет `order`/`submit`/`cancel`/подписи торговли в margin-пути. | verify grep. **Анти-плацебо:** любой order-endpoint в margin-коде → FAIL |
| **MI-I-4** | **Fixed-point.** `"19932592.2856805"` → `to_fixed` → i64 ×1e8 без потери >8 знаков; отрицательных/пустых нет (пул ≥0). | `red_margin_inventory.rs::fixed_point`. |

## §Анти-плацебо чек-лист
- **Множественность:** ≥2 актива (USDT+USDC) в одном ответе → 2 события.
- **Отсутствие:** asset не в фильтре → не эмитится; `assets` пустой/битый JSON → пусто (fail-closed).
- **Аддитивность:** старый сегмент (без дискриминанта 7) читается без ошибок.
- **Провенанс:** collector хранит СЫРОЙ `available`, НЕ Δ (деривация — downstream, не подмешивать).

## Allowed / Forbidden paths
- **architect (sacred, CT-RFC):** `milestones/M-35-margin-inventory.md`, `crates/contracts/src/lib.rs` (T1 вариант + schema, ТОЛЬКО через этот RFC), сген. JSON Schema, `crates/contracts/tests/ct_rfc05.rs`, `crates/venue-binance/tests/red_margin_inventory.rs`, `scripts/verify_M-35.sh`.
- **venue-dev (impl):** `crates/venue-binance/src/lib.rs` (parse + auth poll + recorder-hook), env-ключ инъекция.
- **Forbidden:** risk/ks/oms, ЛЮБОЙ order-egress (submit/cancel/торговая подпись — read-only!), другие T1-варианты вне CT-RFC-05.

## Gates
- **CT-RFC → critic (doc-гейт §1.1) + risk-critic (§5 contracts)** — ОБА обязательны (contract-форма + первый auth-источник). risk-critic верифицирует read-only (нет order-пути, ключ без trade/withdraw).
- **§8** доступен (CI green, TD-037 закрыт). Merge держится до risk-critic PASS + reviewer APPROVED.

## Acceptance (`scripts/verify_M-35.sh`)
CI-точно (fmt+clippy). MI-I-1 (ct_rfc05) GREEN; MI-I-2/4 (red_margin_inventory) GREEN; MI-I-3 canary grep
(нет order-egress в margin-пути); контракт roundtrip старых RFC (ct_rfc01/red_rfc02) регресс-GREEN;
`SCHEMA_VERSION==4`; финал `VERDICT: PASS`.

## Handoff (staged)
Task 1 (CT-RFC код + RED) — **architect, следующий фокус-шаг** (контракт+schema-эпоха заслуживают
аккуратного прохода с dual-build reachability, не хвост марафона). Затем **critic → risk-critic** (governance),
затем **venue-dev** (task 2), затем **reviewer** §8. Данные копятся с момента деплоя task 2.
