//! RED `M-84` (sacred, architect-only) — **ВХОД ЧЕКПОИНТЕРА: без `--bands` бинарь берёт
//! ПРОДУКТОВЫЙ набор; явный флаг остаётся переопределением переходного периода и объявляется.**
//!
//! Заведён решением арбитра `A-033` D-2/D-3. RUNTIME-RED: файл собирается, падает поведением.
//! Сегодня дефолт бинаря — `vec![0.001]` (`crates/gateway/src/bin/gateway-checkpoint.rs:188`,
//! справка `:239`), то есть сетку задаёт оператор, а не сервер.
//!
//! ## ПОЧЕМУ ЗАМЕР СНИМАЕТСЯ НА БИНАРЕ, А НЕ НА БИБЛИОТЕКЕ
//!
//! `A-033` D-2 убрал гвард СОСТАВА из `gateway::validate_selector` и
//! `checkpoint::advance_to`: библиотекой пользуются все, и гвард там отверг собственную
//! боевую настройку прода (`docker-compose.yml:217` — `--bands=${GATEWAY_BANDS:-0.001}`),
//! уронив 209 тестов из 329. Состав — дело ПРОДУКТОВЫХ входов, а вход чекпоинтера — это его
//! argv. Значит и судить надо argv, прод-формой вызова (`testing.md` §«Механизм несущего
//! пути обязан иметь оракул точки входа»: `test -f binary` и grep по имени — описания
//! намерений, а не проверки).
//!
//! ## КАК УСТРОЕН ЗАМЕР — БЕЗ ЗНАНИЯ ЧУЖИХ ДЕФОЛТОВ
//!
//! Имя слепка детерминировано отпечатком селектора (`ckpt-<fp_hex16>.bin`), а в отпечаток
//! входят ВСЕ оси. Чтобы не гадать, какие значения бинарь подставляет для venue/symbol/
//! timeframe/window, сравниваются ДВА ЕГО СОБСТВЕННЫХ прогона:
//!   - без `--bands`;
//!   - с `--bands`, равным продуктовому набору ЯВНО.
//! Если бинарь по умолчанию берёт продуктовый набор, имена СОВПАДУТ. Сегодня не совпадают.
//! Прочие оси при этом held constant — варьируется ровно измеряемая (`testing.md`,
//! «Целостность гейта», свойство 2).
//!
//! ## `testing.md` чек-лист
//! - **setup-guard**: третий прогон с легаси-набором обязан дать ИНОЕ имя — иначе сравнение
//!   имён ничего не значит (имя могло бы не зависеть от полос вовсе), и файл был бы плацебо;
//! - п.3 **отсутствие** — флага НЕТ: это и есть новая норма;
//! - парный vantage: переопределение обязано работать (иначе merge меняет набор прода молча)
//!   И быть названным в выводе (иначе переходный режим невидим оператору).

use contracts::{to_fixed, DataSource, EventKind, MdPayload, Side, Venue};
use journal::{Journal, WriterConfig};
use std::path::Path;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_gateway-checkpoint");

/// Продуктовый набор ЛИТЕРАЛОМ (`A-033`: ссылка на несуществующий символ сделала бы файл
/// несобираемым и заглушила бы корпус крейта — так и не заметили 209 красных).
const PRODUCT_BANDS: [f64; 7] = [0.015, 0.03, 0.05, 0.08, 0.15, 0.30, 0.60];
const PRODUCT_BANDS_ARG: &str = "0.015,0.03,0.05,0.08,0.15,0.3,0.6";
/// Сегодняшняя боевая настройка (`docker-compose.yml:217`). После M-84 — переопределение.
const LEGACY_ARG: &str = "0.001";

const T: i64 = 1_700_000_000_000;

fn journal_of(n: u64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = WriterConfig {
        max_segment_bytes: 1 << 20,
        min_free_bytes: 0,
        source: DataSource::OwnCapture,
        provenance: "test".to_string(),
        epoch_id: "own-test".to_string(),
    };
    {
        let mut j = Journal::open_with(dir.path(), cfg).expect("open_with");
        for i in 0..n {
            j.append(EventKind::md(
                Venue::Binance,
                "BTCUSDT",
                MdPayload::Trade {
                    price: to_fixed(65_000.0 + i as f64),
                    size: to_fixed(1.0),
                    side: if i % 2 == 0 { Side::Buy } else { Side::Sell },
                    ts_exch_ms: T + i as i64,
                },
            ))
            .expect("append");
        }
        j.flush().expect("flush");
    }
    dir
}

struct Run {
    code: i32,
    output: String,
    ckpt: String,
}

/// Прогон бинаря в ПРОД-ФОРМЕ argv (`--flag=value`, как в `docker-compose.yml`), с
/// перенацеленными путями. `bands` = `None` — флаг НЕ передаётся вовсе.
fn run(bands: Option<&str>) -> Run {
    let jdir = journal_of(200);
    let ckpt = tempfile::tempdir().expect("ckpt");
    let cov = tempfile::tempdir().expect("cov");
    let mut cmd = Command::new(BIN);
    cmd.arg(format!("--dir={}", jdir.path().display()))
        .arg(format!("--ckpt-dir={}", ckpt.path().display()))
        .arg(format!(
            "--coverage-out={}",
            cov.path().join("covered_through_seq").display()
        ))
        .arg("--symbol=BTCUSDT")
        .arg("--timeframe-ms=1000")
        .arg("--window-ms=60000")
        .arg("--cursor=LATEST");
    if let Some(b) = bands {
        cmd.arg(format!("--bands={b}"));
    }
    let out = cmd.output().expect("запуск gateway-checkpoint");
    let mut names: Vec<String> = std::fs::read_dir(ckpt.path())
        .expect("read_dir ckpt")
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with("ckpt-") && n.ends_with(".bin"))
        .collect();
    names.sort();
    Run {
        code: out.status.code().unwrap_or(-1),
        output: format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
        ckpt: names.join(","),
    }
}

