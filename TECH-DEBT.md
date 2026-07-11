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
- **TD-010** `binance-rest-depth-limit-5000-undercount` (M-05 task 5 / B1, venue-dev, ОТКРЫТА).
  Заведено по флагу founder'а от venue-dev: REST-resnapshot глубины Binance ограничен
  `limit=5000` уровнями на один вызов — дальние полосы книги за пределами топ-5000 одним
  снапшотом не покрываются, а reconcile против diff-книги ограничен этим потолком. Прямое
  следствие для anti-phantom eviction (B1): в самых дальних полосах устаревшие лимитки могут
  не эвиктиться из-за неполноты reference-снапшота. Точный масштаб undercount + стратегия
  (пагинация vs принятие потолка с явной границей достоверности полос) — за venue-dev при
  посадке task 5/B1; на этом merge (engine-dev part) код venue не трогался. Связано с TD-004.
  Severity: NOTE (граница достоверности данных дальних полос, не риск ордер-пути).

## Замечания reviewer'а M-05 (не блокирующие, 2026-07-11)
- **RN-8** (fmt-гейт под-покрытие) `verify_M-05.sh` fmt-гейт проверяет только `journal+book`, не
  `recorder` — из-за чего v2 recorder-файлы без trailing newline (`cargo fmt --all --check` FAIL)
  прошли verify зелёным. Поймано reviewer'ом вручную (`cargo fmt --all`), engine-dev пофиксил
  (`7db4479`). → architect: расширить fmt-гейт verify_M-05.sh на recorder. Также урок: verify-скрипт
  milestone'а обязан fmt-check ВСЕ тронутые крейты, не подмножество.
- **RN-4** (AUDIT sacred-файла) engine-dev правил `scripts/verify_M-05.sh` (architect/tester-owned,
  SACRED per scope-guard) в коммите `2a21b8c` (task #4). Правка УЗКАЯ: замена placeholder
  `echo PENDING J1 + FAIL++` на реальный прогон `run "J1 …" cargo test -p recorder --test
  red_shutdown_j1` — оракул J1 стал доступен после task #2. Reviewer подтверждает допустимость:
  (а) явная авторизация founder'а на эту J1-строку; (б) правка НЕ ослабляет гейт — конвертирует
  форсированный FAIL в честный тест-прогон; (в) сверено построчно — J2/J3/B1/fmt/clippy-строки и
  FAIL-агрегатор не тронуты. РЕВЕРТ НЕ ТРЕБУЕТСЯ. На будущее: wiring sacred-скрипта — отдельный
  коммит tester/architect (паттерн M-06 task 6), не бандл в feature-коммит dev'а.
- **RN-5** (partial-merge, founder-authorized) engine-dev part M-05 (tasks 2/3/4) смержен в `main`
  ДО полного close-out milestone'а. `verify_M-05.sh` → `VERDICT: FAIL (1)`, и единственный FAIL —
  `B1 resnapshot anti-phantom` (venue-dev task 5) PENDING, ортогональный к journal/recorder-фиксу.
  Push разрешён явным founder-override правила auto-push-only-on-exit-0 (B1 не в зоне engine-dev,
  фикс journal-integrity прод-критичен). Milestone остаётся IN_PROGRESS до B1 (task 5) + wiring
  task 6 (verify exit 0). НЕ close-out. **⚠ ОТКАЧЕНО через ~4 мин — прод-регрессия, см. TD-011.**
  Урок: eyes-on §8 ssh-проверка ОБЯЗАТЕЛЬНА и поймала то, что зелёный CI/юнит-тесты/Deploy-success
  пропустили; «Deploy success» ≠ «прод пишет данные».
- **RN-6** (DET-I-1 подтверждение) `read_all` остался STRICT (Err на первом CRC-mismatch +
  postcard-decode→Err — сверено на `b22583c`); resync-толерантность вынесена в ОТДЕЛЬНУЮ
  `recover()` (честный побайтовый ресинк, без rand/wall-clock). DET-I-1 exact-replay НЕ ослаблен.
  `next_seq = meta.max(seg-scan)` — источник истины сегмент, reuse исключён (мета-lag не даёт
  отката; мета-ahead даёт gap, не reuse — оба безопасны для монотонности).

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
- **TD-011** `scan_next_seq-full-segment-read-oom` (M-05 task#3) — **RESOLVED 2026-07-11**.
  Инцидент: v1 `Journal::open()` делал `read_to_end` ВСЕГО сегмента (прод 2.65 GiB) в RAM на каждом
  старте → recorder не писал (101% CPU, 2.48 GiB RAM, OOM-риск); юнит-RED на крошечных фикстурах не
  поймал; healthcheck обманут; поймано eyes-on §8. Откачено (`c2ad02c`/`ffdc410`/`e190356`).
  ФИКС (v2, `a356c81`): `scan_tail_for_last_seq` — читает последние ≤4 MiB (seek+read_exact),
  `next_seq = max(meta, tail+1)`, O(1) память. Верификация: (а) architect RED-оракул
  `red_open_bounded.rs` (64 MiB + counting-allocator, бюджет 8 MiB) GREEN; (б) reviewer НЕЗАВИСИМЫЙ
  прод-масштаб харнес (2.94 GiB): open()=4 ms, max RSS 6 MiB, next_seq корректен; (в) eyes-on §8 на
  VPS после merge/deploy: новый recorder пишет (CPU 0.53%, MEM 5.41 MiB, tail-scan реального 2.71 GiB
  прод-сегмента → `next_seq=3467845`, сегмент растёт). Урок закреплён в `.claude/rules/testing.md`
  (прод-масштаб RED для sacred I/O) + RN-8 (fmt-гейт под-покрытие).
