//! Контрактный слой T1 — единый источник правды для форм, пересекающих границы
//! (docs/fa/contracts.md, docs/05-contract-layer.md).
//!
//! Кодировки (locked): деньги/цены/размеры — fixed-point i64 ×1e8 (PRICE_SCALE); время —
//! ts_mono_ns (порядок) + ts_wall_ms (int64 UTC, отчёты) + биржевой ts_exch_ms в payload.
//! Изменения T1 — только через contract-RFC (CT-I-2). schema_version в каждом сегменте (CT-I-6).

use serde::{Deserialize, Serialize};

/// Множитель fixed-point для денег/цен/размеров (×1e8). Никогда не f64 в деньгах (JR-I-7).
pub const PRICE_SCALE: i64 = 100_000_000;

/// Версия схемы журнального формата = **эпоха эмитируемых вариантов** событий. В каждом
/// сегменте (CT-I-6). Bump'ится при КАЖДОМ новом варианте `EventKind`/`MdPayload`, который
/// recorder может писать, — потому что `decide_open_segment` использует её как машинный маркер
/// изоляции сегмента (reuse требует совпадения schema_version), а НЕ provenance: в
/// runtime-контейнере `git rev-parse` недоступен → provenance — КОНСТАНТА на всех деплоях
/// (TD-031), поэтому изоляция по provenance ВОИД. Отделена от `SEGMENT_MAGIC` (framing-эпоха,
/// стабильна `HFTJRN02`): magic = формат фрейминга, schema_version = набор вариантов.
/// 1: CT-RFC-01 — аддитивно OpenInterest/Liquidation/MarginRate + Venue::BinanceFutures.
/// 2: CT-RFC-02 — `SegmentHeader` (первый фрейм сегмента) + provenance/эпохи.
/// 3: CT-RFC-04 rev2 — `MdPayload::L2Delta` (эмитируемый вариант) ⇒ новая эпоха сегмента
///    (сегмент schema-2 не reuse'ится schema-3 бинарём → L2Delta изолирован; TD-031 fix).
pub const SCHEMA_VERSION: u32 = 3;

/// Версия, при которой сегменты ещё писались БЕЗ `SegmentHeader` (боевой сегмент,
/// пишется с 2026-07-10). Читается навсегда (CT-I-3) через вменённый заголовок.
pub const SCHEMA_VERSION_PRE_HEADER: u32 = 1;

/// `epoch_id` legacy-сегмента, ЯВНО задекларированного в манифесте (CT-RFC-02 §3).
pub const LEGACY_EPOCH_ID: &str = "own-legacy-pre-rfc02";

/// Магия в начале КАЖДОГО сегмента schema ≥ 2 (CT-RFC-02 **rev 2**, находка critic C-005 C2).
///
/// Прежнее правило «первый фрейм не разобрался как заголовок → считаем `OwnCapture`» было
/// **FAIL-OPEN**: битый или чужой сегмент тихо получал бы наше происхождение — ровно та
/// приписка эпохи, против которой этот RFC и написан. Классификация теперь однозначна:
/// - магия есть → заголовок ОБЯЗАН разобраться (иначе `Err`, сегмент не читается);
/// - магии нет → сегмент legacy ТОЛЬКО если ЯВНО задекларирован в манифесте и отпечаток
///   совпал (иначе `Err` — «чужой/неизвестный сегмент», не «наш»).
pub const SEGMENT_MAGIC: [u8; 8] = *b"HFTJRN02";

/// Сколько первых байт сегмента покрывает отпечаток legacy-декларации.
pub const LEGACY_FINGERPRINT_BYTES: u64 = 1024 * 1024;

/// Декларация legacy-сегмента (без заголовка) — ЯВНОЕ утверждение оператора: «эти байты
/// имеют такое-то происхождение». Живёт в `journal.legacy.json` рядом с сегментами.
///
/// Fail-closed: незадекларированный сегмент без магии НЕ читается вовсе (`Err`), а не
/// «считается нашим». Отпечаток (sha256 первого MiB) + размер на момент декларации
/// защищают от подмены файла под знакомым именем. Боевой сегмент РАСТЁТ — реализация
/// обязана допускать рост хвоста, но не изменение префикса.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LegacySegmentDecl {
    pub file_name: String,
    /// sha256 первых `LEGACY_FINGERPRINT_BYTES` байт файла (hex).
    pub fingerprint_sha256: String,
    pub size_bytes_at_decl: u64,
    pub source: DataSource,
    pub provenance: String,
    pub epoch_id: String,
}

