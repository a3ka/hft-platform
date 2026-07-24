# M-32 — верификация ДОСТОВЕРНОСТИ глубины стакана (Track C предусловие, TPP-блокер)

STATUS: **PROPOSED** (2026-07-24, architect). Пивот P-COCKPIT, founder-приоритет №1 ЭТОЙ сессии —
**ВЫШЕ M-28/M-31 в очереди диспетча**. Верифицирует ИЗМЕРИТЕЛЬ глубины (не строит TPP). Критик НЕ
триггерится (`crates/research-cli/{src,tests}`, `research/data-quality/` — не contracts/risk/ks/oms/venue,
не новый крейт) → reviewer-бэкстоп. Class-A doc (verdict-gate) → reviewer перед main.

## Мотивация (BINDING — что доказано, что НЕТ)

Весь кокпит/TPP строится на глубине 3-30% от mid. Мы вскрыли, что раньше принимали **на веру**, но
**не доказали**. Принцип C-020/M-10 «сначала провалидируй ИЗМЕРИТЕЛЬ, потом доверяй» ещё НЕ доведён до
глубины. Этот milestone доводит.

**Что РЕАЛЬНО знаем (замерено — `research/data-quality/depth-probe-{binance,staleness}.md`):**
- Наша diff-книга достигает p50 ≈ 54-58% от mid (структурный кап ±60% `MAX_REL_DIST`); полосы 3-30% на
  Binance ЗАПОЛНЕНЫ и РАСТУТ 3%→30% (дальние уровни там ЕСТЬ).
- REST-снапшот Binance (`depth?limit=5000`) капит **~1.3% BTC spot / 0.09-0.26% futures** — это
  ЕДИНСТВЕННЫЙ биржевой эталон, с которым мы можем сверяться. Глубже эталона у биржи нет.
- Hyperliquid: книга ≤20 уровней, reach 0.03-0.1% → дальние полосы НЕвычислимы (Binance-only для TPP-полос).
- Фантом-сигнатура TD-016 (растёт-вечно: up→100%, dd→0) на shell-notional **НЕ подтвердилась**; НО валидность
  дальних полос **ТОЖЕ НЕ доказана** — `dd=100%` конфаундится resync-циклами (REST-ресинк ~1.3% → дальние
  полосы обнуляются → добираются из diff). Shell-notional тест не смог развести churn от resync.

**ДВА ОТКРЫТЫХ ВОПРОСА (milestone обязан ЗАКРЫТЬ — они НЕ решены, НЕ переоткрывать как решённые):**

- **Q1 (ресёрч, unverified):** есть ли источник глубины ГЛУБЖЕ 1.3% с БИРЖЕВОЙ правдой? Прежнее
  утверждение «ни у кого нет глубже, мы на паритете с Bookmap/TPP» — это **НЕПРОВЕРЕННАЯ инференция
  architect'а, НЕ факт**. Доказать/опровергнуть ФАКТАМИ: как Bookmap / TensorCharts(TPP) / квант-вендоры
  РЕАЛЬНО берут глубину? Binance WS `@depth` без REST-капа / deep-snapshot / L3-MBO? Вендоры полного стакана?
  Другие endpoint'ы? — это прямой тест «паритета».
- **Q2 (эмпирика):** достоверны ли наши полосы 3-30% БЕЗ более глубокого эталона? Нужны эмпирические
  trust-тесты БЕЗ deeper-эталона: **(а) staleness/lifetime** дальних уровней — получают ли они `size=0`
  (отмены) в потоке, или замерзают (=фантом); **(б) order-flow консистентность** — сделки соответствуют
  показываемой книге (поток diff'а верен?); **(в) cross-source сверка** — сравнить нашу реконструкцию с
  независимым deep-источником, ЕСЛИ найдётся в Q1.

## КЛЮЧЕВОЙ ENABLER (почему Q2а теперь ПРЯМО измерим, а не proxy)

Прод пишет `MdPayload::L2Delta` для **BTCUSDT** (Binance spot, CT-RFC-04, с ~2026-07-21) — СЫРОЙ `@depth`
diff с ТОЧНОЙ семантикой (contract `crates/contracts/src/lib.rs`):
- `size == 0` = **явный remove уровня от биржи** (отмена/исполнение); `size > 0` = set; уровень, которого
  в дельте НЕТ, — **не изменился** (дельта не источник истины о неупомянутом — `testing.md` «отсутствие»).
- Sequencing: `first_update_id`(U) / `final_update_id`(u) / `prev_final_update_id`(pu) → **gap-детекция
  БЕЗ обращения к бирже** (реплей видит любой пропуск дельт).

**Следствие:** staleness (Q2а) измеряется НАПРЯМУЮ на сыром diff-потоке — «получает ли дальний уровень
`size=0` при жизни» — и **де-конфаундится resync'ом через sequencing** (уровень, исчезнувший ЧЕРЕЗ gap,
цензурируется, а не считается ни отменой, ни заморозкой). Это ровно то, что shell-notional depth_probe
СДЕЛАТЬ НЕ МОГ. depth_probe оперировал реконструированными снапшотами (resync их обнуляет); L2Delta-lifetime
оперирует событиями отмен напрямую.

