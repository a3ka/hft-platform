# M-05 — Data foundation: journal integrity + deep-book quality (P1)

STATUS: 🚧 IN_PROGRESS. Authored: architect (Fable), 2026-07-11.
Гейты: critic (DET-I-1-чувствительно + ≥5 коммитов) → engine/venue-dev → tester →
reviewer. **Без contract-RFC** (T1 не трогается — чистая робастность; расширение
типов данных — отдельный CT-RFC-01). risk-critic НЕ требуется (risk/oms/venues-ордера
не трогаются; venue-binance правится только в part книги/resnapshot, reviewer обязателен).

## Мотивация (находки сессии 2026-07-11)
Прод-журнал VPS (18.29h, 1.42GB) на КАЖДОМ редеплое рекордера получает:
1. **Рваный фрейм в середине** (unclean SIGKILL посреди BufWriter-flush) → `read_all`
   жёстко падает на первом CRC-mismatch (`journal/src/lib.rs:142`), читается лишь
   **713 714 из 1 954 182** восстановимых событий (37%). Накопление многодневных данных
   бесполезно, пока это так.
2. **Коллизия seq** — `next_seq` персистится только в `flush()` (отстаёт от сегмента);
   при рестарте seq ОТКАТЫВАЕТСЯ и переиспользуется для ДРУГИХ событий (проверено:
   seq 713710 = L2Snapshot в seg0 и Trade в seg1). Нарушение JR-I «seq монотонный,
   переживает рестарт» + DET-I-1 (недетерминированный replay через границу рестарта).
3. **Возможная фантомная ликвидность** в дальних полосах книги (5–60%): diff-sync без
   периодического resnapshot копит устаревшие лимитки при пропущенном cancel-диффе.

