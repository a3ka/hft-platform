# TECH-DEBT — открытый долг

> **Reviewer-owned.** Открытые долги/риски, замеченные при работе. Закрытые переносятся вниз.

## OPEN
- **TD-001** recorder Docker-образ работает root'ом (M-00 заглушка). Hardening (non-root +
  права journal-тома) — при реальном recorder (M-01). Severity: MINOR.
- **TD-002** `hetzner-server` приватный ключ был вставлен в чат (скомпрометирован). Пересоздать
  на лэптопе + заменить на VPS при случае. Severity: MINOR (доступ и так только founder+ключи).
- **TD-003** `[verify-at-impl]` по Hyperliquid и Binance (rate-лимиты, подпись действий для
  ордеров) — уточнить при реализации order-стороны. Severity: NOTE.
- **TD-004** Binance L2 сейчас `@depth20@100ms` (частичный снапшот, топ-20). Для OBI-сигнала
  (полосы 3%/8%) нужна бОльшая глубина → полноценный snapshot+diff-sync (recon §A/§D). Severity: NOTE (следующая фаза).
- **TD-005** HL `l2Book` даёт снапшоты по изменению книги (наблюдалось ~реже Binance). Проверить
  полноту cadence; при нужде добавить `bbo`. Funding/liquidations пока не подписаны. Severity: NOTE.
- **TD-006** Журнал — один сегмент без ротации/ретеншена/cold-выгрузки (docs/06). Пока места вдоволь
  (150GB). Добавить сегмент-ротацию + retention→Storage Box когда объём вырастет. Severity: NOTE.
- **TD-007** DET-I-1 (бит-идентичный replay + state_hash) реализован частично (seq+read_all).
  Полный snapshot/state_hash — следующая фаза journal. Severity: NOTE.
- **TD-008** `t1-report-forms-promotion` (M-04). Rust-типы T1-форм `TrialRecord`/
  `ValidationReport` временно живут в `crates/research-cli/src/types.rs` со статусом
  «T1-designate» (per docs/fa/research-cli.md §N amendment 2026-07-10 + critic C-001 M1).
  Единственный продюсер/консюмер сейчас — research-cli; JSON несёт `report_schema_version`.
  Промоушен в `crates/contracts` + генерация JSON Schema (CT-I-4) — отдельным contract-RFC
  при появлении первого кросс-языкового консюмера (Python-тулинг). Severity: NOTE.
- **TD-009** `obi-track-a-report-pending` (M-04 задача 8, ОТКРЫТА). Прогон OBI Трек A/B →
  `research/reports/R-001*` гейтится накоплением данных полной книги (VPS пишет с 2026-07-10),
  вердиктом risk-critic (анти-оверфит чек-лист gates.md §6) и подписью founder ★. Merge
  M-04-кода risk/oms/venues/contracts не трогал — risk-critic обязателен на ОТЧЁТЕ, не на
  этом merge. Также см. TD-004 (Binance @depth20 недостаточен для полос 3%/8% — нужен
  full-book snapshot+diff). Severity: NOTE (гейт пути к деньгам, не долг кода).

## Замечания reviewer'а M-04 (не блокирующие, 2026-07-10)
- **RN-1** (NOTE) `verify_M-04.sh` T6 объединяет `contracts+journal+book` в один `check` —
  провал любого из трёх не различается по строке. Приемлемо для регресс-гейта (все GREEN),
  но при росте числа крейтов стоит разнести на per-crate строки для точной диагностики.
- **RN-2** (NOTE) Латентность δ_md — эмпирика из журнала, но δ_submit/δ_cancel — measured WS
  RTT ×2 (пессимизм-прокси, НЕ реальный order-path замер: P1 order-path ещё нет, D7 это
  честно фиксирует в provenance). Честность δ_submit/δ_cancel обязана быть предметом
  risk-critic на отчёте R-001 (чувствительность ×2 латентности per gates.md §6.4) — уже
  учтено дизайном стресс-вариантов, отмечаю для явной проверки на задаче 8.

## Замечания reviewer'а (фикс ts_exch_ms=0 у L2Snapshot, 2026-07-11)
- **RN-3** (NOTE) В фикс-коммите `1477bca` sacred inline-модуль `ts_exch_tests`
  (architect-owned) получил rustfmt-переносы (multi-line `assert_eq!`/let-else/let-binding).
  Сверено построчно: семантика тестов идентична (те же литералы 1_752_000_000_123 / 777_000 /
  1_600_000_000_000, те же ассерты и сообщения, та же структура). Переформатирование
  ВЫНУЖДЕНО гейтом `verify_M-04.sh` T1a (`cargo fmt --check`) — architect закоммитил RED-тесты
  с строками >100 col (допустимо: compile-RED всё равно не собирается), а GREEN обязан пройти
  fmt-гейт. Приемлемо (whitespace-only, semantics-preserving); отмечено для аудита касания
  sacred-файла dev-агентом.

## CLOSED
- (пусто)