/// Содержимое `journal.legacy.json` (манифест деклараций).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LegacyManifest {
    pub declarations: Vec<LegacySegmentDecl>,
}

impl LegacyManifest {
    pub fn find(&self, file_name: &str) -> Option<&LegacySegmentDecl> {
        self.declarations.iter().find(|d| d.file_name == file_name)
    }
}

/// Происхождение данных сегмента (CT-RFC-02). Расширяется СТРОГО в конец
/// (сохраняет postcard-дискриминанты).
///
/// Зачем: купленная история и собственный захват — РАЗНЫЕ реальности (чужая глубина книги,
/// чужие часы, чужие гэпы). Смешать их в обучении альфы без пометки = обучать на данных,
/// которых у нас никогда не было. Журнал бессмертен — задним числом источник не проставить.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum DataSource {
    /// Наш собственный live-захват (recorder → venue-адаптеры).
    OwnCapture,
    /// Импорт исторических данных стороннего поставщика.
    Vendor,
    /// Синтетика (тесты/стресс-фикстуры). В обучение по умолчанию НЕ попадает.
    Synthetic,
}

/// Первый фрейм КАЖДОГО сегмента (CT-I-6, CT-RFC-02). Делает эпоху данных ЧИТАЕМЫМ ФАКТОМ,
/// а не устной договорённостью.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SegmentHeader {
    pub schema_version: u32,
    pub source: DataSource,
    /// Чем/кем/когда собран: для `OwnCapture` — версия recorder'а + git sha; для `Vendor` —
    /// поставщик, датасет, дата выгрузки, лицензия.
    pub provenance: String,
    /// Стабильный ключ эпохи, по которому research фильтрует данные
    /// (`own-2026-07`, `tardis-binance-spot-2024`). Смешение эпох — ЯВНОЕ решение.
    pub epoch_id: String,
    /// Часы создания сегмента (отчёты; в детерминизм реплея НЕ входит).
    pub created_wall_ms: i64,
    /// seq первого события сегмента (сшивка при ротации).
    pub first_seq: u64,
}

impl SegmentHeader {
    /// Заголовок legacy-сегмента, построенный ИЗ ЯВНОЙ ДЕКЛАРАЦИИ манифеста (CT-RFC-02 rev 2).
    ///
    /// **Это НЕ вменение по умолчанию** (прежнее fail-open правило убито находкой C-005 C2):
    /// источник/эпоха берутся из того, что оператор явно записал в `journal.legacy.json`,
    /// и применяются лишь после сверки отпечатка. Незадекларированный сегмент без магии —
    /// ошибка чтения, а не «наш захват».
    pub fn from_legacy_decl(
        decl: &LegacySegmentDecl,
        created_wall_ms: i64,
        first_seq: u64,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION_PRE_HEADER,
            source: decl.source,
            provenance: decl.provenance.clone(),
            epoch_id: decl.epoch_id.clone(),
            created_wall_ms,
            first_seq,
        }
    }
}

/// Единица упорядоченного журнала (docs/fa/journal.md §5). `seq` — тотальный порядок,
/// назначается журналом (единственный писатель, JR-I-1). Коннекторы seq НЕ проставляют.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Event {
    pub seq: u64,
    pub ts_mono_ns: u64,
    pub ts_wall_ms: i64,
    pub kind: EventKind,
}

/// Закрытый версионируемый enum видов событий. Новые варианты — только аддитивно (в конец)
/// через contract-RFC (CT-I §6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum EventKind {
    /// Системное: жив/связь.
    Sys(SysEvent),
    /// Рыночные данные (нормализованные из venue-адаптеров).
    Md(MdEvent),
    // Ord(..), Risk(..), Recon(..), Ctl(..) — добавляются в P3 via contract-RFC.
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum SysEvent {
    Heartbeat,
    ConnUp(Venue),
    ConnDown(Venue),
    /// Аудит сверки локальной книги с независимым REST-снапшотом биржи (OPS-I-1, CT-RFC-03).
    /// Пишется, когда recon обнаружил расхождение выше порога — durable-след «каким участкам
    /// данных верить». Метрики (`book_divergence_bps`) в журнал НЕ пишутся (OPS-I-6): расхождение
    /// — доменный ФАКТ о данных, а не наблюдательная метрика. **Строго в конце enum** (postcard-
    /// дискриминант 3; Heartbeat/ConnUp/ConnDown = 0/1/2 неизменны, CT-I §6).
    ReconDivergence(ReconAudit),
}