## Objective

Провалидировать ИЗМЕРИТЕЛЬ глубины и выдать **founder-вердикт**, а НЕ строить TPP:
1. **Q1 memo** — фактический обзор источников глубины (закрывает «паритет» фактами).
2. **Q2 trust-харнес** — RED-first анализаторы над журналом: (а) L2Delta-lifetime/staleness,
   (б) order-flow faithfulness, (в) cross-source (условно — только если Q1 найдёт источник).
3. **Вердикт-гейт** → founder-решение: (i) достижим ли эталон глубже 1.3%; (ii) достоверны ли полосы 3-30%
   по staleness/order-flow; (iii) строить TPP-полосы на РЕАЛЬНОМ эталоне ИЛИ честно помечать
   `depth_band_provenance: diff-reconstructed` (VB-I-5) — ОСОЗНАННО, не случайно.

**TPP-полосы 3-30% НЕ строить, пока этот вердикт не вынесен.**

## Отношение к M-31 (эвикция) — ОРТОГОНАЛЬНО, НЕ дублирует

- **M-31** (book eviction, `feat/M-31`) делает книгу КОРРЕКТНО-ПОДДЕРЖАННОЙ: bounded, dead-near вычищен
  ≤1.3% против REST-эталона, дальнее помечено провенансом. Это **корректность поддержки**.
- **M-32** ВЕРИФИЦИРУЕТ, достоверна ли дальняя глубина как ИЗМЕРЕНИЕ (against staleness/order-flow/эталон).
  M-31 не отвечает «реальны ли полосы 15/30%» — только «книга не разъезжается и не течёт». M-32 отвечает.
- Порядок: M-32 (верификация) — предусловие включения TPP-полос в export-контракт (Track C). M-31 —
  предусловие того, что книга, которую мы верифицируем, не деградировала. Оба нужны; не конфликтуют.

## Contract impact (T1) — НЕТ

Milestone ПОТРЕБЛЯЕТ существующий `MdPayload::L2Delta` (CT-RFC-04, дискриминант 6). Ничего в T1 не
меняется. CT-RFC не нужен. Новых крейтов нет. → **critic doc-гейт §9 НЕ триггерится**, reviewer-бэкстоп.

## §Tasks (RED-first; Q1 без RED, Q2 — RED architect ПЕРЕД impl)

