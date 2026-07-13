//! Сегменты, эпохи, стрим-чтение, ретеншен (M-08 / CT-RFC-02).
//!
//! Каркас (типы + сигнатуры + `todo!()`) — architect (M-08 task 1).
//! Реализация — engine-dev (задачи 2/3). Инварианты — RED в `tests/` (sacred).
//!
//! Три вещи, которых сегодня нет и из-за которых сбор данных конечен:
//!  1. **Ротация** — имя сегмента захардкожено (`segment-00000000.jrnl`), файл растёт вечно;
//!     при 2.8 GB/сут (замер VPS 2026-07-13) диск (120 GB свободно) кончится за ~43 дня.
//!  2. **Bounded-memory чтение** — `read_all()` грузит ВЕСЬ журнал в `Vec<Event>`; на 8.3 GB
//!     это не запускается (класс TD-011). Альфы на прод-объёме построить нельзя.
//!  3. **Provenance** — источник данных нигде не записан; докупленная история станет
//!     неотличима от собственного захвата (CT-RFC-02).

use std::io;
use std::path::{Path, PathBuf};

use contracts::{DataSource, Event, SegmentHeader};

/// Порог ротации по размеру (E2). Сегмент закрывается, когда превысил его.
pub const DEFAULT_MAX_SEGMENT_BYTES: u64 = 1024 * 1024 * 1024; // 1 GiB

/// Порог свободного места (E4, fail-closed): ниже — запись ОСТАНАВЛИВАЕТСЯ явно.
pub const DEFAULT_MIN_FREE_BYTES: u64 = 10 * 1024 * 1024 * 1024; // 10 GiB

/// Конфиг писателя (E2/E4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriterConfig {
    pub max_segment_bytes: u64,
    /// Свободного места меньше → `append` возвращает `Err(StorageGuard)`, а не «пишет,
    /// пока диск не кончится». Тихо переполнить диск = тот же остановленный сбор,
    /// только без предупреждения.
    pub min_free_bytes: u64,
    pub source: DataSource,
    pub provenance: String,
    pub epoch_id: String,
}

impl WriterConfig {
    /// Дефолт для recorder'а: собственный захват.
    pub fn own_capture(provenance: impl Into<String>, epoch_id: impl Into<String>) -> Self {
        Self {
            max_segment_bytes: DEFAULT_MAX_SEGMENT_BYTES,
            min_free_bytes: DEFAULT_MIN_FREE_BYTES,
            source: DataSource::OwnCapture,
            provenance: provenance.into(),
            epoch_id: epoch_id.into(),
        }
    }
}

/// Сегмент на диске + его заголовок (у legacy-сегмента — вменённый, CT-RFC-02 §3).
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentInfo {
    pub path: PathBuf,
    pub index: u32,
    pub header: SegmentHeader,
    pub size_bytes: u64,
}

/// Какие эпохи читатель СОГЛАСЕН смешивать (E6, CT-RFC02-3/4).
///
/// Дефолта «всё подряд» нет: смешение купленной истории с собственным захватом —
/// решение, которое принимается ЯВНО, иначе альфа обучается на данных, которых у нас
/// не было. Тот же класс ошибки, что TD-015, но дороже (там метрики, здесь — обучение).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EpochFilter {
    /// Только собственный захват (дефолт research-пути).
    OwnCaptureOnly,
    /// Явно перечисленные эпохи (`epoch_id`) — осознанное смешение.
    Explicit(Vec<String>),
    /// Всё, включая `Vendor`/`Synthetic`. Только для диагностики/дампов, не для обучения.
    All,
}

impl EpochFilter {
    /// Проходит ли сегмент фильтр.
    pub fn accepts(&self, _header: &SegmentHeader) -> bool {
        todo!("M-08 task 2 (engine-dev): OwnCaptureOnly | Explicit(epoch_id) | All")
    }
}

/// Перечислить сегменты каталога по возрастанию индекса; у сегмента без заголовка —
/// вменённый legacy-заголовок (CT-RFC02-1). Не читает события (O(1) на сегмент).
pub fn segments(_dir: impl AsRef<Path>) -> io::Result<Vec<SegmentInfo>> {
    todo!("M-08 task 2 (engine-dev): скан каталога, парс заголовка/вменение legacy")
}

/// Bounded-memory поток событий журнала (E5). Память НЕ зависит от размера журнала:
/// сегменты читаются по одному, фрейм за фреймом.
///
/// Потребитель обязан назвать `EpochFilter` — эпоху НЕЛЬЗЯ не заметить (CT-RFC02-2:
/// типовой барьер, а не дисциплина).
pub struct EventStream {
    _private: (),
}

impl EventStream {
    /// Заголовки сегментов, попавших в выборку — эпоха читаемо присутствует в отчёте.
    pub fn headers(&self) -> &[SegmentHeader] {
        todo!("M-08 task 2 (engine-dev)")
    }
}

impl Iterator for EventStream {
    type Item = io::Result<Event>;
    fn next(&mut self) -> Option<Self::Item> {
        todo!("M-08 task 2 (engine-dev): фрейм-за-фреймом, переход через границу сегментов")
    }
}

/// Открыть поток чтения (E5/E6). Единственный прод-путь чтения журнала:
/// `read_all()` остаётся ТОЛЬКО для тестов/малых фикстур.
pub fn stream(_dir: impl AsRef<Path>, _filter: EpochFilter) -> io::Result<EventStream> {
    todo!("M-08 task 2 (engine-dev): открыть сегменты по фильтру, вернуть итератор")
}

/// Доказательство того, что сегмент ВЫГРУЖЕН в холодное хранилище и копия сверена
/// по контрольной сумме (E3).
///
/// Конструктор ПРИВАТНЫЙ: единственный способ получить `ColdCopyProof` — реально
/// выгрузить и сверить (`verify_cold_copy`). Поэтому «удалить невыгруженный сегмент»
/// невозможно ВЫРАЗИТЬ в этом API — это типовой барьер, а не дисциплина оператора
/// (тот же приём, что `RiskApproved<Order>` в риск-слое).
#[derive(Debug)]
pub struct ColdCopyProof {
    _private: (),
}

/// Выгрузить сегмент в холодное хранилище и сверить контрольную сумму.
/// Только успешная сверка выдаёт `ColdCopyProof`.
pub fn verify_cold_copy(_seg: &SegmentInfo, _cold_root: &Path) -> io::Result<ColdCopyProof> {
    todo!("M-08 task 3 (engine-dev): копия + sha256-сверка; расхождение → Err, НЕ proof")
}

/// Удалить горячую копию сегмента. Требует `ColdCopyProof` — данные не могут исчезнуть
/// «по политике ретеншена», не оказавшись сперва в холодном хранилище.
pub fn prune_segment(_seg: &SegmentInfo, _proof: ColdCopyProof) -> io::Result<()> {
    todo!("M-08 task 3 (engine-dev): удалить ТОЛЬКО горячую копию, после proof")
}

/// Свободное место на файловой системе каталога (E4).
pub fn free_bytes(_dir: impl AsRef<Path>) -> io::Result<u64> {
    todo!("M-08 task 3 (engine-dev): statvfs/statfs")
}