Ground-truth (resync-сканер architect'а по прод-сегменту): 4 чистых сегмента, 3 границы
повреждения, ВСЕГО восстановимо **1 954 182** события. Это acceptance-число для J3.

## Objective
Сделать прод-журнал (а) читаемым НАСКВОЗЬ через любые редеплои и (б) DET-I-1-корректным
через рестарт; книгу — свободной от фантомных дальних уровней. Фундамент под накопление
3–7 дней и формальный прогон Track B, а также под будущее расширение типов данных (CT-RFC-01).

## Contract impact (T1)
**НЕ трогается.** Block-C не срабатывает. Никаких новых `EventKind`/`MdPayload`.

## Allowed / Forbidden paths (scope-guard)
| Агент | Allowed | Forbidden |
|---|---|---|
| architect (Fable) | `milestones/M-05-*.md`, `crates/{journal,book}/tests/**` (RED, sacred), `scripts/verify_M-05.sh`, тест-фикстуры повреждённого сегмента | impl-код |
| engine-dev | `crates/journal/src/**` (clean-shutdown flush, next_seq-из-сегмента, recover-reader), `crates/recorder/src/**` (SIGTERM/SIGINT handler → Journal::flush перед exit) | tests, contracts, risk |
| venue-dev | `crates/venue-binance/src/**` (периодический REST-resnapshot + reconcile против diff-книги), `crates/book/src/**` (если нужен eviction-примитив — carve-out по RED-тесту) | tests, contracts, risk |
| все dev | — | `crates/contracts/**`, `*/tests/**`, `scripts/**`, `.claude/**`, `docs/**` |

## §Tasks
| # | Status | Задача | Агент | Verify |
|---|---|---|---|---|
| 1 | ⏳ | RED-оракулы (sacred): journal clean-shutdown/seq/recover + book anti-phantom + verify-скрипт | architect | компиляция OK; RED-suite падает на текущем коде |
| 2 | ⏳ | recorder: SIGTERM/SIGINT handler → `Journal::flush()` (seg+meta) перед exit; graceful stop | engine-dev | J1 GREEN |
| 3 | ⏳ | journal: `next_seq` авторитетно из скана последнего валидного фрейма сегмента при `open()` (не из отстающей меты) | engine-dev | J2 GREEN (нет reuse) |
| 4 | ⏳ | journal: `recover()` — resync-толерантное чтение через рваные фреймы (read_all остаётся strict для DET-I-1 exact-replay); CLI-tool восстановления существующего журнала | engine-dev | J3 GREEN; recover(prod-fixture)=1_954_182 |
| 5 | ⏳ | venue-binance: периодический REST-resnapshot (cadence — фикс. конфиг) + reconcile → evict фантомные уровни | venue-dev | B1 GREEN |
| 6 | ⏳ | `scripts/verify_M-05.sh` exit=0 (fmt+clippy+все тесты+канарейки) | tester | exit=0 |

## RED-тесты (sacred, architect-only)
- `crates/journal/tests/red_shutdown.rs`:
  - **J1 `clean_shutdown_flushes_meta_and_segment`** — append N без явного flush → имитация
    graceful-stop → reopen → next_seq == last_seq+1, seg читается целиком без торна.
  - **J2 `next_seq_authoritative_from_segment_not_stale_meta`** — сегмент содержит фреймы
    до seq=K, мета искусственно отстаёт (< K) → `open()` → next_seq == K+1 (НЕ мета).
    Падает на текущем коде (берёт мету). Анти-плацебо: на заглушке (return meta) FAIL.
  - **J3 `recover_resyncs_across_torn_frame`** — сегмент=[valid][торн-фрейм(crc-bad)][valid]
    → `recover()` возвращает ВСЕ валидные фреймы обеих сторон; `read_all()` по-прежнему
    Err (strict, для DET-I-1). Канарейка: recover(prod-fixture) == 1_954_182 событий.
- `crates/book/tests/red_resnapshot.rs`:
  - **B1 `resnapshot_evicts_phantom_far_level`** — diff ставит уровень на 40% от mid →
    пропущен cancel-дифф → применяется resnapshot без этого уровня → уровень удалён
    (книга == снапшот, notional_within(40%) не содержит фантом). Падает на pure-diff.

Все RED обязаны падать на текущем коде/заглушке (анти-плацебо, `.claude/rules/testing.md`).

## Acceptance
`bash scripts/verify_M-05.sh; echo "exit=$?"` → `VERDICT: PASS`, exit=0. Ключевая
канарейка: recover() по реальному повреждённому прод-сегменту = 1_954_182 события, 4 сегмента.

## Handoff
architect (RED+милестоун) → critic (DET-чувствительно) → engine-dev(2,3,4)‖venue-dev(5) →
tester(6) → reviewer. Параллельно: CT-RFC-01 (расширение типов данных) — отдельный трек.

## Amendment 2026-07-11 (founder «делай п.1»; architect design-close)

Решения приняты (по рекомендации architect, founder «1»):
- **recorder → lib+bin** (J1 seam). Приземлено: `recorder/src/lib.rs::run_writer(rx, journal,
  hb, shutdown: impl Future)` skeleton + RED `tests/red_shutdown_j1.rs` (runtime-RED, контракт
  clean-shutdown = drain буфера + flush). engine-dev выносит select!-цикл из `main.rs:89-113`
  + врап SIGTERM.
- **derive крейт** (C5, M-06): чистые downstream-агрегаты; funding_breadth + RED. GREEN — research-dev.

**B1 ПЕРЕОРИЕНТИРОВАН (по чтению `venue-binance/src/lib.rs`).** Найдено: diff-sync ведёт
continuity `U==last+1` и инвалидирует+ре-снапшотит на разрыве; `book::apply_snapshot`
(`book/src/lib.rs:25-27`) уже REPLACE (clear+insert). Значит «фантомная ликвидность» —
СЛАБЫЙ риск (пропуск cancel без gap протокол не даёт). Реальный риск глубоких полос —
**undercount**: REST `limit=5000` (`lib.rs:24`) достаёт лишь ~top-5000 уровней; глубже —
пусто на бутстрапе, пока по уровню не пройдёт дифф (само-заживает для АКТИВНЫХ уровней;
ре-снапшот НЕ чинит — REST max 5000).

Следствие:
1. **Тестируемый инвариант B1 (unit) ОТЛОЖЕН до seam:** «gap-инвалидация → полная замена
   книги (нет stale-переноса)». venue-binance book-maintainer приватен (`struct OrderBook`
   `lib.rs:37`); нужен testable seam (pub(crate) + тест-доступ) — СО-ДИЗАЙН с venue-dev,
   чтобы RED совпал с их рефактором (не плодим placebo под выдуманный API).
2. **Лимитация limit=5000 (→ reviewer TECH-DEBT):** глубокие полосы 15-60% недосчитываются
   на бутстрапе. Решить митигацию: (а) многостраничный REST/иной источник глубины,
   (б) принять undercount (diff само-заживает активные уровни), (в) диагностик-сверка
   поддерживаемой книги vs свежий REST на глубине. Это ВОПРОС ДАННЫХ, не баг кода.

Итог п.1: C5 ✓, J1 ✓ (RED landed); B1 — design-closed (инвариант + лимитация
специфицированы; unit-RED ждёт venue-dev seam, осознанно НЕ форсирую placebo).