| # | Статус | Задача | Кто | Acceptance |
|---|---|---|---|---|
| 1 (Q1) | ⏳ | **Depth-source survey memo** → `research/data-quality/depth-sources-survey.md`. Фактами: (a) Binance — есть ли endpoint/поток глубже REST-5000-капа (`@depth` WS полный, deep snapshot, L3/MBO — крипта их обычно НЕ даёт, доказать); (b) вендоры (Tardis, Kaiko, Amberdata, CoinAPI, Coinalyze) — дают ли ВАЛИДИРОВАННЫЙ полный стакан или реконструируют из того же diff-потока (тот же 1.3% REST-якорь); (c) как Bookmap / TensorCharts(TPP) РЕАЛЬНО берут глубину. Явно пометить прежнюю инференцию architect'а «паритет» → **CONFIRMED / REFUTED** с цитатами. | research-dev | Memo отвечает на Q1 фактами (≥3 независимых источника/вендора, cross-check); «паритет» явно CONFIRMED или REFUTED; вывод: существует ли эталон глубже 1.3% (y/n) + условие Q2в |
| 2a | ✅ | **RED DV-I-1..5** (`crates/research-cli/tests/red_depth_lifetime.rs`) — L2Delta-lifetime/staleness анализатор. Sacred. | architect | compile-RED; каждый DV-I FAIL против заглушки (reachability в ОБЕ стороны); degraded-чек-лист покрыт |
| 2b | ⏳ | **impl** `research_cli::depth_lifetime::analyze(...)` + прогон над свежим VPS-срезом BTCUSDT → числа `cancel_fraction` per band → допись в `depth-sources-survey.md`/новое memo | research-dev | DV-I-1..5 GREEN; прогон на реальном журнале (gap-free окна); memo с per-band cancel_fraction near vs far |
| 3a | ✅ | **RED DV-I-6** (`crates/research-cli/tests/red_orderflow_faith.rs`) — order-flow faithfulness. Sacred. | architect | compile-RED; DV-I-6 FAIL против «всегда consistent» заглушки |
| 3b | ⏳ | **impl** `research_cli::orderflow::consistency(...)` + прогон → доля trade↔book-decrement совпадений | research-dev | DV-I-6 GREEN; прогон на журнале; число в memo |
| 4 (Q2в) | ⏳ (условно) | **cross-source recon** — ТОЛЬКО если Q1 (task 1) нашёл независимый deep-источник: сверить нашу реконструкцию против него. Если источника нет → задача N/A с явной записью «эталона нет, Q2в закрыт отрицательно» | research-dev | Либо recon-числа, либо документированное N/A с обоснованием из Q1 |
| 5 (verdict) | ⏳ | **Вердикт-memo** → `research/data-quality/depth-verdict.md`: синтез Q1+Q2 → 3 founder-решения (эталон глубже 1.3% y/n; полосы достоверны y/n по staleness+order-flow; TPP на эталоне ИЛИ diff-provenance). Гейт: TPP-полосы НЕ в контракт до подписи. | architect (синтез) → **founder** | Вердикт называет каждый Q1/Q2 результат; §D handoff — founder-подпись (что именно решает) |

## §Инварианты (RED-оракулы; sacred, architect-only) — DV-I-1..6

Анализатор `depth_lifetime` — **чистый редьюсер** над упорядоченным потоком L2Delta-тиков (для RED —
in-memory фикстуры, как `red_depth_series.rs`; для прода — `journal::stream` с `EpochFilter` НАЗВАН).
Внутри трекает mid (переиспользует `book::OrderBook::apply_l2delta`) для атрибуции уровня к полосе
(дистанция от mid) + per-price жизненный цикл (birth / explicit-cancel / censored-at-gap). Вывод —
детерминирован (BTreeMap-порядок). Полоса-суффикс: `cancel_fraction = cancelled / (cancelled + frozen)`,
**censored исключены** из знаменателя.

| ID | Инвариант | Оракул (fixture-семантика) |
|---|---|---|
| **DV-I-1** (cancel = live) | Уровень, получивший явный `size=0` в contiguous-seq окне, → CANCELLED (живой), lifetime записан. | Дальний bid P set→ (contiguous) P size=0 ⇒ band.cancelled≥1, frozen=0. **Анти-плацебо:** заглушка «всё frozen» → FAIL |
| **DV-I-2** (freeze = phantom) | Уровень, созданный и НИ РАЗУ не получивший `size=0` до конца contiguous-окна, → FROZEN (фантом-кандидат). | P set, далее K contiguous тиков без упоминания P, окно кончилось ⇒ band.frozen≥1, cancelled=0. **Анти-плацебо:** заглушка «всё cancelled» → FAIL |
| **DV-I-3** (resync де-конфаунд — ЯДРО) | Уровень, исчезнувший ЧЕРЕЗ sequence-GAP (разрыв `pu`-цепочки / скачок `U`), → CENSORED (ни cancel, ни frozen). | P present → тик с GAP, P отсутствует далее ⇒ band.censored≥1, cancelled=0, frozen=0. **Анти-плацебо:** наивный «gap-исчезновение = отмена» → cancelled≥1 → FAIL; «игнор gap = frozen» → frozen≥1 → FAIL. Только censored верно (то, что depth_probe НЕ мог) |
| **DV-I-4** (отсутствие ≠ удаление) | Уровень, не упомянутый в дельте, — НЕ изменён (не стареет, не отменяется). Только явный `size=0` — отмена. | P set, много contiguous тиков про ДРУГИЕ цены (P молчит), затем P size=0 ⇒ P cancelled с lifetime = ПОЛНЫЙ пролёт (не усечён «молчанием»). **Анти-плацебо:** «истекает после N молчаливых тиков» → неверный lifetime / преждевременный frozen → FAIL |
| **DV-I-5** (детерминизм) | Тот же вход → байт-идентичный отчёт (VB-I-1 класс; без wall-clock/rand/неупоряд. итерации). | Дважды тот же fixture ⇒ идентичный `LifetimeReport`. **Анти-плацебо:** HashMap-итерация в выводе → нестабильный порядок → FAIL |
| **DV-I-6** (order-flow faithfulness, Q2б) | Trade на цене P объёмом S ДОЛЖЕН сопровождаться соответствующим убыванием книги на P (дельта, уменьшающая/снимающая P) в seq-окне; Trade на P БЕЗ book-decrement → INCONSISTENT (поток лжёт). | Trade@P,S + L2Delta P−S ⇒ consistent++. Trade@P без decrement ⇒ inconsistent++. **Анти-плацебо:** заглушка «всегда consistent» → FAIL на mismatch-fixture |

