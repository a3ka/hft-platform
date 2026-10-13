//! RED DV-I-10..14 — per-LIFE метрика жизненного цикла уровня (sacred, architect-only) — M-58.
//!
//! ПОЧЕМУ ЭТОТ ФАЙЛ СУЩЕСТВУЕТ. Метрика M-32 считала судьбу НА ЦЕНУ и фиксировала её на первом
//! `size=0` (`depth_lifetime.rs:171` — `new_birth = !states.contains_key(&price)`, `states` не
//! чистится). Поэтому она измеряла «доля distinct-ЦЕН, получивших хотя бы один `size=0` за окно»,
//! а не долю жизней, закончившихся отменой. Две беды, обе подтверждены замером
//! (`research/arbitration/A-002-depth-metric-tpp.md` §0):
//!   1. смещение В СТОРОНУ вывода: уровень «отменён однажды, дальше стоит вечно» — сигнатура
//!      фантома TD-016 — засчитывался как `cancelled`;
//!   2. насыщение: величина растёт с длиной окна (проба арбитра: 0.5 → 1.0 при ОДИНАКОВОЙ
//!      живости конца окна), поэтому сравнение полос сравнивало насыщение, а не физику.
//!
//! КОНТРАКТ M-58 (impl — research-dev, `crates/research-cli/src/depth_lifetime.rs`):
//!   единица учёта — **ЖИЗНЬ** уровня, не цена.
//!   - жизнь НАЧИНАЕТСЯ: цена не жива и приходит `size>0` → `lives_born += 1`; полоса жизни
//!     определяется по mid ТОГО тика, в котором эта жизнь родилась (не первой жизни цены);
//!   - жизнь КОНЧАЕТСЯ ровно одним из трёх: `size=0` при живой → `lives_cancelled`;
//!     sequence-gap при живой → `lives_censored`; конец окна при живой → `lives_frozen`;
//!   - перерождение той же цены — НОВАЯ жизнь; предыдущая судьба на неё не переносится;
//!   - `cancel_fraction = lives_cancelled / (lives_cancelled + lives_frozen)`; `censored`
//!     исключены из знаменателя (как в M-32); знаменатель 0 → `None`;
//!   - БАЛАНС (машинно проверяемый инвариант):
//!     `lives_born == lives_cancelled + lives_frozen + lives_censored` в КАЖДОЙ полосе;
//!   - вывод детерминирован (BTreeMap-порядок; `bands` по `(side, lo_bps)`).
//!
//! Поля `born/cancelled/frozen/censored` ПЕРЕИМЕНОВАНЫ в `lives_*` намеренно: старое имя
//! означало другую величину, и молчаливая смена смысла под тем же именем — ровно тот класс
//! ошибки, из-за которого понадобился M-58. Переименование заставляет пересмотреть каждый
//! call-site.
//!
//! АНТИ-ПЛАЦЕБО. Против ДЕЙСТВУЮЩЕЙ реализации обязаны падать все пять:
//!   DV-I-10 → даст `born=1, cancelled=1, frozen=0` (ждём 2/1/1);
//!   DV-I-11 → даст `1.0` в обеих конфигурациях (ждём 0.5 и 0.75);
//!   DV-I-12 → цензура наследуется, второй жизни нет (ждём born=2);
//!   DV-I-13 → баланс не сходится (жизни не считаются);
//!   DV-I-14 → полоса зафиксирована по первой жизни (ждём разные полосы).
//! Против заглушки «всё cancelled» падают DV-I-10/11/12; «всё frozen» — DV-I-10/11/13.

use contracts::{Level, Side};
use research_cli::depth_lifetime::{analyze, DeltaTick};

const UNIT: i64 = 100_000_000;
const MID: i64 = 64_000 * UNIT;