/// Полезная нагрузка recon-аудита (CT-RFC-03). Поля ФИКСИРОВАНЫ: добавление поля в struct ломает
/// postcard-формат старых записей ⇒ расширение — только новым contract-RFC. Набор выбран
/// минимальным и достаточным, чтобы офлайн-тулинг (research-dev, `research/data-quality/`) мог
/// ответить «каким сегментам данных верить»: venue+symbol+момент (из `Event.seq/ts`)+магнитуда+
/// была ли порча лучшей цены+что сделали.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReconAudit {
    pub venue: Venue,
    /// Канонический тикер площадки (как в `MdEvent.symbol`).
    pub symbol: String,
    /// Максимальное расхождение сумм ценовых полос локальной книги vs REST-снапшота, bps (×1).
    /// Магнитуда (≥ 0 по смыслу; тип `i64` — единая fixed-point-конвенция контрактов).
    pub divergence_bps: i64,
    /// Расхождение по ЛУЧШЕЙ цене (bid/ask). `true` — это порча (`ε_test`), а не шум полос:
    /// эвикция C1-класса стирала именно best bid. Отдельный флаг, потому что это качественно
    /// иной класс, чем расхождение дальних сумм.
    pub best_price_diverged: bool,
    /// Что recon сделал по факту расхождения.
    pub action: ReconAction,
}

/// Действие recon по факту расхождения (CT-RFC-03). Порядок вариантов ФИКСИРОВАН (postcard).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum ReconAction {
    /// Расхождение зафиксировано, принудительный ресинк НЕ выполнялся (ниже resync-порога,
    /// либо ресинк уже идёт по venue-логике).
    AlertOnly,
    /// Выполнен принудительный ресинк книги из REST-снапшота (`book_resync_total`++).
    Resynced,
}

/// Площадка. Расширяется аддитивно (СТРОГО в конец — CT-I §6, сохраняет postcard-индексы).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
pub enum Venue {
    /// Binance СПОТ-рынок.
    Binance,
    /// Hyperliquid ПЕРП (основной рынок HL: l2Book/trades перпа).
    Hyperliquid,
    /// Binance USDT-M ПЕРП-фьючерсы (fstream). Добавлено CT-RFC-01.
    BinanceFutures,
}

/// Сторона.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum Side {
    Buy,
    Sell,
}

/// Уровень стакана. price/size — fixed-point ×1e8.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Level {
    pub price: i64,
    pub size: i64,
}

/// Нормализованное рыночное событие. `symbol` — канонический тикер площадки как есть
/// (Binance "BTCUSDT" / Hyperliquid "BTC"); нормализация кросс-venue — задача выше (book/strategy).
/// Для MarginRate `symbol` — актив ("USDT"/"USDC"); для OpenInterest/Liquidation — инструмент.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MdEvent {
    pub venue: Venue,
    pub symbol: String,
    pub payload: MdPayload,
}

