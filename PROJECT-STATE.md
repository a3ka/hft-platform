# PROJECT-STATE — что реализовано

> **Reviewer-owned.** Обновляется ТОЛЬКО reviewer'ом после merge (scope-guard). Отражает
> фактическое состояние `main`, не планы (планы — `docs/DESIGN.md §10` роадмап).

> **Закрытые милестоуны переехали, а не исчезли** (reviewer, close-out 2026-08-16):
> `docs/archive/PROJECT-STATE-closed-2026-08-16.md` — 41 раздел завершённой работы.
> Здесь остаётся ТОЛЬКО живое: незакрытые милестоуны, действующая инфраструктура и
> «Пока НЕ реализовано». Файл — состояние `main`, не летопись.
> Обращение к обоим файлам — грепом по предмету (`docs/workflow/reading-map.md` ярус C).

## Байтовый курсор хвоста (M-57 «tail-follower» — ✅ ВНЕДРЁН НА ПРОД `650f22d` 2026-08-08, `R-044`; **гейт пройден, но ЦЕЛЬ НЕ ДОСТИГНУТА**)

**Читать вместе с `R-044` §D.** Разделены два разных утверждения, смешение которых и есть
способ закрыть milestone поверх недостигнутой цели:

1. **Acceptance-гейт пройден и механизм на проде.** `verify_M-57.sh` PASS на дереве слияния,
   CI по `650f22d` **success**, Deploy `31283309383` **success** (запущен намеренно через
   `workflow_dispatch`, со свидетелем), `grep stream_from_at` на VPS: 0 → **9**.
2. **Предсказание эффекта провалено по названному ЗАРАНЕЕ порогу.** `R-043` §C фиксировал:
   «`c` > 15 % ⇒ цель НЕ достигнута». Замерено `c` = **19.2 %** при цели ≤5 %.

**Что M-57 реально дал (замер до/после, симметричная методика, `R-044` §D):**

| показатель | до (`dca889a`) | после (`650f22d`) | выигрыш |
|---|---|---|---|
| CPU на сессию (N=1) | 100.1 % | **19.2 %** | ×5.2 |
| CPU при 5 сессиях | 397 % (потолок 4 ядер) | 89.3 % | ≥×4.4 |
| CPU при 20 сессиях | 397 % (потолок) | 237.3 % (не потолок) | — |
| **потолок сессий на 4 ядрах** | **4** | **≈34** | ×8.4 |
| RssAnon на сессию | 6 906 kB | 3 805 kB | ×1.8 |
| латентность подключения, медиана | 2 098 ms | 1 161 ms | ×1.8 |
| латентность, РАЗБРОС | 1 883–3 799 ms (×2.0) | 1 143–1 187 ms (**×1.04**) | схлопнулся |

**Под устранённым пересканированием обнажился пол ≈19 % на сессию** — к пересканированию
отношения не имеющий. Это ровно та альтернативная гипотеза, которую `R-043` §C назвал ДО
замера. Главный подозреваемый — `TD-120` (метаданные-путь `O(#сегментов)` на тик; сегментов
на проде **219**, и число растёт ⇒ пол будет ползти вверх). До объяснения этого пола
дальнейшая оптимизация умножает неизвестную константу: многопоточность (×4) дала бы ≈136
сессий против цели 10 000+, что подтверждает вывод `SESSION-HANDOFF` §00 п.2 — решает переход
«состояние на инструмент, а не на сессию».

**База «до» переснята и это было необходимо:** за сутки код прода не менялся, но сегмент
вырос 207 → 219, и цена сессии с ним — с 52 % (`R-043` §B) до 100.1 %. Вчерашняя база для
сверки уже не годилась.

- **Что изменилось по существу.** Тик удержания WS-сессии перестал пересканировать активный
  сегмент журнала от начала: сессия держит байтовый курсор (`TailHint`) в ПАМЯТИ, и
  `journal::stream_from_at` открывает активный сегмент со смещения. Работа тика становится
  пропорциональна приращению, а не длине сегмента. Побочно введён честный счётчик
  `events_scanned` (ПРОЧИТАННЫЕ события) рядом с `events_decoded` (ДОШЕДШИЕ) — прежние меры
  работы были слепы к пересканированию (`TD-109`).
- **Цена вопроса, замеренная на живом проде ДО merge** (`R-043` §B — исторически первая
  оценка; ПЕРЕКРЫТА пересъёмом `R-044` §B, потому что снималась на более коротком сегменте):
  1 зритель ≈52 %, 5 → ≈261 % (линейно), 20 → ≈397 % (потолок 4 ядер), точка насыщения ≈7.7
  сессии. **На момент деплоя те же величины были уже вдвое хуже** (100.1 % / потолок при
  N=5 / насыщение при 4 сессиях) — цена сессии есть функция длины активного сегмента, а он
  растёт непрерывно. Это и есть предмет M-57.
- **Гейты — ЗЕЛЁНЫЕ, и прогнаны на САМОМ ДЕРЕВЕ СЛИЯНИЯ, а не на ветке:** `verify_M-57.sh`
  **VERDICT: PASS, exit=0** (14 PASS / 0 FAIL), `cargo test --all` **passed=803 failed=0**
  (196 блоков), clippy `--all-targets --all-features -D warnings` чист, fmt чист. Дерево
  прогона и дерево merge сверены побайтно по `crates/`+`scripts/` — 0 расхождений.
  Прогон на дереве слияния был не перестраховкой, а необходимостью: **CI исполнить гейт не
  может** (`TD-114` — квота GitHub Actions, все 6 джобов `steps=0`).
- **Мутационный контроль — главное доказательство круга 3** (`R-040` §C). Откат механизма
  (`stream_from_at` → `stream_from`) валит гейт ЧЕРЕЗ оракулы прод-формы `f035_*`, причём
  журнальные `O-1..O-4` при этом остаются ЗЕЛЁНЫМИ (`PASS T3` при `FAIL T3b`) — прямая
  демонстрация, что они структурно слепы к дефекту. Композитная мутация (плюс подмена
  измерителя) — та самая пара строк, что в круге 2 давала `PASS` 11/11 против вернувшегося
  P0, — сегодня красная. Числа фикстуры воспроизвели прод-замер до единиц: **8003 события
  прочитано при приращении 3**.
- **Деплой состоялся НАМЕРЕННО и со свидетелем** (`R-044` §A/§C). Первая попытка (по merge
  `710b1ad`) была честно заблокирована: `Gate on CI` → `failure` (steps=0, квота — `TD-114`),
  `Deploy (build on VPS)` → **SKIPPED**. После восстановления квоты CI по `650f22d` стал
  **success**, и Deploy запущен вручную через **`workflow_dispatch`** (`31283309383`, success,
  1m51s). Ручной триггер существует в `deploy.yml` с `M-00` (`25fe2c8`) — ни правка гейта, ни
  пустой коммит ценой гэпа записи не понадобились.
- **Прод после деплоя (`gates.md` §8 eyes-on, `R-044` §C):** оба контейнера `healthy`,
  heartbeat отставание 0.6–0.7 с, журнал растёт (`next_seq` 196 211 041, сегмент 219),
  `gateway-serve` RssAnon 332 kB / threads 1 / CPU 0.0 % в покое, recorder CPU 1.6 %,
  load 0.16, свободно 61 GB, остаточных проб 0. Sanity свежих кадров: `schema_version=8`,
  серии заполнены (`ohlcv=48 cvd=48 heatmap=739 cob=12`), курсор двигается.