fn ok(r: &Run, what: &str) {
    assert_eq!(
        r.code, 0,
        "SETUP НЕ СОСТОЯЛСЯ: прогон «{what}» обязан завершаться кодом 0, получено {}. \
         Вывод: {}",
        r.code, r.output
    );
    assert!(
        !r.ckpt.is_empty(),
        "SETUP НЕ СОСТОЯЛСЯ: прогон «{what}» не оставил ни одного слепка — сравнивать нечего"
    );
}

#[test]
fn default_argv_uses_the_product_set() {
    // ЯДРО. Два прогона ОДНОГО бинаря: без флага и с продуктовым набором явно. Совпадение
    // имён = «дефолт бинаря и есть продуктовый набор». Сегодня дефолт — одна полоса.
    let bare = run(None);
    ok(&bare, "без --bands");
    let explicit = run(Some(PRODUCT_BANDS_ARG));
    ok(&explicit, "--bands=<продуктовый>");
    assert_eq!(
        bare.ckpt, explicit.ckpt,
        "без --bands чекпоинтер обязан считать ПРОДУКТОВЫЙ набор из семи полос: имя слепка \
         детерминировано отпечатком, и совпадение имён — единственное наблюдаемое \
         доказательство. Получено {} против {}. Пока дефолт бинаря — `vec![0.001]` \
         (gateway-checkpoint.rs:188), прогретый слепок будет не тем, под которым сервер \
         потом придёт за данными",
        bare.ckpt, explicit.ckpt
    );
}

#[test]
fn setup_guard_name_really_depends_on_bands() {
    // Без этого стража предыдущий тест — плацебо: если бы имя от полос не зависело, оно
    // совпадало бы ВСЕГДА, и «дефолт продуктовый» доказывалось бы ничем.
    let legacy = run(Some(LEGACY_ARG));
    ok(&legacy, "--bands=0.001");
    let explicit = run(Some(PRODUCT_BANDS_ARG));
    ok(&explicit, "--bands=<продуктовый>");
    assert_ne!(
        legacy.ckpt, explicit.ckpt,
        "имя слепка ОБЯЗАНО зависеть от набора полос — на этом стоит и нулевое окно выкатки, \
         и весь смысл сравнения выше. Совпадение здесь означает, что слепки разных наборов \
         затирают друг друга"
    );
}

#[test]
fn explicit_override_still_works() {
    // ПАРНЫЙ vantage: переопределение обязано РАБОТАТЬ весь переходный период. Отвергать
    // легаси-набор здесь значит уронить прод в момент merge'а: docker-compose.yml:217
    // подаёт `--bands=0.001`, и падение чекпоинтера останавливает обновление слепка.
    let legacy = run(Some(LEGACY_ARG));
    assert_eq!(
        legacy.code, 0,
        "явный легаси-набор обязан приниматься на переходный период: его подаёт боевой \
         compose. Получено code={}, вывод: {}",
        legacy.code, legacy.output
    );
}

#[test]
fn explicit_override_is_announced_as_deprecated() {
    // Молчаливый переходный режим неотличим от дефекта. Ищутся ТОКЕНЫ и ТОЧНОЕ значение, а
    // не голое число (TD-166: цифры совпадают с куском таймстампа, красный исход становится
    // вероятностным).
    let legacy = run(Some(LEGACY_ARG));
    ok(&legacy, "--bands=0.001");
    assert!(
        legacy.output.contains("--bands"),
        "переопределение обязано НАЗВАТЬ флаг — оператору нужно знать, что убирать при \
         завершении переходного периода. Вывод: {}",
        legacy.output
    );
    assert!(
        legacy.output.contains("deprecated"),
        "флаг обязан быть помечен deprecated: у режима есть КОНЕЦ (задача 7 M-70), и он \
         объявляется, а не подразумевается. Вывод: {}",
        legacy.output
    );
    assert!(
        legacy.output.contains(LEGACY_ARG),
        "предупреждение обязано назвать ПРИМЕНЁННОЕ значение, иначе оператор не проверит, \
         что именно работает. Вывод: {}",
        legacy.output
    );
}

#[test]
fn product_set_run_is_silent_about_override() {
    // Обратная сторона: предупреждать в штатном режиме — приучать игнорировать
    // предупреждения. Прогон БЕЗ флага молчит о переопределении.
    let bare = run(None);
    ok(&bare, "без --bands");
    assert!(
        !bare.output.contains("deprecated"),
        "без переопределения вывод обязан молчать о нём: шум в штатном режиме обесценивает \
         предупреждение переходного. Вывод: {}",
        bare.output
    );
    // Заодно фиксируем, что продуктовый набор действительно семичленный — свойство судится
    // там, где набор наблюдаем (гвард состава из библиотеки убран, `A-033` D-2).
    assert_eq!(PRODUCT_BANDS.len(), 7, "продуктовый набор — семь полос");
}