/// Тип рыночного апдейта. price/size — fixed-point ×1e8; ставки — ×1e8.
/// L2Snapshot: и Binance, и HL шлют СНАПШОТ стакана целиком на апдейте — пишем как снапшот.
/// Новые варианты — только аддитивно В КОНЕЦ (CT-I §6, сохраняет postcard-дискриминанты).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum MdPayload {
    Trade {
        price: i64,
        size: i64,
        side: Side,
        ts_exch_ms: i64,
    },
    L2Snapshot {
        bids: Vec<Level>,
        asks: Vec<Level>,
        ts_exch_ms: i64,
    },
    Funding {
        rate_e8: i64,
        ts_exch_ms: i64,
    },
    /// Открытый интерес перп-контракта. `oi_e8` — в БАЗОВОМ активе ×1e8 (нотионал = oi×mark,
    /// derive downstream). Добавлено CT-RFC-01.
    OpenInterest {
        oi_e8: i64,
        ts_exch_ms: i64,
    },
    /// Форс-ликвидация (forced order). `side` — ЛИКВИДИРУЕМАЯ сторона (НЕ сторона агрессора;
    /// M-06 парсер обязан сохранить смысл — C-003 note). Добавлено CT-RFC-01.
    Liquidation {
        price: i64,
        size: i64,
        side: Side,
        ts_exch_ms: i64,
    },
    /// Прокси спроса на займы: margin interest rate ×1e8 (интервал ставки — в provenance
    /// артефакта/парсера). `symbol` = актив ("USDT"/"USDC"). Добавлено CT-RFC-01 (Tier-3 impl).
    MarginRate {
        rate_e8: i64,
        ts_exch_ms: i64,
    },
    /// Инкрементальная book-дельта — СЫРОЙ `@depth` diff-апдейт, персистится ДО применения к
    /// книге (CT-RFC-04). Позволяет ТОЧНО реконструировать эволюцию стакана
    /// (absorption/iceberg/DOM — book-flow «как Fabio»), что бакетированный `L2Snapshot`
    /// теряет безвозвратно. Аддитивно В КОНЕЦ (postcard-дискриминант 6; старые сегменты
    /// содержат только 0..5 → читаются байт-в-байт, CT-I-3).
    ///
    /// **Семантика уровней (та же дисциплина, что `venue-binance::apply_diff_to_book` §A и
    /// `.claude/rules/testing.md` «отсутствие»):** `bids`/`asks` — ТОЛЬКО ИЗМЕНИВШИЕСЯ уровни.
    /// `size > 0` = установить объём на цене; `size == 0` = УДАЛИТЬ уровень (явный remove от
    /// биржи). Уровень, которого в дельте НЕТ, — НЕ изменился: дельта НЕ является источником
    /// истины о неупомянутом, реконструкция его НЕ трогает. Пустая сторона (`[]`) = «на этой
    /// стороне ничего не менялось», а НЕ «очистить сторону».
    ///
    /// **Sequencing (детерминированная реконструкция + gap-детекция БЕЗ обращения к бирже):**
    /// `first_update_id` = Binance `U`, `final_update_id` = `u` — границы update-id этой дельты.
    /// `prev_final_update_id` = futures `pu` (чейнится: `pu == предыдущий final_update_id`);
    /// для СПОТ-потока `None` (там непрерывность выражается как `U == prev.u + 1`). Реплей
    /// журнала по этим полям восстанавливает точный порядок и видит любой пропуск дельт.
    L2Delta {
        bids: Vec<Level>,
        asks: Vec<Level>,
        first_update_id: u64,
        final_update_id: u64,
        prev_final_update_id: Option<u64>,
        ts_exch_ms: i64,
    },
}

impl EventKind {
    /// Хелпер: собрать рыночное событие.
    pub fn md(venue: Venue, symbol: impl Into<String>, payload: MdPayload) -> Self {
        EventKind::Md(MdEvent {
            venue,
            symbol: symbol.into(),
            payload,
        })
    }
}

/// Перевод float-цены в fixed-point ×1e8 (для парсеров venue).
pub fn to_fixed(x: f64) -> i64 {
    (x * PRICE_SCALE as f64).round() as i64
}

/// Обратно в float (для отчётов/логов).
pub fn from_fixed(x: i64) -> f64 {
    x as f64 / PRICE_SCALE as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_roundtrips_through_json() {
        let e = Event {
            seq: 1,
            ts_mono_ns: 42,
            ts_wall_ms: 1_700_000_000_000,
            kind: EventKind::md(
                Venue::Hyperliquid,
                "BTC",
                MdPayload::Trade {
                    price: to_fixed(65000.5),
                    size: to_fixed(0.1),
                    side: Side::Buy,
                    ts_exch_ms: 1_700_000_000_123,
                },
            ),
        };
        let s = serde_json::to_string(&e).unwrap();
        let back: Event = serde_json::from_str(&s).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn fixed_point_roundtrip() {
        assert_eq!(PRICE_SCALE, 100_000_000);
        assert_eq!(to_fixed(1.0), 100_000_000);
        assert!((from_fixed(to_fixed(65000.5)) - 65000.5).abs() < 1e-6);
    }
}