## §Анти-плацебо чек-лист (BINDING — `testing.md` «фикстура счастливого пути»)

Каждый RED обязан включать ДЕГРАДИРОВАННЫЙ вход, не только идеальный:
1. **Асимметрия:** обновляется/приходит ТОЛЬКО одна сторона; односторонний diff — mid не съезжает ложно.
2. **Множественность:** ≥2 уровня/дельты в одном тике; ≥2 отмены — наивный «один» ловится.
3. **Отсутствие:** дельта молчит об уровне ≠ удалить (DV-I-4 — прямой пункт).
4. **Границы:** пустая сторона `[]`, один уровень, переход через sequence-gap (DV-I-3), первый тик без mid.
5. **Resync-цензура:** gap-исчезновение НЕ считается ни отменой, ни заморозкой (DV-I-3 — ядро де-конфаунда).

## Allowed / Forbidden paths

- **architect (sacred):** `milestones/M-32-depth-verification.md`, `crates/research-cli/tests/red_depth_lifetime.rs`,
  `crates/research-cli/tests/red_orderflow_faith.rs`, `scripts/verify_M-32.sh`, вердикт-структура docs.
- **research-dev (impl + memo):** `crates/research-cli/src/{depth_lifetime.rs,orderflow.rs,lib.rs}` (новый модуль
  `depth_lifetime` + расширение `orderflow::consistency`), `research/data-quality/*.md` (Q1 memo + числа + вердикт-черновик),
  свой `crates/research-cli/Cargo.toml` `[dependencies]` (добавлять СВОИ, если нужно — book уже есть).
- **Forbidden:** `crates/contracts` (T1 — L2Delta уже есть, НЕ трогать), `crates/{risk,killswitch,oms,venue-*,book/src}`,
  `research/trials-ledger.json` (append-only механизм), order-path. Правка сырого журнала — нет (offline read-only).

## Acceptance (`scripts/verify_M-32.sh`)

`set -euo pipefail` ИЛИ агрегатор+FAIL-счётчик. CI-точно (RN-17/TD-035): `cargo fmt --all -- --check` +
`cargo clippy --all-targets --all-features -- -D warnings`. ≥1 проверка на задачу:
- DV-I-1..5 GREEN (`cargo test -p research-cli --test red_depth_lifetime`);
- DV-I-6 GREEN (`--test red_orderflow_faith`);
- существование memo Q1 (`research/data-quality/depth-sources-survey.md`) и вердикта (`depth-verdict.md`) —
  grep на обязательные секции (Q1 CONFIRMED/REFUTED; вердикт называет 3 founder-решения);
- финал `VERDICT: PASS`/`FAIL`, exit соответствует.

## Данные для замеров

Локальный срез `/tmp/m10-vps-journal` УДАЛЁН (прошлая сессия). Для эмпирики — подтянуть СВЕЖИЙ срез с VPS
(последние сегменты, содержат BTCUSDT L2Delta; прод здоров, сегмент 80 растёт) или гонять против живого
VPS-журнала. VPS: `ssh -i /home/nous/.ssh/hft_deploy -o IdentitiesOnly=yes root@167.233.192.131`; журнал —
`/var/lib/docker/volumes/hft-platform_journal-data/_data/`.

## Handoff

Task 1 (Q1) — research-dev, БЕЗ RED (research memo), reviewer-бэкстоп. Task 2a/3a (RED) — architect ПЕРЕД
impl 2b/3b. Task 4 условна на Q1. Task 5 (вердикт) → **founder-подпись** (граница C: решение о данных TPP).
