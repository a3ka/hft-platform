//! RED M-88 (sacred, architect-only) — **цена находки `R-195` Б-1, выраженная ОТКАЗОМ ВЫДАЧИ.**
//!
//! Отдельный бинарь, и это не стиль: сценарий двигает ПРОЦЕССНЫЙ предел объёма ответа
//! (`set_effective_max_response_bytes`), а он глобален для всего тест-бинаря. Один
//! сценарий на файл — единственная форма, в которой он не портит соседей и не зависит
//! от порядка запуска (`testing.md`, целостность гейта: гейт меряет СВОЙ инвариант,
//! а не окружение).
//!
//! **Что доказывается.** Прочие оракулы M-88 судят ФОРМУ списка наблюдений. Этот — цену:
//! список, растущий с историей, не просто «дорог», он ОТНИМАЕТ БЮДЖЕТ У ПОЛЕЗНОЙ
//! НАГРУЗКИ и роняет `snapshot` целиком. Ревьюер предъявил это на проде-подобной глубине:
//! 220 000 бакетов → `Err`, `limit=2000000 observed=2423152`, и все содержательные
//! счётчики в сообщении НУЛЕВЫЕ — ответа нет вовсе, кокпит молчит.
//!
//! Воспроизводить 220 000 бакетов в оракуле незачем: величина `наблюдений × ≈11 Б`
//! линейна по истории, поэтому та же цена предъявляется дешевле — историей в 20 000
//! бакетов при СОРАЗМЕРНО уменьшенном пределе. Пропорция сохранена, стоимость прогона —
//! секунды вместо минут. Предел двигается ЯВНО и восстанавливается стражем на любом
//! выходе, включая панику.

use contracts::{to_fixed, DataSource, EventKind, Level, MdPayload, Side, Venue};
use gateway::{Cursor, Selector};
use journal::{EpochFilter, Journal, WriterConfig};

const BASE_TS: i64 = 1_752_000_000_000;
const WINDOW_MS: i64 = 60_000;
/// История 20 000 бакетов ≈ 5.5 часов. При ≈11 Б на объявленную колонку нерезанный
/// список весит ≈220 КБ — величина, замеренная `R-195` на 20 000 (и та же, из-за которой
/// `red_gateway_bounded` со своим порогом 8 МиБ находку ПРОПУСТИЛ).
const HISTORY_BUCKETS: i64 = 20_000;
/// Предел СОРАЗМЕРЕН: ≈220 КБ нерезанного списка обязаны его пробить, а оконная выдача
/// (≈60 бакетов карты + depth + ohlcv) обязана в него уложиться с запасом.
const TIGHT_LIMIT: usize = 150_000;

/// Восстановление процессного предела на ЛЮБОМ выходе. Без стража упавший сценарий
/// оставил бы чужое значение следующему тесту — класс «гейт меряет окружение».
struct LimitGuard(usize);
impl Drop for LimitGuard {
    fn drop(&mut self) {
        gateway::set_effective_max_response_bytes(self.0);
    }
}

fn cfg() -> WriterConfig {
    WriterConfig {
        max_segment_bytes: 8 * 1024 * 1024,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    }
}

fn book(jitter: i64, ts: i64) -> EventKind {
    let mk = |base: i64| -> Vec<Level> {
        (0..4)
            .map(|k| Level {
                price: base + k * 100,
                size: 1_000 + k + jitter,
            })
            .collect()
    };
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::L2Snapshot {
            bids: mk(6_400_000_000_000),
            asks: mk(6_400_100_000_000),
            ts_exch_ms: ts,
        },
    )
}

fn trade(ts: i64, i: i64) -> EventKind {
    EventKind::md(
        Venue::Binance,
        "BTCUSDT",
        MdPayload::Trade {
            price: to_fixed(64_000.0),
            size: to_fixed(1.0),
            side: [Side::Buy, Side::Sell][(i % 2) as usize],
            ts_exch_ms: ts,
        },
    )
}

fn sel() -> Selector {
    Selector {
        venue: Venue::Binance,
        symbol: "BTCUSDT".to_string(),
        timeframe_ms: 1_000,
        bands: vec![0.001],
        window_ms: Some(WINDOW_MS),
        depth_cadence_ms: None,
    }
}

#[test]
fn observed_list_does_not_eat_the_response_budget() {
    let restore = LimitGuard(gateway::effective_max_response_bytes());

    let dir = tempfile::tempdir().expect("tempdir");
    {
        let mut j = Journal::open_with(dir.path(), cfg()).expect("open_with");
        for b in 0..HISTORY_BUCKETS {
            let ts = BASE_TS + b * 1_000;
            j.append(book(b, ts)).expect("append book");
            j.append(trade(ts, b)).expect("append trade");
        }
        j.flush().expect("flush");
    }

    // SETUP-СТРАЖ (1): без предела снимок обязан СТРОИТЬСЯ. Иначе отказ ниже нельзя было бы
    // отнести к объёму — он мог бы быть чем угодно, и сценарий судил бы не то.
    gateway::set_effective_max_response_bytes(usize::MAX);
    let unbounded = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::LATEST,
    )
    .expect("setup-страж: снимок не строится даже без предела — отказ ниже был бы не про объём");
    assert!(
        (unbounded.series.ohlcv.len() as i64) < HISTORY_BUCKETS,
        "setup-страж: окно не обрезало историю — сценарий вырожден"
    );
    // SETUP-СТРАЖ (2): полезная нагрузка в окне обязана быть НЕПУСТОЙ, иначе «уложились в
    // предел» означало бы «отдали пустоту», и зелёный ничего не стоил бы.
    assert!(
        !unbounded.series.heatmap.is_empty()
            && !unbounded.series.heatmap_observed_time_s.is_empty(),
        "setup-страж: карта или список наблюдений пусты — фикстура не подала L2 на путь карты"
    );

    gateway::set_effective_max_response_bytes(TIGHT_LIMIT);
    let res = gateway::snapshot(
        dir.path(),
        EpochFilter::OwnCaptureOnly,
        &sel(),
        Cursor::LATEST,
    );

    match res {
        Ok(s) => {
            // Положительная сторона: уложились — и уложились СОДЕРЖАТЕЛЬНО, а не пустотой.
            assert!(
                !s.series.heatmap.is_empty(),
                "снимок уложился в предел, отдав ПУСТУЮ карту — это не победа, а тот же отказ \
                 в другой одежде"
            );
            assert!(
                s.series.heatmap_observed_time_s.len() <= (WINDOW_MS / 1_000) as usize + 2,
                "список наблюдений ({}) шире окна — бюджет ответа всё ещё уходит в него",
                s.series.heatmap_observed_time_s.len()
            );
        }
        Err(e) => panic!(
            "R-195 Б-1 (PL-I-5): снимок НЕ ОТДАН на истории в {HISTORY_BUCKETS} бакетов при \
             пределе {TIGHT_LIMIT} Б — «{e}». Бюджет ответа съеден списком наблюдений, который \
             растёт с ИСТОРИЕЙ, а не с окном. На проде история — два месяца, и это означает \
             не «карту с призраками», а ОТСУТСТВИЕ снимка: кокпит молчит. Счётчики в сообщении \
             читать обязательно — они называют ПОЛЕЗНУЮ нагрузку: всё, чего в них нет, и есть \
             съеденный списком бюджет. На прод-глубине (замер R-195, 220 000 бакетов) они \
             ОБНУЛЯЮТСЯ вовсе: ответа нет совсем"
        ),
    }

    drop(restore);
}