/// Уровень на `pct` от `mid_ref`: bid = mid·(1−pct), ask = mid·(1+pct). `size_units`·UNIT (0 = remove).
fn lvl_at(mid_ref: i64, pct: f64, side: Side, size_units: i64) -> Level {
    let price = match side {
        Side::Buy => mid_ref as f64 * (1.0 - pct),
        Side::Sell => mid_ref as f64 * (1.0 + pct),
    };
    Level {
        price: price as i64,
        size: size_units * UNIT,
    }
}

fn lvl(pct: f64, side: Side, size_units: i64) -> Level {
    lvl_at(MID, pct, side, size_units)
}

/// Спот-тик (`pu=None`), непрерывность задаётся явно.
fn tick(u_first: u64, u_final: u64, bids: Vec<Level>, asks: Vec<Level>) -> DeltaTick {
    DeltaTick {
        bids,
        asks,
        first_update_id: u_first,
        final_update_id: u_final,
        prev_final_update_id: None,
        ts_exch_ms: 1_700_000_000_000,
    }
}

/// Якоря лучших цен — задают mid и НЕ трогаются тестовыми уровнями (те всегда глубже).
fn seed(mid_ref: i64) -> (Vec<Level>, Vec<Level>) {
    (
        vec![lvl_at(mid_ref, 0.001, Side::Buy, 10)],
        vec![lvl_at(mid_ref, 0.001, Side::Sell, 10)],
    )
}

/// Полоса `[500,800)` bps. Берём 6% (600 bps), а НЕ 5%: ровно 500 bps — граница полосы, и
/// f64-округление цены уводит уровень в соседнюю `[300,500)`. Замерено при отладке фикстуры:
/// на 5% уровень оказывался в `[300,500)`, оракул падал по неверной причине (класс «слепая
/// фикстура», `.claude/rules/testing.md`). Фикстура не должна стоять на границе диапазона.
const FAR_PCT: f64 = 0.06;
const FAR_LO_BPS: i64 = 500;