- **Открыто после merge:** `TD-109` (слепые меры работы), `TD-116` (`hint.pos` без валидации),
  `TD-117` (три stateless-вызова `stream_from` без оракула), `TD-118` (загрязнение замеров
  пробами — формулировка уточнена в `R-044` §F.4), `TD-119` (гейт не отличает «код красный»
  от ENOSPC), **`TD-120`** (метаданные-путь `O(#сегментов)` на тик — главный подозреваемый на
  остаточный пол 19 %), `TD-121` (комментарий guard'а `JR-I-11` недосчитывает пути),
  **`TD-122`** (`wsprobe --secret` несовместим с сервером: hex-декод против ASCII),
  **`TD-123`** (наблюдение: 397 % CPU при нуле клиентов — не воспроизведено, нужен оракул).
  `TD-115` — **CLOSED** (M-57 доехал на прод при свидетеле).
- **Цепочка гейтов:** `R-035` (REJECT) → `R-039` (REJECT) → `R-040` (APPROVED) · критики
  `C-059`/`C-060`/`C-063` · арбитр `A-004` · close-out `R-043` → деплой и замер `R-044`.
  RISK-BLOCK не применялся:
  диф не трогает `crates/{risk,killswitch,oms,venue-*}`; `crates/contracts/**` не тронут
  (`PASS T6`) ⇒ Block-C не применяется.

## Инфраструктура (готово, проверено 2026-07-10)
- Репо `a3ka/hft-platform` (private, ветка `main`).
- VPS cpx32 (`167.233.192.131`): Ubuntu 26.04, Docker 29.6 + Compose, Rust 1.97; репо
  склонирован в `/root/hft-platform` (read-only deploy-key).
- CI/CD: `.github/workflows/ci.yml` (fmt+clippy+test+audit) + `deploy.yml` (build-on-VPS,
  `git push main` → SSH → `docker compose up --build` → healthcheck → rollback). **Проверено
  сквозным деплоем: recorder-заглушка Up/healthy, journal-том persistent.**

## Процессный слой (M-00 — готово)
- `CLAUDE.md` + `.claude/rules/` (5) + `.claude/agents/` (9) — EINHARD-модель под трейдинг.
- `PROJECT-STATE.md` + `TECH-DEBT.md` (reviewer-owned).

### Doc-гейт + protected-artifacts барьер (C-006 — CLOSED, MERGED 2026-07-15, reviewer APPROVED)
Цикл C-006 завершён (founder-решение: принять после P1/P17, без новых витков критика). Смержены
ДВЕ ветки (порядок td021-rules → doc-gate): `af33d61` (td021-rules) + `0d5f8f8` (doc-gate), полная
история rev3–rev12 (architect fix ⇄ critic REJECT) сохранена. Reviewer резолвил конфликты:
PROJECT-STATE/TECH-DEBT → в пользу main (надмножество); `.claude/rules/testing.md` → ОБЕ секции
(TD-021 «оракул мерит то, что обещает» + «Целостность гейта — 4 свойства»); `ci.yml` → ОБА job'а
(delivery + protected-artifacts, `status-check.needs` = все четыре).
- **`scripts/check_protected_artifacts.sh`** — барьер: коммит/мерж не смеет удалить/подменить/усечь
  вердикт критика (`research/critiques/`), milestone, RFC. **База сравнения из СОБЫТИЯ** (не
  `origin/main` — иначе диапазон пуст и гейт зелён всегда, блокер B1); пустая/zero/переписанная база
  → **fail-closed**. Ловит: удаление, rename-out, evil-merge, merge-born-then-dropped, подмену типа
  (каталог/симлинк), усечение в 0 байт.
- **`scripts/tests/red_protected_artifacts.sh`** — проба барьера ТОЙ ЖЕ проводкой, что CI (17
  сценариев). **Анти-плацебо доказан reviewer'ом независимо:** rev8-барьер → FAIL(3) P14/P15/P16
  (подмена типа/усечение); guard-мутация (P7 merge→true, P17 echo→true) → «SETUP НЕ СОСТОЯЛСЯ», не
  ложный PASS. На merged-дереве VERDICT: PASS (17/17).
- **CI job `protected-artifacts`** (`ci.yml`) — base-from-event, fail-closed, в `status-check.needs`.
- **Мета-правило `.claude/rules/testing.md` «Целостность гейта — 4 свойства»**: гейт обязан (1)
  гонять прод-форму, (2) мерить свой инвариант не окружение, (3) падать против слома И несостоявшегося
  setup, (4) наблюдать ОТСУТСТВИЕ не только сбой. Итог ~10 дефектов серии за сессию (D8/D9 — эталон).
- **Открытый пункт (founder ★):** барьер force-push ДЕТЕКТИРУЕТ (fail-closed на zero/переписанную
  базу), но не ПРЕДОТВРАЩАЕТ — закрывается branch protection «no force-push» на `main` (GitHub-настройка).

### Откат разрастания правил по арбитражу `A-003` (✅ MERGED `d50b1e0`, reviewer APPROVED 2026-08-04, `R-032`)
Слой правил вырос ×2.7 от базовой линии (`99b1329`, 566 строк) в основном ДНЕВНИКОМ инцидентов.
Арбитр (`research/arbitration/A-003-rules-vs-workflow.md`, Fable, свежий контекст) отсудил 36 блоков:
16 ОСТАВИТЬ / 16 ПЕРЕПИСАТЬ / 4 УДАЛИТЬ. Исполнено architect'ом в 3 шага + 6 коммитов доработки
по вердикту. **Итог: rules 1515 → 946, `CLAUDE.md` 108 → 92.**
- **Маршруты возвращены к эталону `docs/04-workflow.md`** (не менялся ни разу): формулировка
  «низкорисковые — в т.ч. docs — без critic'а» восстановлена дословно; отдельного маршрута
  architect → reviewer на doc-правку больше нет; reviewer — бэкстоп в конце цепочки (04 §2).
  `.claude/rules/*` вернулись в self-push зону architect'а — так оно и работало (19 из 24 правок).
- **`gates.md` §9 переписан:** класс-таксономия A/B, «тест на класс», петля самоправки, ретро-раздел
  удалены (~90 строк); critic на доки остаётся ПО ТРИГГЕРАМ §1. Механический барьер сохранён.
- **Git-identity предписания удалены везде** (правило, `commit-discipline.md`, бутстрап `pi-dev.sh`):
  замер `A-003` #27 + `R-031` §C.3 — ролевая личность наследуется копированием worktree и как признак
  роли не работает в принципе. Действующая норма — единая подпись владельца + метка роли в subject'е.
  Отсюда же снят `TD-101` как беспредметный.
- **Гейт поймал 6 находок, 2 блокирующие** (`R-032`, круг 1 REJECT → круг 2 APPROVED):
  (F-1) вместе с таксономией §9 удалено требование **risk-critic на документы safety-пути** — а это
  сегодня ЕДИНСТВЕННЫЙ невакуумный риск-гейт: крейтов `risk`/`killswitch`/`oms` нет, §5 привязан к
  несуществующим путям, `RK-I-*` живут исключительно текстом в `docs/fa/*` (47 упоминаний);
  (F-11) §8 eyes-on сведён к liveness — потеряны ДВЕ нормы базы (sanity свежих событий после смены
  формата, TD-031; «CPU/MEM в норме», TD-011). Обе возвращены.
- **Барьер артефактов РАСШИРЕН** (`R-032` F-10): `research/reviews/*.md` и `research/arbitration/*.md`
  теперь защищены механически — вердикт reviewer'а есть условие merge (§4), решение арбитра
  обязательно к исполнению (§0), а защищены они не были. Проба **18 → 20 сценариев** (P19/P20 новые;
  текст §9 и проба синхронны — сверено, дрейфа «17 против 18» не повторилось;
  P1 setup-guard починен — мёртвый вызов `is_protected` давал `command not found` и guard не
  срабатывал НИКОГДА). **Анти-плацебо доказан reviewer'ом независимо:** откат `is_protected` → P19/P20
  FAIL; откат списка путей `ls-tree` → P19/P20 FAIL (обе половины несущие); стенд ложных срабатываний:
  rename внутри защиты exit=0, правка содержимого exit=0, увод в `research/archive` exit=1.
- **§8 деплой-гейт GREEN:** CI success (`30860944968`), Deploy НЕ триггерился (path-фильтр — только
  `crates/**`/`Cargo.*`/`Dockerfile`/`compose`), значит рестарта recorder'а и гэпа записи не было;
  VPS eyes-on: оба контейнера `(healthy)` аптайм 2ч, heartbeat 7с, `writable:true`, журнал +1.45 MB/20с,
  `RssAnon` recorder 21.5 MB / gateway-serve 19.4 MB, load 0.24.
- **Незакрытое — `TD-102`** (F-6..F-9: целевой объём `A-003` §3 не достигнут — §6 ledger и
  startup-блок не сжаты; ярлык «класс A» пережил таксономию в 5 живых доках). Норм не теряет.

## Даталеер / поток данных (M-01 — РАБОТАЕТ, проверено на VPS 2026-07-10)
- `crates/contracts` — T1 `Event`/`EventKind::Md(MdEvent)`: Trade/L2Snapshot/Funding,
  fixed-point i64 ×1e8, Venue/Side/Level. Тесты: 2 GREEN.
- `crates/journal` — append-only (postcard+crc32 фреймы), монотонный seq персистится
  через рестарт, `read_all` replay. Единственный писатель. Тесты: 2 GREEN.
- `crates/venue-binance` — spot combined-stream `@trade` + `@depth20@100ms` → MdEvent.
- `crates/venue-hyperliquid` — WS `trades` + `l2Book` (уровни-объекты {px,sz,n}), ping-keepalive.
- `crates/recorder` — venue-supervisor (reconnect+backoff) → mpsc(EventKind) → журнал + heartbeat.
- **Проверено в проде (VPS):** Binance + Hyperliquid оба пишутся в персистентный журнал,
  реальные цены/стакан, seq монотонный, контейнер healthy, автодеплой работает.

## Journal integrity (M-05 — engine-dev part MERGED + прод-верифицирован 2026-07-11; milestone IN_PROGRESS)
Tasks 2/3/4 (engine-dev) НА main (`a356c81`/`e8c3540`/`7db4479`, push `7db4479`), founder ★-authorized
partial-merge (RN-5; B1 остаётся PENDING). Прошёл полный цикл: v1 откатан из-за TD-011 (full-segment
`read_to_end` в `open()` → recorder не писал, 101% CPU/2.48 GiB); v2 — **ХВОСТОВОЙ tail-scan O(1)
памяти**; reviewer НЕЗАВИСИМО перепроверил §8 на прод-масштабе (2.94 GiB синт-сегмент: open()=4 ms,
max RSS 6 MiB, next_seq корректен) ДО merge, и eyes-on на VPS после deploy: **новый recorder пишет**
(CPU 0.53%, MEM 5.41 MiB, сегмент растёт, `journal progress next_seq=3467845` — tail-scan реального
2.71 GiB прод-сегмента отработал за ~секунды). TD-011 **RESOLVED**.
- `crates/recorder` — `run_writer` select-seam в lib (юнит-тестируемый J1); `main` враппит SIGTERM/SIGINT
  → ветка `shutdown` дрейнит буфер (`try_recv`) + `flush()` перед exit. Heartbeat wall-clock — в отдельный
  `.heartbeat` файл, НЕ в journal-payload (детерминизм журнала сохранён).
- `crates/journal` — `next_seq` при `open()` из `scan_tail_for_last_seq` (читает последние ≤4 MiB
  сегмента, seek+read_exact, buf освобождается до write-open) = `max(meta, tail_seq+1)`; O(1) память,
  нет reuse (J2/TD-011 GREEN на прод-масштабе). `recover()` — resync-толерантное чтение (offline CLI,
  НЕ в горячем `open()`; полный read_to_end допустим только offline). `read_all` STRICT (Err на
  CRC-mismatch) — DET-I-1 exact-replay не ослаблен.
- **Урок (зафиксирован в `.claude/rules/testing.md`):** RED-оракул sacred I/O-пути ОБЯЗАН включать
  прод-масштаб (арх-оракул `red_open_bounded.rs` — 64 MiB + counting-allocator бюджет 8 MiB); зелёные
  юнит-тесты + Deploy-success ≠ рабочий прод — eyes-on §8 решающий. См. TD-011 (CLOSED), RN-4..8.
- **M-05 остаётся IN_PROGRESS:** task 5 B1 (venue-dev, anti-phantom resnapshot) PENDING → `verify_M-05.sh`
  exit=1 (только B1); task 6 (tester, verify exit 0) после B1. TD-010 (REST limit=5000) открыт.

## Data expansion (M-06 — #4 reland MERGED, reviewer APPROVED 2026-07-13; close-out pending)
Смержены ДВЕ инертные (не потребляются recorder'ом до #4 poller → прод-поведение НЕ изменено)
APPROVED-ветки; main стал полностью GREEN (впервые за цикл RED-on-main). §8 eyes-on после deploy:
recorder БЕЗ изменений (CPU 0.79%, MEM 5.6 MiB, сегмент растёт +261 KB/12s, next_seq растёт, restarts=0).
- `crates/venue-binance-futures` (venue-dev, tasks #2/#3 + N2/N3) — USDT-M перп fstream-адаптер:
  парсеры `@depth@100ms`→L2Snapshot, `@forceOrder`→Liquidation (side = ликвидируемая сторона, C2),
  `/fapi/v1/openInterest`→OpenInterest (C3); `parse_mark_price` (`markPriceUpdate`→Funding, знак, N3);
  `FuturesDepthBook.apply_snapshot` = REPLACE-семантика (INV-N2: gap-ресинк эвиктит stale дальние
  уровни → анти-фантомная ликвидность). 5/5 RED GREEN, MD-only (ордер-путь не тронут → risk-critic
  не нужен, gates.md §5 N4 carve-out). НЕ потребляется recorder'ом (нет в его deps).
- `crates/derive::funding_breadth` (research-dev, task #5) — чистый детерминированный агрегат
  funding-breadth (%+/−, top-N по universe); проходит ХАРДЕНУТЫЙ red_breadth (асимметрия 60/20,
  хардкод-пруф). Потребители — research-cli/signals (downstream, journal-first).
- **#4 recorder-wire BinanceFutures — ПОПРОБОВАН, РЕВЕРТНУТ (§8 eyes-on поймал прод-регрессию,
  2026-07-11).** engine-dev wiring (`2eee4bf`: default_venues loop + `Box<dyn Fn>` type-erasure,
  supervise() неизменён) прошёл code-review A+B (MD-only, boundary чист, fmt/clippy/workspace-test/
  verify_M-06 GREEN) + CI + Deploy success — и БЫЛ смержен. Но §8 eyes-on на VPS показал: живой
  futures-адаптер попал в hot-loop REST-ресинка → **133 × HTTP 418 (Binance IP ban) за 25s, депт-книга
  не бутстрапится, 0 futures L2Snapshot в журнал**, ~5 req/s абьюз биржи с IP, общего со спот-сбором.
  Дефект — в уже-инертном `venue-binance-futures` (no-backoff на snapshot-fail, `lib.rs:596-600`/
  `:613-620`), который #4 сделал LIVE (НЕ в engine-dev wiring — оно корректно). **Реверт**
  (`6ddf810`+`6de58e8`), main = tree(`3f38ab0`), прод re-verified inert-safe (418=0, CPU 0.99%,
  MEM 5.22 MiB, seg растёт +133KB/12s, hb свежий, 0 restarts). Заведён **TD-013 (BLOCKING #4,
  MAJOR)**. Реленд #4 — после фикса TD-013 (architect RED backoff-оракул → venue-dev impl → re-apply
  `2eee4bf`). Урок TD-011 подтверждён 3-й раз (RN-9).
- **TD-013 фикс (Backoff) — MERGED inert, reviewer APPROVED 2026-07-12.** Цепочка реленда:
  architect RED `449bb38` (`tests/red_backoff.rs` — политика `Backoff::next_delay`/`reset`: ≥100ms
  первый ретрай, exp-рост, cap 5мин, honor Retry-After, reset на success) → venue-dev `cc4f529`
  (impl + wiring в `handle_snapshot`). Reviewer подтвердил **анти-плацебо WIRING** (ключевой риск:
  RED тестит ТОЛЬКО чистую политику, НЕ I/O-await): `make_snapshot_future(.., Some(delay))` делает
  **РЕАЛЬНЫЙ `tokio::time::sleep(delay).await` ПЕРЕД `fetch_snapshot`** (не сконструированный-но-
  проигнорированный Backoff); `fetch_snapshot` распознаёт 418→120s/429→10s cooldown ДО
  `error_for_status` → hot-loop рвётся на первом 418. sleep суспендит только futures данного символа
  (FuturesUnordered), не runner. red_backoff + red_parse/red_funding/red_resnapshot все GREEN,
  workspace GREEN, fmt/clippy clean. **INERT** (recorder НЕ зависит от venue-binance-futures на
  этом merge — dep реверта #4 отсутствует; Backoff-код недостижим из recorder). §8 inert-safety
  на VPS: recorder БЕЗ изменений (spot+HL only, 0 futures/418, CPU 0.64%, MEM 5.4 MiB, seg
  +98KB/8s, hb ~10s cadence свежий, 0 restarts). Джиттер НЕ добавлен (спека оракула его не требует;
  политика детерминирована) — NOTE в TD-013, не блокер.
- **#4 reland после TD-013 — REJECTED / REVERTED (§8 live NOT GREEN, 2026-07-12).**
  Reland `8b26d6c` (recorder dep `venue-binance-futures`, `default_venues()`, итерационный spawn
  supervisor'ов; `supervise()` не тронут) прошёл локально RED `red_futures_wired` (1 passed),
  fmt/clippy/workspace tests, `verify_M-06.sh` PASS exit=0, GitHub CI + Deploy success. §8 eyes-on
  на VPS подтвердил часть TD-013: **hot-loop 418 НЕ повторился** (rate-limit retries spaced
  ~50-60s / cooldown, не 133×418/25s), CPU/MEM нормальные, restarts=0, heartbeat свежий, journal
  растёт, seq непрерывен. Но продуктовый критерий #4 НЕ выполнен: в live journal-tail были
  `BinanceFutures` OpenInterest + ConnUp, но **0 BinanceFutures L2Snapshot и 0 Funding** (20 MiB и
  115 MiB хвосты), при повторяющихся `depth continuity gap` / `snapshot stale ... backoff` циклах.
  Funding из `!markPrice@arr` не rare-event, поэтому это не §8-GREEN. Реверт `e6b4a75` + `d819cc3`;
  main снова inert-safe: VPS HEAD `d819cc3`, spot+HL only, 0 futures/418, hb age 8s, segment +60KB/5s,
  CPU ~0.7-5%, MEM ~5.8 MiB, restarts=0. Открыт **TD-014 (BLOCKING #4)**.
- **TD-014 fix + #4 RELAND-2 — REJECTED (§8 live NOT GREEN, 2026-07-12).**
  Цепочка `0f924dc` RED `red_live_emit` → `595fc24` FuturesSession seam/run() delegation →
  `3d9c214` RED recorder wiring → `af7725f` engine-dev reland прошла локально:
  `red_futures_wired` PASS, `venue-binance-futures` 7/7 PASS, workspace tests PASS,
  fmt/clippy clean, `verify_M-06.sh` PASS exit=0. Static review подтвердил: `run()` реально
  делегирует WS/snapshot/tick через `FuturesSession`, recorder wiring итерационный,
  `supervise()` не тронут, diff MD-only. Но pre-merge §8 deploy на VPS показал: 3 `venue connect`
  строки есть, `BinanceFutures` ConnUp + OpenInterest пишутся, seq непрерывен (`seq_gaps=0`),
  heartbeat свежий, CPU/MEM нормальные, restarts=0; **при этом live journal-tail с момента deploy:
  0 `BinanceFutures.L2Snapshot`, 0 `BinanceFutures.Funding`**. Логи продолжают цикл
  `depth continuity gap detected` / `snapshot stale vs buffered diffs` / 429 backoff.
  Это НЕ §8-GREEN; branch НЕ смержен. VPS восстановлен на `origin/main` `2bbcbd7`
  (spot+HL only, no futures supervisor, healthy, hb age ~3s).
- **TD-014 v2 + #4 reland `fac7c07` — REJECTED (§8 live NOT GREEN, 2026-07-12).**
  Цепочка `71255c5` strong live-lifecycle RED + `fac7c07` recovery-snapshot T/E fix прошла
  локально: `red_futures_wired` PASS, `venue-binance-futures` 7/7 PASS, workspace tests PASS,
  fmt/clippy clean, `verify_M-06.sh` PASS exit=0; static review подтвердил MD-only и реальное
  recorder wiring. Pre-merge deploy на VPS (`fac7c07`) стартовал 3 venue, был healthy,
  heartbeat свежий, seq непрерывен (`seq_gaps=0`), OI писал. Но §8 journal-tail с deploy:
  `BinanceFutures.L2Snapshot=16`, `OpenInterest=16`, **`Funding=0`**; L2 sparse, не ~1/s/symbol.
  Логи за live-window: `depth continuity gap` 311, `snapshot stale` 44, `429` 18, CPU до 6.99%
  на старте (позже ~1.2%). Это НЕ §8-GREEN; branch НЕ смержен. VPS восстановлен на
  `origin/main` `3eff0db` (spot+HL only, no futures supervisor, healthy, hb age ~4.5s).
- **TD-014 T2 + #4 reland `669ce40` — REJECTED (§8 live NOT GREEN, 2026-07-12).**
  Цепочка `38c3175` RED futures-continuity (`pu`, не spot `U == last+1`) + `669ce40`
  dual-rule fix прошла локально: `red_futures_wired` PASS, `venue-binance-futures` 8/8 PASS,
  workspace tests PASS, fmt/clippy clean, `verify_M-06.sh` PASS exit=0. Static review подтвердил
  MD-only и корректное разделение: steady-state strict `pu == last_update_id`, reconcile-loop
  Binance-style `U <= L+1 && u >= L+1`, `pu` fail-closed. Pre-merge §8 на VPS показал реальный
  прогресс: 3 venue стартовали, recorder healthy, heartbeat свежий, CPU ~1.1%, MEM ~7.5 MiB,
  restarts=0, `seq_gaps=0`, fresh tail с deploy: `BinanceFutures.L2Snapshot=470`,
  `OpenInterest=54`; после стартового окна последние 3 минуты имели `gap=0`, `stale=0`, `429=0`
  (одиночный 418 без hot-loop). Но обязательный live-критерий всё ещё НЕ выполнен:
  **`BinanceFutures.Funding=0`** в 48 MiB journal-tail за несколько минут live-window.
  `!markPrice@arr` не является rare-event, поэтому это не §8-GREEN. Branch НЕ смержен; VPS
  восстановлен на `origin/main` `4012c55` (spot+HL only, healthy, hb age ~6s, CPU 0.58%).
- **TD-014 T3 + #4 reland `99b1329` — REJECTED (§8 live NOT GREEN, 2026-07-13).**
  Цепочка `c747a97` RED per-symbol markPrice + `99b1329` per-symbol `<sym>@markPrice@1s`
  subscription прошла локально: `red_futures_wired` PASS, `venue-binance-futures` 9/9 PASS,
  workspace tests PASS, fmt/clippy clean, `verify_M-06.sh` PASS exit=0. Static review подтвердил:
  runner подписывает per-symbol `@markPrice@1s`, `FuturesSession` поддерживает одиночный
  `markPriceUpdate` и legacy `!markPrice@arr`, diff MD-only. Pre-merge §8 на VPS показал:
  recorder healthy, 3 venue стартовали, heartbeat свежий, CPU ~1.1-1.2%, MEM ~6-7 MiB,
  restarts=0, `seq_gaps=0`, fresh tails с deploy: `BinanceFutures.L2Snapshot=637`,
  `OpenInterest=66`; позднее окно имело `gap=0`, `stale=0`, `429=0`. Но обязательный
  live-критерий всё ещё НЕ выполнен: **`BinanceFutures.Funding=0`** в persisted journal
  после нескольких минут live-window; logs за позднее окно также `markPrice/Funding=0`.
  Branch НЕ смержен; VPS восстановлен на `origin/main` `1d5ecfa` (spot+HL only, healthy,
  futures logs after restore=0).
- **TD-014 T4 + #4 reland `c123bbd` — APPROVED / MERGED (§8 live GREEN, 2026-07-13).**
  Цепочка `d9b3b1c` RED premiumIndex REST funding poll + `c123bbd` venue-dev pivot прошла:
  local reviewer gates GREEN (`red_futures_wired`, `venue-binance-futures` 10/10 including T4,
  workspace tests, fmt, clippy, `verify_M-06.sh` PASS exit=0), remote Docker verify on VPS
  GREEN (`VERDICT: PASS exit=0` after installing rustfmt/clippy components in `rust:1-slim`),
  and §8 live GREEN. VPS candidate `c123bbd`: recorder healthy, 3 venue connect, heartbeat fresh,
  CPU ~1.5%, MEM ~9.5 MiB, restarts=0, late window `418=0`, `429=0`, `gap=0`, `stale=0`.
  Persisted journal since deploy: `seq_gaps=0`, `BinanceFutures.L2Snapshot=465`,
  `OpenInterest=48`, **`Funding=48`**. Merge commit: `1504d8b` (`M-06 reland #4
  (TD-014 v2+T2+T3+T4)`). TD-014 CLOSED.
- **M-06 статус после reviewer:** #1 compile/C1 green, inert venue-futures + derive части на main,
  **#4 recorder-wire BinanceFutures merged and live-green**, #5 funding-breadth green. Milestone
  close-out остаётся за tester/architect chain: tester #6 clean-checkout `verify_M-06.sh` /
  architect close-out docs. Reviewer НЕ трогал milestone status columns.
  Data-quality долг:
  TD-012 (futures REST depth limit=1000 undercount). TD-013 anti-hot-loop live-verified; TD-014
  live funding/depth emission closed by T4.

## Governance контрактного слоя: CT-RFC-05 ретро-документ (✅ MERGED `c4caddb`, reviewer APPROVED 2026-08-02; docs-only) + CT-RFC-06 приземлён (✅ MERGED `c019ba9`, `R-021` APPROVED; STATUS документа — PROPOSED, ратификация за founder'ом)
**CT-RFC-05 — дыра ЗАКРЫТА.** `docs/rfc/CT-RFC-05-margin-inventory.md` приземлён в `main`: изменение
T1 (`MdPayload::MarginInventory`, `SCHEMA_VERSION` 3→4), жившее с 2026-07-25 в коде и на проде
**без формального RFC-документа**, теперь имеет его ретроспективно. Дыра Д2
(`docs/plans/contracts-current-state.md`) и TD-061 — CLOSED. Документ ничего не переигрывает:
T1-форма уже на проде (§8 «не переигрывает изменение»), это ретро-фиксация факта.
- **Цепочка гейтов:** critic `C-044` **REJECT** (3 из 4 цитируемых SHA не существовали в `main`;
  список мест правки занижен 4 вместо 5) → фикс architect'а `03815dd` → `C-046` **PASS** →
  reviewer `R-018` **APPROVED** → merge `c4caddb`.
- **Reviewer перепроверил независимо** (не по вердиктам критика — первый круг сорвался именно на
  непроверенных цитатах): 12/12 SHA существуют И входят в `origin/main`; карта мест правки = **5**,
  собственный греп с `examples/**` и `src/bin/**`; форма T1, порядок вариантов 0..7,
  `SCHEMA_VERSION=4`, JSON Schema, фикстуры valid+invalid, CHANGELOG, три теста `ct_rfc05.rs` —
  всё на месте; мотивация §6 дословно возводится к M-35 и `margin-source-survey` §9.
- **Гейты:** `verify_ct_rfc_atomic.sh` PASS exit=0 · `verify_design_claims.sh --merge-preview
  origin/main` PASS (0 нарушений) exit=0 — прогон на MERGE-ЦЕЛИ, не только на ветке (урок R-013).
- **Что теперь машинно защищено от повтора:** `scripts/verify_ct_rfc_atomic.sh` (`557be33`,
  подключён к CI `b3b42d2`) при правке `crates/contracts/src/**` требует В ТОМ ЖЕ диффе наличия
  `docs/rfc/CT-RFC-NNN-*.md`. Правило §4 больше не держится на добросовестности.
- **Остатки:** TD-070 (нет прямого оракула на reuse-барьер эпохи 3→4 — документ фиксирует честно),
  TD-068 (карта влияния T1-варианта составлялась по памяти, а не грепом — дважды за два дня).

**CT-RFC-06 (`L2Delta`) — ✅ ПРИЗЕМЛЁН в `main` (merge `c019ba9`, reviewer `R-021` APPROVED,
2026-08-02, круг 2; docs/research-only). `STATUS документа — PROPOSED, НЕ ратифицирован.`**
- **Что означает merge:** документ и его пруф-база живут в `main` как аудит-трейл. Merge **НЕ**
  ратифицирует RFC: §9 перечисляет 4 пункта под подпись founder'а (первый — «ратификация
  CT-RFC-06 как whole»), `gates.md` §7 запрещает любому агенту подставлять approve. Более того,
  документ САМ объявляет впереди risk-critic (шапка + §0.3 «`contracts`-тематика = RISK-BLOCK»);
  по `gates.md` §5 на ЭТОМ диффе триггер не срабатывает (ни строки кода, ни `crates/contracts/**`),
  поэтому merge законен без risk-critic — но **самозаявленная цепочка документа НЕ считается
  пройденной**, и трактовать APPROVED как «risk-critic пройден» нельзя (`R-021` N1).
- **Что приземлено:** `docs/rfc/CT-RFC-06-l2delta.md` (421 стр.) + пруф-якоря
  `research/measurements/{td-053-event-size,m-45-l2delta-impact}.md` + вердикты `C-045`,
  `R-019`, `R-021`. Каталог `research/measurements/` появился в `main` впервые — TD-069 CLOSED.
- **Круг 1 (`R-019`, `22715b7`) — CHANGES REQUESTED:** содержательно документ прав (подтверждено
  дважды независимо), но 5 нарушений `verify_design_claims.sh`. **Круг 2 (`R-021`) — все закрыты:**
  F1/F2 (пруфы приземлены merge-коммитами `87181b4`/`df03366`), F3 (путь
  `docs/07-cockpit-backend-roadmap.md` развёрнут), F4/F5/F6 закрыты ПО СУЩЕСТВУ — названы
  остаточные классы эпох (незамеченный семантический сдвиг + забытый `EPOCH_ID`, машинного
  fail-closed нет), условие невакуумности `JR-I-10` (COLD читаем только при фактическом
  монтировании; сегодня Storage Box не заведён ⇒ инвариант держится на HOT/WARM), фактическое
  покрытие `DET-I-1` (фикстур с `L2Delta` — ноль, замер reviewer'а подтвердил → TD-072).
- **Гейт на MERGE-ЦЕЛИ:** `verify_design_claims.sh --merge-preview origin/main` → `VERDICT: PASS
  (0 нарушений)`, exit=0 (26 SHA / 104 пути); версия из `main` (без RFC-проверок) на той же
  merge-цели — тоже PASS exit=0. Прогон reviewer'а, не перенос из отчёта.
- **Карта exhaustive-`match` = ПЯТЬ — подтверждена ТРЕТЬИМ независимым методом** (`R-021` §2):
  греп по 8-му варианту `MdPayload::MarginInventory` даёт те же 5 файлов (+ `venue-binance`,
  где это конструирование, не `match`). В `segments.rs` — РОВНО ОДИН exhaustive `match` по
  `MdPayload` (`event_data_ts`, стр. 2566-2580), остальные `match` в файле имеют другой
  scrutinee. Побочно: утверждение `docs/NEXT-SESSION-PROMPT.md:149` о «неполноте» замера —
  **ложно** (замер называет пять и перечисляет те же пять) → TD-071.
- **Содержательный вывод документа (в силе):** посылка «нужен contract-RFC на НОВЫЙ вариант»
  **опровергнута** — `MdPayload::L2Delta` (дискр. 6) уже в T1 с `CT-RFC-04`/M-18 (`lib.rs:293`,
  merge `f635bd2`) ⇒ **M-45 = расширение allow-list `L2DELTA_CAPTURE_SYMBOLS`, без
  contract-пакета, без бампа `SCHEMA_VERSION`, без RISK-BLOCK** (для M-45-реализации MD-only
  carve-out подтверждает reviewer M-45 отдельно). Механизм эпох `epoch_id` существует
  end-to-end; `DET-I-1..3` (M-51) смешанным журналом не ломаются.

**История круга 1 (для аудита):** ветка `docs/ct-rfc-06-l2delta` @ `22715b7`,
вердикт `research/reviews/R-019-ct-rfc-06-l2delta.md` — **CHANGES REQUESTED**.
- **Содержательно документ ПРАВ, и это подтверждено дважды независимо** (critic `C-045` + reviewer
  своим грепом): посылка мандата «нужен contract-RFC на НОВЫЙ вариант» **опровергнута** — вариант
  `MdPayload::L2Delta` (дискр. 6) уже в T1 с `CT-RFC-04`/M-18 (`lib.rs:293`, merge `f635bd2`);
  вводить нечего ⇒ **M-45 сводится к расширению allow-list `L2DELTA_CAPTURE_SYMBOLS`, без
  contract-пакета `05-contract-layer.md` §4, без бампа `SCHEMA_VERSION` и без RISK-BLOCK.**
  Карта exhaustive-`match` — **ПЯТЬ** мест, не три (см. TD-068); механизм эпох `epoch_id`
  существует end-to-end (T1-поле `SegmentHeader` → reuse-условие `decide_open_segment` → env
  `EPOCH_ID` в recorder); `DET-I-1..3` (M-51, `d896b98`) смешанным журналом НЕ ломаются —
  смешанный вход уже под оракулом `book/tests/red_det_projection.rs`.
- **Блокирует не содержание, а пруф-база:** документ не проходит `verify_design_claims.sh`
  (5 нарушений, exit=1, в т.ч. на `--merge-preview origin/main`). Два из них — TD-069: §0.2/§6/§8.2
  стоят на артефактах `research/measurements/**`, которых в `main` нет (живут на ветках); ещё
  три — усечённый путь `docs/07` и те же два пути. Плюс NOTE F4–F6 (что механизм эпох НЕ решает;
  `JR-I-10` вакуумен без определения «читаемого хранилища»; у оракула DET-I-1 фикстур с `L2Delta` нет).
- **Дальше:** architect правит (объём — один коммит), повторный прогон гейта на merge-цели;
  повторный critic не требуется — содержательная часть проверена дважды.

## Data durability (M-08 «сбор не останавливается» + CT-RFC-02 — MERGED + В ПРОДЕ 2026-07-14, reviewer APPROVED; **milestone НЕ закрыт: цель E7/E3 не достигнута**)
Прод: `b7721d1` (merge `1123b13` + фикс TD-018). CI+Deploy success; **§8 eyes-on ВЫПОЛНЕН**
(4.2 ч наблюдения). Прод здоров: `restarts=0`, `panic/ERROR=0`, `backstop=0`, heartbeat свежий.
**Что подтверждено на боевых данных:** старый сегмент 15 188 347 171 B **заморожен** (mtime =
момент деплоя, байт-в-байт цел) → пишется НОВЫЙ `segment-00000001.jrnl` с магией `HFTJRN02`;
`seq` непрерывен через границу (legacy `max=16049333` → new `min=16049334`, `seq_gaps=0`);
**РОТАЦИЯ ПОДТВЕРЖДЕНА ВЖИВУЮ** (13:10 UTC): `segment-00000001` закрылся на 1 073 741 818 B
(порог 1 GiB) → создан `segment-00000002` с магией; `seq` сшит через границу
(`17800473` → `17800474`, `seq_gaps=0`), `restarts=0`, healthy, полосы стабильны;
`declare_legacy` выполнен (`sha256:db1ef99e…`, size зафиксирован), и **fail-closed доказан**:
без манифеста `stream` отдаёт `foreign segment (no magic, no declaration)`, при этом **запись не
прерывается** (T7c в проде); полосы OBI на прогретой книге НЕ деградировали (`avg buckets
1154/969` vs baseline `1316/1452`; полоса 600–6000 bps `1115/873` vs `975/845`).
**Чего milestone НЕ дал (открыто):** ретеншен никем не вызывается (TD-020 — «сбор не остановится
никогда» НЕ достигнуто, ~40 дней до disk-guard); эвикция книги не удерживает рост (TD-016 остаётся
OPEN: уровни 5k → 13.8k за 4 ч, окно ±60% ничего не режет); `storage_status` не публикуется в
heartbeat (TD-019). Отдельно: метрика памяти, по которой TD-016 был заведён, оказалась
загрязнена page cache — настоящий рост кучи +1 MiB/час, не +8 (TD-021).

### rev 6 (задачи 11/12/13) — КОД MERGED + В ПРОДЕ (`8882c1e`, reviewer APPROVED 2026-07-14); **milestone ВСЁ ЕЩЁ НЕ ЗАКРЫТ: ГЛАВНАЯ цель (TD-020) не достигнута**
Цепочка: architect RED (`4475bfa`, `6f1b7f4`) → engine-dev (`8b4dc6f` task 11, `24d8e83` task 12) →
tester PASS → reviewer. Гейты (перепрогнаны reviewer'ом независимо на чистом worktree):
workspace **172 passed / 0 failed**; `verify_M-08.sh` **28/28 PASS, exit=0**; fmt/clippy clean;
CI + Deploy на merge-коммите — success. **Анти-плацебо доказан reviewer'ом независимо:** все 7
оракулов `red_retention_operator` (R1–R7) + `red_heartbeat_status` **FAIL против пред-фиксного
дерева `4475bfa`** (`not yet implemented` в `retention_plan`; heartbeat не JSON), GREEN на HEAD.
- `crates/journal` (**task 11, TD-020**) — `retention_plan(dir, policy, now_wall_ms)` /
  `retention_execute(...)` + **бинарь `journal-retention`** (`src/bin/`). Часы СНАРУЖИ (план
  детерминирован, `DET-I-1`-дисциплина); **`DryRun` — дефолт CLI** (конструктивный барьер против
  «случайно удалил»); Apply идёт ТОЛЬКО через `verify_cold_copy` → `ColdCopyProof` → `prune_segment`
  (сверка sha256 холодной копии; сбой сверки → сегмент остаётся ГОРЯЧИМ и попадает в `failed`,
  exit=2). Активный сегмент никогда не в плане; `keep_min_segments` защищает последние N;
  НЕЗАДЕКЛАРИРОВАННЫЙ legacy не удаляется (нет эпохи → нет права); `disk_pressure` при пустом плане
  поднимает флаг (exit=3), а не молчит. Оракулы содержат деградированные входы (недоступное
  холодное хранилище, чужой сегмент, пустой план) — per `.claude/rules/testing.md`.
- `crates/recorder` (**task 12, TD-019**) — heartbeat = JSON `{ts_wall_ms, next_seq, segment_index,
  events, free_bytes, min_free_bytes, writable}` вместо 13 байт таймстампа; финальный heartbeat при
  выходе. В журнал НЕ пишется (детерминизм). Healthcheck compose'а смотрит на **mtime** файла, не на
  содержимое → смена формата прод-безопасна (проверено: контейнер healthy после деплоя).
- `crates/venue-binance` (**task 13, TD-016 переспека после TD-021**) — `BACKSTOP_LEVELS_PER_SIDE`
  50k → **200k**: приоритет развёрнут (точность данных > экономия памяти), т.к. «лик» был измерен
  загрязнённой page-cache метрикой, а эвикция резала уровни внутри полос OBI 6–60 %. Кап остаётся
  ТОЛЬКО аварийным потолком от OOM.
- **§8 eyes-on (прод `8882c1e`, 2026-07-14) — GREEN по деплоенной части:** контейнер healthy,
  `restarts=0`, `panic/ERROR/backstop = 0`; **боевой legacy-сегмент цел БАЙТ-В-БАЙТ** — полный
  sha256 15 188 347 171 B до и после деплоя совпал (`234583c8e5c0…`), mtime заморожен (08:47);
  recorder продолжает писать в `segment-00000002.jrnl` (магия `HFTJRN02`, растёт 437 → 528 MB);
  **heartbeat несёт состояние** (`writable=true`, `free_bytes=119 134 494 720`,
  `min_free_bytes=10 737 418 240`, `next_seq=18 733 828`, `segment_index=2`) ⇒ **TD-019 CLOSED**;
  `RssAnon = 11 376 kB` (правильная метрика per TD-021), `book levels` ≈ 5000/сторона после
  рестарта — baseline для наблюдения асимптоты (задача 13).
- **БЛОКЕР close-out'а (найден reviewer'ом на PR-гейте, подтверждён на проде): TD-020 НЕ ЗАКРЫТ —
  бинарь `journal-retention` НЕ ДОСТАВЛЯЕТСЯ В ПРОД.** `Dockerfile` собирает `cargo build --release
  **--bin recorder**` и копирует в runtime-образ ТОЛЬКО `recorder` (факт на проде:
  `docker exec hft-recorder ls /usr/local/bin/` → один `recorder`); на VPS нет Rust toolchain;
  холодное хранилище не смонтировано (`/mnt/*` пуст, Storage Box не заведён); cron отсутствует
  (`/etc/cron.d/` → только `e2scrub_all`). ⇒ §8-пункты «dry-run ретеншена на проде» и «cron»
  **физически невыполнимы**, ретеншен по-прежнему **никем не вызывается**. Это тот же класс дефекта,
  что и исходный TD-020, этажом выше: раньше была библиотека без оператора — теперь оператор без
  доставки. Диск: 111 GB свободно, ~2.8 GB/сут ⇒ таймер ~40 дней тикает. Нужна **задача 14**
  (доставка: сборка `journal-retention` в образ/на хост + монтирование холодного хранилища +
  cron + алерт на exit≠0) — спека architect, impl engine-dev.
- **M-08 остаётся 🚧 IN_PROGRESS.** Закрывается ТОЛЬКО после задачи 14 + §8 с реальным dry-run
  ретеншена на проде.

### rev 9 (задачи 15/16) — REVIEWER APPROVED (код) → §8 PROD REJECTED + REVERTED (`82b33db`, 2026-07-14)
Стек rev9 (`cb46e34` RED C7-C9+D7 / `9cf5acf` task 15 crash-window self-heal / `1ff1b55` task 16
оператор компакции) закрыл ОБА rev8-блокера reviewer'а и подтверждён фактом:
- **D-COMP-1** (дубликаты в прод-пути): `segments()` теперь дедуплицирует raw+.zst через общий
  `dedup_indexed_paths` (raw побеждает при коллизии). Репро крах-окна: было 3172 события → 3000.
- **D-COMP-2** (self-heal): ветка `dst.exists()` сверяет sha256 распакованного `.zst` с оригиналом;
  совпало → доделать (удалить оригинал), битый `.zst` → удалить `.zst`, оригинал ГОРЯЧИЙ, `Err`.
- **D-COMP-3** (оператор): `--mode compact` у `journal-retention` + compose-сервис + cron + гейт D7
  (реальный запуск бинаря, не греп).

Локальные гейты на **merge-коммите** (не только feat): `fmt` 0, `clippy -D warnings` 0,
`cargo test --workspace` **181/0**, `verify_M-08` PASS, `verify_delivery` PASS (вкл. D5a+D7),
`crontab -n` 0. Анти-плацебо: C7/C8/C9 FAIL против `cb46e34`, GREEN на HEAD; наивная C5-мутация
"распаковать в RAM" валит C5 (100.7 MB пик). Merge `2b2311f` запушен в main.

**CI-флак (не блокер merge, но задержал):** первый CI на `2b2311f` — RED, exit 101 на
`td016_memory_bounded_when_price_drifts_out_of_band` (**НЕ** тест компакции; глобальный
аллокатор-счётчик, флак под параллельным `cargo test --all`). Re-run того же коммита — GREEN
(флак подтверждён). Заведён **TD-023**. Deploy re-run → success, компакция доехала до VPS.

**§8 PROD RED — CRITICAL data-loss дефект (доказан фактом, prod НЕ тронут):** eyes-on на VPS
показал, что `segment-00000000.jrnl` (15 GB) — **LEGACY** (магия `0c 00…`, не `HFTJRN02`;
задекларирован в `journal.legacy.json`). Оператор `--mode compact` жмёт СТАРЕЙШИЕ закрытые первыми
⇒ выбрал бы legacy-0. `compact_segment` его сжимает (sha сырых == sha распакованных → верификация
проходит → **оригинал удаляется**), но обратное чтение `.zst` требует v2-магии
(`skip_v2_header_forward`) → `CorruptHeader` → `list_segments`/`stream` падают ⇒ **ВЕСЬ ЖУРНАЛ
НЕЧИТАЕМ, 15 GB невосполнимой истории стёрты.** Воспроизведено в песочнице (legacy-0+v2-1+v2-2 →
`compact_closed_segments(keep_raw=1)` → `list_segments`/`stream` = `corrupt SegmentHeader`);
**реальную компакцию на prod-каталоге НЕ запускал** (prod цел: 5 сырых сегментов, cron НЕ
установлен). По правилу §8 «красный/опасный прод → revert» весь стек rev9 откатан `82b33db`.
См. **TD-022 rev9** (виток: компакция ОБЯЗАНА не трогать legacy; RED-набор обязан включать
legacy-сегмент — C1-C9 строят только v2, прод-раскладка не покрыта) + **TD-023** (флак-оракул).
**M-08 остаётся IN_PROGRESS; TD-020, TD-006, TD-022 остаются OPEN; TD-023 новый.**

### rev 10 (задачи 17/18 — legacy-безопасность компакции) — REVIEWER APPROVED + MERGED (`8a2e377`, §8 PROD GREEN, 2026-07-15)
Реленд rev9-стека + фикс CRITICAL data-loss (TD-022). Ветка `feat/M-08-compaction-reland` (5 коммитов,
линейна, fast-forward): `4d92373` (**чистый revert-of-revert** `82b33db` — восстановил rev9-стек 1:1;
reviewer сверил `tree(4d92373)==tree(2b2311f)` побайтово — architect НЕ дописывал impl, механическое
восстановление уже-ревьюненного) → `7754308` C10 RED (architect) → `0c7bef4` TD-023 fix (architect) →
`0cd4eca` **D-COMP-4** (engine-dev) → `8a2e377` §8-план (architect).
- **D-COMP-4** (`crates/journal/src/segments.rs`): `compact_segment` возвращает `Err` на сегменте, чьи
  первые байты `!= SEGMENT_MAGIC` (`HFTJRN02`), **ДО любой мутации** (конструктивный барьер — тот же
  принцип, что «активный не сжимаем»); `compact_closed_segments` тихо пропускает legacy/foreign (Err по
  маркеру → в `failed`, не пробрасывает). Legacy читается как есть; сжатие legacy архитектурно запрещено.
- **C10** (sacred RED, architect): прод-раскладка legacy-0(declared, no-magic) + v2-закрытые + активный;
  реальная `compact_closed_segments(keep_raw=1)` → legacy НЕ тронут, `.zst` для legacy НЕ создан,
  `stream` до==после. Закрывает дефект фикстуры C1-C9 (строили только v2 — прод-раскладка не покрыта).
- **Гейты (reviewer перепрогнал независимо на чистом worktree):** fmt/clippy clean, **workspace 182/0**,
  `red_compaction` **10/10** (C1-C10), `red_book_bounded` 7/7, `verify_M-08` PASS, `verify_delivery` PASS
  (D1-D7 + **deep** D1-deep/D2-deep, реальный образ). **Анти-плацебо доказан независимо:** C10 FAIL
  против `7754308` (без барьера — «legacy стёрты», `red_compaction.rs:562`), GREEN на HEAD. CI+Deploy
  на merge success.
- **§8 (два шага; `--mode compact --dry-run` НЕ существует — режимы взаимоисключающи):**
  - **Step A (delivered binary на sandbox):** образ `hft-platform-recorder:local` на faithful прод-
    раскладке → legacy байт-в-байт цел, legacy `.zst` НЕ создан, 32 v2 сжаты (14.5×), `stream`=3500
    до и после (потерь нет). Барьер доказан в ДОСТАВЛЕННОМ артефакте до касания боевого legacy.
  - **Step B (РЕАЛЬНАЯ компакция боевого `/journal`):** через доставленный cron-скрипт (exit=0, alert
    не взведён). **Боевой legacy-0 БАЙТ-В-БАЙТ ЦЕЛ** — полный sha256 `234583c8…bdbdc72` == эталон,
    size=15188347171, mtime=1784018822 не изменились (D-COMP-4 сработал на живом 15 GB legacy);
    сегменты 1-5 → `.jrnl.zst`, `zstd -t` каждого = исходный raw-размер (данные целы); **свободно
    111.20 → 115.88 GB (+4.69 GB) — диск ДВИНУЛСЯ**; recorder healthy, restarts=0, next_seq растёт,
    heartbeat свежий (конкурентная компакция закрытых не задела живого писателя).
- **⇒ TD-022 CLOSED** (legacy-безопасность доказана на РЕАЛЬНОМ активе), **TD-023 CLOSED** (флак устранён).
- **§8 ПОЙМАЛ новый delivery-дефект → TD-024 (MAJOR, OPEN):** compose-сервисы `journal-compaction`/
  `journal-retention` держат `command:` в equals-form (`--dir=/journal`), а бинарь `=`-форму НЕ
  разбирает → задокументированные `docker compose run --rm journal-<svc>` СЛОМАНЫ; работает только
  точный cron-argv (раздельная форма), через который reviewer и выполнил §8-B. `verify_delivery`
  гонял только cron-argv, не `command:`-блок против живого бинаря.
- **M-08 всё ещё IN_PROGRESS (НЕ закрыт):** cron НЕ установлен на проде (компакция разовая-ручная →
  для durable-сдвига дедлайна нужна установка + фикс TD-024); ретеншен (`--mode apply`, cold-выгрузка)
  не запускался — нет Storage Box (founder ★); TD-016 наблюдение и TD-006/TD-020 остаются OPEN.

### rev 12 (задача 20 — активация cron, хвост 1) — REVIEWER APPROVED + MERGED (`d3e7db2`, §8 CRON АКТИВЕН, 2026-07-15)
Durable-компакция: cron активирован на проде + позитивный heartbeat (silent-absence детектируется).
Ветка `feat/M-08-cron-activation` (2 коммита, ff): `eb0e6cc` architect (README модель активации +
мониторинг + гейт **D9 RED** + milestone) → `d3e7db2` engine-dev (cron-скрипты пишут `*.last-success`).
- **Positive heartbeat (D9):** оба cron-скрипта на УСПЕШНОМ прогоне пишут `*.last-success` (UTC).
  `*.alert` ловит «прогон УПАЛ», `*.last-success` freshness ловит «cron НЕ запускался» (не установлен/
  crond мёртв/ребут) — РАЗНЫЕ классы отказа, нужны ОБА (урок сессии: и сбой, и МОЛЧАНИЕ видимы). D9-гейт
  прогоняет скрипт со стабом-успехом, проверяет запись маркера — не грепом.
- **Модель активации (governance):** артефакты доставляются через репо/образ, но `install /etc/cron.d`
  — ОСОЗНАННЫЙ РУЧНОЙ шаг с founder-★ (не авто-`deploy.yml`: цена ошибки на автомате с data-модифи-
  цирующим расписанием выше). Retention остаётся `--mode=dry-run` (apply — после Storage Box).
- **Гейты (reviewer независимо):** fmt/clippy clean, verify_delivery PASS (D8+**D9** обоих сервисов),
  crontab -n 0. **CI на `d3e7db2` GREEN** (adequate-disk runner). RED-first: D9-гейт при `eb0e6cc` есть,
  `*.last-success` в скриптах — только с `d3e7db2`.
  ⚠ **`verify_M-08.sh` FAIL ЛОКАЛЬНО** на `red_prod_migration` (`error: StorageGuard`) — pre-existing
  **env-флейк**: тест берёт `WriterConfig::own_capture` (min_free=10 GiB), а локальный диск 8.9 GiB/98%.
  НЕ логика, НЕ эта ветка (crates/journal не тронут): **CI на adequate-disk = GREEN**. Заведён **TD-026**
  (перенумерован из TD-025 2026-07-19 — коллизия с recon-flood TD-025) (architect: min_free_bytes:0 в
  тесте как у соседних фикстур ИЛИ требование к test-env; блокирует tester task 7 на full-disk чекауте).
- **§8 CRON АКТИВАЦИЯ на VPS (founder-★ авторизация relayed через диспетч):** deploy НЕ триггерился
  (deploy-only пути), поэтому reviewer обновил чекаут VPS до `d3e7db2` (скрипты несут `.last-success`),
  установил `/etc/cron.d/hft-journal-retention` (компакция `50 3`, ретеншен dry-run `7 4`) + `/var/{log,
  lib}/hft`, restart cron. **Eyes-on АВТО-прогона** (temp every-minute schedule): cron сам отработал →
  **свежий `compaction.last-success` 2026-07-15T18:56:02Z** (heartbeat пишется), alert не взведён,
  **legacy-0 байт-в-байт цел** (`234583c8…`), recorder healthy restarts=0; прогон компактил 0 (keep_raw=2
  берёг единственные 2 закрытых, legacy skipped — штатно). Real-schedule восстановлен. Disk-moving
  компакция через ЭТОТ код-путь доказана §8-B rev10/rev11 дважды (+4.69, +1.94 GB); recurring 03:50
  сожмёт по мере накопления. ⇒ **хвост 1 закрыт; TD-024 CLOSED; TD-006/TD-020 durable-замедлены.**
- **M-08 всё ещё IN_PROGRESS:** хвост 2 — Storage Box + retention apply (founder ★). После него:
  tester clean-checkout verify (см. TD-026 про disk) → architect close-out → reviewer финальный §8.

### rev 11 (задача 19 — TD-024 equals-form CLI) — REVIEWER APPROVED + MERGED (`e31e23e`, §8 PROD GREEN, 2026-07-15)
Фикс delivery-дефекта, пойманного §8 rev10: операторский путь через `docker compose run` был сломан.
Ветка `feat/M-08-td024` (3 коммита, fast-forward): `475bbd5` architect RED `red_cli_argv.rs` (гоняет
НАСТОЯЩИЙ бинарь: equals dry-run/compact + регресс раздельной) + гейт **D8** → `935bc9b` engine-dev
фикс парсера → `e31e23e` engine-dev README §4.
- **Фикс (`crates/journal/src/bin/journal-retention.rs`):** нормализация argv ДО цикла разбора —
  `--flag=value` → `split_once('=')` → `[--flag, value]` (equals-форма из compose `command:` и `--help`
  теперь понимается); раздельная форма (cron) проходит без изменений — регрессии нет.
- **D8** (`verify_delivery`): извлекает `command:`-блок ОБОИХ сервисов из `docker-compose.yml`, гонит
  реальный бинарь ровно этой формой argv → закрывает слепое пятно D5a/D7 (гоняли только cron-argv).
- **Гейты (reviewer независимо):** fmt/clippy clean, workspace **185/0**, `red_cli_argv` 3/3,
  `red_compaction` 10/10, verify_M-08 PASS, verify_delivery PASS (D8 обоих сервисов), crontab -n 0.
  **Анти-плацебо:** оба equals-теста FAIL против `475bbd5` (без фикса), раздельный проходит; GREEN на HEAD.
- **§8 PROD (VPS `e31e23e`, CI+Deploy success):** `docker compose --profile ops run --rm
  journal-compaction` (equals-form команда, падала «неизвестный флаг» до фикса) → **exit=0**, сжаты
  сегменты 6,7 (10.43×), **диск +1.94 GB**; `... journal-retention` (dry-run) → **exit=0**, 0 prune,
  legacy/active/young skipped, disk_pressure нет; **boевой legacy-0 БАЙТ-В-БАЙТ ЦЕЛ** (sha256
  `234583c8…bdbdc72`), recorder healthy, restarts=0. ⇒ **TD-024 CLOSED**; операторский compose-путь
  работает end-to-end.
- **M-08 всё ещё IN_PROGRESS:** остаётся установка cron (durable-компакция) + Storage Box (ретеншен
  apply, founder ★); TD-006/TD-020 OPEN до этого.

### rev 7/8 (задачи 14/15) — REVIEWER REJECTED + REVERTED (`b43044d`, 2026-07-14)
Стек `d43d923..91f11aa` (task 14 delivery + task 15 compaction + D5/C5 fixes) прошёл локальные
reviewer-гейты: `fmt`, `clippy -D warnings`, workspace **178 passed / 0 failed**,
`verify_M-08.sh` PASS, `verify_delivery_M-08.sh` PASS, deep delivery PASS. Анти-плацебо:
старый cron из `e4f23d1` валит новый D5 (`bad minute`, exit=1); C1-C6 валятся на `76aadb2`;
наивная C5-мутация "распаковать .zst в `Vec<u8>`" валится на ~100 MB пика.

**§8 PROD RED:** после merge/push `91f11aa` CI и Deploy были зелёные, VPS был healthy
(`restarts=0`, heartbeat свежий, recorder писал), но реальное задание
`/root/hft-platform/deploy/bin/journal-retention-cron.sh` упало ДО плана:
`journal-retention: неизвестный флаг --dir=/journal`. Причина: cron/compose передают
`--flag=value`, а CLI `journal-retention` парсит только пару `--flag value`. Это тот же класс
TD-020: артефакт в образе/cron существует, но операторский путь не отрабатывает на боевом
каталоге. Установленный для проверки cron-артефакт и alert-marker удалены.

По правилу §8 "красный прод → revert" стек откатан одним коммитом `b43044d`; rollback CI
`29359107762` и Deploy `29359107734` GREEN. Прод после отката: `/root/hft-platform` HEAD
`b43044d`, `hft-recorder` healthy/restarts=0, heartbeat свежий, активный `segment-00000003.jrnl`
растёт, `/etc/cron.d/hft-journal-retention` отсутствует. **M-08 остаётся IN_PROGRESS; TD-020,
TD-006 и TD-022 остаются OPEN.**

- `crates/contracts` (**CT-RFC-02**, atomic RFC `docs/rfc/CT-RFC-02-journal-provenance.md`) —
  `SCHEMA_VERSION` 1→2; provenance живёт в ЗАГОЛОВКЕ СЕГМЕНТА, не в `Event` (при 2.8 GB/сут тег в
  каждом событии = гигабайты мусора): `SegmentHeader{schema_version, source, provenance, epoch_id,
  created_wall_ms, first_seq}`, `DataSource{OwnCapture,Vendor,Synthetic}`, `LegacySegmentDecl`/
  `LegacyManifest`, `SEGMENT_MAGIC = HFTJRN02`. **`Event`/`EventKind` НЕ изменены** (аддитивно,
  старые журналы читаются навсегда — CT-I-3; в дифе только `derive(JsonSchema)`). Пакет полный:
  типы + JSON Schema **сгенерированная** (`examples/gen_schema.rs`, сверяется с типами тестом
  `red_schema`) + фикстуры valid/invalid + `CHANGELOG.md`. Классификация сегментов **fail-closed**
  (находка critic C-005 C2): магия есть → заголовок ОБЯЗАН разобраться; магии нет → сегмент
  читается ТОЛЬКО по ЯВНОЙ декларации в `journal.legacy.json` со сверкой отпечатка (sha256 первого
  MiB + размер). Прежнее «не разобрался → считаем OwnCapture» было fail-open — чужие данные
  получали наше происхождение.
- `crates/journal` — **ротация** (`segment-NNNNNNNN.jrnl`, 1 GiB, `seq` сквозной через границы,
  заголовок в каждом сегменте); **`stream(dir, EpochFilter)`** — bounded-memory итератор (прод-путь
  research; RED на 16/64 MiB с counting-allocator, пик < 8 MiB — на 15 GB `Vec<Event>` не влезет
  никогда, класс TD-011 этажом выше); **`EpochFilter`** (дефолт `OwnCaptureOnly` — вендор/синтетика
  в обучение по умолчанию НЕ попадают); **retention**: `prune_segment` требует `ColdCopyProof` с
  приватным конструктором → «удалить невыгруженный сегмент» невозможно ВЫРАЗИТЬ (типовой барьер,
  `compile_fail`-доктест), битая холодная копия → proof не выдан; **disk-guard fail-closed**
  (свободно < `min_free_bytes` → `append` → `Err`, ни байта и ни одного `seq`; `storage_status()
  .writable=false` в heartbeat; `Sys`-событие в журнал НЕ пишется — писать в журнал в момент,
  когда запись запрещена, самопротиворечиво).
- **Задача 10 (МИНА, поймана architect'ом при разборе SVR):** `read_all`/`recover` были
  захардкожены на `segment-00000000.jrnl` и парсили магию v2 как len-поле → на новом журнале
  **молча вернули бы 0 событий**, а их зовут `book/examples/{bands,obi_probe}.rs` — вся диагностика
  полос OBI. Исправлено: обход ВСЕХ сегментов, понимание v2 + legacy wire-формата. Остаются
  ОФЛАЙН-диагностикой с мягкой классификацией; барьер **T11e** (verify) запрещает звать их из
  любых `crates/*/src` кроме `journal` — прод и research ходят ТОЛЬКО через `stream` с явным
  `EpochFilter`. Reviewer проверил фактически: новый `recover` читает боевой legacy-хвост
  (14 119 событий из 40 MiB хвоста прод-сегмента).
- `crates/recorder` — пишет заголовок сегмента (provenance = версия recorder'а + git sha, эпоха
  `own-YYYY-MM`), переживает ротацию без потери/дублей `seq`. **Прод-миграция под тестом (T7c)**:
  recorder СТАРТУЕТ на каталоге с НЕзадекларированным боевым сегментом (запись не зависит от
  декларации — деплой не может остановить сбор), пишет в НОВЫЙ `segment-00000001.jrnl`, старый
  сегмент байт-в-байт нетронут (дописывать в безголовый запрещено).
- `crates/research-cli` — чтение переведено на `journal::stream` + `EpochFilter` (грид больше НЕ
  держит `Vec<Event>`); RED требует **ЭКВИВАЛЕНТНОСТИ** стрим-грида и in-memory (PnL до цента,
  интенты, филлы) — «оптимизация» не смеет тихо изменить измеряемую логику (урок M-07); gap-статистика
  (`data_quality`) → `research/data-quality/gaps-<epoch>.json`, отчёт обязан на неё ссылаться.
- `crates/venue-binance` (**TD-016, задачи 9/9b**) — эвикция уровней книги. **v1 отреджекчена
  reviewer'ом на PR-гейте** (кап 5000 + side-filter по mid ДИФФА стирал живые уровни, включая best
  bid, на асимметричном диффе → тихая порча `L2Snapshot` при зелёных RSS/health). **v2 (`421d5b6`)**:
  уровень удаляет ТОЛЬКО `size==0`; эвикция — по расстоянию от mid КНИГИ за пределами окна ЭМИССИИ
  (`MAX_REL_DIST` ±60%) ⇒ режется ровно то, что никогда не эмитится; `BACKSTOP_LEVELS_PER_SIDE
  = 50_000` от OOM (+`tracing::warn`); наблюдаемость D (`book levels` ≥1/мин) — чтобы §8 мерил
  УРОВНИ, а не только RSS. Reviewer доказал анти-плацебо независимо: 2 новых оракула FAIL против
  v1-impl, GREEN против v2. **Атрибуция лика к книге на проде НЕ доказана** — §8 покажет.
- `.github/workflows/deploy.yml` — Deploy гейтится на CI (fail-closed) + `set -euo pipefail` в
  ssh-скрипте (раньше упавший `git fetch/reset` не останавливал сборку → фантомный деплой).
  **Гейт РАБОТАЕТ, доказан сквозным прогоном** (TD-017 + TD-018 CLOSED): run @`1123b13` → Deploy
  FAILURE (гейт не пустил, 403 на чтении статуса CI), после `permissions: actions:read`
  (`b7721d1`) → CI success → Deploy success. Deploy при красном CI более невозможен.
- Гейты (reviewer перепрогнал независимо): workspace **164 passed / 0 failed**; `verify_M-08.sh`
  **26/26 PASS, exit=0**; fmt/clippy clean; CI на merge-коммите success.
- **Урок (зафиксирован architect'ом в процессе, `5fabd2b`):** два milestone'а подряд дефект прошёл
  ВСЕ зелёные оракулы и был пойман reviewer'ом на PR-гейте — оба раза причина одна: **фикстура
  «счастливого пути»** (M-07 — событие с одним филлом; M-08 — симметричный дифф). Оракул границы
  ресурса обязан иметь и деградированный/асимметричный вход.

## Data safety net (M-09 «система сама сообщает о тихой деградации», P2.5 — 🚧 ACTIVE; task 1 CT-RFC-03 MERGED + В ПРОДЕ inert-safe, reviewer Block-C APPROVED 2026-07-16)
Milestone открыт (план принят: critic C-007 APPROVE → reviewer → founder ★ P2.5). **Задача 1
(CT-RFC-03, T1, БЛОКИРУЮЩАЯ) — смержена в прод**, merge `cf53e81` (`--no-ff` feat/M-09 → main).
Цепочка: architect (`64c0a9e`) → critic C-008 APPROVE (`367c2de`) → reviewer Block-C APPROVED.
- `crates/contracts` (**CT-RFC-03**, atomic RFC `docs/rfc/CT-RFC-03-recon-audit.md`) — аддитивный
  вариант `SysEvent::ReconDivergence(ReconAudit)` (postcard-дискриминант **3** строго в хвост;
  Heartbeat/ConnUp/ConnDown=0/1/2 неизменны) + `ReconAudit{venue,symbol,divergence_bps,
  best_price_diverged,action}` + `ReconAction{AlertOnly,Resynced}`. Durable-след recon-расхождения
  в ЖУРНАЛЕ (не лог/метрика, OPS-I-6): офлайн отвечает «каким участкам данных верить».
  `best_price_diverged` отделяет порчу лучшей цены (ε_test / C1-класс, эвикция стирала best bid)
  от шума дальних полос. Поля struct ФИКСИРОВАНЫ — расширение только новым RFC (postcard
  struct-append не аддитивен для старых записей). `schema_version` НЕ бампится (остаётся 2):
  аддитивный `EventKind`, не изменение формата сегмента (прецедент CT-RFC-01); старые журналы
  читаются байт-в-байт (CT-I-3). Пакет полный: типы + JSON Schema **сгенерирована**
  (`gen_schema`, гейт `red_schema` CT-I-4) + фикстуры valid/invalid + CHANGELOG + RED `red_rfc03.rs`.
  `Event`/`EventKind`/`MdEvent`/`Venue`/`SegmentHeader`/старые `SysEvent`-варианты НЕ изменены.
- **Contract Block-C (reviewer, независимая верификация на чистом worktree):** scope — диф только
  `crates/contracts/**` внутри атомарного RFC-коммита + критик-вердикт C-008; не-RFC T1-правок нет;
  Cargo.toml/Cargo.lock/order-path/risk/oms/venue не тронуты. CT-I-1 канарейка: `EventKind`/
  `SysEvent`/`ReconAudit`/`ReconAction` определены ровно в одном крейте. CT-I-4: регенерация схемы →
  нулевой diff. **Анти-плацебо доказан reviewer'ом независимо:** удаление варианта → compile-RED
  (exit 101); восстановление → GREEN 6/6. Гейты: `red_rfc03` 6/6, `red_schema` 2/2, contracts all
  GREEN, **workspace 191/0** (аддитивность не сломала потребителей), fmt/clippy clean.
- **MD-only observability — risk-critic НЕ требуется** (gates §5 carve-out): вариант — чистое
  аудит-событие, НЕ несёт risk/order/money-формы (нет `RiskApproved`/`Order`/`Ctl`/`Decision`),
  нет order-egress; consistent с founder-★ M-09 планом и прецедентом CT-RFC-01/02 (contract-RFC
  гейтится critic'ом, risk-critic — когда контракт трогает safety/деньги-путь). Reviewer подтвердил
  отсутствие order-пути в дифе.
- **§8 eyes-on (прод `cf53e81`, 2026-07-16) — GREEN, инертно:** CI success + Deploy success (гейт
  `deploy: needs: ci` отработал). VPS git sha = `cf53e81`; recorder healthy, **restarts=0**
  (started 10:01:59Z), `panic/ERROR/backstop = 0`; heartbeat свежий (~2с) и несёт состояние
  (`writable=true`, `free_bytes=111 GB`, `next_seq=30 627 037`, `segment_index=14`); ротация идёт
  (`segment-00000014.jrnl` растёт, `seq` сквозной); 3 venue подключены (binance/hyperliquid/
  binance_futures); `RssAnon = 11 828 kB` (норма per TD-021, ср. M-08 baseline 11 376 kB).
  **`ReconDivergence` в журнал НЕ пишется** — эмиттера ещё нет (recon runtime = task 2), поведение
  recorder тождественно до-деплойному. book levels baseline после рестарта: BTC `5030/5027`,
  ETH `5033/5028` (точка отсчёта для наблюдения TD-016).
- **Разблокирует task 2** (recon runtime OPS-I-1..9). **M-09 остаётся 🚧 ACTIVE** — далее architect:
  RED-оракулы OPS-I-1..9 + `scripts/verify_M-09.sh` (task 2+).

### task 2 (recon runtime) — КОД MERGED (`b1adec0`) → §8 PROD FAILED → **дефект A (seed-gate) ИСПРАВЛЕН (`e9fc258`) + дефект B ЗАКРЫТ B2 (`4939d8f`, §8 PROD GREEN, reviewer APPROVED 2026-07-18)**; **Task 2 функционально ЗАКРЫТ (best-only+seed-gate); M-09 остаётся 🚧 ACTIVE (tasks 3/4/5/6)**
Цепочка (25 коммитов): venue REST-fetcher (Binance spot+futures, TD-013 structural) + `crates/ops`
(recon/budget/metrics/silence) + windowed-персистентность (§4.3) + depth-skip pin (Concern-1
reviewer → architect `5c621f5` → critic `3483a04`) + books-feeder (`apply_md_to_books` +
per-venue MD fanout-tap, `9db808c`). Reviewer APPROVED по коду; merge `b1adec0`.
- **Юнит-гейты ЗЕЛЁНЫЕ (не поймали прод):** ops 33/33, recorder 10/10 (`red_recon_wiring` 2/2,
  `red_recon_loop` 1/1), **workspace 256/0**, clippy 0, `verify_M-09.sh` VERDICT PASS.
  Анти-плацебо depth-skip подтверждён reviewer'ом (`sed 's/ref_reach<band/false/'` валит
  `unreachable_band_is_skipped_not_flooded` + `deep_local_vs_truncated`). MD-only → risk-critic N/A.
- **§8 EYES-ON ПРОВАЛ (reviewer, прод `b1adec0`, 2026-07-18):** контейнер healthy, restarts=0,
  sha=`b1adec0`, heartbeat свежий — но recon **ФЛУДИТ** `Sys(ReconDivergence)` на здоровом рынке.
  Замер журнала (декодер по `journal::read_all` активного сегмента, все post-deploy Sys):
  - **4 стартовых** (`best_diverged=true div_bps=10000 Resynced`, по 1 на venue×symbol) — сравнение
    с ПУСТОЙ local-книгой до первого L2Snapshot (fetcher делает ПЕРВЫЙ fetch немедленно на старте,
    до наполнения feeder'ом). Транзиент на КАЖДЫЙ рестарт → пишет ложную «порчу+ресинк» в durable-
    журнал (мелкий, но реальный дефект — TD ниже).
  - **12+ оконных** (`best_diverged=false div_bps=41..1129 Resynced`) по ВСЕМ 4 символам, ~1/мин —
    оконное знаковое среднее near-touch ОБЪЁМА НЕ сходится к 0 на живом рынке: остаточный churn
    41..1129 bps ≫ ε_prod=5, часть ≫ ε_max=50. **Третий §8-провал того же класса** (near-touch
    объём local WS-книга vs REST-reference), уже ПОСЛЕ windowed-редизайна.
  - **Cadence-расхождение:** дизайн (`ops.md §4.3`) полагает cadence 5 мин (K=12 → окно 1 час);
    прод-fetcher budget-gated (`ops::budget`: RECON_BASE_DELAY=100ms, next_delay≈0 на Ok,
    `may_request` = `max_per_min`/60с) → реальная cadence ~1/мин, окно K=12 ≈ 12 мин на
    КОРРЕЛИРОВАННЫХ выборках → mean не гасится.
- **Действие reviewer'а:** blanket merge-revert ОТВЕРГНУТ (удалял бы critic-вердикты C-009/010/011 =
  потеря аудита + trip `protected-artifacts` CI → deploy не запустится → прод всё равно флудит).
  Прод НЕ инертен и льёт ~1 ложный audit/мин — **remediation срочная, под founder ★** (targeted:
  gate recon-эмиссии до наполнения книги + рекалибровка K/ε_prod + фикс cadence на 5 мин; либо
  artifact-preserving откат recorder recon-wiring — это КОД, зона architect/engine-dev). Detail: TD-025.
- **Итог:** recon-ЛОГИКА и books-feeder корректны в юнит-мире; §8 показал, что windowed K=12 +
  ε_prod=5 + sub-min cadence НЕ дают тишину на проде. Task 2 acceptance («recon молчит на здоровой
  книге») НЕ достигнут. **Milestone M-09 остаётся 🚧 ACTIVE.**

#### §8 RE-RUN (дефект A seed-gate, reviewer, 2026-07-18, merge `e9fc258`) → PASS
Architect (RED-first) → engine-dev: self-seeding `ReconDetector` (`crates/ops/src/recon.rs`,
`seeded: bool`, early-return ДО reconcile и ДО push в окно, `seeded=true` на первой непустой local).
Sacred RED `red_recon_window.rs`: 9a `empty_local_before_first_seed_does_not_emit`, 9b
`empty_local_after_seed_is_corruption_and_emits` (анти-плацебо over-suppress), 9c
`pre_seed_empty_does_not_poison_window` (C-012 gap). critic C-012 re-audit `5ec8094` → APPROVE.
- **Юнит (clean-checkout tester + reviewer merged-tree):** ops 36/36, `red_recon_window` 11/11,
  workspace clippy 0, `verify_M-09.sh` VERDICT PASS. Scope: только `crates/ops/src/recon.rs` (+37/-2).
- **§8 LIVE eyes-on (reviewer, нативный release-recorder против live Binance spot+futures, 4 recon-пары):**
  - **healthy ~8 мин: 0 `Sys(ReconDivergence)`** (best=true:0, best=false:0). Seed Binance BTC/ETH
    @ ~2s (seq 96/97); первый REST-fetch (~0.3s) пришёл ДО seed, но пустая local ПОДАВЛЕНА gate'ом,
    не сэмитила. **Стартовый флуд A (`best_diverged=true div=10000`, 4/рестарт) УСТРАНЁН.**
  - **injection (спот-WS заморожен: kill сокета + conntrack flush + DROP reconnect; спот-REST жив):
    6× `best_diverged=true` эмит** (Binance spot BTC/ETH, div_bps 700..1952) — seed-gate НЕ
    over-suppress'ит реальную порчу (анти-плацебо 9b подтверждён на живом проде). Инъекция полностью
    обратима: после снятия — WS восстановился, L2 возобновился, прод-сеть чиста.
  - **дефект B (оконный объёмный флуд) ПОДТВЕРЖДЁН ЖИВЫМ:** 12× `best_diverged=false` (div_bps
    103..747) на ~12-мин отметке (окно K=12 наполнилось), в т.ч. на **нетронутом BinanceFutures**
    (systematic WS(T1)-vs-REST(T2) near-touch объёмный bias, НЕ zero-mean churn). **Остаётся OPEN,
    развилка §4.3.2 (B1 vs B2; architect рекомендует B2) — founder ★, эскалация через architect.**

#### B2 (дефект B закрыт: рантайм recon = best-only + seed-gate, merge `4939d8f`, reviewer §8 PROD GREEN + APPROVED 2026-07-18)
Founder ★ 2026-07-18 принял **B2** (`docs/fa/ops.md §4.3.2`): объёмную near-touch сверку УБРАТЬ из
рантайма (REST-неверифицируема — систематический WS(T1)-vs-REST(T2) bias, три §8-провала одного
класса), рантайм-alert ⟺ `best_price_diverged` + seed-gate; объём → офлайн-трек research-dev (BACKLOG,
не блокирует). Цепочка: architect RED (`060da7f` `red_recon_window.rs`→`red_recon_runtime.rs`, объёмные
оконные оракулы 1–7 СНЯТЫ, добавлены `runtime_persistent_volume_deficit/surplus_is_silent`,
`runtime_nonbest_eviction_is_silent`, seed-gate 9a/9b/9c остаются) → engine-dev impl (`4939d8f`).
- `crates/ops/src/recon.rs` (+/−, чистое удаление window-машинерии) — `struct Window`/`windows`/push-цикл/
  `window_divergence_bps`/`gauge_divergence_bps` слиты в один `divergence_bps` (per-cycle гейдж);
  `observe()`: `alert = per_cycle.best_price_diverged`. **Seed-gate СОХРАНЁН полностью** (§4.3.1).
  `RECON_WINDOW`/`thr` — вестигиальные (`#[allow(dead_code)]`) для API-совместимости с sacred-RED и
  recorder-carve-out. `ReconDetector::new(ReconThresholds)` НЕ изменён → **сигнатуры
  `sink::handle_recon_snapshot`/recorder НЕ выросли (carve-out НЕ расширен)**; `crates/recorder/src/**`
  НЕ тронут. **T1 `ReconAudit` НЕ меняется → CT-RFC НЕ нужен.** MD-only (recon только читает REST) →
  risk-critic N/A.
- **Диф ⊂ `{crates/ops/src/recon.rs, crates/ops/src/sink.rs}`** (Block-scope), sacred-тесты
  (`crates/ops/tests/**`, `crates/recorder/tests/**`) НЕ тронуты dev-коммитом (RED-first: architect
  `060da7f` до impl `4939d8f`).
- **Гейты (reviewer перепрогнал независимо на чистом worktree):** ops **33/33** (`red_recon_runtime`
  8/8, `red_recon_sink` 4/4), recorder **8/8** (`red_recon_wiring` 2/2), workspace clippy 0, fmt clean,
  `verify_M-09.sh` **VERDICT PASS (12/12)**. **Анти-плацебо reviewer'ом независимо:** против pre-B2 src
  (`a418968`, window-active) РОВНО 3 B2-silent оракула ПАДАЮТ (`runtime_persistent_volume_deficit/
  surplus_is_silent`, `runtime_nonbest_eviction_is_silent`: `FAILED. 5 passed; 3 failed`); против B2 —
  33/33 GREEN. Best-emit оракулы (`runtime_post_seed_empty_local_still_emits`,
  `best_desync_emits_immediately`) запрещают «заглушить всё».
- **§8 EYES-ON PROD GREEN (reviewer, pre-merge деплой кандидата `4939d8f` на VPS, нативный
  release-recorder против live Binance spot+futures + HL, 2026-07-18):** декодер `journal::stream`
  (bounded-memory, только активный сегмент; recon-эмиссии видны ТОЛЬКО в журнале — stdout/лог их не
  несёт, `/metrics`-эндпоинта нет — см. NOTE ниже).
  - **BASELINE (window-impl `e3491d9`, тот же журнал ДО деплоя):** `RECON_DIVERGENCE=1414, best_true=0,
    best_false=1414, div_bps=[5..816]` — объёмный флуд B на здоровом рынке подтверждён живым.
  - **healthy ~9 мин (post-B2, ~13k событий, seq≥43683000): `RECON_DIVERGENCE=0`** (best_true=0,
    best_false=0), panic/ERROR=0, ConnDown=0. **Флуд B удалён** (не подавлен порогом — путь удалён).
  - **injection (спот-WS 9443 заморожен через `DOCKER-USER` DROP, спот-REST 443 + futures WS 443 + HL
    живы → дыра ограничена спотом; книга заморожена — доказано stale `book levels` BTC 5334/4861
    неизменны):** ETH REST-дрейф $1863.17→$1861.58 = **8.5 bps > 5** → **4× `best_diverged=true` эмит**
    (Binance ETHUSDT, div_bps 725..2597, Resynced), best_false=0. Best-путь §8-жив под B2 (гейт против
    always-silent на живом проде). Инъекция полностью обратима: rule снят, спот-WS восстановился (fresh
    changing `book levels`), iptables чист, контейнер healthy весь прогон (restarts=0, hb свежий,
    seq монотонный, writable=true).
- **Итог task 2:** дефект B закрыт УДАЛЕНИЕМ единственного флудившего пути; best-путь+seed-gate
  §8-зелёные. Объёмная сверка жива как per-cycle ГЕЙДЖ `book_divergence_bps` (наблюдаемость, офлайн-трек
  research-dev — BACKLOG «M-09 хвост»). **Task 2 функционально ЗАКРЫТ.** M-09 остаётся 🚧 ACTIVE:
  tasks 3 (сохранность/restore-drill), 4 (`/metrics` HTTP + правила алертов), 5 (verify финал),
  6 (tester+reviewer финальный §8).
- **NOTE (наблюдаемость, не блокер B2):** `/metrics` HTTP-эндпоинт ещё НЕ существует (нет сервера в
  recorder — это task 4), поэтому §8-пункт «/metrics отдаёт book_divergence_bps» для B2 НЕ проверялся
  (преждевременен). Recon-эмиссии на проде наблюдаемы ТОЛЬКО через журнал (`Sys(ReconDivergence)`) —
  ни stdout-лога, ни метрик-скрейпа. Это делает §8 recon трудоёмким (нужен bounded-memory декодер
  журнала); закрывается task 4.

### task 4 (метрики + алерты) — MERGED + В ПРОДЕ (`9a352d6`, reviewer APPROVED + §8 PROD GREEN 2026-07-19); **приёмка task 4 выполнена, НО наблюдаемость ещё не функциональна — TD-027**
Цепочка: architect RED (`08c8d89` `red_ops_server`/`red_ops_alerts`/`red_metrics_endpoint`) + verify
(`b5b7604`) → critic C-013 APPROVE_WITH_NOTES (`e2d1c33`) → engine-dev 4A (`604ea0b` /metrics loopback-
сервер) + 4B (`9a352d6` каталог правил + паритет). Reviewer ff-merge `9a352d6`.
- **(4A) `/metrics` scrape-сервер** — `crates/ops/src/server.rs` (ЧИСТАЯ `http_response(request_line,
  &Metrics)`: GET /metrics→200+тело, не-/metrics→404, не-GET→405; без tokio/IO, детерминирована) +
  `crates/recorder/src/metrics_server.rs` (socket accept-loop, read-line лимит 8 KiB анти-slowloris,
  per-conn task, cancel-safe). `main.rs`: `spawn_metrics_server` — bind `METRICS_BIND_ADDR` (дефолт
  **`127.0.0.1:9101`** loopback-only, без внешнего доступа §3); **бинд-сбой → WARN + продолжение БЕЗ
  эндпоинта** (метрики — не data-path; запись в журнал не падает из-за bind). recorder/Cargo.toml:
  +tokio features `net`+`io-util` (свои, shared-access). **journal-write/order путь НЕ тронут**
  (Block-scope подтверждён: recorder diff ⊂ {main.rs spawn, metrics_server.rs}). MD-only, read-only
  scrape → risk-critic N/A.
- **(4B) каталог правил `ops::alerts`** — `ALERT_RULES` (incident→severity→metric→summary), P0/P1 +
  P2-observational carve-out; рендер `to_prometheus_rules()` → `deploy/alerts/ops.rules.yml`
  (reviewer перегенерил `dump_rules` — IDENTICAL, нет drift, критик N2). **Двусторонний паритет OPS-I-5**
  (правило→метрика, класс §7.1→правило, нет orphan-rule) — verify shell + RED, анти-плацебо reviewer'ом
  независимо: http_response always-200 → `red_ops_server` FAIL (405/404); пустой `ALERT_RULES` →
  `red_ops_alerts` FAIL (coverage/parity).
- **Гейты (reviewer независимо на чистом worktree):** ops **52/52**, recorder **10/10**, clippy 0,
  fmt clean, `verify_M-09.sh` **VERDICT PASS** (18 проверок вкл. T4A/T4B). Scope чист (tests только
  architect `08c8d89`; contracts НЕ тронуты → CT-RFC не нужен; Cargo.toml только +net/io-util). Критик
  C-013 APPROVE_WITH_NOTES; N1 (tokio features) + N2 (yml derived) — оба проверены reviewer'ом.
- **§8 PROD GREEN (VPS `9a352d6`, CI+Deploy success, 2026-07-19):** контейнер healthy, restarts=0,
  hb свежий, panic/ERROR=0, `metrics-server bound 127.0.0.1:9101`. `/metrics` через busybox-sidecar
  (`--network container:hft-recorder`, loopback не проброшен наружу — §3): **HTTP 200**, тело несёт
  **`book_divergence_bps{venue,symbol}` для всех 4 символов с НЕНУЛЕВЫМИ значениями** (binance BTC/ETH
  + binance_futures BTC/ETH: 6..332) + `venue_http_status_total{code=200}` non-zero. **Приёмка task 4
  (эндпоинт+каталог+паритет+§8 book_divergence_bps) ВЫПОЛНЕНА.**
- **⚠ БЛОКЕР РЕАЛЬНОЙ НАБЛЮДАЕМОСТИ (reviewer §8, TD-027 OPEN):** из 15 объявленных семейств живые
  SAMPLES ТОЛЬКО у 2 (`book_divergence_bps`, `venue_http_status_total`). Остальные 13 —
  **объявлены+зацитированы в правилах, но НЕ инкрементируются** (grep call-site пуст). Следствие:
  правила алертов ТРЁХ ФОРМИРУЮЩИХ инцидентов ссылаются на МЁРТВЫЕ метрики — **TD-011 (P0) →
  `journal_bytes_written_total`, TD-014 (P1) → `md_events_total`, TD-016 (P1) → `recorder_rss_anon_bytes`,
  OPS-GAP → `journal_seq_gaps_total`** — эти алерты НИКОГДА не сработают. Паритет OPS-I-5 реестрово-
  статичен (имена, не эмиссия) → зелёный, но ложно-успокаивает. Эмиссия требует journal-write пути +
  recorder hot-loop, которые carve-out task 4 ЯВНО запрещает → это ОТДЕЛЬНАЯ задача (4C, architect
  RED-first со своим carve-out). ДО Alertmanager (§O) метрики обязаны быть живыми. **M-09 остаётся
  🚧 ACTIVE:** task 3 (Storage Box, founder ★), **task 4C (метрик-эмиссия, TD-027)**, task 5/6.
- Перенумерация: коллизия `TD-025` (recon-flood vs M-08 disk-migration) разведена — M-08 disk →
  **TD-026**; новый долг эмиссии — **TD-027**.

### task 4C (живая эмиссия метрик, OPS-I-10) — MERGED + В ПРОДЕ (`ac645ac`, reviewer APPROVED + §8 PROD GREEN 2026-07-20); **TD-027 CLOSED — безопасник теперь на ЖИВЫХ метриках**
Фикс TD-027: контракт **OPS-I-10 «объявлена ⟹ эмитится»**. Цепочка: architect RED `f28e78d` +
6 critic re-audit'ов C-014 (#1–#6, hardening оракула: label-aware value-ассерты, dead-zero,
dimension/value-collapse, kind-aware, RssAnon≠VmRSS) → engine-dev impl `ac645ac`. Reviewer ff-merge.
- `crates/recorder/src/lib.rs` — `emit_post_append(metrics, journal, event)`: ЕДИНАЯ точка эмиссии
  после КАЖДОГО `append` (`journal_seq_current`/`journal_segment_index`/`journal_disk_free_bytes` gauge,
  `journal_bytes_written_total`++ , `md_events_total{venue,symbol,kind}`++ для `Md`); `journal_write_
  errors_total`++ на Err append (event-триггер, все 3 пути). `run_books_feeder` (живой loop) эмитит
  `book_levels{venue,symbol,side}`. **JR-I-1 append/flush/shutdown семантика НЕ изменена** (добавлена
  ТОЛЬКО эмиссия — OPS-I-7 lock-free атомики; OPS-I-6 без journal-зависимости).
- `crates/recorder/src/metric_emit.rs` — `sample_rss` (RssAnon из `/proc/self/status`, НЕ VmRSS —
  TD-021; None без fallback), `sample_md_age` (возраст per-venue, dead-zero избегается — растёт на
  тишине, OPS-I-8), `parse_rss_anon`. `crates/recorder/src/main.rs` — sampler-таск (1 Гц) зовёт
  `sample_rss`/`sample_md_age`, feeder зовёт `run_books_feeder`, writer зовёт `run_writer` +
  supervisor эмитит `venue_ws_reconnects_total` (live-wiring: все продюсеры в живом `main`, не helper).
- **Scope:** dev-коммит `ac645ac` ⊂ `recorder/src/{lib,main,metric_emit}.rs` (sacred tests/contracts/
  journal/alerts/docs/milestones НЕ тронуты; всё это — architect/critic коммиты цепочки). MD-only.
- **Sacred RED `red_metrics_emission.rs`** прогоняет РЕАЛЬНЫЕ `run_writer`/`run_books_feeder`/samplers
  с мульти-вендор/символ/kind/side фикстурой и ассертит ТОЧНЫЕ per-label значения (30/5/7/10, 5/3/4/2/
  6/1, age 200/1000) + dead-zero (`>0`) + RssAnon≠VmRSS → registry-only/collapse НЕ проходят. Verify-гейт
  OPS-I-10 (покрытие §3-карты + live-wiring канарейка).
- **Гейты (reviewer независимо на чистом worktree):** workspace **282/0** (94 блока), red_metrics_emission
  5/5, clippy 0, fmt clean, `verify_M-09.sh` **VERDICT PASS (20 проверок)** вкл. T4C OPS-I-10 + OPS-I-5
  паритет в обе стороны + OPS-I-10 покрытие/live-wiring. Критик C-014 re-audit #6 **APPROVE**.
- **§8 PROD GREEN (VPS `ac645ac`, CI+Deploy success, 2026-07-20):** healthy, restarts=0, panic/ERROR=0.
  `/metrics` через busybox-sidecar (loopback) — **13 ранее-мёртвых метрик теперь несут ЖИВЫЕ SAMPLE'ы:**
  `journal_bytes_written_total=15245` (TD-011 P0 liveness жив), `journal_seq_current=51923737`,
  `journal_segment_index=49`, `journal_disk_free_bytes=103.6G`, `md_events_total{venue,symbol,kind}`
  живой kind-aware (trade 8124/5414, l2snapshot, funding, open_interest — TD-014 жив),
  `md_event_age_ms{venue}` 83/992/77, `book_levels{venue,symbol,side}` живой per-серия (HL=20 — TD-016
  жив), `recorder_rss_anon_bytes=17506304` (RssAnon ~17 MB, TD-016 P1 жив). Event-метрики
  (`journal_write_errors_total`/`journal_seq_gaps_total`/`venue_ws_reconnects_total`) корректно
  отсутствуют на здоровом прогоне (реальный триггер не наступил). **TD-027 CLOSED.**
- **Остаточные NOTE (в TD-027):** (1) **✅ DONE task 4D** (см. ниже) — метрика переименована в
  `journal_frames_written_total`; (2) `journal_seq_gaps_total` без writer-продюсера → правило **OPS-GAP**
  на writer-пути не сработает (gap детектируется только на read/replay — нужен продюсер там или пересмотр
  правила; зона architect, придёт с task 3).

#### task 4D (NOTE-1 rename metric-contract) — MERGED + В ПРОДЕ (`83c340c`, reviewer APPROVED + §8 PROD GREEN 2026-07-20)
Чистый rename `journal_bytes_written_total → journal_frames_written_total` (NOTE-1 TD-027: честное имя —
счётчик КАДРОВ, +1/append). Цепочка: architect RED `028fe08` (oracle → новое имя) + docs → critic C-015
audit+re-audit APPROVE → engine-dev `f442c96`. Диф dev-коммита ⊂ 5 файлов (ops/{alerts,metrics}.rs,
recorder/{lib,metric_emit}.rs, deploy/alerts/ops.rules.yml) — только строковый литерал + комментарии,
никаких новых веток/guard/флагов, поведение идентично. TD-011 PromQL перерендерена
(`rate(journal_frames_written_total[1m]) == 0`); yml == renderer (drift 0); старое имя 0 hits в crates/+deploy.
Sacred (oracle) обновлён architect'ом, не dev. Гейты reviewer'ом независимо: workspace **282/0**, clippy 0,
fmt clean, `verify_M-09.sh` **PASS (21)**. **§8 PROD (`83c340c`):** `journal_frames_written_total=4099`
растёт, старое имя ОТСУТСТВУЕТ, journal_seq/segment/disk живые, healthy restarts=0, panic/ERROR=0.
**TD-027 NOTE-1 DONE; NOTE-2 (seq_gaps read-side) остаётся до task 3.**
- **M-09 остаётся 🚧 ACTIVE:** task 3 (сохранность/restore-drill — заблокирован Storage Box founder ★),
  task 5 (verify финал), task 6 (tester+reviewer финальный §8 milestone'а). Живое alerting-роутинг
  (Alertmanager, §O) — отдельное founder ★ решение; метрики под него теперь ЖИВЫЕ.

## P-COCKPIT — charter-набор пивота (Слои 8-9 ACTIVE-charter; docs-only MERGED 2026-07-22, reviewer APPROVED)
Пивот founder'а (2026-07-22): ближний фокус — **не торговый стек, а ДАННЫЕ + виз-бэкенд + AI-копилот**
для Bookmap-подобного кокпита (Order Flow Intelligence Terminal) на Binance+HL; фронт — founder
(`code2alpha`), мы даём бэкенд+экспорт. **P3 (risk/oms)/P4 (live)/сигналы (M-10) — ОТЛОЖЕНЫ, НЕ отменены.**
Смержен **Class-A doc-набор** (ff `cce11a5`, чистый fast-forward `0cd447d..cce11a5`). Прошёл critic
doc-гейт (`research/critiques/C-021-cockpit-docs.md` = NOTE; NOTE-1 SemanticEvent≠T1 + NOTE-3 HL-wording
приземлены на ветке; NOTE-2 production-streaming → зашит в acceptance M-22, не этот merge).
- `docs/DESIGN.md` — **Слои 8-9** (`viz-backend` дериватив+Read Gateway+export v2; `ai-copilot` Event
  Engine+AI-Context+LLM-сервис+audit, ВНЕ детерминизма) + фаза **P-COCKPIT** в роадмапе. Слой 8
  детерминирован (live==replay); слой 9 вне DET-I-1 (LLM недетерминирован → выводы вне journal).
- `docs/07-cockpit-backend-roadmap.md` (LIVING DOC, architect-owned) — решения сессии D1–D5 + decision-log
  + milestone-порядок (виз-first) + открытые founder-вопросы.
- `docs/fa/viz-backend.md` (**VB-I-1..8**) — дериватив-слой read-only консюмер `journal::stream`; export v2
  аддитивно; data-quality gate (глубже 1.3% — `depth_band_provenance`); модель сессии 00:00 UTC.
- `docs/fa/ai-copilot.md` (**AI-I-1..8**) — 5 слоёв; AI read-only, вне ядра, audit обязателен;
  `SemanticEvent`/`AiEvent` ≠ T1 `Event`/`EventKind` (канарейка AI-I-8).
- `docs/05-contract-layer.md` — governance: виз/AI-контракты (export v2, SemanticEvent, AI-Context,
  Strategy-Definition, Audit) — **T-designate, БЕЗ contract-RFC** (аддитивно, bump `export_schema_version`);
  промоушен в `crates/contracts` только при кросс-языковом консюмере (паттерн TD-008). T1-ядро read-only.
- `milestones/BACKLOG.md` — виз-first баннер; торговый трек «ОТЛОЖЕН, НЕ удалён».
- `research/data-quality/depth-probe-staleness.md` + `crates/book/examples/depth_probe.rs` (OFFLINE-диагностика,
  `read_all` — НЕ прод-путь, T11e carve-out `examples/`≠`src/`) — фантом-тест дальних полос: сигнатура
  чистого фантома TD-016 НЕ подтвердилась, но и «полосы достоверны» не доказано (конфаунд resync-обнулением);
  рефрейм — глубже ~1.3% валидированного эталона нет НИ У КОГО ⇒ планка = **корректность книги** (TD-016), не «вендор».
- **Scope/Block-C:** тронуты ТОЛЬКО `docs/`, `milestones/BACKLOG.md`, `research/{critiques,data-quality}/`,
  `crates/book/examples/depth_probe.rs` (пример, НЕ `src`). `crates/contracts/**` НЕ тронут; кода/risk/
  killswitch/oms/venue нет ⇒ reviewer-only, risk-critic НЕ требуется. Гейты (reviewer перепрогнал):
  `cargo fmt --all --check` exit=0; `cargo clippy --workspace --all-targets -- -D warnings` exit=0 (пример
  собирается workspace'ом). **§8 деплой-гейт (light-touch, прод ИНЕРТЕН — recorder-бинарь не меняется,
  Dockerfile `--bin recorder`, пример в образ не попадает):** CI `29948492195` completed/success (5m57s),
  Deploy `29948492152` completed/success (7m17s, build-on-VPS 1m9s — триггернулся `crates/**` из-за примера).
  VPS eyes-on: `hft-recorder Up (healthy)`, `restarts=0`, heartbeat свежий (`ts_wall_ms=1784746746502`,
  age ~3.8s), `writable=true`, `free_bytes≈95 GB` (> min 10 GB), `next_seq=69171054` растёт, `segment_index=68`.
  Прод инертен — recorder пересобрался идентично и продолжает писать; поведение данных не изменилось.
- **Дальше (architect):** спека **M-22 (Read Gateway)** на `feat/M-22` (RED не живёт на main) с NOTE-2 в
  acceptance (`journal::stream`+`EpochFilter`, bounded-memory, канарейка против `read_all` в прод-пути);
  параллельно M-20 (VWAP)/M-23 (heatmap). Открытые founder-решения (docs/07 §10): TPP `formula_pending`,
  LLM-провайдер, Tardis-бюджет/окно, масштаб универсума TPP TOTAL.

### D6 App-плоскость + D1 транспорт-уточнение (docs-only MERGED `4741846`, reviewer APPROVED 2026-07-23; deploy ИНЕРТЕН)
Ветка `docs/cockpit-d6-appplane` (ff `ab9aba6..4741846`, чистый fast-forward поверх `b8c60db`).
Прошла critic doc-гейт с витком: **C-023 REJECT (`27d827b`) → architect r2 (`83cd926`) → C-023 r2 PASS
(`4741846`)** — REJECT снимал остаточную Fastify-двусмысленность, r2 её закрыл.
- **D1 (уточнён):** горячий WS держит Rust `gateway-serve` **напрямую** (tokio-tungstenite), **Fastify
  как обязательный middle-tier ОТМЕНЁН** (Node-релей = непротестированный слой в детерм-пути). Бинарные
  фреймы — postcard. `docs/07` §5 D1 + `docs/fa/viz-backend.md` §1/§4-B переписаны согласованно.
- **D6 (принято, Path 1):** **две плоскости по природе данных.** (1) Market-плоскость — `gateway-serve`:
  журнал→snapshot/frames/replay, **read-only, детерминированная, stateless по юзеру** (architect-зона).
  (2) Application-плоскость — **Next.js + Postgres** (`users/strategies/ai_chats/settings/audit_log`),
  зона founder'а; Fastify лишний. **Мост auth:** Next выпускает короткоживущий подписанный JWT
  (HS256/Ed25519), Rust ТОЛЬКО верифицирует подпись (`jsonwebtoken`), в user-БД НЕ ходит.
- **Инвариант VB-I-9** (новый, sacred/architect-only, RED — future): `gateway-serve` не читает/не пишет
  application-БД; auth = stateless verify JWT без user-БД-lookup; grep-канарейка «gateway не импортирует
  postgres/sqlx/diesel». User-состояние (стратегии/чаты/настройки) вне market-журнала → `DET-I-1` цел
  (тот же класс, что `AI-I-1` «AI-выводы вне журнала»). `docs/fa/ai-copilot.md` §3: Audit-Log/чаты/
  strategies живут в Postgres, `ai-copilot` пишет в app-БД, но НЕ в market-журнал.
- **Scope/Block-C:** тронуты ТОЛЬКО `docs/07-cockpit-backend-roadmap.md`, `docs/fa/{viz-backend,ai-copilot}.md`,
  `research/critiques/C-023-d6-appplane.md`. `crates/contracts/**` НЕ тронут; кода/risk/killswitch/oms/venue
  нет ⇒ reviewer-only, risk-critic НЕ требуется. Авторство коммитов чистое (2 architect + 2 critic).
  **§8 deploy — ИНЕРТЕН** (docs-only, `crates/**` не тронут → Dockerfile/бинарь не меняются).
- **Дальше (architect):** транспортный milestone `gateway-serve` (D1/D6 superseding над историческим
  M-22-текстом `milestones/M-22-read-gateway.md:52-56`) + RED-оракул на VB-I-9 (import-канарейка). D6 —
  надмножество для транспортного решения.

## Кокпит-транспорт (M-28 «gateway-serve WS» — КОД MERGED в main (`40b8113`), но **§8 NOT GREEN → milestone НЕ закрыт**, reviewer 2026-07-26)
Цепочка: architect spec/RED (GS-I-1..5) → critic **C-024 REJECT → r2 PASS** → engine-dev impl (tasks #2-4) →
reviewer BLOCK на §8 (B1 deploy-gap + B2 секрет) → architect B1 (`8cadba0` deploy.yml health-gate) + founder B2
(GATEWAY_JWT_SECRET на VPS `.env` 600) → **reviewer консолидация + merge + §8**. Merge `40b8113` (`--no-ff`;
консолидация: 3 impl-коммита engine-dev cherry-pick'нуты поверх B1 на `origin/feat/M-28-gateway-serve`, идентичность
engine-dev сохранена). Гейты reviewer НЕЗАВИСИМО на чистом worktree: **workspace build exit 0; test 389 passed/0 failed
(129 блоков); `verify_M-28.sh` VERDICT: PASS exit=0** (fmt --all, clippy --workspace -D warnings, GS-I-2/4/5, канарейки
GS-I-1/GS-I-3, bin build). CI 30199831629 + Deploy 30199831637 — оба **success**.
- `crates/gateway-serve` (engine-dev, тонкая IO-оболочка над `crates/gateway` M-22) — WS-транспорт кокпита:
  `auth::verify_token` (jsonwebtoken HS256, **stateless — без user-БД**, GS-I-2), `wire::ServeMsg` (JSON-конверт,
  JS-декодируемо, GS-I-4), `serve::{snapshot_msg,frames_msgs}` (тонкий passthrough над `gateway::{snapshot,frames_since}`,
  GS-I-5), `server::{bind,serve}` + bin (tokio-tungstenite: accept→verify JWT из `?token=`→snapshot+push(250ms)+replay).
  **Read-only:** нет journal-writer/app-БД (канарейки GREEN); клиентские WS-фреймы читаются-и-игнорируются (MVP).
  risk-critic N/A (read-only, нет order-path). Block-C N/A (contracts не тронуты).
- `deploy.yml` (**B1**, architect `8cadba0`, класс M-35 «сервис в compose, но пайплайн не стартовал») — deploy-шаг
  теперь `up -d --build recorder gateway-serve` + **fail-closed health-gate ОБОИХ** (`docker inspect Health.Status
  hft-recorder` И `hft-gateway-serve`) + rollback обоих. `docker-compose.yml`: сервис `gateway-serve` (образ
  `hft-platform-recorder:local`, RO-bind `/journal:ro`, healthcheck TCP-проба 127.0.0.1:8080, `depends_on recorder
  healthy`, secret через `${GATEWAY_JWT_SECRET:?}`). Dockerfile собирает+копирует 3-й бинарь `gateway-serve`.
- **§8 eyes-on (прод `40b8113`, 2026-07-26) — ЧАСТИЧНО GREEN, но продуктовый критерий ПРОВАЛЕН:**
  ✅ `hft-gateway-serve` Up (healthy) — **B1 сработал** (сервис стартовал пайплайном И прошёл health-gate);
  ✅ `hft-recorder` healthy, `restarts=0`, heartbeat свежий (`next_seq` растёт 91534926→91569413, `writable=true`) —
  **сбор НЕ задет**; ✅ read-only подтверждён (`/journal` mount `rw=false mode=ro`); ✅ JWT-auth E2E: wrong-key → `Error`,
  expired → `Error`, оба reject без snapshot. ❌ **ВАЛИДНЫЙ JWT → snapshot НЕ приходит:** `ws auth ok` →
  `conn ended with error error=frame crc mismatch` (ДЕТЕРМИНИРОВАННО 3/3). Дефект — НЕ в транспорте M-28
  (auth/handshake/passthrough корректны), а в `gateway::snapshot`/`journal::stream` на живой раскладке журнала
  (legacy 15GB + 88 компактированных `.zst` + активные сегменты); M-28 — первый запуск gateway-кода на проде,
  §8 вскрыл. ⇒ **TD-038 (BLOCKING M-28, MAJOR)**. Прод НЕ повреждён, revert НЕ требуется (gateway-serve read-only,
  безвреден-но-нефункционален). **M-28 остаётся 🚧 IN_PROGRESS** до фикса TD-038 (architect RED → engine-dev) +
  повторного §8 E2E-GREEN (валидный JWT → Snapshot со `schema_version`).
- **ОБНОВЛЕНИЕ 2026-07-26 (M-36 merge `65519ae` + §8 ops-purge):** crc-корень TD-038 (торн-crc фрейм в legacy)
  УСТРАНЁН физическим удалением legacy-сегмента на проде, но валидный JWT → Snapshot ВСЁ РАВНО не приходит —
  теперь из-за **OOM** (gateway-serve дорастает до ~7.3 GB RSS на unbounded reduce и убивается host-OOM). Активный
  блокер M-28 переехал с TD-038 (crc, закрыт purge'ем) на **TD-039 (OOM, BLOCKING M-28 И M-36)**. M-28 по-прежнему
  🚧 IN_PROGRESS — §8 E2E-GREEN (валидный JWT → Snapshot) недостижим до фикса TD-039.

## Кокпит-снапшот прод (M-36 «gateway snapshot: legacy purge + VWAP all-time» — КОД MERGED в main (`65519ae`), §8 ops-purge ВЫПОЛНЕН, но **§8 E2E NOT GREEN → milestone НЕ закрыт**, reviewer 2026-07-26)
Цепочка: architect (RED `red_vwap` all-time + `red_seg0_removed` guard + `verify_M-36.sh` + `docs/fa/viz-backend.md`
VB-I-6 per-series anchor) → critic **C-026 REJECT (fmt sacred) → rev2 PASS** (`1458885` fmt-фикс + усиление seg0-guard)
→ engine-dev (`ef21fec`, tasks #1-3) → tester PASS → **reviewer APPROVED + merge + §8 ops-purge**. Merge `65519ae`
(`--no-ff`; push-scope чист — только M-36-коммиты). CI 30206945723 + Deploy 30206945673 — оба **success**.
- **Code-контракт (reviewer НЕЗАВИСИМО на чистом worktree @`ef21fec`):** scope чист — engine-dev тронул ТОЛЬКО
  `crates/gateway/src/lib.rs` (+17/-12), sacred tests/verify/milestone/docs не тронуты; contracts/risk/killswitch/oms/
  venue-* — 0 файлов (risk-critic N/A: read-path, нет order-egress, MD-only carve-out класс; Block-C N/A). RED-first
  соблюдён (тесты в отдельных architect-коммитах); **анти-плацебо доказан reviewer'ом независимо** — против
  пред-impl дерева (session-reset) `vwap_cumulative_across_midnight` FAIL (`left 200e8 / right 150e8`). Гейты:
  fmt/build/clippy clean, **workspace 390 passed/0 failed (130 блоков)**, `verify_M-36.sh` **VERDICT: PASS exit=0**.
- `crates/gateway/src/lib.rs` (engine-dev) — `VwapAcc`: session-reset СНЯТ (`session_id` поле удалено, `apply_trade`
  потерял `ts_ms`); `sum_pv/sum_v` (i128, VW-I-2 анти-переполнение) копятся all-time от `Cursor::START` через границу
  00:00 UTC. `SeriesBundle.vwap` doc: session→all-time. `GATEWAY_SCHEMA_VERSION` **5→6** (форма `Vec<(i64,i64)>`
  неизменна, семантика пересмотрена; консюмеров ещё нет). VB-I-6 → per-series anchor (VWAP=journal-cumulative;
  SVP/CVD остаются session, НЕ тронуты). **T1 не тронут** (VWAP живёт в gateway, не в `Event`/`EventKind` — CT-RFC не нужен).
- **§8 ops-purge legacy на VPS (task 5, reviewer/founder, founder-подтверждён в-сессии) — ВЫПОЛНЕН, IRREVERSIBLE:**
  baseline (recorder healthy `restarts=0`, heartbeat свежий, активный `segment_index=94`, legacy seg0 заморожен Jul 14)
  → backup `journal.legacy.json` в `/root/journal.legacy.json.m36bak.*` → снята декларация legacy (manifest =
  `{"declarations": []}`) → `rm segment-00000000.jrnl` (15 188 347 171 B, необратимо) → recorder не задет
  (`restarts=0`, продолжает писать в seg94, heartbeat `writable=true`), диск 43%→34% (освобождено ~14 GB).
- **§8 E2E snapshot (task 5, schema_version=6 + latency) — NOT GREEN → milestone НЕ закрыт.** После purge crc mismatch
  БОЛЬШЕ НЕ воспроизводится (TD-038 crc-корень снят), но валидный JWT → Snapshot НЕ приходит: gateway-serve
  **OOM-killed на построении снапшота** (host-OOM ~7.3 GB RSS, dmesg oom-killer; `RestartCount` 0→1 на одно
  подключение; живой замер `RssAnon` 308 kB→672 MB за 8 s монотонно ~90 MB/s). Это НЕ порча (crc/parse-ошибок нет) —
  unbounded reduce всего журнала (93 `.zst` + активные, ~16 GB) на каждое подключение. ⇒ **TD-039 (BLOCKING M-28 И
  M-36, MAJOR)** — отложенный вопрос M-36 §Objective п.4 (чекпоинт-редьюсер) эскалирован: замер дал OOM, а не
  «медленно» ⇒ bounded-memory снапшот / checkpoint-редьюсер ОБЯЗАТЕЛЕН. latency-число не снято (снапшот не строится).
- **Прод НЕ повреждён, revert НЕ требуется:** recorder — отдельный процесс, healthy/`restarts=0`/heartbeat свежий/
  `writable=true`, сбор данных не задет; gateway-serve idle-healthy, падает ТОЛЬКО на подключении; живых
  cockpit-консюмеров нет (M-28 IN_PROGRESS). M-36 code (VWAP all-time) корректен и остаётся — OOM предшествует M-36.
- **M-36 остаётся 🚧 IN_PROGRESS.** Закрывается ТОЛЬКО после фикса TD-039 (новый milestone: architect RED
  прод-масштаб bounded-reduce/checkpoint → engine-dev → reviewer §8 E2E-GREEN валидный JWT → Snapshot schema_version=6
  + latency) и founder-подписи выбора архитектуры снапшота.

## Чекпоинт-редьюсер (M-38b «Путь Б: снапшот от чекпоинта, а не от START» — КОД MERGED `606aa62`, reviewer APPROVED 2026-07-28; **§8 NOT GREEN → milestone НЕ закрыт, TD-044 OPEN**)

Цель — TD-044: первый `Snapshot` строился 409.74 s (полный реплей журнала на КАЖДОЕ подключение).
Путь Б: персистентный чекпоинт состояния `Reducer` + докорм хвостом от курсора чекпоинта.

**Что реально на `main` (код APPROVED, гейты воспроизведены reviewer'ом НЕЗАВИСИМО на чистом
worktree 88dc625, не пересказом tester'а):** `fmt=0`, `clippy -D warnings=0`,
`cargo test --workspace` **470 passed / 0 failed / 0 ignored** (147 блоков),
`scripts/verify_M-38b.sh` **VERDICT: PASS, exit=0** (37 шагов, 0 FAIL).
- `crates/gateway` — модуль `checkpoint` (postcard-состояние + `ckpt_schema_version`, детерминированное
  имя `ckpt-<selector_fingerprint:016x>.bin` (RN-23), `flock` + уникальный tmp `<final>.tmp.<pid>.<nanos>`
  (RN-22)), `snapshot_from_checkpoint` (чекпоинт = КЭШ: любая невалидность/отсутствие → тихий rebuild),
  `LiveReducer`, `advance`/`advance_to` (возвращают ДОСТИГНУТЫЙ курсор — B2), бинарь
  `src/bin/gateway-checkpoint.rs` (argv принимает и `--flag=value`, и `--flag value` — B1).
- `crates/journal` — `stream_from` (сегментный skip, GW-I-11) + `ReadStats{events_decoded,
  segments_opened}`; retention-гейт по покрытию чекпоинта (C-030 R1): `RetentionPolicy.
  {checkpoint_covered_through_seq, allow_prune_without_checkpoint}`, `RetentionPlan.offload_only`,
  `RetentionReport.pruned_without_checkpoint_coverage`; флаги `--checkpoint-coverage`,
  `--allow-prune-without-checkpoint` у `journal-retention`.
- `crates/book` — ТОЛЬКО `#[derive(Serialize, Deserialize)]` на `OrderBook` (все 4 поля, вкл. приватные)
  + RED `red_orderbook_serde_roundtrip`. `crates/gateway-serve` — `ServeConfig.checkpoint_dir` из
  `GATEWAY_CHECKPOINT_DIR`, `snapshot_msg` возвращает `ReadStats`, лог на `debug` (дефолт-фильтр
  `info,gateway_serve=debug` ⇒ в проде виден). `docker-compose.yml` — ops-сервис `gateway-checkpoint`
  (`profiles: ["ops"]`, journal `:ro`, ckpt-том RW) + `gateway-ckpt:/ckpt:ro` сервису `gateway-serve`.
- **Цепочка гейтов соблюдена:** RED-first (`14d6642` architect — ТОЛЬКО `*/tests/`+milestone+verify —
  предшествует всему impl), plan-time critic ДВАЖДЫ (`C-030` REJECT R1/R2/R3 → rev2 → `C-031` NOTE,
  engine-dev разблокирован), risk-critic не требуется (read-path, `gates.md` §5 MD-only carve-out;
  `risk`/`killswitch`/`oms`/`venue-*` в диффе отсутствуют), `crates/contracts/**` не тронут —
  `GATEWAY_SCHEMA_VERSION` остаётся **7** (канарейка GREEN).

**Почему milestone НЕ закрыт — §8 eyes-on на VPS (`606aa62`, CI+Deploy оба success):**
фича **ИНЕРТНА В ПРОДЕ**. Штатный ops-путь поднятия чекпоинта падает:
`advance_to` fail-loud'ит, потому что `first_visible_seq=16049334 > 0` (сегмент 0 удалён purge'ем
M-36 — необратимо), а escape-hatch у бинаря нет ⇒ `/ckpt` пуст ⇒ `gateway-serve` каждый раз уходит
в fallback-реплей. E2E JWT → первый `Snapshot` (DECODE, не grep): **382.657 s**, `schema_version=7`,
`cursor={"upto_seq":111647115}`, `ohlcv len=51`, `heatmap len=1789`, реальные цены BTC — то есть
относительно 409.74 s улучшения НЕТ. Сырой вывод, корень, асимметрия «read-path усечённую историю
ОТДАЁТ, checkpoint-path её СОХРАНИТЬ отказывается» и не доставленная ops-цепочка (`deploy/**` не
входил в Allowed paths, cron чекпоинтера не заведён) — **TD-048** (MAJOR).
**Прод при этом здоров и не деградировал:** recorder пишет (`next_seq` 111615379 → 111649748 за 8 мин,
`writable:true`), оба контейнера `(healthy)`, оба тома читателям смонтированы `rw=false`.
⇒ **TD-044 остаётся OPEN**, close-out M-28/M-36/M-38b по-прежнему заблокирован.

**Процессные находки PR-гейта (NOTE, не блокеры merge):** (а) `Dockerfile` правился вне
`Allowed paths` — механическая предпосылка явно разрешённого ops-сервиса; (б) engine-dev правил
Forbidden `crates/gateway-serve/tests/{red_serve_passthrough,smoke_ws}.rs` — чистая адаптация
call-site под сигнатуру, которую сменил САМ architect в задаче #15 (инварианты не тронуты, сверено
по диффу); правку `crates/journal/tests/red_retention_operator.rs` (`..Default::default()`) architect
откатил сам (задача 0d). Урок для architect: меняешь публичную сигнатуру — вези адаптацию ВСЕХ
call-site'ов в том же RED-коммите, иначе dev вынужден лезть в sacred-зону.
(в) `Cargo.toml` workspace — правился и полностью откачен (net-diff пуст, задача #11, сверено).

## Мастер-документ — `docs/DESIGN.md` в редакции AlphaQuant ✅ **ACTIVE** (Блок 1, merge 2026-08-01, R-014 APPROVED)

`docs/DESIGN.md` переведён из редакции v1 (однопользовательская торговая система) в редакцию
**AlphaQuant** — многопользовательский SaaS-терминал. Статус шапки: `PROPOSED → **ACTIVE**`.
Основание merge — подпись founder'а `docs/PENDING-SIGNATURE.md` **П-001** (2026-08-01, граница C:
смена продуктовой модели и приоритетов фаз).

**Эволюция аддитивная, не замена.** Каркас `§0`–`§12` сохранён вместе с нумерацией (от него
производны 14 документов `docs/fa/*` и сотни ссылок вида `DESIGN.md §N` в правилах, милестоунах и
коде); новое содержание добавлено разделами `§13`–`§23`. Проверено reviewer'ом на дереве слияния:
разделов **25** (`§0`–`§23` + `§1.5`), `§1`–`§8` **байт-в-байт неизменны** относительно merge-base
`8c1890e` (0 изменённых строк). `docs/DESIGN-v2.md` удалён — слит в основной документ.

**DOC-гейт класса A пройден полностью** (`.claude/rules/gates.md` §9), пять кругов:
`C-041` (critic, REJECT → устранено) · `C-042` (risk-critic, CONCERNS + 2 BLOCKING → устранено) ·
`R-004` (reviewer, REJECT, 3 блокера) · `R-011` (Б-1: §10 P2.5 заявлена пройденной) ·
`R-013` (Б-2/Б-3, найдены прогоном на дереве слияния) · **`R-014` APPROVED**.

**Машинный гейт документа** — `scripts/verify_design_claims.sh` (**влит в `main`** позже
тем же днём, `R-016` APPROVED — см. раздел «Машинные гейты» ниже). Проверяет
обеспеченность маркеров `[ЕСТЬ]`, числа покрытия `§22` против реального грепа по оракулам,
ссылки `DESIGN.md §N` и `docs/*.md`, согласованность фаз. На дереве слияния и на `main` после
merge: **VERDICT PASS (0 нарушений)**.

**Процессный урок, закреплённый этим блоком (`R-013` → `gates.md`):** документ класса A
проверяется на **ДЕРЕВЕ СЛИЯНИЯ**, а не на ветке. Блок 1 был зелёным на ветке и красным на
merge-превью: пока правка проходила пять кругов, в `main` въехали M-50 (оракул `JR-I-9` — §22
заявляла 3, стало 4) и алертинг `ops-watchdog` (§23.1/§9.1 утверждали «push-алертинга нет вовсе»,
`[ПЛАН]`, поверх смерженного кода). Гейт получил режим `--merge-preview <base-ref>`.

**Открытые NOTE для architect (не блокеры, заведены reviewer'ом в `R-014`):**
**NOTE-1** — формулировка правила «расхождение прогонов = блокер» ненаправленная и при буквальном
применении самопротиворечива (документ, приведённый к дереву слияния, обязан краснеть на
отстающей ветке); блокер — именно **preview-красный**. **NOTE-2** — проверка `[1-ЕСТЬ]` покрывает
только маркеры в таблицах (3 из 20); 17 живут в прозе и держатся на глазах reviewer'а — фикс Б-3
приземлился именно в эту слепую зону и был проверен вручную.

## M-52 — journal hardening: `JR-I-10`/`JR-I-11`/`JR-I-12` (TD-052 / TD-030 / TD-067) ✅ КОД В MAIN (merge `b0723d4`, `R-022` APPROVED 2026-08-02; §8 — см. ниже)

Три последние щели надёжности журнала. Две из них сидели на пути защиты от **seq-reuse** —
необратимой порчи append-only журнала, — третья была не порчей, а слепотой.

- **`JR-I-10` (TD-052 + механизм TD-054) — ограниченность РАБОТЫ скана пола.** До M-52
  `readable_floor` был ограничен по ПАМЯТИ (`op_8`, rev5 M-49), но не по ВРЕМЕНИ: в состоянии
  `Unknown` он прочитывал и декодировал ВЕСЬ каталог (прод: 158 сегментов ≈ 140 GiB сырых), и
  входился этот путь ровно тогда, когда оператор поднимает recorder ПОД ИНЦИДЕНТОМ. После M-50
  стало хуже сверхлинейно (замер: 16 MiB мусора — `0.182 s` до M-50 против `384.94 s` после).
  Теперь работа списывается против ЕДИНОЙ именованной константы
  `READABLE_FLOOR_WORK_BUDGET_BYTES = 8 × DEFAULT_MAX_SEGMENT_BYTES` — одной на ВЕСЬ вызов
  (все сегменты каталога), включая байты side-верификации крупных кандидатов. Исчерпание в
  ЛЮБОЙ точке ⇒ весь вызов даёт `Unknown`; **частичный `Known` невозможен** — заниженный пол
  и есть seq-reuse. Деградация только в сторону отказа; свойства rev5 (терпимость, O(1)
  памяти) и JR-I-9 (размер ≠ порча) не разменяны — `READABLE_SCAN_MAX_CARRY` и
  `FRAME_LEN_SANITY_CAP` не тронуты (канарейка T7).
- **`JR-I-11` (TD-030) — машинный guard монотонности сшивки.** Один общий хелпер
  `check_first_seq_monotonic` на ТРЁХ путях чтения: `segments()` (⇒ `stream`/`stream_from`),
  `read_all`, `readable_floor` (guard идёт ПЕРВЫМ, до бюджетного скана).
  > **Поправка 2026-08-14 (merge `362784a`, `R-077` APPROVED): путей ЧЕТЫРЕ, а не три.**
  > `recover` был четвёртым публичным путём сшивки и жил БЕЗ guard'а с M-52 — дефект
  > до-существующий, а не внесённый (спека M-52 `recover` не называла никогда). Закрыт
  > `TD-141`: `crates/journal/src/lib.rs:473` зовёт `check_monotonic_paths` ЗЕРКАЛЬНО
  > `read_all` — до толерантного чтения тел сегментов; оракул `mn_9`
  > (`red_stitch_monotonic.rs`, +`mn_10` позитивный контроль, +`mn_11` границы).
  > Толерантность `recover` к CRC/torn ВНУТРИ сегмента не разменяна и запиннена обратной
  > мутацией (строгий `recover` роняет `recover_resyncs_across_torn_frame`).
  > **Число «три» держится только в этом абзаце и подобных перечислениях — сверять кодом:**
  > `grep -n "check_monotonic_paths\|check_first_seq_monotonic" crates/journal/src/{lib,segments}.rs`.
  Правило: сравнимые
  `first_seq` НЕ УБЫВАЮТ по индексу сегмента; отказ — `Err`, называющий ОБА файла и их
  `first_seq`. Два обязательных carve-out'а: legacy исключается **по `schema_version`**, а не
  по значению `first_seq == 0` (у первого v2-сегмента здорового журнала он тоже 0 — наивный
  признак выключил бы guard на самом частом каталоге, класс TD-011), и равенство законно для
  ПУСТОГО левого сегмента (`JR-I-8` случай 3; проверка стоит ОДИН фрейм и выполняется только
  при равенстве). Условие закрытия TD-030 из `R-002`/`R-003` («покрыть и `readable_floor`») —
  ВЫПОЛНЕНО.
- **`JR-I-12` (TD-067) — детерминизм стал НАБЛЮДАЕМ в поле.** `--mode replay-digest
  [--from/--to/--expect]` в уже доставляемом `journal-retention`: потоковый расчёт (не
  `read_all`/`recover` — 26 GB/148 млн событий в RAM это класс TD-011), печать
  `events/first_seq/last_seq/state_hash`, атомарная запись `journal.replay-digest.json`,
  выделенный **exit 4** на расхождении с `--expect` (1/2/3 заняты), read-only по данным
  журнала. Recorder дайджест не считает НИКОГДА. **Оговорка доставки — TD-075 ниже.**

**Гейты (замер reviewer'а, не перенос).** `verify_M-52.sh` — **31 PASS / 0 FAIL / exit 0**
(воспроизведено независимо от прогона architect'а и от Done Block критика);
`cargo test --workspace` — **721 passed / 0 failed / exit 0**; `cargo fmt --all --check` и
`cargo clippy --workspace --all-targets -D warnings` — чисто. Оракулы M-49/M-50/M-51 прошли
**без единой правки** (проверено диффом, не утверждением). Регресс `-p gateway` (потребитель
`segments()`/`stream`) GREEN.

**Анти-плацебо — проверено по существу, а не по форме.** Гейт на 2/3 состоит из греп-канареек,
поэтому reviewer отдельно разбирал, что закрыто ИСПОЛНЯЕМЫМ поведением: все три долга стоят на
поведенческих оракулах (`wb_*` — через публичный `Journal::open_with` с операторской
декларацией; `mn_*` — реальный re-stitch через подмену файлов; `rd_*` — спавн РЕАЛЬНОГО бинаря
через `CARGO_BIN_EXE`), а греп занимает ровно те позиции, где теста быть не может (форма
константы, признак legacy, состав образа). Мутационный контроль критика (`C-047`, 8 независимых
точек) подтвердил это фактически: восемь поломок реализации — восемь пойманных оракулами; его
мутация №6 отдельно доказала, что греп-канарейки T3/T4 сами по себе обманываются
нейтрализованным guard'ом, а решающим слоем является исполнение T8.

**Процессные отклонения этого milestone'а (названы, не спрятаны):**
- **plan-time critic ПРОПУЩЕН** — milestone сам объявлял цепочку «critic → engine-dev →
  tester → reviewer», триггеры `gates.md` §1 сработали; вердикт `C-047` получен ПОСТФАКТУМ
  (итог NOTE, не REJECT). Tester тоже пропущен — его роль выполнил reviewer собственным
  прогоном.
- **3 коммита на 7 задач.** Разобрано в `R-022` F-5: 4 из 5 задач в бандле физически
  неразделимы (без carve-out'а guard роняет 6 тестов `gateway`; бюджет меняет сигнатуры
  насквозь), но бюджет и guard — два независимых механизма и следовали разными коммитами.
  Названная цена: guard встал на ПРОД-путь чтения и откатить его отдельно от бюджета нельзя.

**Долги, приземлённые вместе с фиксом:** **TD-075** (режим доставлен в образ, но не в
ops-поверхность: сервис `journal-retention` монтирует `/journal:ro`, а режим требует RW —
канарейка T5 проверяет `Dockerfile` и не проверяет `docker-compose.yml`), **TD-076**
(`recover()` — четвёртый путь сшивки без guard'а; найдено reviewer'ом и критиком независимо),
**TD-077** (guard применяется ДО фильтра эпох ⇒ смешанный по эпохам каталог станет нечитаем
целиком), **TD-078** (оракулы меряют СЕКУНДЫ, а CI гоняет DEBUG: замер — 21.15 s при потолке
60 s, запас ×2.84; риск красного `main` на медленном раннере), **TD-079** — уже CLOSED тем же днём (`docs/DESIGN.md:840`
§22 не обновлён под `JR-I-10/11/12` ⇒ **`verify_design_claims.sh` краснеет на `main` начиная с
этого merge** — замер: PASS на `origin/main`, FAIL на merge-коммите). **TD-054** остаётся
OPEN: бюджет ограничивает сверхлинейность side-верификации, но не устраняет её.

**Прод-форма — снята ЗАМЕРОМ (ssh, read-only, 2026-08-02), а не вспомнена:** 158 сегментов
(индексы 1..158 — `segment-00000000` удалён ретеншеном), 152 `.zst` + 6 сырых, дублей индекса
нет, `journal.legacy.json` = `{"declarations": []}` — **legacy-сегментов на проде НЕТ ни
одного**. Последнее уточняет обоснование carve-out'а в milestone'е (`R-022` F-6): carve-out
корректен и обязателен, но названная в спеке причина («наивный guard уронил бы чтение боевого
каталога») для СЕГОДНЯШНЕГО прода неверна.

**§8 деплой-гейт — GREEN (прод `a1fc098`, 2026-08-02).** CI success (`30750811080`, job
`fmt + clippy + test` 8m56s против ~6m00s базовых — оракулы M-52 в debug добавили ~3 минуты,
риск TD-078 на этом прогоне не реализовался) + Deploy success (`30750811083`); VPS HEAD совпал;
`hft-recorder` и `hft-gateway-serve` — `healthy`, `restarts=0`; heartbeat свежий, `next_seq`
149 477 911 → 149 478 174 за 10 s без разрыва, `segment_index=158` не прыгнул, `writable=true`,
84.4 GB свободно.

**Главный прод-риск milestone'а закрыт замером, а не рассуждением.** Guard JR-I-11 встал на
`segments()` — путь, от которого зависят старт recorder'а, `gateway-serve` и cron ретеншена;
ложное срабатывание на боевом каталоге (158 сегментов, 152 `.zst` + 6 сырых) означало бы
остановку СБОРА. Проверено с двух сторон: (1) оба контейнера поднялись на новом коде и пишут;
(2) явный прогон пути `segments()` cron-командой — `docker compose --profile ops run --rm
journal-retention` (dry-run) → `EXIT=0`, `offloaded: 0 pruned: 0 failed: 0`, ни одного
`monotonic`, ни одной ошибки чтения каталога.

**TD-067 ЗАКРЫТ прод-прогоном (условие из его же карточки).** Два запуска подряд на ЗАКРЫТОМ
окне `[149000000, 149000999]` командой из runbook'а — обе выдали ОДИН `state_hash`
`d0ce3d95…57e59d4`, exit 0, машинная запись `journal.replay-digest.json` создана.
`--expect` проверен на проде: совпадение → exit 0, `deadbeef` → **exit 4** с печатью обеих
величин. **Потоковость доказана на БОЕВОМ журнале:** первый замер пика по cgroup
(`memory.peak` = 1479 MiB на окне 2 млн) оказался ловушкой — это page cache; честная величина
`VmHWM` процесса — **5608 kB на окне 200 тыс. и 5748 kB на окне 2 млн событий** (окно ×10 →
память +2.5 %). Класс TD-011 на новом операторском пути не воспроизводится.

**TD-075 подтверждён на проде, а не остался гипотезой:** штатный ops-сервис считает дайджест
верно и тут же объявляет прогон неуспешным — `не удалось записать journal.replay-digest.json:
Read-only file system (os error 30)`, `EXIT=1`; плюс переопределение args в `docker compose run`
ТЕРЯЕТ `--dir` из compose-`command` (`No such file or directory`), о чём не предупреждает ни
runbook, ни `deploy/README.md`. Работает только обходная команда через сервис `recorder`.


## M-46 — сквозная проверка read-path без фронта (⚠️ КОД В MAIN, milestone **НЕ ЗАКРЫТ**; reviewer `R-025`: артефакты APPROVED, close-out **REJECTED**)

**Что теперь ЕСТЬ (проверено reviewer'ом на чистом worktree `25c8b67`).**

- **Восемь sacred-оракулов транспортного слоя** в `crates/gateway-serve/tests/`
  (`red_ws_series_vs_replay.rs`, `red_ws_protocol.rs`, `red_ws_honesty_sessions.rs`, все —
  architect): O-1 присутствие всех 10 полей `SeriesBundle` на СМЕШАННОЙ фикстуре
  (`Trade`+`L2Snapshot`+`L2Delta`, две UTC-сессии, асимметричный дифф, мульти-филл);
  **O-2 — главный: WS-выдача поэлементно == `gateway::snapshot` того же журнала по КАЖДОМУ из 10
  полей отдельным ассертом**; O-2w — то же в WINDOWED-режиме (режим прода); O-3 сходимость кадров;
  O-4 матрица авторизации (5 веток fail-closed); O-5 окно/чекпоинт по `ReadStats`; O-6 честность
  `history_truncated`; O-7 парный — CVD сбрасывается на 00:00 UTC, VWAP нет.
  Это закрывает дыру `smoke_ws.rs`, чья фикстура из 4 `Trade` структурно оставляла `heatmap`/`cob`/
  `depth_series` всегда пустыми, а `verify_M-28.sh:51` проверял лишь `[ -f ... ]` — форму, не поведение.
- **Мутационный контроль — не декларация.** Critic (`C-055`, вердикт NOTE) лично воспроизвёл все 4
  заявленные мутации 1-в-1 (включая точное значение CVD `+5.0`) и внёс пятую, которая вскрыла
  слепую зону: `cvd_session_base` сравнивался как `[]==[]`, т.к. заполняется только после оконной
  эвикции. Закрыто O-2w (`314541d`). **Прод подтвердил, что зона была реальной:**
  `cvd_session_base = 1` (непусто) при `GATEWAY_WINDOW_MS=60000`.
- **`wsprobe`** (`crates/gateway-serve/src/bin/wsprobe.rs`, engine-dev) — read-only WS-харнесс:
  дамп `snapshot.json`/`frames.jsonl`/`summary.json`, ASCII-панель и автономный `panel.html`
  (инлайн CSS/JS, без внешних ресурсов), режим `--self-test` без сети. Собирается и копируется в
  прод-образ (`Dockerfile`, задача 5a; `ENTRYPOINT` не тронут).
- Гейты на `25c8b67`: `fmt` exit=0 · `clippy --workspace --all-targets -D warnings` exit=0 ·
  `cargo test --workspace` **778 passed / 0 failed (188 блоков)** · `verify_M-46.sh` **VERDICT: PASS,
  exit=0** (T0–T9). Совпадает с независимым прогоном tester'а на `48c93b3`.
- Scope/Block-C чисты: `crates/gateway/src/**`, `crates/contracts/**`, `risk`/`killswitch`/`oms`/
  `venue-*`, `docker-compose.yml` — **не тронуты** (замер `git diff --name-only` → пусто).
  RISK-BLOCK неприменим: order-egress отсутствует, `VB-I-3`/`GS-I-1` соблюдены. Граница C не
  затрагивается.

**Почему milestone НЕ закрыт.** Задача #5 (sidecar-прогон против живого прода) не была выполнена
никем; reviewer выполнил её на §8-гейте — и она **провалена по существу**:

- **`TD-083` (CRITICAL, блокер).** Любое WS-подключение навсегда заклинивает прод-`gateway-serve`.
  Воспроизведено 3 раза подряд: после ухода клиента процесс остаётся на **100% CPU**, `CLOSE_WAIT`
  копится (2→10 за 4 мин), новый клиент получает `connect timeout после 30s` (в логе нет ни
  `ws auth ok`, ни `rejected` — accept-loop не исполняется), кадров не приходит ни одного
  (`frames_received=0`). Docker при этом рапортует **`(healthy)`** — healthcheck TCP-connect
  удовлетворяется ядром из listen-backlog. Причины: `frames_since` читает журнал **с головы** на
  каждом тике (`crates/gateway/src/lib.rs:1772` `journal::stream` — тогда как snapshot-путь `:1885`
  уже использует `stream_from`, фикс M-38b/GW-I-11 на push-путь не распространили) ⇒ ≈12 минут на
  один 250-мс тик; плюс однопоточный рантайм (`main.rs:17` `flavor = "current_thread"`,
  `/proc/1/task` = 1) с синхронным journal-read внутри `select!`. **Дефект предсуществовал — M-46 его
  НАШЁЛ**, что и было целью задачи #5; на фикстурах он ненаходим (O-3 зелёный и останется зелёным).
- **`TD-084` (MAJOR, блокер).** `wsprobe --secret` не аутентифицируется против прода: харнесс
  hex-декодирует секрет (`wsprobe.rs:155-158`), сервер берёт сырые UTF-8 байты
  (`gateway-serve/src/lib.rs:629`). Прод-секрет — 64 hex-символа ⇒ подпись не сходится всегда.
  `--self-test` использует не-hex секрет, поэтому ветка не исполнялась в гейте ни разу.

**Замеры прода (задача #5, чек-лист §7 milestone'а).** Первый `Snapshot`: латентность **8737 ms**
(повтор 7877 ms) против 1.056 s в M-48 — деградация из-за `TD-085` (чекпоинт не обновляется с
04:00, отставание 1 710 345 событий, cron/timer отсутствуют). Размер `Snapshot` — **1 277 442 B
(1.22 MiB)** при запасе ×52 до дефолтного лимита `tungstenite` 64 MiB, но лимит достижим
операторской ручкой `GATEWAY_WINDOW_MS` (`TD-091`); размер наибольшего `Frame` замерить не удалось —
кадров нет (`TD-083`). Длины десяти серий: `ohlcv/cumulative_delta/vwap` = 52, `cvd_session_base` = 1,
`depth_series` = 2, `volume_profile`/`vp_session_max_time_s` = 1, `heatmap` = 2110, `cob` = 90,
`volume_bubbles` = 170 — **ни одно поле не пусто**. `history_truncated=true`,
`history_start_seq=16049334` — ожидаемо. **Осмысленность подтверждена глазами:** книга не скрещена
(best bid 62648.82 < best ask 62648.83, спред 1 цент), цены ≈62.6k правдоподобны, VWAP 64489.71
выше спота — корректно, т.к. all-time БЕЗ ресета (M-36); CVD −86.18 не абсурден.

**§8 деплой-гейт: прод жив по ЗАПИСИ, деградирован по ЧТЕНИЮ.** Четыре красных Deploy разобраны по
логам — ни один не про код M-46: гонка одновременных деплоев с конфликтом имени контейнера
(`TD-086`, `concurrency` не задан) и сетевая флака `cargo audit` (503 от advisory-db), уронившая
весь CI при зелёном `fmt+clippy+test` (`TD-087`). Образ на VPS собран и содержит `wsprobe`
(проверено `ls` внутри контейнера). Eyes-on после восстановления: `hft-recorder` и
`hft-gateway-serve` — `(healthy)`, CPU 0.00%/2.10%, heartbeat свежий (отставание 7.6 s),
`next_seq` растёт (154 993 921 → 155 035 617), `writable=true`, 76 GiB свободно, `segment_index=164`.
**Запись данных не пострадала ни разу** — весь дефект лежит на read-path. Прод восстановлен
reviewer'ом `docker restart hft-gateway-serve` (перезапуск сервиса; env/конфиг/тома не изменялись) —
**заклинит снова при первом реальном подключении, до фикса `TD-083`**.

**Следующий шаг:** architect — RED-оракул ОГРАНИЧЕННОЙ работы push-цикла на прод-масштабном журнале
(по образцу `crates/journal/tests/red_open_bounded.rs`, через `ReadStats`, не wall-clock), затем
дизайн фикса `TD-083`/`TD-084`. Milestone остаётся `IN_PROGRESS`.

## M-54 «единая стоимость подключения» (`TD-093`) — ⚠️ КОД В MAIN (merge `aeb409b`), milestone **НЕ ЗАКРЫТ**; reviewer `R-029`: артефакты APPROVED, close-out **FAIL**

**Что в `main`.** Подключение к `gateway-serve` больше не считает состояние дважды.
У `LiveReducer` появился `snapshot(&self) -> Snapshot` — **без** параметров `dir`/`filter`, то
есть второй проход по журналу невозможен ПО ПОСТРОЕНИЮ (барьер держит компилятор, тот же приём,
что `RK-I-1`). `run_authorized_session` (`crates/gateway-serve/src/lib.rs:348-604`) теперь:
`resume` → `pump` до хвоста → `ServeMsg::Snapshot(live.snapshot())`. Состояние живёт в новом
поле `full: Reducer`, которое `pump()` кормит КАЖДЫМ событием (та же per-event оконная эвикция,
что у независимого реплея).

**Эталон M-46 сохранён намеренно:** `snapshot_from_checkpoint` остался в `serve::snapshot_msg` —
на нём стоит сверка WS↔реплей (`red_ws_series_vs_replay`); удалить его значило бы превратить
эту сверку в тавтологию. Обратная канарейка T4b в `verify_M-54.sh` это охраняет.

**Оракулы (architect, sacred).** `crates/gateway/tests/red_connect_cost_single.rs`: O-1
(снапшот из состояния, не из журнала — доказывает сигнатура), O-2 (поэлементное равенство
независимому полному реплею `gateway::snapshot(..., LATEST)`), O-3 (нет дыры между снапшотом и
началом push — закрывает `TD-093(а)`). **Нетавтологичность проверена reviewer'ом мутациями
реализации, а не принята на слово:** выброс чекпоинт-базы → O-2 FAILED; снятие докорма хвоста →
O-2 FAILED; пустой снапшот → O-1 + O-2 FAILED.

**Гейты (прогон reviewer'а на РЕЗУЛЬТАТЕ МЕРЖА, не на типе ветки).** `fmt`/`clippy`/
`cargo test --workspace` (**788 passed / 0 failed**, 191 блок) / `verify_M-54.sh` **PASS 12/12** —
все `exit=0`; барьер артефактов при CI-проводке OK, самопроба 18/18. §8: CI success + Deploy
success на `aeb409b`, контейнеры healthy, heartbeat свежий, `next_seq` растёт.

**Почему milestone НЕ закрыт — §6 требовал прогона против прода, и прогон цель не подтвердил.**
Замер (18 подключений, все при CPU < 5 %):

```
ДО    M-54 (24f9201):  250 ms + 6.67 мкс/событие
ПОСЛЕ M-54 (aeb409b):  654 ms + 1.70 мкс/событие
```

Наклон (догон хвоста) — **в 3.9 раза лучше**: второй проход действительно устранён. Константа —
**+404 ms (×2.6 хуже)**, память ×3 (28.8 → 91.6 MiB). Точка безубыточности ≈ 81 300 событий
backlog'а при рабочем диапазоне прода 0…66 600 ⇒ **на реальном проде подключение стало дороже
всегда**. Долг: **`TD-097`** (MAJOR); `TD-093(б)`/`(в)` остаются OPEN, закрыт только `(а)`.

**Побочно исправлена методика замеров всего проекта.** Прежние числа латентности (5-14 s и
28.5/142.5/66.3 s в `R-026`) мерили **конкуренцию за однопоточный рантайм** `gateway-serve`
(`/proc/1/status → Threads: 1`), а не стоимость подключения: при `CPU_before=98 %` тот же код
даёт 553/534/**1494** ms, при `CPU 0 %` — 676/640/646 ms. Тезис `M-54` §6 о «постоянной
составляющей 9-12 секунд» опровергнут: на свободном сервере она была **250 ms**. Требование ко
всем будущим замерам — проверять CPU ПЕРЕД прогоном (зафиксировано в `TD-097`).

**`TD-094` CLOSED** тем же merge'ем: флакающий sacred-оракул цены тика
(`td083_tick_wallclock_does_not_grow_with_history`) переведён с настенного времени на
детерминированную **работу** (`ReadStats.events_decoded`) + минимум из 5 прогонов как
подтверждающая мера — лечение класса `TD-023`, а не поднятый порог.

## M-56 «снапшот без клонирования состояния» (`TD-097`) — ⚠️ КОД MERGED (`c714d0f`) + В ПРОДЕ, reviewer APPROVED (`R-030` §2); **milestone НЕ ЗАКРЫТ** — замер на проде: критерий §6 НЕ выполнен (`R-030` §3)

Закрывает регрессию, которую reviewer нашёл на PR-гейте M-54 (`R-029` §C): M-54 устранил
второй проход по журналу (наклон **×3.9 лучше**), но константа подключения выросла на
**+404 ms**, а точка безубыточности (backlog ≈81 300) при рабочем диапазоне прода 0…66 600
недостижима — то есть подключение стало дороже при ЛЮБОМ backlog'е.

**Причина (одна строка).** `LiveReducer::snapshot(&self)` делал
`self.full.clone().finish_with_at()` — клон ВСЕГО редьюсера, включая книгу целиком, потому
что `Reducer::finish(self)` потребляет `self`.

**Что сделано.** `Reducer::finish_ref(&self) -> SeriesBundle` строит серии из ссылок;
парный `finish_ref_with_at(&self)`; `snapshot()` зовёт его. Клон исчез.

**Ключевое для сопровождения — формула существует в ОДНОМ месте, не в двух:**

```rust
fn finish(self) -> SeriesBundle { self.finish_ref() }   // тонкая обёртка
fn finish_ref(&self) -> SeriesBundle { /* вся формула здесь */ }
```

Владеющие дубликаты не оставлены, а удалены: `VolumeProfileAcc::into_rows(self)` →
`vp_rows(&self)`; `compute_vp_row`, `build_heatmap_and_cob`, `build_volume_bubbles`
переведены на `&BTreeMap`. Двум копиям формул разойтись негде — это была явная цель задачи
и она достигнута сужением сигнатур, а не копипастой.

**Гейты.** `verify_M-56.sh` — **PASS 10/10**, exit=0 (прогон reviewer'ом на результате
merge'а, не на ветке). `cargo test --workspace` — **791 passed / 0 failed** (192 блока;
против 788/191 на M-54 — ровно +3 блока O-1..O-3). CI `c714d0f` — success.
RED-first подтверждён логом: `3b01077` (architect, оракулы) предшествует `bcb47c8` (impl);
тесты devом не переписаны.

**Мутационный контроль воспроизведён reviewer'ом независимо** (не принят на слово из отчёта):
возврат `self.full.clone().finish_with_at()` валит O-1 — аллокации 151089 → 854705 (**×5.66**)
при книге ×8 и неизменном выходе (×1.00).

⚠️ **`TD-098`:** в том режиме, которым O-1 зовёт acceptance-гейт (весь набор в одном бинаре,
параллельные потоки), глобальный счётчик аллокаций может загрязниться соседними тестами —
поймано **один раз из нескольких прогонов одной и той же команды**: отношение упало до
**×2.89** при пороге 2.5 (запас 16 % вместо 126 %). `--test-threads=1` загрязнение снимает,
чем диагноз и подтверждается. Непостоянство здесь хуже стабильности, а снос направлен ВНИЗ —
к ложному GREEN. Не блокирует merge: корректность влитого кода установлена независимо от O-1
(сигнатура `finish_ref(&self)` не может потребить состояние; канарейка T4 ловит `.clone()`
грепом детерминированно; O-2 сверяет с полным реплеем). Дизайн защиты — за architect'ом.

**§8 деплой-гейт GREEN.** CI `c714d0f` success + Deploy success; на проде проверено не по
SHA деплоя, а по исходникам: `grep -c finish_ref crates/gateway/src/lib.rs` = 13, контейнер
собран 20:43:45Z, оба контейнера healthy, recorder пишет (`writable: true`, heartbeat свежий
2.3 s, свободно 76 GB).

### ⚠️ Замер на проде: цель milestone'а НЕ достигнута — `TD-097` остаётся OPEN

Протокол `R-029` §C: предсказание зарегистрировано ДО замера (20:34:11Z), замер начат
20:44:22Z, CPU 0.00 % перед каждым прогоном, 3 точки backlog'а × 3 подключения.

| backlog | latency первого `Snapshot`, ms | MEM |
|---|---|---|
| 46 363 | 3711 / 1901 / 3658 | 20.64 MiB (контейнер холодный, Up 1 мин) |
| 38 122 | 2679 / 1837 / 1845 | 14.34 MiB |
| **509** | **2742 / 3555 / 1834** | 20.03 MiB (Up 17 мин) |
| **426** | **3790 / 2875 / 1934** | 20.42 MiB (Up 32 мин, отдельный чекпоинт) |

**Решающие точки — третья и четвёртая:** backlog практически НОЛЬ (509 и 426 событий, снято
через 18 s и 12 s после своих чекпоинтов), а подключение всё равно стоит **1.8–3.8 s**.
Четвёртая точка снята сверх мандата специально — через 15 минут, на другом чекпоинте, при
прогретом 32 минуты контейнере — чтобы вывод не держался на одном замере. Разброс внутри
одной точки перекрывает разницу между точками ⇒ зависимости от backlog'а в данных нет вовсе.
Критерий §6 («константа ≈250 ms при наклоне ≈1.7 мкс/событие») **НЕ ВЫПОЛНЕН** — константа
осталась в районе 1.8–3.7 s, то есть хуже и целевых 250 ms, и замеренных в `R-029`
post-M-54 654 ms.

**Что M-56 всё-таки сделал, и это подтверждено на живом проде:** клон исчез не только из
кода, но и из памяти — RSS `gateway-serve` под нагрузкой **91.6 MiB → 14.3–20.6 MiB**
(в 4.5 раза). Механическая цель достигнута; пользовательская — нет. Значит **+404 ms из
`R-029` объяснялись не только клоном**.

**Куда смотреть (гипотеза, контрольным экспериментом НЕ подтверждена).** Единственная
систематическая разница между замерами — **размер открытого сегмента журнала**: `R-029`
мерил около 19:0x при сегменте 167 в ~50–150 MB, `R-030` — при **571 MB** (сегменты режутся
по 1 GiB раз в ~3.5 ч). Рост сегмента в 4–6 раз против роста латентности в 3–5 раз. Это
ровно то, что предсказывает `DESIGN.md` §16.2 шаг 3 («прод деградирует уже при 1–2 зрителях,
а к сегменту в 1 GiB и при одном») — и делает шаг 3 следующим по очереди не по плану, а по
замеру.

**Следствие для методики.** «Константа подключения» не воспроизводима без фиксации размера
открытого сегмента: `R-029` и `R-030` оба соблюдают протокол §16.3 и всё равно несравнимы,
потому что доминирующее условие в протоколе не перечислено. Протокол нужно дополнить —
тем же способом, каким его дополнили CPU.

Откат M-56 не требуется и не предлагается: память строго лучше, тесты 791/791, регресс-наборы
M-46/M-53/M-54 зелёные.

## Пока НЕ реализовано (следующие фазы)
- Крейты `risk`/`killswitch`/`oms`, `runner` — пофазно per DESIGN §10 (M-08: fail-closed риск-гейт
  между `strategy` и `oms`). MM-котирование, wiring весов из `signals.json` (граница B),
  netting/корреляции — вне M-07 (named-not-silent). `book` microprice/depth-полосы сверх
  M-04-примитивов — по мере надобности.
- Формат журнала: сегмент-ротация и компакция — ЕСТЬ (M-08/M-40); `state_hash` и
  бит-идентичный реплей окна — **ЕСТЬ с M-51** (`journal::replay_digest`, `DET-I-1/2/3`).
  Остаётся: снапшоты/чекпоинты как первоклассный формат журнала (сейчас чекпоинт живёт в
  gateway, M-38b/M-48) и версионирование кода редьюсера в watermark проекции (`DESIGN.md`
  §14, blue/green) — пофазно.
