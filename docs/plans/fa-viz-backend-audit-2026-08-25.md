<!-- FACTS: audited_head=721461cb7d41a625b28026f73d1aa5a3e62091cd collected=2026-08-25 -->

# Аудит `docs/fa/viz-backend.md` против `origin/main` (721461c)

> **ДИФФ §8 ПРИМЕНЁН ЦЕЛИКОМ — проверено 2026-08-29, все двадцать пунктов.** Документ
> приземляется как ЗАПИСЬ проверки (30 утверждений, снятых командами), а не как задача.
> Проверка каждого пункта: `grep -qF '<маркер СТАЛО>' docs/fa/viz-backend.md` — `Д-1`…`Д-18`
> присутствуют в `main`, `Д-19` (порядок `VB-I-10`/`VB-I-11`) соблюдён, `Д-20` (указатель
> дома `GW-I`/`GS-I`) на месте.
>
> Головной маркер `FACTS` НЕ сдвинут намеренно: он датирует сбор 25.08 над `721461c`, и
> утверждения §1–§7 сняты именно там. Сдвиг датировал бы их сегодняшним числом, чего я не
> делал.

**Роль:** клон architect'а (аудит; замер и суждение, НЕ авторство — правку FA авторит ведущий).
**Дерево:** `/tmp/hft-audit-fa2`, ветка `docs/fa-viz-audit-2026-08-25` от `origin/main` = `721461cb7d41a625b28026f73d1aa5a3e62091cd` (merge PR #95, 2026-08-25).
**Предмет:** `docs/fa/viz-backend.md` целиком (181 строка, 26 784 B).
**Ярус B прочитан целиком:** `fa/book.md` · `fa/venues.md` · `fa/journal.md` · `fa/research-cli.md` · `fa/ai-copilot.md` · `07-cockpit-backend-roadmap.md` · `06-data-layer-and-storage.md` · `05-contract-layer.md` · `CT-RFC-09` · `A-002` · тела `П-011`/`П-013`/`П-014`/`П-017`; `depth-verdict.md` и `depth-probe-binance.md` — цитируемые разделы построчно. `fa/contracts.md` — §1–§5 + карта секций (остальное грепом). Ярус C — грепом по `TD-004 TD-016 TD-039 TD-045 TD-047 TD-048 TD-158 TD-159` (включая `docs/archive/TECH-DEBT-*`).

**Итог одной строкой:** документ конструктивно жив (§1/§5/§6 в основном верны), но §2-таблица статусов протухла целиком, §4-блок «два предусловия П-014» ложен по существу в части (а) — код с тех пор закрыл сторону и охват, — а `П-013` (подписан 2026-08-17) снял `formula_pending` с состава TOTAL, о чём FA не знает. Плюс три протухших номера строк, одна висячая ссылка на несуществующий milestone (M-31) и внутреннее противоречие postcard↔JSON между §3 и VB-I-6.

---

## §1. Таблица A — каждое утверждение FA о коде

Вердикты: **ВЕРНО** · **ПРОТУХ НОМЕР** (существо верно, строка/номер уехали) · **ЛОЖНО ПО СУЩЕСТВУ**.

| # | FA (строка) | Утверждение | Команда | Факт | Вердикт |
|---|---|---|---|---|---|
| A-1 | :14 | `journal::stream(dir, EpochFilter)` — путь связи с данными | `grep -n "pub fn stream(" crates/journal/src/segments.rs` | `segments.rs:1829: pub fn stream(dir: impl AsRef<Path>, filter: EpochFilter)`; gateway импортирует (`crates/gateway/src/lib.rs:20`) | **ВЕРНО** |
| A-2 | :14 | ссылка «`docs/06` §5, bounded-memory итератор» | чтение `docs/06` целиком | §5 = «Сжатие»; streaming-чтение описано в §6 (`journal::reader` — итератор) | **ПРОТУХ НОМЕР** (ссылка неточна) |
| A-3 | :26-27 | JWT: «крейт `jsonwebtoken`, HS256/Ed25519» | `grep -n jsonwebtoken crates/gateway-serve/Cargo.toml`; `grep -n from_secret crates/gateway-serve/src/lib.rs` | `jsonwebtoken = "9"` (:33); код — `DecodingKey::from_secret` = HS256; Ed25519 не реализован | **ВЕРНО** как контракт-опция; реализована только HS256 |
| A-4 | :37 | OHLCV/footprint/CVD/bins/depth — «✅ есть (M-17 export v1)» | `head -3 research/exports/format.md` | `export_schema_version: 1`, стабильный контракт | **ВЕРНО** |
| A-5 | :38 | Heatmap/COB/Bubbles — «🟡 новый (M-23)» | `ls docs/archive/ \| grep M-23`; `ls crates/gateway/tests/ \| grep -E "heatmap\|bubbles"` | `M-23-heatmap.md` в архиве (DONE); `red_heatmap.rs`, `red_bubbles.rs`; `SeriesBundle.{heatmap,cob,volume_bubbles}` | **ЛОЖНО ПО СУЩЕСТВУ** (сделано) |
| A-6 | :39 | Volume Profile — «❌ новый (M-24)» | `ls docs/archive/ \| grep M-24`; `ls crates/gateway/tests/red_volume_profile.rs` | M-24 в архиве (DONE); оракул есть; `SeriesBundle.volume_profile` | **ЛОЖНО ПО СУЩЕСТВУ** (сделано) |
| A-7 | :40 | VWAP «(session/anchored, HLC3, σ-полосы) — 🟡 M-20 (QUEUED)» | `ls docs/archive/ \| grep -E "M-20\|M-36"`; `grep -in "hlc3\|sigma" crates/gateway/src/lib.rs` | M-20 и M-36 в архиве; реализован journal-cumulative `Σ(p·v)/Σv` (lib.rs:403 «session-reset СНЯТ»); HLC3/σ-полос нет | **ЛОЖНО ПО СУЩЕСТВУ** — строка противоречит VB-I-6 ЭТОГО ЖЕ документа |
| A-8 | :41 | Liq/OI/Funding профили — «🟡 новый (M-25)» | `ls milestones/ docs/archive/ \| grep M-25`; поля `SeriesBundle` | файла M-25 нет (только план в BACKLOG:16); серий liq/oi/funding в `SeriesBundle` нет | **ВЕРНО** (не сделано; номер не аллоцирован файлом) |
| A-9 | :42 | TPP полосы «✅ APPROVED (M-32; 30–60% — follow-up live-замер)» | `ls docs/archive/ \| grep M-33`; `depth-verdict.md:64` | M-33 (follow-up 30–60%) СОСТОЯЛСЯ и его вердикт стоит на поражённой метрике (A-002 З-2, `depth-verdict.md:18`); действующий статус — П-014 + TD-158/TD-159 | **ПРОТУХ** (follow-up уже не «предстоит») |
| A-10 | :43, :139, :170 | TOTAL/TOTAL1-3/OTHERS + Secrets — «⛔ formula_pending (founder-спека)» | `sed -n '823,862p' docs/PENDING-SIGNATURE.md` | **П-013 ПОДПИСАН**: состав TOTAL-семейства подписан; `formula_pending` остаётся ТОЛЬКО на Secrets; П-013 прямо предписывает правку FA через гейт §9 | **ЛОЖНО ПО СУЩЕСТВУ** (наполовину снято подписью) |
| A-11 | :61 | «A. Indicator Engine — `crates/derive` + `crates/research-cli` (расширяются)» | `ls crates/derive/src/`; `head -3 crates/derive/src/lib.rs`; `grep -rln footprint crates/` | `derive` = скелет с единственной функцией Breadth; живые виз-редьюсеры — `crates/gateway::Reducer` + `research-cli/{orderflow,export_io}.rs` | **ЛОЖНО ПО СУЩЕСТВУ** (engine вырос в gateway, не в derive) |
| A-12 | :66-67 | «Тяжёлые серии (heatmap/depth) — бинарные фреймы (postcard)» | `head -9 crates/gateway-serve/src/lib.rs`; `grep -rn serde_json::to_vec crates/gateway-serve/src/` | lib.rs:8-9: «Wire-формат MVP — JSON… postcard — Rust-only, НЕ годится для фронта»; весь провод — `serde_json::to_vec` (:444, :1401, :1719); postcard живёт в ЧЕКПОИНТЕ (GW-I-9, gateway/Cargo.toml:21-23) | **ЛОЖНО ПО СУЩЕСТВУ** + внутреннее противоречие с VB-I-6 (:153 «serde_json::to_vec») |
| A-13 | :68-69 | export v2: «`format.md` расширяется АДДИТИВНО (bump `export_schema_version`)» | `grep -n export_schema_version research/exports/format.md` | format.md остался `v1`; версия серий уехала в `GATEWAY_SCHEMA_VERSION` (=8); оракул «export v2» (`red_gateway_export_v2.rs`) проверяет GATEWAY_, не export_ | **ПРОТУХ** (референт версии сместился) |
| A-14 | :78-80 | охват diff-книги: spot 50.12/59.13 и 50.07/59.16, futures 57–59 % (`depth-probe-binance.md:62-65`) | `sed -n '62,65p' research/data-quality/depth-probe-binance.md` | строки 62-65 несут ровно эти числа (fut 59.27/59.51 и 57.33/58.45) | **ВЕРНО** |
| A-15 | :80-81 | `MAX_REL_DIST = 0.60` (`crates/venue-binance/src/lib.rs:33`) | `grep -n MAX_REL_DIST crates/venue-binance/src/lib.rs` | `:33: const MAX_REL_DIST: f64 = 0.60;` | **ВЕРНО** |
| A-16 | :85-86 | метка = `"diff-reconstructed, validated<=1.3%"` | `sed -n '1363,1385p' crates/gateway/src/lib.rs` | живое СОДЕРЖИМОЕ строки другое: `None` (≤1.3%) / `"not-observed band=… reach=…"` / `"diff-reconstructed, liveness=confirmed\|unconfirmed"` | **ПРОТУХ** (форма поля та же, содержимое сменено П-014-фиксом) |
| A-17 | :98-102 | «З-1 СНЯТ подписью `П-014` путём (b) A-002 §2; З-2 остаётся» | `sed -n '749,801p' docs/PENDING-SIGNATURE.md`; `A-002` §2 | дословно совпадает с П-014 и A-002 §2 «Что снимает замок» (b) | **ВЕРНО** |
| A-18 | :104-105 | `GATEWAY_BANDS` дефолт `0.001`, `docker-compose.yml:134,197` | `grep -n GATEWAY_BANDS docker-compose.yml` | `:134` env, `:203` `--bands` (было 197) | **ПРОТУХ НОМЕР** (197→203; дефолт верен) |
| A-19 | :106-109 | M-58: ask опровергнут `[300,500)`=0.419, `[800,1500)`=0.247, `[3000,6000)`=0.403; bid 0.713–0.992 (`depth-verdict.md:80-98`) | `grep -n "\[300,500)" research/data-quality/depth-verdict.md` | строки 89-98 полной таблицы: числа совпадают | **ВЕРНО** |
| A-20 | :114-115 | «метка ставится ТОЛЬКО по ширине: `crates/gateway/src/lib.rs:1035` — `(row.band_pct_e8 > 1_300_000).then(...)`» | `grep -n "band_pct_e8 > 1_300_000\|depth_provenance_label" crates/gateway/src/lib.rs` | формы `.then(...)` в коде НЕТ; живёт `depth_provenance_label(band_pct_e8, side, reach)` — fn на `:1363`, вызов на `:1106`; охват снимается `:975-976` (`book.max_reach_pct` per side); оракул `red_depth_provenance_by_reach.rs` в CI | **ЛОЖНО ПО СУЩЕСТВУ** — находка ведущего №1 ПОДТВЕРЖДЕНА |
| A-21 | :115-118 | `REST_DEPTH_LIMIT = "5000"` (`:27`); ~1.3 % BTC / ~4.5 % ETH / фьючерсы 500 ур. ~0.09/0.26 % (`depth-probe-binance.md:15-18`) | `grep -n REST_DEPTH_LIMIT crates/venue-binance/src/lib.rs`; `sed -n '15,18p' …depth-probe-binance.md` | `:27: const REST_DEPTH_LIMIT: &str = "5000";`; probe :15-18 несут ровно эти cap'ы | **ВЕРНО** — находка ведущего №3 ПОДТВЕРЖДЕНА |
| A-22 | :120 | «Барьера, удерживающего эмиссию до восстановления глубины, НЕТ» | `sed -n '244,262p' crates/venue-binance/src/lib.rs` | верно и сегодня: `resyncing` — только дедуп запроса снапшота; эмиссия не удерживается. НО следствие «метка о них молчит» снято: полоса за охватом получает `"not-observed…"` (`:1372-1377`) | **ВЕРНО наполовину**: барьера нет, молчания метки больше нет |
| A-23 | :124-126 | каденция: депт-серия snapshot-only, «`crates/gateway/src/lib.rs:938-941`» | `grep -n "депт-серия остаётся snapshot-only" crates/gateway/src/lib.rs` | комментарий жив на `:984-986` (M-23-ветка L2Delta); существо верно — `depth_series` в ветке `L2Delta` не пересчитывается | **ПРОТУХ НОМЕР** — находка ведущего №2 ПОДТВЕРЖДЕНА (:986, не :938-941) |
| A-24 | :129, :153, :157 | bump-цепочка `GATEWAY_SCHEMA_VERSION`: M-36 5→6, M-38a 6→7, M-48 7→8 | `grep -n "GATEWAY_SCHEMA_VERSION: u32" crates/gateway/src/lib.rs` | `:65: pub const GATEWAY_SCHEMA_VERSION: u32 = 8;` | **ВЕРНО** |
| A-25 | :141 | эпохи: «CT-RFC-02, `docs/06` §эпохи» | чтение `docs/06` целиком; `ls docs/rfc/ docs/data-epochs.md` | в `docs/06` секции «эпохи» НЕТ; эпохи живут в `CT-RFC-02` + `docs/data-epochs.md` | **ПРОТУХ НОМЕР** (висячая половина ссылки) |
| A-26 | :153 (VB-I-6) | `SeriesBundle` несёт `cvd_session_base` и `vp_session_max_time_s`, оба `#[serde(default)]`, провод — `serde_json::to_vec` целиком | `grep -nE "pub (cvd_session_base\|vp_session_max_time_s)" crates/gateway/src/lib.rs`; `grep -n serde_json::to_vec crates/gateway-serve/src/lib.rs` | оба поля есть; сериализация `ServeMsg` целиком через `serde_json::to_vec` (:444/:1401/:1719) | **ВЕРНО** |
| A-27 | :157 (VB-I-11) | `Snapshot` несёт `history_start_seq` + `history_truncated`, одинаково на обоих путях | `grep -n "history_start_seq\|history_truncated" crates/gateway/src/lib.rs` | поля `:326`/`:334`; логика first_folded_seq `:1394+` | **ВЕРНО** |
| A-28 | :96 | «(TD-016, Track A — **M-31**)» | `ls milestones/ docs/archive/ \| grep -c M-31`; `grep -n M-31 milestones/BACKLOG.md` | **0** — файла M-31 не существует нигде; в BACKLOG не значится | **ЛОЖНО ПО СУЩЕСТВУ** (висячий milestone-номер) |
| A-29 | :150 (VB-I-3) | grep-канарейка read-only | `ls crates/gateway/tests/red_gateway_readonly.rs` | есть, в CI (`cargo test --all`) | **ВЕРНО** |
| A-30 | :156 (VB-I-9) | канарейка «gateway не импортирует postgres/sqlx/diesel» | `grep -rn "GS-I-1 (VB-I-9a)" scripts/verify_M-28.sh`; `grep -rn verify_M .github/workflows/ci.yml` | канарейка живёт в `scripts/verify_M-28.sh:22-27` и **в CI НЕ гоняется** (ci.yml зовёт только `verify_delivery_M-08`/`verify_contracts`/`verify_ct_rfc_atomic`); JWT-половина (`red_jwt_verify.rs`, VB-I-9b) — в CI | **ВЕРНО с оговоркой** (механизация частичная, FA о расщеплении 9a/9b молчит) |

Числа `L2Delta`-охвата (`DESIGN` §17 «BTC-only») сверены отдельно: `parse_capture_symbols` дефолт `PROD_DEFAULT = "BTCUSDT"` (`crates/venue-binance/src/lib.rs:778-791`), env `L2DELTA_CAPTURE_SYMBOLS` в `docker-compose.yml` НЕ пробрасывается — прод-эмиссия L2Delta по-прежнему BTC-only; механизм M-45 (env-конфигурируемый allow-list) влит, раскатка — Граница C, не выполнена. FA §2 строка heatmap «L2Delta (M-18)» этому не противоречит.

## §2. Таблица B — `VB-I-1..11`: оракулы, CI, соответствие текста

CI гоняет `cargo test --all` (`.github/workflows/ci.yml:24`) ⇒ всё, что в `crates/*/tests/`, — в гейте.

| ID | Живой оракул (файлы с меткой ID) | В CI | Текст ↔ оракул |
|---|---|---|---|
| VB-I-1 | `red_vwap.rs`, `red_gateway_bounded.rs`, `red_depth_lifetime.rs` (+ детерминизм-кейсы в heatmap/VP) | да | соответствует |
| VB-I-2 | `red_gateway_live_eq_replay.rs`, `red_checkpoint_byte_identity.rs`, `red_gateway_window.rs`, `red_frames_seek_bound.rs`, `red_serve_consumes_checkpoint.rs`, `red_tail_scan_bounded.rs` | да | соответствует |
| VB-I-3 | `red_gateway_readonly.rs` | да | соответствует |
| VB-I-4 | `red_gateway_export_v2.rs` (шапка: GW-I-5 = VB-I-4) | да | **дрейф референта**: текст говорит «bump `export_schema_version`», оракул пиннит `GATEWAY_SCHEMA_VERSION`; `format.md` остался v1 |
| VB-I-5 | `red_depth_provenance_by_reach.rs`, `red_gateway_export_v2.rs` (GW-I-6) | да | соответствует; но пример метки в §4 (:85-86) — прежняя строка, не живое содержимое |
| VB-I-6 | `red_gateway_cvd_session.rs`, `red_vwap.rs`, `red_volume_profile.rs`, `red_retention_checkpoint_coverage.rs` | да | соответствует |
| VB-I-7 | **НИ ОДНОГО** (`grep -rln "VB-I-7" crates/*/tests/` → пусто; `formula_pending` в коде — только комментарий `lib.rs:330`) | — | **вакуум**: ни одна серия не эмитит `formula_pending`, оракула нет; после П-013 инвариант СУЖЕН до Secrets — текст FA этого не знает |
| VB-I-8 | **метки нет**; `red_volume_profile.rs` несёт существо (vp_poc/vp_value_area…), именованного теста «цены без сделок не выдумываются» я не нашёл | да (по существу) | долг маркировки, а возможно и покрытия — проверить ведущему |
| VB-I-9 | расщеплён: **VB-I-9a** — канарейка `scripts/verify_M-28.sh:22-27` (**вне CI**); **VB-I-9b** — `red_jwt_verify.rs` (в CI) | частично | FA не знает о расщеплении 9a/9b и о том, что канарейка app-БД не в CI |
| VB-I-10 | `red_gateway_window.rs`, `red_frames_seek_bound.rs`, `red_checkpoint_byte_identity.rs`, `red_window_guard_startup.rs` | да | соответствует |
| VB-I-11 | `red_checkpoint_bootstrap_truncated.rs`, `red_gateway_schema_version.rs`, `red_ws_honesty_sessions.rs` | да | соответствует |

Счёт «в оракулах»: с меткой — 8 полных ID + 9a/9b-варианты; сходится с `DESIGN` §22 «VB-I: 11 заявлено / 8 в оракулах [ЧАСТИЧНО]».

**Порядок строк §5: …VB-I-8, VB-I-9, VB-I-11, VB-I-10 — разрыв СЛУЧАЕН.** VB-I-11 (M-48) вставлен позже VB-I-10 (M-37) не на своё место; семантической группировки нет — сам текст VB-I-11 называет себя «тот же класс честности, что VB-I-5 и VB-I-7», но стоит не рядом с ними. Перестановка в числовой порядок — изложение (но любая правка FA идёт через §9).

**VB-I, объявленные вне FA:** `VB-I-9a`/`VB-I-9b` (в `milestones/M-28` (архив), `scripts/verify_M-28.sh`, `red_jwt_verify.rs`) — суффиксы в FA не заведены. `VB-I-12+` не существует нигде (`grep -rn "VB-I-1[2-9]"` → пусто).

## §3. Семейство `GW-I` — обратный дрейф

Замер: `grep -rhoE '\bGW-I-[0-9]+\b' crates/ | sort -uV` → `GW-I-1..12, GW-I-14` — **13 уникальных, `GW-I-13` — дыра** (M-69 добавил `GW-I-14`, `red_window_guard_startup.rs:1`).

- `DESIGN` §22 (:924): «GW-I | gateway-serve | 0 | 13 | обратный дрейф» — **счёт верен** (13), но не говорит, что это 1..12+14, а не 1..13.
- `reading-map.md` §2 (:82): «GW-I — 12 оракулов» — **протух** (не мой файл; назвать ведущему).
- Фактический дом деклараций: `docs/plans/gateway-ws-contract.md` §7 (таблица GW-I-* и GS-I-*) + архивные milestone-файлы. Оба — не нормативный FA-корпус: `plans/` — фактура, архив — история.

**Где семейство ДОЛЖНО быть объявлено — предложение с ценой обоих вариантов (решение не моё; «ничего нового» BINDING — 8c2d972):**

- **(a) §5bis в `viz-backend.md`** («смежные семейства Слоя 8: GW-I — gateway, GS-I — gateway-serve; декларации — таблицей, оракулы — по файлам тестов»). Цена: FA слоя 8 принимает инварианты двух крейтов, документ пухнет (~+50 строк), и «viz-backend» перестаёт быть FA одного предмета. Выгода: ноль новых файлов, `reading-map` §2 уже направляет сюда («опора: fa/viz-backend.md + DESIGN §22»), долг «FA не существует» закрывается ближайшим носителем.
- **(b) новый FA крейта `gateway` (+`gateway-serve`) — файла такого НЕТ и в этом весь предмет** — честный дом. Цена: новый FA = новый носитель под полный круг §9 + правка `reading-map` §2 и `DESIGN` §22 тем же коммитом; идёт против решения founder'а «ничего нового» от 24.08 — требует его явного слова. Выгода: слой 8 расщеплён по крейтам, как весь остальной корпус.

Минимальный ход, не требующий решения: в FA §5 добавить ОДНУ строку-указатель «оракульные семейства GW-I (13: 1..12,14) и GS-I (1..5) живут в `crates/gateway*/tests/` + `docs/plans/gateway-ws-contract.md` §7; докс-дом не заведён — долг». Это констатация факта, не новая норма.

## §4. Стыки — по абзацу на соседа

**book.** `fa/book.md` §7 определяет `depth(side, pct_band)` **от microprice** с монотонностью BK-I-6. Gateway считает полосы СВОЕЙ функцией `depth_within` (`crates/gateway/src/lib.rs:1312`) **от mid** = `(best_bid+best_ask)/2` по уровням эмитированного `L2Snapshot`. Это РАЗНЫЕ величины по референс-цене; владельца определения «полосы глубины» де-факто двое, и FA viz-backend об этом молчит (в §2 пишет «L2 (book depth)»). Дополнительно: `book::max_reach_pct` (`crates/book/src/lib.rs:287`) стал несущим для провенанса (:975-976 gateway), но ни `fa/book.md`, ни FA viz-backend его не упоминают. `DESIGN` §22: BK-I «8 заявлено / 0 оракулов» — при этом `crates/book/tests/` содержит 6 файлов без BK-I-маркировки (класс VN-I: долг маркировки).

**venues.** `MAX_REL_DIST=0.60` (:33) и `REST_DEPTH_LIMIT="5000"` (:27) — верны (A-15/A-21). Состав `L2Delta`: BTC-only по дефолту `parse_capture_symbols` (:778-791), env не проброшен в compose — `DESIGN` §17 («исключение — BTC») актуален, M-45-раскатка не случилась. `fa/venues.md` — исторический документ (описывает несуществующий крейт `crates/venues/`, HL как первый адаптер) со встроенным дисклеймером в шапке; FA viz-backend на него не опирается — конфликта нет.

**journal.** `journal::stream(dir, EpochFilter)` жив (`segments.rs:1829`), gateway строит на нём (шапка `lib.rs:12` прямо запрещает `read_all`); bounded-memory пиннится `red_tail_scan_bounded.rs` (journal) + VB-I-10-оракулами (gateway). Стык здоров; ссылка FA «docs/06 §5» неточна (A-2).

**gateway-serve.** Дом `GS-I-1..5` — `docs/plans/gateway-ws-contract.md` §7 + архивный `milestones/M-28`; оракулы живы и в CI (`red_jwt_verify`, `red_serve_passthrough`), НЕ осиротели — но нормативного дома нет (тот же долг, что GW-I, §3). `VB-I-9` и `GS-I-1` — один инвариант с двух сторон, разведён как `VB-I-9a`; FA расщепления не знает (A-30). **Крупнее:** FA §3.B описывает транспорт ДО-сессионной эпохи — `CT-RFC-09` (форма подписана founder'ом 2026-08-11) и `M-65` (**ЗАКРЫТ 2026-08-24**, PR #29 `63c3866`) ввели subscribe-протокол v1 (подписка = параметр СЕССИИ, мультиплекс, лимит 16); FA не упоминает ни RFC, ни `GATEWAY_WIRE_VERSION`. §8 cross-references CT-RFC-09 не содержит.

**derive.** Крейт СУЩЕСТВУЕТ (`crates/derive/`), но это скелет: единственная функция Breadth (фандинг-разрез), один оракул `red_breadth.rs`. «Indicator Engine = crates/derive + research-cli» (§3.A) — не реальность: несущие редьюсеры кокпита живут в `crates/gateway::Reducer` (A-11).

**contracts / T-designate.** `05` §2 держит виз-формы T-designate («промоушен в T1 — при первом кросс-языковом консюмере»). Факт: кросс-языковой консюмер УЖЕ существует — JS-фронт читает JSON-провод (`GS-I-4`), и `format.md` v1 писался под `code2alpha`. Формально это тот самый триггер промоушена; решение о промоушене — architect+критик (contract-RFC), не этот аудит. Заметь: `fa/contracts.md` виз-форм не упоминает вовсе (`grep "T-designate\|viz\|gateway\|export" docs/fa/contracts.md` → пусто) — знание живёт только в `05` §2.

## §5. Статусы решений

- **`A-002`** (прочитан целиком). З-1/З-2 определены в §2; снятие путём (b) = «явная founder-подпись, принимающая включение на основании ТОЛЬКО провенанса». FA :98-102 пересказывает ТОЧНО. З-2 в силе — подтверждено и `depth-verdict.md:64,119`, и П-014.
- **`П-011`** — история пути: исход M-58 «третий, смешанный»; замок остался; варианты (в)/(г)/(б) ушли founder'у. Снят последующей подписью П-014. FA согласован.
- **`П-014`** — ПОДПИСАН 2026-08-17: «включать», различение сторон «пометка посторонне». Четыре пункта исполнения: (1) метка посторонне — **СДЕЛАНО** (`depth_provenance_label`, оракул `red_depth_provenance_by_reach.rs`); (2) каденция названа в выдаче — **НЕ сделано** (`TD-158` OPEN, Ф2/Ф4 MAJOR); (3) bump при смене формы — не потребовался (П-014 п.3: форма `Option<String>` не менялась, менялось содержимое — так и записано в коде `:1361-1362`); (4) состав `GATEWAY_BANDS` на канонический набор — **НЕ сделано** (`0.001` в compose:134,203; `TD-159` OPEN «блокирует П-014 п.4»).
- **`П-013`** — ПОДПИСАН: состав TOTAL-семейства (TOTAL=Σ всех; TOTAL1=BTC; TOTAL2=Σ−BTC; TOTAL3=Σ−BTC−ETH; OTHERS=Σ−топ-10; суммирование прямое; минутная сетка avg/last-carry). `formula_pending` остаётся ТОЛЬКО на Secrets. FA (:43, :139, :170) НЕ знает — П-013 сам называет правку FA отдельным предметом через §9.
- **`П-017`** — «Следствие для П-014: два предусловия, ни одно не закрыто» — было верно на 20.08 и **протухло**: предусловие (а) в части «метка не знает ни о стороне, ни о ресинке» закрыто кодом (метка знает сторону; полоса за охватом — `not-observed`, что закрывает ресинк-окно на уровне СТРОКИ). Остаток (а): `TD-159` — метка ОДНА на ряд точек разного качества (точка из окна ресинка описывается меткой последнего наблюдения). Предусловие (б) — открыто (`TD-158`).

**Ответ на вопрос мандата «если (а) закрыто — меняется ли вывод „включать нельзя"?»:** вывод НЕ снимается, но его основание сместилось и FA обязана назвать новое: блокируют теперь не «метка слепа к стороне/ресинку» (ложь на текущем коде), а **`TD-158`** (каденция не названа в выдаче — П-014 п.2 не исполнен) и **`TD-159`** (per-point честность — блокирует П-014 п.4). Оба — MAJOR, OPEN. Снимать ли включение с них — решение ведущего/критика, не этого аудита.

## §6. TD-ссылки

| TD | Статус (команда: grep TECH-DEBT.md + docs/archive/TECH-DEBT-*) | FA |
|---|---|---|
| TD-016 | **OPEN** (сводная :86, Ф1 MAJOR; тело :579-636 «остаётся OPEN» многократно) | FA говорит в настоящем времени — ВЕРНО; но «Track A — M-31» — висячий номер (A-28) |
| TD-039 | **CLOSED** (M-37 DONE: `docs/archive/M-37-bounded-snapshot.md:3` «TD-039/040/042 CLOSED») | в VB-I-10 как история происхождения — корректно |
| TD-045 | **CLOSED 2026-07-27** (`docs/archive/TECH-DEBT-closed-2026-08-16.md:1613`) | в VB-I-6/10 как история — корректно |
| TD-047 | **CLOSED** (M-47: `docs/archive/M-47-timeframe-session-guard.md:10`) | в VB-I-6 как история — корректно |
| TD-048 | **CLOSED** (`TECH-DEBT-closed-2026-08-16.md:1462`; M-48 DONE) | в VB-I-11 как история — корректно |
| TD-004 | **закрыт триажем 2026-08-17 как ЛОЖНАЯ карточка** (`docs/archive/TECH-DEBT-triage-2026-08-17.md:40`) | §8 (:180) ссылается как на живой — ПРОТУХ |
| TD-010 | **ОТКРЫТА** (TECH-DEBT.md:2804) | §8 ссылка жива |
| **TD-158** | **OPEN** (:92 сводной, Ф2/Ф4 MAJOR) — «каденция не названа в выдаче, П-014 п.2» | **в FA НЕ назван** — а это живой блокер темы §4 |
| **TD-159** | **OPEN** (:93 сводной, Ф2/Ф4 MAJOR) — «метка одна на ряд точек разного качества; блокирует П-014 п.4» | **в FA НЕ назван** — остаток предусловия (а) |

## §7. Мёртвые ссылки, история vs действующее, самопротиворечия

1. **M-31 (:96)** — milestone-файла не существует нигде (A-28). Самая старая висячая ссылка документа.
2. **`docs/06` §эпохи (:141)** — секции нет; эпохи — `CT-RFC-02` + `docs/data-epochs.md` (A-25). Смежно: «docs/06 §5» для stream (:14) — §5 про сжатие.
3. **`crates/gateway/src/lib.rs:1035` (:114)** и **`:938-941` (:126)** — оба номера мертвы (A-20/A-23).
4. **`docker-compose.yml:197` (:104)** → :203 (A-18).
5. **postcard (:66-67) ↔ serde_json (VB-I-6, :153)** — прямое самопротиворечие внутри документа; истина — JSON (A-12).
6. **строка VWAP §2 (:40) ↔ VB-I-6 (:153)** — самопротиворечие по семантике якоря (session-anchored+HLC3 против journal-cumulative); истина — VB-I-6 (A-7).
7. **История, не отделённая от действующего:** §2-таблица статусов (:37-43) читается как текущее состояние, будучи снимком 2026-07-22; блок «ВЕРИФИКАЦИЯ M-32… FOUNDER APPROVED… диапазон 1.5–60%» (:130-137) помечен историей строкой :112 — этот приём (`reading-map` §3) надо распространить на §2-таблицу.
8. **§7 «Открытые вопросы» (:169-173):** п.1 наполовину закрыт П-013 (A-10); п.3 «транспорт бинарных фреймов (msgpack/protobuf) — детали M-22» закрыт фактом: MVP-провод — JSON, смена кодека предусмотрена бампом `v` (CT-RFC-09 §3); п.4 (resync-целостность) остаётся открытым, но его формулировка не знает `not-observed`-метки.

## §8. ГОТОВЫЙ ДИФФ (`docs/fa/viz-backend.md`, строки — по 721461c)

Применять механически; правку авторит и коммитит ведущий architect (гейт §9 обязателен — см. §9 ниже).

**Д-1 (:38).**
БЫЛО: `| **Heatmap / COB / Volume Bubbles** | L2Delta (M-18) / Trade | 🟡 новый (M-23) |`
СТАЛО: `| **Heatmap / COB / Volume Bubbles** | L2Delta (M-18) / Trade | ✅ есть (M-23 DONE; \`SeriesBundle.{heatmap,cob,volume_bubbles}\`) |`
ПОЧЕМУ: `ls docs/archive/M-23-heatmap.md`; поля в `crates/gateway/src/lib.rs` (`grep -n "pub heatmap" …`).

**Д-2 (:39).**
БЫЛО: `| **Volume Profile** (SVP/CVP/FRVP/Anchored/Composite: POC/VAH/VAL/HVN/LVN, VA%) | Trade | ❌ новый (M-24) |`
СТАЛО: `| **Volume Profile** (SVP/CVP/FRVP/Anchored/Composite: POC/VAH/VAL/HVN/LVN, VA%) | Trade | ✅ есть (M-24 DONE; session-anchored, VB-I-6/VB-I-8) |`
ПОЧЕМУ: `ls docs/archive/M-24-volume-profile.md`; `crates/gateway/tests/red_volume_profile.rs`.

**Д-3 (:40).**
БЫЛО: `| **VWAP** (session/anchored, HLC3, σ-полосы) | Trade | 🟡 M-20 (QUEUED) |`
СТАЛО: `| **VWAP** (journal-cumulative — семантика пересмотрена M-36, см. VB-I-6; HLC3/σ-полосы НЕ реализованы) | Trade | ✅ есть (M-20→M-36 DONE) |`
ПОЧЕМУ: `milestones/M-36-gateway-snapshot-prod.md` (в редакции аудита путь назывался как `docs/archive/…`; переезда не было — поправлено при приземлении 2026-08-29); `grep -n "session-reset СНЯТ" crates/gateway/src/lib.rs` → :403; `grep -in hlc3 crates/gateway/src/lib.rs` → пусто.

**Д-4 (:42).**
БЫЛО: `| **TPP Bid/Ask, Delta** (полосы 1.5/3/5/8/15/30/60%, per side, COIN-scope) | L2 (book depth) | ✅ APPROVED (M-32, provenance; 30–60% — follow-up live-замер) |`
СТАЛО: `| **TPP Bid/Ask, Delta** (полосы 1.5/3/5/8/15/30/60%, per side, COIN-scope) | L2 (book depth) | ✅ включение подписано \`П-014\`; в проде НЕ включено (\`GATEWAY_BANDS=0.001\`) до TD-158/TD-159; follow-up 30–60% (M-33) состоялся, вердикт suspended — A-002 З-2 |`
ПОЧЕМУ: `docs/archive/M-33-depth-band-3060.md`; `depth-verdict.md:18,64`; `TECH-DEBT.md:92-93`; `docker-compose.yml:134,203`.

**Д-5 (:43).**
БЫЛО: `| TPP TOTAL/TOTAL1-3/OTHERS, Secrets (MLSP/Margin/Ratio/Speed) | — | ⛔ \`formula_pending\` (founder-спека) |`
СТАЛО: `| TPP TOTAL/TOTAL1-3/OTHERS | — | ✅ состав подписан \`П-013\` 2026-08-17 (TOTAL=Σ всех; T1=BTC; T2=Σ−BTC; T3=Σ−BTC−ETH; OTHERS=Σ−топ-10); реализации нет |` + новой строкой `| Secrets (MLSP/Margin/Ratio/Speed) | — | ⛔ \`formula_pending\` (founder-спека; VB-I-7 сужен до Secrets — П-013) |`
ПОЧЕМУ: `docs/PENDING-SIGNATURE.md:823-862` (П-013, включая «Операционно»).

**Д-6 (:61).**
БЫЛО: `- **A. Indicator Engine** — \`crates/derive\` + \`crates/research-cli\` (расширяются). Новые агрегаты — новые`
СТАЛО: `- **A. Indicator Engine** — фактически \`crates/gateway\` (\`Reducer\`) + \`crates/research-cli\`; \`crates/derive\` остался скелетом (Breadth). Новые агрегаты — новые`
ПОЧЕМУ: `ls crates/derive/src/` (один lib.rs, Breadth); `grep -n "struct Reducer" crates/gateway/src/lib.rs`.

**Д-7 (:66-67).**
БЫЛО: `фреймы (postcard). **Read-only**: не пишет журнал, recorder не зависит от gateway.`
СТАЛО: `фреймы — БЫЛ план; факт: провод целиком JSON (\`serde_json::to_vec\`, postcard отвергнут для фронта — \`gateway-serve/src/lib.rs:8-9\`; postcard живёт в чекпоинте, GW-I-9). **Read-only**: не пишет журнал, recorder не зависит от gateway.`
ПОЧЕМУ: `head -9 crates/gateway-serve/src/lib.rs`; согласование с VB-I-6 того же файла.

**Д-8 (:96).**
БЫЛО: `Track A — **M-31**); (б) целостность при resync (ресинк к мелкому снапшоту не должен ронять восстановимые`
СТАЛО: `Track A — milestone НЕ ЗАВЕДЁН, номер M-31 из ранней очереди не аллоцирован файлом); (б) целостность при resync (ресинк к мелкому снапшоту не должен ронять восстановимые`
ПОЧЕМУ: `ls milestones/ docs/archive/ | grep -c M-31` → 0.

**Д-9 (:104).**
БЫЛО: `(сегодня дефолт \`0.001\`, \`docker-compose.yml:134,197\`; смена состава — зона \`deploy/**\`);`
СТАЛО: `(сегодня дефолт \`0.001\`, \`docker-compose.yml:134,203\`; смена состава — зона \`deploy/**\`);`
ПОЧЕМУ: `grep -n GATEWAY_BANDS docker-compose.yml` → 134, 203.

**Д-10 (:113-123) — переписать блок предусловия (а); нормативная правка, критик обязателен.**
БЫЛО (:113-115, начало блока): `- **⛔ ДВА ПРЕДУСЛОВИЯ \`П-014\`, НИ ОДНО НЕ ЗАКРЫТО — включать до них нельзя.**` / `  - **(а) Провенанс не знает ни о стороне, ни о ресинке.** Сегодня метка ставится ТОЛЬКО по` / `    ширине полосы: \`crates/gateway/src/lib.rs:1035\` — \`(row.band_pct_e8 > 1_300_000).then(...)\`.`
СТАЛО: `- **⛔ ВКЛЮЧЕНИЕ ОСТАЁТСЯ ЗАБЛОКИРОВАННЫМ — но состав блокеров сменился.**` / `  - **(а) ЗАКРЫТО НА УРОВНЕ СТРОКИ (пост-20.08):** метка знает сторону и охват — \`depth_provenance_label(band_pct_e8, side, reach)\` (\`crates/gateway/src/lib.rs:1363\`, вызов :1106; охват — \`book.max_reach_pct\` per side, :975-976; оракул \`red_depth_provenance_by_reach.rs\`). Полоса за наблюдённым охватом (в т.ч. окно ресинка после REST-снимка \`REST_DEPTH_LIMIT="5000"\`, \`venue-binance/src/lib.rs:27\` — ~1.3 % BTC / ~4.5 % ETH spot, \`depth-probe-binance.md:15-18\`) получает \`"not-observed"\`. ОСТАТОК: метка ОДНА на ряд точек разного качества — **TD-159** (блокирует \`П-014\` п.4); барьера, удерживающего саму эмиссию до восстановления глубины, по-прежнему нет (\`venue-binance/src/lib.rs:246\` — resyncing только дедупит запрос).`
ПОЧЕМУ: A-20/A-21/A-22 (§1); `TECH-DEBT.md:93`.

**Д-11 (:124-126).**
БЫЛО: `  - **(б) Каденция.** Депт-серия пересчитывается ТОЛЬКО в ветке \`L2Snapshot\`: ветка \`L2Delta\`` / `    книгу и heatmap обновляет, а \`depth_series\` — нет (\`crates/gateway/src/lib.rs:938-941\`,` / `    комментарий «депт-серия остаётся snapshot-only»). Это 1 Гц против дельт 100 мс. Пока не`
СТАЛО: то же, с заменой номера: `(\`crates/gateway/src/lib.rs:984-986\`,` и добавлением в конец пункта: `Заведено долгом **TD-158** (\`П-014\` п.2 не исполнен).`
ПОЧЕМУ: `grep -n "депт-серия остаётся snapshot-only" crates/gateway/src/lib.rs` → 986; `TECH-DEBT.md:92`.

**Д-12 (:85-86).**
БЫЛО: `- **Инвариант провенанса:** каждая серия по книге глубже 1.3% несёт` / `  \`depth_band_provenance: "diff-reconstructed, validated<=1.3%"\`. Фронт/AI не выдают её за биржевой факт.`
СТАЛО: `- **Инвариант провенанса:** каждая серия по книге глубже 1.3% несёт \`depth_band_provenance\`; живое содержимое строки после \`П-014\`: \`"diff-reconstructed, liveness=confirmed|unconfirmed"\` (по стороне) либо \`"not-observed band=… reach=…"\` (за охватом) — \`depth_provenance_label\`, \`crates/gateway/src/lib.rs:1363\`. Фронт/AI не выдают её за биржевой факт.`
ПОЧЕМУ: A-16 (§1).

**Д-13 (:170, §7 п.1).**
БЫЛО: `1. TPP \`formula_pending\`: состав TOTAL1/2/3/OTHERS + формулы Secrets (founder).`
СТАЛО: `1. TPP \`formula_pending\`: ТОЛЬКО формулы Secrets (MLSP/Margin/Ratio/Speed/Market Diff) — состав TOTAL-семейства подписан \`П-013\` 2026-08-17.`
ПОЧЕМУ: П-013 «Операционно» (`PENDING-SIGNATURE.md:860-861`).

**Д-14 (:180, §8).**
БЫЛО: `- TD-016 (эвикция/фантом), TD-004/010 (REST-глубина).`
СТАЛО: `- TD-016 (эвикция/фантом, OPEN), TD-010 (REST-глубина, OPEN; TD-004 закрыт триажем 2026-08-17 как ложная карточка), TD-158 (каденция, OPEN), TD-159 (метка per-point, OPEN).`
ПОЧЕМУ: §6 этого аудита.

**Д-15 (:141).**
БЫЛО: `временного охвата (backtest/replay). Импорт — \`Vendor\`-эпоха (CT-RFC-02, \`docs/06\` §эпохи), fail-closed`
СТАЛО: `временного охвата (backtest/replay). Импорт — \`Vendor\`-эпоха (CT-RFC-02, \`docs/data-epochs.md\`), fail-closed`
ПОЧЕМУ: секции «эпохи» в docs/06 нет; `ls docs/data-epochs.md`.

**Д-16 (:154, VB-I-7).**
БЫЛО: `| VB-I-7 | \`formula_pending\`-серия НЕ эмитит вычисленное значение (только маркер), пока формула не подписана founder'ом |`
СТАЛО: `| VB-I-7 | \`formula_pending\`-серия НЕ эмитит вычисленное значение (только маркер), пока формула не подписана founder'ом. После \`П-013\` (2026-08-17) действует ТОЛЬКО для Secrets; оракула нет — серий \`formula_pending\` пока не существует в коде |`
ПОЧЕМУ: П-013; `grep -rn formula_pending crates/` → один комментарий lib.rs:330, тестов ноль.

**Д-17 (:176-180, §8 cross-refs) — добавить строку.**
СТАЛО (добавка): `- \`docs/rfc/CT-RFC-09-ws-session.md\` (WS-сессия: subscribe-протокол v1, лимит 16 — подписан 2026-08-11; реализация M-65 ЗАКРЫТ 2026-08-24).`
ПОЧЕМУ: §4-стык gateway-serve этого аудита; `milestones/M-65-ws-session.md:4`.

**Д-18 (:66, §3.B / :172, §7 п.3).** §7 п.3 «Транспорт бинарных фреймов (msgpack/protobuf) — детали M-22» → `3. ~~Транспорт бинарных фреймов~~ — закрыт фактом: провод JSON (MVP), смена кодека — бамп \`v\` (CT-RFC-09 §3).`
ПОЧЕМУ: A-12; CT-RFC-09 §3.

**Д-19 (§5, порядок строк).** Переставить VB-I-10 перед VB-I-11 (числовой порядок). Содержимое не менять.
ПОЧЕМУ: §2 этого аудита — разрыв случаен.

**Д-20 (§5, опционально — по решению ведущего).** Строка-указатель про дом GW-I/GS-I (§3, «минимальный ход»).

## §9. Что НЕЛЬЗЯ править без критика / founder'а

- **Любая правка этого FA** — уставная зона `gates.md` §9 (`docs/fa/**`): перепроверка независимым Fable обязательна всегда; Д-10/Д-11/Д-12/Д-16 меняют текст ИНВАРИАНТОВ и нормативного блока предусловий — это «изменение формы» ⇒ **критик по §9 обязателен** (не только перепроверка). П-013 и П-017 прямо называют правку FA «отдельным предметом через гейт §9».
- **risk-critic НЕ требуется:** документ не трогает `fa/risk|killswitch|oms`, `RK-I-*`/`INTG-I-*`, анти-оверфит (§9 второй абзац).
- **Граница C — не трогать этим заходом:** фактическое включение полос (`GATEWAY_BANDS` на канонический набор), любое изменение состава записи (L2Delta-раскатка M-45), промоушен виз-форм в T1 (§4-стык contracts) — только founder / contract-RFC. Дифф §8 нигде состава выдачи не меняет.
- **Дом GW-I/GS-I (§3, варианты a/b)** — вариант (b) = новый FA-файл, против «ничего нового» (founder, 24.08, `8c2d972`) — требует его явного слова; вариант (a) — крупная правка формы FA ⇒ критик. Д-20 (строка-указатель) — минимум, проходящий как констатация факта, но тоже через §9.
- **Новых инвариантов НЕ предложено** (мораторий П-017 A3 соблюдён): весь дифф приводит документ К ФАКТУ.

## §10. Незакрытые оси

1. **VPS-`.env`** не проверялся (нет ssh в этом прогоне): живое значение `GATEWAY_BANDS` на сервере — по репо-дефолту предполагается `0.001`; та же оговорка была у A-002 §5.
2. **Тесты не прогонялись локально** — «в CI» означает «файл в `crates/*/tests/` + ci.yml гоняет `cargo test --all`»; зелёность main принята по факту merge PR #95.
3. **`red_volume_profile.rs` прочитан по заголовкам функций** — покрывает ли он дословно «цены без сделок не выдумываются» (VB-I-8), не проверено построчно; назван как проверка ведущему (§2).
4. **`research/exports/format.md`** сверен только по версии и шапке; попольная сверка серий v1 с текущими агрегаторами research-cli не делалась.
5. **`crates/gateway-serve/src/lib.rs`** читан фрагментами (шапка, serde_json-точки, subscribe); полная сверка §3.B FA с сессионной логикой M-65 — не делалась.
6. **`fa/contracts.md`** — §1-§5 + карта секций; §7-§A по существу не читаны (утверждение о нём в §4 — только грепом, команда показана).
7. **`depth-probe-staleness.md`, `depth-lifetime-results.md`** — не открывались (FA на них не ссылается номерами строк).
8. **Протухшие соседи вне предмета** (не мой Writes, назвать ведущему): `reading-map.md:82` «GW-I — 12» (факт 13); `П-014` п.1-2 сами несут протухшие `:1035`/`:938-940` (историческая запись — править нельзя, но читателя собьёт); `П-017` «ни одно не закрыто» протух тем же способом.