// ─────────────────────────────────────────────────────────────────────────────────────────────
// DV-I-10 — ЯДРО: перерождение цены = НОВАЯ жизнь, судьба не наследуется.
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// Сценарий, который действующая метрика читает неверно и ровно в сторону вывода:
/// уровень отменён (жизнь 1 = cancelled), затем РОДИЛСЯ СНОВА на той же цене и стоит живым
/// до конца окна (жизнь 2 = frozen — кандидат в фантом TD-016).
///
/// Действующая реализация вернёт `born=1, cancelled=1, frozen=0`, `cancel_fraction=1.0` —
/// то есть покажет «полоса живая» там, где половина эпизодов замёрзла.
#[test]
fn dv_i_10_rebirth_on_same_price_is_a_new_life() {
    let (sb, sa) = seed(MID);
    let ticks = vec![
        tick(1, 1, sb, sa),
        // жизнь 1 — рождение
        tick(2, 2, vec![lvl(FAR_PCT, Side::Buy, 5)], vec![]),
        // жизнь 1 — отмена
        tick(3, 3, vec![lvl(FAR_PCT, Side::Buy, 0)], vec![]),
        // жизнь 2 — рождение на ТОЙ ЖЕ цене
        tick(4, 4, vec![lvl(FAR_PCT, Side::Buy, 7)], vec![]),
        // окно кончается, жизнь 2 всё ещё жива
        tick(5, 5, vec![], vec![]),
    ];

    let rep = analyze(&ticks);
    let b = rep
        .band(Side::Buy, FAR_LO_BPS)
        .expect("полоса [500,800) bid обязана быть в отчёте");

    assert_eq!(b.lives_born, 2, "две жизни одной цены — два рождения");
    assert_eq!(b.lives_cancelled, 1, "жизнь 1 закончилась явным size=0");
    assert_eq!(
        b.lives_frozen, 1,
        "жизнь 2 жива на конце окна — это frozen, а НЕ отмена; \
         именно здесь старая метрика прятала сигнатуру фантома"
    );
    assert_eq!(b.lives_censored, 0, "gap'ов в фикстуре нет");
    assert_eq!(
        b.cancel_fraction(),
        Some(0.5),
        "1 отменённая из 2 завершённых эпизодов"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// DV-I-11 — НАСЫЩЕНИЕ: метрика обязана быть долей эпизодов, а не «цена хоть раз отменялась».
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// Строит окно: `cycles` полных отменённых жизней одной цены, затем ещё одна жизнь,
/// остающаяся живой до конца окна.
fn window_with_cycles(cycles: u64) -> Vec<DeltaTick> {
    let (sb, sa) = seed(MID);
    let mut ticks = vec![tick(1, 1, sb, sa)];
    let mut u = 2u64;
    for _ in 0..cycles {
        ticks.push(tick(u, u, vec![lvl(FAR_PCT, Side::Buy, 5)], vec![]));
        u += 1;
        ticks.push(tick(u, u, vec![lvl(FAR_PCT, Side::Buy, 0)], vec![]));
        u += 1;
    }
    // последняя жизнь — рождается и остаётся живой
    ticks.push(tick(u, u, vec![lvl(FAR_PCT, Side::Buy, 9)], vec![]));
    u += 1;
    ticks.push(tick(u, u, vec![], vec![]));
    ticks
}

/// Живость КОНЦА окна в обеих конфигурациях одинакова: ровно одна незавершённая жизнь.
/// Меняется только число завершённых эпизодов. Насыщающаяся величина обязана дать 1.0 в обоих
/// случаях (проба арбитра A-002 §0-3: 0.5 → 1.0), per-life — разные и предсказуемые доли.
#[test]
fn dv_i_11_metric_does_not_saturate_with_window_length() {
    let short = analyze(&window_with_cycles(1));
    let long = analyze(&window_with_cycles(3));

    let bs = short.band(Side::Buy, FAR_LO_BPS).expect("полоса short");
    let bl = long.band(Side::Buy, FAR_LO_BPS).expect("полоса long");

    assert_eq!(
        (bs.lives_born, bs.lives_cancelled, bs.lives_frozen),
        (2, 1, 1)
    );
    assert_eq!(
        (bl.lives_born, bl.lives_cancelled, bl.lives_frozen),
        (4, 3, 1)
    );

    assert_eq!(bs.cancel_fraction(), Some(0.5), "1 из 2 эпизодов");
    assert_eq!(bl.cancel_fraction(), Some(0.75), "3 из 4 эпизодов");

    // Суть инварианта: незавершённая жизнь ОСТАЁТСЯ видимой сколько бы циклов ни было.
    // Насыщающаяся метрика теряет её (frozen=0) и уходит в 1.0 — здесь это запрещено.
    assert_eq!(
        bl.lives_frozen, 1,
        "живая на конце окна жизнь не должна исчезать"
    );
    assert!(
        bl.cancel_fraction() < Some(1.0),
        "величина не имеет права насыщаться до 1.0 при живом уровне на конце окна"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// DV-I-12 — цензура не наследуется: после gap'а цена вправе прожить новую жизнь.
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// Жизнь 1 обрывается sequence-gap'ом (fate неизвестен → censored, НЕ отмена и НЕ заморозка —
/// инвариант DV-I-3 сохраняется). Затем поток восстанавливается, и та же цена рождается снова.
/// Вторая жизнь ОБЯЗАНА существовать и быть frozen (жива на конце окна).
///
/// Действующая реализация помечает цену Censored навсегда и второй жизни не заводит.
#[test]
fn dv_i_12_censored_life_does_not_poison_later_lives() {
    let (sb, sa) = seed(MID);
    let ticks = vec![
        tick(1, 1, sb, sa),
        // жизнь 1 — рождение
        tick(2, 2, vec![lvl(FAR_PCT, Side::Buy, 5)], vec![]),
        // РАЗРЫВ непрерывности: ожидался first_update_id=3
        tick(9, 9, vec![lvl(FAR_PCT, Side::Buy, 6)], vec![]),
        // поток продолжается от последнего ПРИНЯТОГО состояния (fail-closed: prev не двигался)
        tick(3, 3, vec![lvl(FAR_PCT, Side::Buy, 7)], vec![]),
        tick(4, 4, vec![], vec![]),
    ];

    let rep = analyze(&ticks);
    let b = rep
        .band(Side::Buy, FAR_LO_BPS)
        .expect("полоса [500,800) bid");

    assert_eq!(rep.gaps, 1, "ровно один разрыв непрерывности");
    assert_eq!(
        b.lives_censored, 1,
        "жизнь 1 оборвана gap'ом — fate неизвестен"
    );
    assert_eq!(
        b.lives_born, 2,
        "после восстановления потока цена родилась заново — это вторая жизнь"
    );
    assert_eq!(b.lives_frozen, 1, "жизнь 2 жива на конце окна");
    assert_eq!(b.lives_cancelled, 0, "явного size=0 не было ни разу");
    assert_eq!(
        b.cancel_fraction(),
        Some(0.0),
        "censored вне знаменателя: 0 отменённых из 1 завершённого эпизода"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// DV-I-13 — БАЛАНС + дегенерированный вход (чек-лист testing.md: асимметрия, множественность,
// отсутствие, границы).
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// Ни одна жизнь не теряется и не считается дважды: в КАЖДОЙ полосе
/// `lives_born == cancelled + frozen + censored`.
///
/// Вход намеренно НЕ «счастливый»:
///   - АСИММЕТРИЯ: часть тиков трогает только bid — ask при этом не обнуляется;
///   - МНОЖЕСТВЕННОСТЬ: рождение и отмена одной цены ВНУТРИ одного тика (полная жизнь за такт);
///   - ОТСУТСТВИЕ: уровни, не упомянутые в дельте, не считаются удалёнными;
///   - ГРАНИЦЫ: пустой тик, одиночный уровень, два разных диапазона полос.
#[test]
fn dv_i_13_lives_balance_on_degraded_input() {
    let (sb, sa) = seed(MID);
    let ticks = vec![
        tick(1, 1, sb, sa),
        // множественность: полная жизнь за один такт (рождение + отмена в одном тике)
        tick(
            2,
            2,
            vec![lvl(FAR_PCT, Side::Buy, 3), lvl(FAR_PCT, Side::Buy, 0)],
            vec![],
        ),
        // асимметрия: только bid; ask молчит и НЕ должен пострадать
        tick(3, 3, vec![lvl(0.20, Side::Buy, 4)], vec![]),
        // одиночный ask-уровень в другой полосе
        tick(4, 4, vec![], vec![lvl(FAR_PCT, Side::Sell, 2)]),
        // отсутствие: пустой тик ничего не удаляет
        tick(5, 5, vec![], vec![]),
        // МНОЖЕСТВЕННОСТЬ ЖИЗНЕЙ: та же far-цена проживает ВТОРУЮ полную жизнь
        tick(6, 6, vec![lvl(FAR_PCT, Side::Buy, 8)], vec![]),
        tick(7, 7, vec![lvl(FAR_PCT, Side::Buy, 0)], vec![]),
        // отмена только одного из живых
        tick(8, 8, vec![lvl(0.20, Side::Buy, 0)], vec![]),
        tick(9, 9, vec![], vec![]),
    ];

    let rep = analyze(&ticks);

    for b in &rep.bands {
        assert_eq!(
            b.lives_born,
            b.lives_cancelled + b.lives_frozen + b.lives_censored,
            "баланс жизней нарушен в полосе {:?} [{},{}): born={} c={} f={} cen={}",
            b.side,
            b.lo_bps,
            b.hi_bps,
            b.lives_born,
            b.lives_cancelled,
            b.lives_frozen,
            b.lives_censored,
        );
    }

    // ДИСКРИМИНИРУЮЩАЯ проверка. Один баланс — слабый оракул: он сходится и при учёте НА ЦЕНУ
    // (у цены ровно одна судьба), что подтверждено мутационным прогоном — с одним лишь балансом
    // этот тест был ЗЕЛЁНЫМ против фиктивной реализации. Поэтому требуем точные числа там, где
    // одна цена проживает ДВЕ полные жизни: per-price даст (1,1), per-life обязана дать (2,2).
    let far_bid = rep
        .band(Side::Buy, FAR_LO_BPS)
        .expect("полоса [500,800) bid");
    assert_eq!(
        (far_bid.lives_born, far_bid.lives_cancelled),
        (2, 2),
        "две полные жизни одной цены — два рождения и две отмены; \
         первая уместилась внутри одного тика и теряться не имеет права"
    );
    assert_eq!(far_bid.lives_frozen, 0, "обе жизни завершены явным size=0");

    // Отсутствие уровня в дельте — не удаление: ask-уровень дожил до конца окна.
    let far_ask = rep
        .band(Side::Sell, FAR_LO_BPS)
        .expect("полоса [500,800) ask");
    assert_eq!(
        far_ask.lives_frozen, 1,
        "ask не упоминался после рождения — он жив, а не удалён"
    );
    assert_eq!(far_ask.lives_cancelled, 0);
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// DV-I-14 — полоса определяется для КАЖДОЙ жизни отдельно + детерминизм.
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// Цена рождается, отменяется, mid уезжает — и та же цена рождается снова уже на ДРУГОМ
/// расстоянии от mid. Вторая жизнь обязана попасть в СВОЮ полосу.
///
/// Действующая реализация атрибутирует цену один раз при первом рождении и разносит обе
/// жизни в старую полосу — то есть приписывает дальней полосе активность ближней.
#[test]
fn dv_i_14_band_is_attributed_per_life_and_output_is_deterministic() {
    let (sb, sa) = seed(MID);
    // цена ровно на 5% ниже исходного mid → полоса [500,800)
    let target = lvl(FAR_PCT, Side::Buy, 5);
    let target_price = target.price;

    // сдвигаем якоря вверх на 10%: mid растёт, та же цена оказывается дальше от mid
    let moved_mid = MID + MID / 10;
    let (nb, na) = seed(moved_mid);

    let ticks = vec![
        tick(1, 1, sb.clone(), sa.clone()),
        // жизнь 1 в полосе [500,800)
        tick(2, 2, vec![target.clone()], vec![]),
        tick(
            3,
            3,
            vec![Level {
                price: target_price,
                size: 0,
            }],
            vec![],
        ),
        // якоря переезжают: старые снимаем, новые ставим
        tick(
            4,
            4,
            vec![lvl_at(MID, 0.001, Side::Buy, 0)],
            vec![lvl_at(MID, 0.001, Side::Sell, 0)],
        ),
        tick(5, 5, nb, na),
        // жизнь 2 — та же цена, но mid уже другой
        tick(
            6,
            6,
            vec![Level {
                price: target_price,
                size: 6,
            }],
            vec![],
        ),
        tick(7, 7, vec![], vec![]),
    ];

    let rep = analyze(&ticks);

    let near = rep
        .band(Side::Buy, FAR_LO_BPS)
        .expect("полоса первой жизни обязана присутствовать");
    assert_eq!(near.lives_born, 1, "в старой полосе — только ПЕРВАЯ жизнь");
    assert_eq!(near.lives_cancelled, 1);
    assert_eq!(
        near.lives_frozen, 0,
        "вторая жизнь родилась при другом mid и в эту полосу не входит"
    );

    // Вторая жизнь ушла в более дальнюю полосу и жива на конце окна.
    let deeper: u64 = rep
        .bands
        .iter()
        .filter(|b| b.side == Side::Buy && b.lo_bps > FAR_LO_BPS)
        .map(|b| b.lives_frozen)
        .sum();
    assert_eq!(
        deeper, 1,
        "вторая жизнь обязана быть атрибутирована по mid НА МОМЕНТ СВОЕГО рождения"
    );

    // Детерминизм: повторный прогон того же входа даёт тождественный отчёт.
    assert_eq!(rep, analyze(&ticks), "вывод обязан быть детерминирован");
}
