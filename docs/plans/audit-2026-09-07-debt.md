<!-- FACTS: audited_head=91a4b84d6f8a71760767e9ca3cfa9a0d1d27fa23 collected=2026-09-07 -->

# Триаж реестра долга (TECH-DEBT.md) — 2026-09-07

Разбор всех 127 заводящих карточек `TD-NNN` в `TECH-DEBT.md` на ревизии
`91a4b84` (`origin/main`). Каждая карточка проверена ОТДЕЛЬНОЙ командой на этой
ревизии (грепом/чтением кода; где дёшево — `git log`/`git merge-base`), не
переписыванием статуса из таблицы или тела. Работа разбита на 6 параллельных
проходов (по ~20-22 карточки), каждый прогонял свои команды независимо на
общем чекауте `/tmp/hft-audit-debt`; здесь — сведённый результат.

**Итог по вердиктам:**

| вердикт | штук |
|---|---:|
| ЖИВОЙ | 95 |
| ЗАКРЫТ (дефекта на коде нет) | 27 |
| НЕПРОВЕРЯЕМ дёшево (нужен прод/синтетика/секрет) | 5 |
| ОТЛОЖЕНО (risk/killswitch/oms/order-execution/live) | 0 — зона проверена явно, см. ниже |
| **Всего** | **127** |

**Зона risk/killswitch/oms/execution проверена и пуста.** Команда, которой это
установлено:

```
$ grep -liE 'crates/risk|crates/killswitch|crates/oms|RK-I-[0-9]|order-egress|живая торговля|live-торгов|submit/cancel' cards/*.txt
(без вывода)
```

Ни одна из 127 карточек не касается отложенного торгового пути — фраза «ВНЕ
ЗОНЫ» в задании была подстраховкой, а не описанием фактического состава реестра
на сегодня: этот слой физически ещё не начат кодом, поэтому долга по нему в
файле пока просто нет.

**Реестр — не два источника правды, а МНОГО.** Задание указывало на пример
TD-162 (таблица «закрыт», тело «MAJOR»). Проверка показала: TD-162 — как раз
образец ПРАВИЛЬНО устроенной карточки (таблица помечена `~~TD-162~~ ✅ ЗАКРЫТ`,
и ПОД ней стоит явный closed-callout с разбором ДО того, как идёт старое тело —
тело сохранено сознательно как носитель разбора, это не расхождение). Реальная
и гораздо более крупная проблема — другая: **21 карточка закрыта в коде, но НЕ
несёт вообще НИКАКОЙ пометки о закрытии** — ни в теле, ни (у 7 из них) в
таблице. Раздел «ПРОТУХШИЕ ЗАПИСИ» ниже — про это.

## 1. Таблица по всем 127 карточкам

Столбцы: `TD-NNN` | вес (severity из карточки) | вердикт | команда → первая
строка её результата (полный вывод — в разделах 2/3 для крупных и протухших) |
суть одной строкой. Символ `¦` — экранированный `|`, встретившийся в самом
тексте команды/вывода (иначе рвёт таблицу).

| TD | вес | вердикт | команда -> результат (кратко) | суть |
|---|---|---|---|---|
| TD-001 | MINOR | ЖИВОЙ | `grep -n "^FROM\¦^USER" Dockerfile ; tail -20 Dockerfile` -> `FROM rust:1-slim AS builder` | Контейнер recorder всё ещё без директивы USER — работает root'ом. |
| TD-002 | MINOR | НЕПРОВЕРЯЕМ | `grep -rn "hetzner-server" deploy/ docs/ 2>/dev/null ¦ grep -i key` -> `(пусто — совпадений нет)` | приватный ключ VPS засветился в чате; задача — пересоздать ключ и заменить на сервере, это опер… |
| TD-003 | NOTE | ЖИВОЙ | `grep -rln "fn submit_order¦fn place_order¦fn cancel_order¦order-e…` -> `crates/venue-binance/src/recon.rs   (MD-only, "без подписи, без o…` | Ордерная сторона (submit/подпись/rate-limit) для Hyperliquid и Binance пока не реализована вооб… |
| TD-005 | NOTE | ЖИВОЙ | `grep -n '"type":' crates/venue-hyperliquid/src/*.rs` -> `30:            "subscription": { "type": "trades", "coin": coin }…` | Hyperliquid-адаптер подписан только на trades и l2Book; bbo/funding/liquidations по-прежнему не… |
| TD-006 | MAJOR | ЖИВОЙ | `'sed -n '1918,1922p' TECH-DEBT.md'` -> `¦ Ф0 «Необратимость данных» — вне пути Ф1–Ф4 (сама Ф0 по 'DESIGN'…` | Журнал по-прежнему растёт без реального ретеншена/офсайт-выгрузки; сам актуальный реестр подтве… |
| TD-008 | NOTE | ЖИВОЙ | `grep -n "T1-designate\¦report_schema_version" crates/research-cli…` -> `5://! report_schema_version. Промоушен в crates/contracts — отдел…` | T1-типы отчётов всё ещё временно живут в research-cli (не в crates/contracts) — ждут появления … |
| TD-009 | NOTE | ЖИВОЙ | `ls research/reports/ ¦ grep -i R-001` -> `(пусто — файлов R-001* нет; каталог содержит только tester/engine…` | Отчёт по OBI Треку A/B (R-001) так и не появился — гейт (данные + risk-critic + подпись founder… |
| TD-010 | NOTE | ЖИВОЙ | `'grep -n "DEPTH_LIMIT\¦limit=5000" crates/venue-binance/src/recon…` -> `crates/venue-binance/src/recon.rs:60:/// 'limit=5000' — топ-5000 …` | REST resnapshot Binance по-прежнему ограничен топ-5000 уровнями, пагинации нет. |
| TD-012 | NOTE | ЖИВОЙ | `grep -n "REST_DEPTH_LIMIT\¦REST_DEPTH_BASE" crates/venue-binance-…` -> `36:const REST_DEPTH_BASE: &str = "https://fapi.binance.com/fapi/v…` | REST-запрос глубины книги для фьючерсов Binance по-прежнему жёстко ограничен 1000 уровнями, паг… |
| TD-015 | MAJOR | ЖИВОЙ | `wc -l research/trials-ledger.jsonl; cat research/trials-ledger.js…` -> `4 research/trials-ledger.jsonl` | Правило «в метрики брать только записи кода ≥5141fd9» по-прежнему актуально: ledger не пополнил… |
| TD-016 | MAJOR | ЖИВОЙ | `grep -rn "BACKSTOP_LEVELS_PER_SIDE\¦MAX_REL_DIST" crates/venue-bi…` -> `crates/venue-binance/src/lib.rs:56:pub const BACKSTOP_LEVELS_PER_…` | Мёртвые уровни книги (цена ушла, size=0 больше не приходит) по-прежнему не эвиктятся по-настоящ… |
| TD-020 | MAJOR | ЖИВОЙ | `'sed -n '1918,1922p' TECH-DEBT.md'` -> `¦ Ф0 «Необратимость данных»... **'TD-020' — offsite-бэкап + resto…` | Ретеншен работает только в dry-run; реального 'apply' в холодное хранилище не было ни разу — по… |
| TD-028 | NOTE | ЖИВОЙ | `grep -n "HashMap<(Venue" crates/research-cli/src/export_io.rs` -> `227:    let mut buckets: HashMap<(Venue, String), InstrumentBucke…` | экспорт по-прежнему копит все trades/snapshots в оперативной памяти за один проход журнала — па… |
| TD-029 | MINOR | ЖИВОЙ | `grep -rln "schema_guard\¦startup_guard\¦refuse_to_start" crates/*…` -> `(пусто — ничего не найдено)` | Recorder при старте всё ещё не проверяет, что активный сегмент журнала декодируется этим бинарё… |
| TD-032 | MINOR | ЖИВОЙ | `'grep -n "rev-parse" crates/recorder/src/main.rs'` -> `crates/recorder/src/main.rs:488: .args(["rev-parse", "--short", "…` | Provenance recorder'а всё ещё вычисляется рантайм-вызовом 'git rev-parse', а не вкомпилирован н… |
| TD-033 | MINOR | ЖИВОЙ | `'sed -n '178p' .github/workflows/ci.yml; grep -n "oneOf.*добавлен…` -> `ci.yml:178: # Классификатор additive/breaking + связь breaking ⇒ …` | Появился машинный классификатор схем в CI, но новый вариант enum классифицируется как ADDITIVE … |
| TD-034 | NOTE | ЖИВОЙ | `'sed -n '674,678p' crates/gateway/src/lib.rs'` -> `let bins: Vec<(i64, i64)> = sorted_bins.into_iter().map(¦(p, v)¦ …` | Экспорт объёмного профиля по-прежнему кастует i128 в i64 без защиты; недостижимо на практике, к… |
| TD-038 | MAJOR | ЖИВОЙ | `grep -n "TD-039" TECH-DEBT.md` -> `966:  host-OOM (TD-039; ...` | CRC-часть дефекта закрыта purge'ем legacy-сегмента (подтверждено самим текстом карточки), но пр… |
| TD-054 | MAJOR | ЗАКРЫТ | `grep -n "READABLE_FLOOR_WORK_BUDGET_BYTES\¦BudgetExhausted" crate…` -> `2610:    let mut budget = WorkBudget::new(READABLE_FLOOR_WORK_BUD…` | M-52 ввёл единый бюджет работы (в байтах); при исчерпании скан деградирует в честный Unknown, о… |
| TD-055 | MAJOR | ЖИВОЙ | `sed -n '83,109p' crates/ops/src/bin/ops-watchdog.rs` -> `for alert in &outcome.delivered {` | При отказе Telegram-доставки код только печатает eprintln!, main() всё равно возвращает Ok(()) … |
| TD-056 | MINOR | ЖИВОЙ | `sed -n '68,73p' crates/ops/src/transport.rs` -> `pub fn from_env() -> Self {` | 'TELEGRAM_API_BASE' из окружения по-прежнему принимается без проверки схемы — опечатка в 'http:… |
| TD-057 | MINOR | ЖИВОЙ | `grep -n "Известные ограничения\¦301\¦redirect" docs/runbooks/aler…` -> `117:## Известные ограничения (на утро, честно)` | Runbook по-прежнему не объясняет редиректы (R-009 F-11) и не называет отдельно «watchdog у дост… |
| TD-058 | MINOR | ЖИВОЙ | `sed -n '362,368p' .claude/rules/gates.md` -> `**Документ проверяется на ДЕРЕВЕ СЛИЯНИЯ, а не на ветке.** ...` | Правило по-прежнему не различает направление расхождения (ветка-красная/main-красная vs preview… |
| TD-059 | MINOR | ЖИВОЙ | `sed -n '415,425p' scripts/verify_design_claims.sh` -> `if n_markers == 0:` | Маркеры [ЕСТЬ], живущие в прозе (не в таблицах), по-прежнему не проверяются машинно — гейт види… |
| TD-060 | MAJOR | НЕПРОВЕРЯЕМ | `н/д` -> `Требуется прогон diff_contract_schema.py на синтетических схемах …` | Классификатор диффа схемы теряет breaking-изменения рядом с additive; проверка требует не grep,… |
| TD-063 | MAJOR | ЖИВОЙ | `sed -n '415,425p' scripts/verify_design_claims.sh (тот же код, чт…` -> `elif n_table == 0:` | Проверки 1/2/5 всё ещё гасятся в INFO при отсутствии нужной структуры в документе — гейт можно … |
| TD-064 | MINOR | ЗАКРЫТ | `grep -n "VERDICT_CLASS_DIRS" scripts/verify_design_claims.sh; git…` -> `700:VERDICT_CLASS_DIRS = ("research/critiques/", "research/review…` | Класс вердиктов исключён из скана битых ссылок — PR #56 (f814347, 2026-08-22) теперь влит в про… |
| TD-065 | MINOR | ЖИВОЙ | `sed -n '425,430p' crates/research-cli/src/export_io.rs; sed -n '2…` -> `_ => "unknown",` | venue_sort_key всё ещё опирается на catch-all _ => "unknown", а Venue всё ещё ровно 3 варианта … |
| TD-066 | NOTE | ЖИВОЙ | `sed -n '363,372p' crates/book/src/lib.rs; sed -n '178,185p' crate…` -> `let mut out: Vec<...> = self` | Канарейка det_22 по-прежнему сравнивает текст ПОСТРОЧНО, а обход self.map.iter() в Books::iter_… |
| TD-068 | MINOR | ЗАКРЫТ | `ls crates/journal/src/segments.rs crates/sim/src/exchange.rs crat…` -> `все 5 файлов существуют (карта из 5 мест подтверждена)` | Дефект был не в коде, а в процессе оценки объёма работ по T1-варианту (карта "по памяти" пропус… |
| TD-070 | MINOR | ЖИВОЙ | `ls crates/contracts/tests/ ¦ grep rfc05 ; grep -n "Прямого RED-те…` -> `ct_rfc05.rs` | Отдельного теста на reuse-барьер эпохи 3→4 по-прежнему нет. |
| TD-073 | MINOR | ЗАКРЫТ | `'sed -n '1193,1206p' scripts/verify_design_claims.sh'` -> `def classify_sha_token(tok, declared, root=None):` | Дефект устранён — маркер теперь машинно проверяется. Реестр не отражает закрытие (карточка всё … |
| TD-074 | MINOR | ЖИВОЙ | `grep -n "SHA_TOKEN_RE" scripts/verify_design_claims.sh` -> `1070:SHA_TOKEN_RE = re.compile(r"'([0-9a-f]{7,64})'")` | регэксп по-прежнему ловит SHA-подобные токены только внутри обратных кавычек — упоминание комми… |
| TD-075 | MAJOR | ЖИВОЙ | `grep -n "replay-digest" docker-compose.yml ; grep -n "^  [a-z-]*:…` -> `(0 совпадений replay-digest)` | Отдельного ops-сервиса для --mode replay-digest в compose так и не завели. |
| TD-076 | MINOR | ЗАКРЫТ | `sed -n '462,472p' crates/journal/src/lib.rs; ls crates/journal/te…` -> `pub fn recover(dir...) {` | 'recover()' теперь тоже прогоняет guard монотонности 'first_seq' — пробел закрыт (устранено вме… |
| TD-077 | MINOR | ЖИВОЙ | `sed -n '1846,1860p' crates/journal/src/segments.rs` -> `pub fn stream_from(...) -> io::Result<EventStream> {` | segments(dir) (несущий JR-I-11 guard монотонности) по-прежнему вызывается на ВЕСЬ каталог до пр… |
| TD-078 | MAJOR | ЗАКРЫТ | `grep -n "CEILING_SCALE" crates/journal/tests/red_floor_work_budge…` -> `98:const CEILING_SCALE: u64 = if cfg!(debug_assertions) { 6 } els…` | Потолок оракула теперь масштабируется ×6 в debug-сборке (какую и гоняет CI) — риск «зелёный лок… |
| TD-080 | MINOR | НЕПРОВЕРЯЕМ | `н/д` -> `Карточка предлагает правку шаблона milestone-спеки; конкретный фа…` | §5 Allowed paths шаблона не перечисляет каталоги артефактов гейтов. |
| TD-081 | MAJOR | ЖИВОЙ | `grep -n "epoch: SystemTime\¦SystemTime::now()" crates/journal/src…` -> `crates/journal/src/lib.rs:69:    epoch: SystemTime,` | ts_mono_ns по-прежнему считается от epoch, который берётся заново при каждом открытии журнала (… |
| TD-082 | MAJOR | ЖИВОЙ | `grep -n "journal_seq_gaps_total" crates/recorder/src/metric_emit.…` -> `crates/recorder/src/metric_emit.rs:15: "нет естественного триггер…` | У метрики "разрывы seq" по-прежнему нет продюсера — обнаружения потери событий журнала как не б… |
| TD-084 | MAJOR | ЖИВОЙ | `grep -n "is_hex\¦from_secret" crates/gateway-serve/src/bin/wsprob…` -> `crates/gateway-serve/src/bin/wsprobe.rs:156:    let is_hex =` | harness wsprobe --secret по-прежнему декодирует hex-подобный секрет в байты, а сервер используе… |
| TD-086 | MAJOR | ЗАКРЫТ | `grep -n "concurrency\¦group:\¦cancel-in-progress" .github/workflo…` -> `concurrency:` | Обе половины устранены — деплои сериализованы, пуш тестов не передеплоивает прод. |
| TD-087 | MINOR | ЖИВОЙ | `sed -n '26,36p' .github/workflows/ci.yml; grep -n "continue-on-er…` -> `security:` | job cargo audit по-прежнему тянет advisory-db по сети и без retry/кэша/послабления входит в бло… |
| TD-088 | MINOR | ЖИВОЙ | `grep -n "Journal::open\¦WriterConfig\¦.append(\¦.flush(" crates/g…` -> `crates/gateway-serve/src/bin/wsprobe.rs:208-289 — WriterConfig/Jo…` | Обе канарейки (текстовый grep GS-I-3 в verify_M-28.sh и текстовый grep LiveReducer:: в verify_M… |
| TD-089 | MINOR | ЖИВОЙ | `grep -rln "slow.consumer¦backpressure¦multi.client¦multi_client" …` -> `(пусто — совпадений нет)` | Зависший клиент (slow-consumer), несколько одновременных подключений и многополосный Selector п… |
| TD-090 | MINOR | ЖИВОЙ | `ls milestones/ ¦ grep M-46` -> `M-46-order-flow-indicators.md` | Номер милестоуна M-46 всё ещё занят двумя разными файлами — путаница для будущего читателя не у… |
| TD-091 | MAJOR | ЖИВОЙ | `grep -n "WebSocketConfig\¦accept_hdr_async" crates/gateway-serve/…` -> `389:    // (callback всегда вызывается синхронно внутри accept_hd…` | Сервер всё ещё принимает WS-соединение без явного WebSocketConfig — действует дефолт tungstenit… |
| TD-092 | MAJOR | ЗАКРЫТ | `grep -n "full-history" scripts/check_protected_artifacts.sh` -> `318: git log --full-history --diff-filter=DR -M --format='%H' "${…` | Флаг --full-history добавлен — барьер больше не объявляет legit rename evil merge. |
| TD-093 | MAJOR | ЖИВОЙ | `grep -n "o3_no_gap_between_snapshot_and_push" crates/gateway/test…` -> `265:fn o3_no_gap_between_snapshot_and_push()` | Гонка снапшот↔live закрыта оракулом (а); двойная стоимость на connect и лишний полный проход в … |
| TD-096 | MINOR | ЖИВОЙ | `find docs/rfc -iname "*CT-RFC-09*"; grep -n "CT-RFC-09" docs/plan…` -> `docs/rfc/CT-RFC-09-ws-session.md` | Номер CT-RFC-09 до сих пор занят дважды — смерженным RFC про WS-сессию и планом миграции, котор… |
| TD-097 | MAJOR | ЖИВОЙ | `(проверка требует живого прода/повторных замеров латентности подк…` -> `(карточка описывает замеры на живом проде 2026-08-03; последняя п…` | Стоимость подключения к панели по-прежнему выше цели milestone'а (1.8–3.8 с вместо ≈250 мс) при… |
| TD-100 | MAJOR | ЖИВОЙ | `grep -n "9.8 MiB" docs/plans/scale-10k-sessions.md docs/DESIGN.md…` -> `docs/plans/scale-10k-sessions.md:34:...≈9.8 MiB...` | Завышенная в ~1.6 раза цифра «9.8 MiB на сессию» так и не исправлена ни в одном из четырёх доку… |
| TD-104 | — | ЖИВОЙ | `grep -n "select_funding_emit" crates/venue-binance-futures/src/li…` -> `1392:    for event in select_funding_emit(events, &subscribed, tr…` | Проводка breadth=true всё ещё жива (call-site подтверждён), но наблюдаемости широты (метрика/ал… |
| TD-105 | — | ЖИВОЙ | `grep -n "branch protection\¦Required status check" .claude/rules/…` -> `gates.md:462: прямой push в main → GH006 Protected branch update …` | Прямой push в main теперь заблокирован защитой ветки, но именно требуемая проверка "merge несёт… |
| TD-109 | — | ЗАКРЫТ | `grep -n "events_scanned\¦O-1 (TD-109)" crates/journal/tests/red_t…` -> `30:  счётчик events_scanned, и только на нём строится главный ора…` | Sacred-оракул переписан: теперь явно проверяет, что events_scanned НЕ является алиасом events_d… |
| TD-112 | MAJOR | ЖИВОЙ | `grep -n 'grep -E "\^(PASS¦FAIL¦VERDICT)"' .claude/rules/commit-di…` -> `68:    verify: bash scripts/verify_M-NN.sh 2>&1 ¦ grep -E "^(PASS…` | Шаблон Done Block по-прежнему предписывает grep ¦ ...; echo exit=$?, что печатает код grep, а н… |
| TD-116 | MAJOR | ЗАКРЫТ | `sed -n '1685,1706p' crates/journal/src/segments.rs; git log --one…` -> `1690: // M-62 §7, условие 6: hint.pos против длины файла.` | Обе недостающие проверки добавлены — hint.pos теперь валидируется против длины файла и границы … |
| TD-117 | — | ЗАКРЫТ | `grep -n "TD-117" crates/gateway/tests/red_tick_read_cost.rs` -> `245: Прямо пиннит то, что TD-117 назвал незапиненным...` | Появился отдельный оракул (red_tick_read_cost.rs, F-036/VB-I-10), меряющий реальное чтение проц… |
| TD-118 | MAJOR | ЖИВОЙ | `grep -n "TD-118" TECH-DEBT.md ¦ head -3` -> `92: TD-118 ¦ ... открыты (а) починка завершения и (в) ревизия чис…` | Реестр сам подтверждает — починка механизма завершения wsprobe не сделана, есть только компенса… |
| TD-119 | MAJOR | ЖИВОЙ | `grep -l "ENOSPC\¦No space left" scripts/verify_*.sh ¦ wc -l; ls s…` -> `1` | Страж «диск кончился, а не код красный» стоит только в одном verify-скрипте из тридцати; gc_wor… |
| TD-121 | MINOR | ЖИВОЙ | `grep -n "общий хелпер на все три пути" crates/journal/src/segment…` -> `1272:    // stream_from (см. check_first_seq_monotonic — общий хе…` | Комментарий по-прежнему говорит «три пути», хотя вызовов guard'а и обслуживаемых путей чтения ф… |
| TD-123 | MAJOR | НЕПРОВЕРЯЕМ | `grep -rln "SIGKILL\¦graceful close\¦обрыв TCP" crates/gateway-ser…` -> `(первая команда — пусто)` | подозрение, что сервер не освобождает поток/сессию, если 20 клиентов обрываются одновременно си… |
| TD-125 | MINOR | ЖИВОЙ | `grep -n "26 сценариев" .github/workflows/ci.yml` -> `107:      # не что инвариант защищён. 26 сценариев — 5 осей × все…` | Имя шага CI и комментарий по-прежнему называют "26 сценариев", хотя проба (по утверждению карто… |
| TD-126 | MAJOR | ЖИВОЙ | `grep -n "^chk()" -A 4 scripts/verify_M-73.sh` -> `chk() {` | в самом свежем verify-скрипте (M-73) шаг проверки по-прежнему прячет весь stdout/stderr упавшей… |
| TD-127 | MAJOR | ЖИВОЙ | `'sed -n '156,183p' scripts/check_artifact_ids.sh'` -> `id="$(id_of "$f")" ¦¦ continue` | Барьер всё ещё тихо пропускает (fail-open) артефакты, недоступные через git, вместо fail-closed… |
| TD-128 | MAJOR | ЖИВОЙ | `sed -n '95,105p' scripts/check_docs_freeze.sh` -> `for c in $(git rev-list "${BASE}..HEAD"); do` | Оба под-дефекта на месте: нет различения merge-объединения от новой правки, и exit 1 по-прежнем… |
| TD-130 | MAJOR | ЖИВОЙ | `grep -n "#\[ignore\]" -B2 crates/gateway/tests/red_catalog_equiva…` -> `1081:/// Пока решения нет, тест помечен '#[ignore]' с явной причи…` | тест, доказывающий, что тёплая сессия не замечает подмену/усечение содержимого закрытого сегмен… |
| TD-131 | MAJOR | ЖИВОЙ | `grep -n "self.file_names = cur_names" crates/journal/src/segments…` -> `segments.rs:427: self.file_names = cur_names;` | Коммит имён сегментов всё ещё в конце is_fresh, мутация перестановки не ловится ничем. |
| TD-132 | MAJOR | ЖИВОЙ | `grep -n "validate_like_full_path\¦check_first_seq_monotonic" crat…` -> `404:        // метод 'validate_like_full_path' — единый именованн…` | кеш по-прежнему переоценивает три fail-closed проверки (усечение, подмена fingerprint, битый за… |
| TD-133 | MINOR | ЖИВОЙ | `sed -n '38,46p;105,109p' scripts/tests/red_segment_meta_battery.s…` -> `# ВТОРОЙ ПРЕДЕЛ, НАЗВАННЫЙ ЧЕСТНО: indexblind ≡ pathblind и guard…` | Атрибуция мутантов внутри пар (indexblind/pathblind, guardskip/guardstub) по-прежнему не различ… |
| TD-134 | MAJOR | ЖИВОЙ | `'grep -rn "CARGO_TARGET_DIR" .claude/rules/*.md'` -> `(пусто — совпадений нет)` | Правило про изоляцию 'CARGO_TARGET_DIR' между деревьями при мутационном контроле так и не появи… |
| TD-136 | MAJOR | ЖИВОЙ | `grep -n ">/dev/null 2>&1\¦2>/dev/null" scripts/verify_M-61.sh ¦ s…` -> `41:  if ( EVENT_NAME=push ... bash "${BARRIER}" >/dev/null 2>&1 )…` | Шаги S и N всё ещё глотают stderr — при падении невозможно отличить «инвариант нарушен» от «мех… |
| TD-137 | MINOR | ЗАКРЫТ | `sed -n '29,40p' scripts/next_artifact_id.sh; sed -n '77,88p' scri…` -> `next_artifact_id.sh: "TD-137: прежняя редакция утверждала «там то…` | Ложное утверждение «побайтовое совпадение» в шапке блока прямо переписано с признанием ошибки и… |
| TD-138 | MAJOR | ЗАКРЫТ | `sed -n '184,205p' docs/fa/journal.md` -> `"ШЕСТОЙ путь и смена МЕХАНИЗМА наследования (M-62, merge d564617;…` | 'docs/fa/journal.md' переписан и теперь верно описывает механизм наследования guard'а после M-6… |
| TD-139 | MAJOR | ЖИВОЙ | `'grep -n "Амендмент 2" docs/archive/M-04-research-core.md; ls res…` -> `docs/archive/M-04-research-core.md:80: Амендмент 2 к задаче 8 (20…` | Самый серьёзный пункт (в, коллизия номеров) уже исправлен правкой спеки M-04; пункты (а) и (б) … |
| TD-140 | MINOR | ЗАКРЫТ | `grep -n "F2 ¦" milestones/M-61-artifact-ids.md` -> `596: F2 ¦ ... Состав — РАЗЛОЖЕНИЕМ, не итогом (TD-140): 26 мутант…` | Расхождение текста спеки (25 vs 26) устранено явным разложением числа 28. |
| TD-148 | MAJOR | ЖИВОЙ | `grep -n "events_decoded\¦segments_opened\¦segment_meta_ops" crate…` -> `114:    /// - возвращает честные 'ReadStats{events_decoded, segme…` | счётчик segment_meta_ops по-прежнему существует только внутри библиотеки и не логируется и не э… |
| TD-149 | MAJOR | НЕПРОВЕРЯЕМ | `grep -n "TD-149" TECH-DEBT.md` -> `85:¦ TD-149 ¦ ... Плато подтверждено — через 2.5 ч простоя байт в…` | Требуется повторный цикл «N сессий → teardown → замер RssAnon» на живом проде несколько раз под… |
| TD-150 | MAJOR | ЖИВОЙ | `sed -n '275,290p' scripts/reserve_artifact_id.sh` -> `if git push "${REMOTE}" "${sha}:refs/reserved/${ID}" >/dev/null 2…` | Механизмы Р-1 (reserve_artifact_id.sh) и Р-4 (деплой через workflow_run по зелёному CI) построе… |
| TD-151 | MINOR | ЖИВОЙ | `'sed -n '51,58p' crates/journal/tests/red_retention.rs'` -> `let free = journal::free_bytes(dir.path()).expect("free_bytes");` | Тест disk-guard всё ещё меряет реальное свободное место хоста в момент замера — гонка с паралле… |
| TD-152 | MAJOR | ЖИВОЙ | `sed -n '160,177p' crates/research-cli/src/depth_lifetime.rs` -> `"До первого mid в окне завершённые жизни копятся в pending_comple…` | Пока в окне не появился mid, память растёт пропорционально числу завершённых жизней (×4 на заме… |
| TD-153 | MINOR | ЖИВОЙ | `sed -n '514,532p' crates/research-cli/src/depth_lifetime.rs; ls c…` -> `514:fn flush_delayed_states(` | ветка Fate::Alive, которая молча считается как Cancelled в защитном fallback'е, по-прежнему нич… |
| TD-155 | MINOR | ЗАКРЫТ | `ls scripts/check_review_fa.sh; grep -n "^  review-fa:" .github/wo…` -> `scripts/check_review_fa.sh` | карточка утверждала, что скрипт и джоб объявлены в правилах, но физически отсутствуют — сейчас … |
| TD-156 | MINOR | ЗАКРЫТ | `grep -n "ЧТО ИМЕННО ПРИНЯТО\¦ЗАКРЫТО как ПРИНЯТЫЙ" cards/TD-156.t…` -> `### ЧТО ИМЕННО ПРИНЯТО — решение founder'а 2026-08-23, вердикт R-…` | Founder явно закрыл 23.08 как принятый постоянный риск — переименование пяти пар запрещено, цен… |
| TD-157 | НЕ ПРИМЕНЯЕТ | ЖИВОЙ | `'ls scripts/ ¦ grep -i resource; git log --oneline --all ¦ grep r…` -> `check_resource_oracles.sh` | Ограничение (однофайловость) остаётся в силе, признано намеренным пределом. Уточнение: барьер у… |
| TD-159 | MAJOR | ЖИВОЙ | `grep -n "pub depth_band_provenance" crates/gateway/src/lib.rs; se…` -> `pub struct DepthRow {` | одна метка достоверности по-прежнему покрывает всю серию точек, хотя точки внутри неё могут быт… |
| TD-160 | MINOR | ЖИВОЙ | `grep -n "bands:" crates/gateway/tests/red_checkpoint_byte_identit…` -> `124:        bands: vec![0.001],` | Оракул чекпоинта по-прежнему гоняет только одну мелкую полосу, новые поля глубины в широких пол… |
| TD-161 | MINOR | ЖИВОЙ | `grep -n "diff-reconstructed" crates/gateway/src/lib.rs; grep -n "…` -> `1991:        let prov_str = "diff-reconstructed".to_string();    …` | heatmap и depth-series по-прежнему выдают разные форматы provenance-метки для одного и того же … |
| TD-162 | MAJOR | ЗАКРЫТ | `'sed -n '2121,2130p' crates/gateway-serve/src/lib.rs; grep -n "TD…` -> `match trimmed.parse::<i64>() {` | Парсинг GATEWAY_WINDOW_MS теперь fail-closed. Реестр верно отражает закрытие. Тело карточки ниж… |
| TD-163 | MAJOR | ЗАКРЫТ | `git log --oneline --all -S"fallback_allowed" -- scripts/check_pro…` -> `38a6df7 fix(harness): TD-163 круг 3 — условие fallback'а задано С…` | Круг 3 (коммит 38a6df7, PR #75/main 15abd0c) закрыл ложно-красный класс — подтверждено кодом на… |
| TD-164 | MAJOR | ЗАКРЫТ | `git log --oneline -3 -S"drop_is_handmade" -- scripts/check_protec…` -> `38ec6c0 fix(harness): TD-164 круг 4 — кандидат B, авто-воспроизво…` | Кандидат B (авто-воспроизводимость дропа через git merge-tree, функция drop_is_handmade, вызыва… |
| TD-165 | MINOR | ЗАКРЫТ | `sed -n '78,82p' .claude/agents/reviewer.md` -> `Пробел предъявляется явно: FA-WAIVER ... отдельной строкой в ФАЙЛ…` | Профиль reviewer'а поправлен 25.08 — waiver теперь предписывается класть туда, где его реально … |
| TD-166 | MINOR | ЗАКРЫТ | `'sed -n '293,300p' crates/gateway-serve/tests/red_max_subs_config…` -> `assert_eq!(` | Оракул больше не сравнивает подстрокой по всей строке лога — сверяет цифры сразу после 'KEY='. … |
| TD-167 | MINOR | ЖИВОЙ | `grep -n "П-017" docs/PENDING-SIGNATURE.md ¦ head -2; grep -c "pul…` -> `863:## П-017 — ПОДПИСАНО 2026-08-20: заморозка харнесса, моратори…` | Класс "документ ложно описывает сам себя" ловится только кругом критика; конструкция-развязка в… |
| TD-168 | MAJOR | ЖИВОЙ | `grep -n "flat_map\¦bucket_time_s\¦distinct_values" crates/gateway…` -> `38://! bucket_time_s (':824-830') делит на 1000 целочисленно. Зам…` | карточка (расширенная 2026-08-29) документирует, что весь корпус тестов слеп к рецидиву дефекта… |
| TD-169 | MAJOR | ЖИВОЙ | `sed -n '380,392p' crates/gateway/tests/red_depth_cadence.rs` -> `/// # d16 ЗЕЛЁН сегодня, и это объявлено, а не замаскировано` | Тест d16 сам объявляет себя вакуумным зелёным пином — чекпоинт на LATEST, хвоста нет, задача ка… |
| TD-170 | MAJOR | ЖИВОЙ | `'sed -n '2720,2724p' crates/gateway/src/lib.rs' (86400000%100==0,…` -> `if sel.timeframe_ms <= 0 ¦¦ 86_400_000 % sel.timeframe_ms != 0 { …` | Гвард проверяет только выравнивание на сутки — 100 и 250 делят 86 400 000 нацело, поэтому подсе… |
| TD-171 | MINOR | ЖИВОЙ | `grep -n "SETTER='set_effective_max_response_bytes'" scripts/tests…` -> `152:SETTER='set_effective_max_response_bytes'` | Защита осталась текстовым инвентарём мест вызова, не оракулом рантайм-переустановки. |
| TD-172 | MINOR | ЖИВОЙ | `'grep -n "SNAPSHOT_ENVELOPE_BYTES\¦LEGACY_DRAIN_BATCH" crates/gat…` -> `crates/gateway/src/lib.rs:2888: const SNAPSHOT_ENVELOPE_BYTES: us…` | Обе константы всё ещё оценки, не замеры на прод-форме — не изменились. |
| TD-173 | MAJOR | ЖИВОЙ | `grep -n "^fn \¦#\[test\]" crates/gateway/tests/red_depth_recomput…` -> `180:#[test] fn md_i8_d6a_counter_actually_measures_visited_levels…` | Оракулы d6a/d6b защищают только счётчик посещённых уровней и число полос — сравнения стоимости … |
| TD-174 | MAJOR | ЗАКРЫТ | `git merge-base --is-ancestor eb1c20a HEAD && echo ancestor; grep …` -> `ancestor` | Различитель причины отказа больше не смотрит на 'io::ErrorKind::Other' (который журнал возвраща… |
| TD-175 | MAJOR | ЖИВОЙ | `git show origin/feat/M-71-rev6:crates/gateway/tests/red_egress_ca…` -> `366:    const RETRIES: usize = 3;` | Setup-guard'ы P4/P5 по-прежнему требуют «3 из 3 отказов» — пин конструкции, а не свойства; P6 (… |
| TD-176 | MAJOR | ЖИВОЙ | `grep -n "expr: journal_disk_free_bytes" deploy/alerts/ops.rules.y…` -> `49:        expr: journal_disk_free_bytes < 10737418240` | Порог алерта всё ещё совпадает 1-в-1 с порогом fail-closed writer'а — оператор узнаёт об остано… |
| TD-177 | MAJOR | ЖИВОЙ | `sed -n '1386,1401p' crates/gateway-serve/src/lib.rs` -> `let current_gen = inner.gens.get(&id).copied();` | при терминальном отказе (cap_terminal) код по-прежнему безусловно снимает подписку и generation… |
| TD-178 | MAJOR | ЖИВОЙ | `grep -n "gateway_serve¦serve::¦WebSocket¦axum¦tokio::spawn¦TcpLis…` -> `(пусто — оракул работает на уровне библиотеки gateway::LiveReduce…` | Оракул точки входа для push-цикла v1/legacy с отказом по пределу так и не написан — P6 и W5 по-… |
| TD-179 | MAJOR | ЖИВОЙ | `'sed -n '4441,4443p;4505,4508p' crates/gateway/src/lib.rs'` -> `for event in &mut stream {` | Побатчевое продвижение курсора при отказе из середины по-прежнему теряет уже собранные кадры. |
| TD-180 | MINOR | ЖИВОЙ | `sed -n '4664,4673p' crates/gateway/src/lib.rs; grep -n "\.snapsho…` -> `pub fn snapshot(&self) -> Snapshot {` | snapshot() по-прежнему может отдать series из состояния, куда уже свёрнут отвергнутый пределом … |
| TD-181 | MAJOR | ЖИВОЙ | `grep -oE 'run_barrier "[^"]*" [^ ]+' scripts/tests/red_protected_…` -> `1 ""` | Ветка 'pull_request' барьера защищённых артефактов по-прежнему не проверяется ни одним сценарие… |
| TD-182 | MINOR | ЖИВОЙ | `git config --local user.name; git config --global user.name` -> `founder` | Локальный git-identity («founder») по-прежнему перекрывает глобальный («Alex K»), хотя правило … |
| TD-183 | MINOR | ЗАКРЫТ | `grep -n 'cifs\¦CIFS' deploy/README.md; git merge-base --is-ancest…` -> `35:## 1. Доступ к Hetzner Storage Box через SSH-субаккаунт (НЕ CI…` | README переписан на SSH-путь, все оставшиеся упоминания CIFS объясняют отказ от него, а не пред… |
| TD-184 | MAJOR | ЗАКРЫТ | `ls -la deploy/bin/builder-prune-cron.sh deploy/cron.d/builder-pru…` -> `-rwxrwxr-x ... deploy/bin/builder-prune-cron.sh` | сторож build-кэша (docker builder prune --filter until=336h, раз в сутки) реально присутствует … |
| TD-185 | MAJOR | ЗАКРЫТ | `grep -n "GATEWAY_SCHEMA_VERSION" crates/gateway/src/lib.rs; grep …` -> `crates/gateway/src/lib.rs:98:pub const GATEWAY_SCHEMA_VERSION: u3…` | Оба ложных утверждения FA (снимок-only и обрыв цепочки версий на 7→8) исправлены, версия в коде… |
| TD-186 | MINOR | ЖИВОЙ | `grep -n "комментар¦докстринг¦carve-out" .claude/rules/scope-guard…` -> `33:## Milestone-файлы — carve-out статус-колонки   (единственный …` | В scope-guard.md по-прежнему нет ответа, кто вправе править комментарий в чужой зоне — гейт про… |
| TD-187 | MAJOR | ЖИВОЙ | `'grep -ni "continue-on-error" .github/workflows/*.yml; ls scripts…` -> `(пусто с обеих сторон)` | Сейчас 'continue-on-error' в ci.yml нет, но и барьера, гарантирующего это впредь, тоже нет — ст… |
| TD-188 | MAJOR | ЖИВОЙ | `grep -rn "MUT-ORDER\¦mut_order" crates/gateway*/tests/*.rs` -> `(пусто — ни один тест не найден)` | По-прежнему нет ни одного оракула, который бы ловил перестановку «коммит каденс-интервала до пр… |
| TD-189 | MAJOR | ЖИВОЙ | `grep -n "fn depth_from_book\¦pub fn depth_within" crates/gateway/…` -> `gateway/src/lib.rs:1191: fn depth_from_book(...)` | Обе реализации подсчёта глубины существуют раздельно, равенство не проверяется ничем. |
| TD-190 | MINOR | ЖИВОЙ | `grep -n "microprice" docs/fa/book.md ¦ head -3; grep -n "fn depth…` -> `docs/fa/book.md:117:  отклоняется от референса (microprice) не бо…` | документ (FA) по-прежнему определяет глубину относительно microprice, а код реально считает её … |
| TD-191 | MAJOR | ЖИВОЙ | `grep -n "ОСОЗНАННЫЙ РУЧНОЙ ШАГ" deploy/README.md; grep -n "for sr…` -> `deploy/README.md:126: «ОСОЗНАННЫЙ РУЧНОЙ ШАГ с подписью founder ★…` | README всё ещё утверждает, что установка cron — ручной шаг, хотя deploy.yml по-прежнему сам ста… |
| TD-192 | MINOR | ЖИВОЙ | `grep -n "crontab -n" scripts/verify_M-73.sh; grep -rln "crontab -…` -> `(в verify_M-73.sh — пусто)` | verify_M-73.sh до сих пор не зовёт crontab -n ни на одном файле deploy/cron.d/*, хотя деплой об… |
| TD-193 | MAJOR | ЖИВОЙ | `grep -n "backup_restore_drill_ok" docs/fa/ops.md ; find . -iname …` -> `84/102: backup_restore_drill_ok ¦ restore-drill (task 3, заблокир…` | Восстановление холодной копии ни разу не проверялось на практике; только объявленная метрика, м… |
| TD-194 | MAJOR | ЗАКРЫТ | `'sed -n '520,540p' .github/workflows/ci.yml; grep -n "L2DELTA_CAP…` -> `rollout-composition:` | Компаратор состава перенесён из разового milestone-скрипта в постоянный джоб CI — именно то леч… |
| TD-195 | MINOR | ЖИВОЙ | `grep -c "cargo test --all\¦cargo test --workspace" scripts/verify…` -> `0` | verify_M-45.sh всё ещё не гоняет полный cargo test --all/--all-features, как того требует парит… |
| TD-197 | MINOR | ЖИВОЙ | `awk '/pub struct SeriesBundle/,/^}/' crates/gateway/src/lib.rs ¦ …` -> `/// Ячейки ТОЛЬКО в окне [mid*(1-W), mid*(1+W)], W=эффективная се…` | Полуширина окна heatmap/COB по-прежнему не передаётся в самом ответе — воспроизвести серию можн… |
| TD-198 | MAJOR | ЖИВОЙ | `grep -c "pub const MAX_BANDS" crates/gateway/src/lib.rs; sed -n '…` -> `0` | validate_selector по-прежнему не ограничивает число полос в запросе клиента — константы MAX_BAN… |
| TD-199 | MAJOR | ЗАКРЫТ | `'sed -n '134p' TECH-DEBT.md; git merge-base --is-ancestor b008f5e…` -> `¦ ~~TD-199~~ ¦ ✅ ЗАКРЫТ 2026-09-07 merge'ем M-77 (PR #167, b008f5…` | Дефект расхождения live/replay устранён merge'ем M-77, реестр верен. |
| TD-200 | MAJOR | ЖИВОЙ | `sed -n '1347,1352p' crates/gateway/src/lib.rs` -> `fn refresh_heatmap_bucket(&mut self, time_s: i64) {` | refresh_heatmap_bucket по-прежнему вызывает book.levels(side) на каждом L2-событии — аллокация … |
| TD-201 | MINOR | ЖИВОЙ | `'ls crates/gateway/tests/ ¦ grep -i "bounded\¦pump_cost"'` -> `red_gateway_bounded.rs` | Новый буфер наблюдений book_series действительно не сторожится ни одним из названных тестов — к… |
## 2. ЖИВЫЕ КРУПНЫЕ — 48 карточек (MAJOR/CRITICAL, вердикт ЖИВОЙ)

Отсортированы по оси приоритета: **дорожает ли долг со временем**, если это
следует из самого текста карточки/замера в ней. Три корзины: дорожает →
стабилен → не знаю. Внутри корзины — по номеру TD.

Из 95 живых карточек 48 — MAJOR или выше. Это резко больше, чем черновой замер
задания («в таблице ~45 открытых, из них ~24 крупных») — потому что тот замер
смотрел только на сводную таблицу «В ПЛАНЕ» (55 строк), а она покрывает 55 из
127 карточек. Остальные 72 живут только как список-бюллет в разделе
«ЗАМОРОЖЕНО» или вообще не имеют строки в таблице (см. §3.1) — среди них тоже
нашлись MAJOR-карточки, которые таблица никогда не считала.


### ДОРОЖАЕТ СО ВРЕМЕНЕМ

**TD-006** (MAJOR, phase=Ф0)
- Суть: Журнал по-прежнему растёт без реального ретеншена/офсайт-выгрузки; сам актуальный реестр подтверждает, что офсайт-копирование ни разу не выполнялось.
- Рост: дорожает: журнал append-only, диск монотонно растёт.
- Команда: ``sed -n '1918,1922p' TECH-DEBT.md``
```
| Ф0 «Необратимость данных» — вне пути Ф1–Ф4 (сама Ф0 по `DESIGN` §10 — «в работе»).
**`TD-020` — offsite-бэкап + restore-drill, acceptance-ворота Ф0, не проводился НИ РАЗУ**
| 6 | `TD-020` `TD-006` `TD-055` `TD-056` `TD-057` `TD-119` |
```

**TD-015** (MAJOR, phase=не указана (бакет «Research / quant-desk — вне пути Ф1–Ф4»))
- Суть: Правило «в метрики брать только записи кода ≥5141fd9» по-прежнему актуально: ledger не пополнился, но и отчёта R-001, к которому правило применяется, всё ещё нет — проверить его пока не на чем.
- Рост: дорожает: ledger append-only, и чем позже применён фильтр, тем больше шансов смешать несопоставимые эпохи в будущем отчёте
- Команда: `wc -l research/trials-ledger.jsonl; cat research/trials-ledger.jsonl | python3 -c "import sys,json;[print(json.loads(l).get('code_hash')) for l in sys.stdin if l.strip()]"`
```
4 research/trials-ledger.jsonl
f7f476178fe7d596a3e16d73925463eb15b161f2f3b6b2dc3cb591bf29261c3c  (×4 — все пре-M-07)
```

**TD-016** (MAJOR, phase=Ф1 (из сводной таблицы реестра))
- Суть: Мёртвые уровни книги (цена ушла, size=0 больше не приходит) по-прежнему не эвиктятся по-настоящему — только аварийный кап на 200k и грубое окно ±60% от mid, которое почти ничего не режет; milestone M-31 (настоящая эвикция) так и не заведён.
- Рост: дорожает — число мёртвых уровней растёт со временем (наблюдался рост ~2000 уровней/час до срабатывания бэкстопа), портит достоверность полос TPP/OBI.
- Команда: `grep -rn "BACKSTOP_LEVELS_PER_SIDE\|MAX_REL_DIST" crates/venue-binance/src/lib.rs; find crates/book -iname "*evict*"; ls milestones | grep M-31`
```
crates/venue-binance/src/lib.rs:56:pub const BACKSTOP_LEVELS_PER_SIDE: usize = 200_000;
crates/venue-binance/src/lib.rs:33:const MAX_REL_DIST: f64 = 0.60;
(book/*evict* — пусто)
(milestones/M-31 — не существует)
```

**TD-020** (MAJOR, phase=Ф0)
- Суть: Ретеншен работает только в dry-run; реального `apply` в холодное хранилище не было ни разу — подтверждено самим текущим реестром.
- Рост: дорожает: журнал растёт, цена задержки увеличивается.
- Команда: ``sed -n '1918,1922p' TECH-DEBT.md``
```
| Ф0 «Необратимость данных»... **`TD-020` — offsite-бэкап + restore-drill,
acceptance-ворота Ф0, не проводился НИ РАЗУ** | 6 | `TD-020` `TD-006` ... |
```

**TD-038** (MAJOR, phase=не указана)
- Суть: CRC-часть дефекта закрыта purge'ем legacy-сегмента (подтверждено самим текстом карточки), но продуктовая цель «снапшот работает на проде» не достигнута — блокер переехал в отдельную карточку TD-039 (вне моего батча). Прод (SSH) я не проверял.
- Рост: дорожает — OOM на unbounded reduce будет расти вместе с журналом (относится к TD-039, не в моём батче)
- Команда: `grep -n "TD-039" TECH-DEBT.md`
```
966:  host-OOM (TD-039; ...
969:  снапшот ООМ-ит (TD-039)...
3522: ...активный блокер переехал в TD-039 (OOM)
```

**TD-082** (MAJOR, phase=Ф1)
- Суть: У метрики "разрывы seq" по-прежнему нет продюсера — обнаружения потери событий журнала как не было, так и нет, хотя алерт OPS-GAP формально висит на ней. Решение зафиксировано в RFC, но не реализовано.
- Рост: дорожает: проектируемое шардирование журнала сделает разрыв seq штатным явлением, и старый признак вообще перестанет работать как основа детектора
- Команда: `grep -n "journal_seq_gaps_total" crates/recorder/src/metric_emit.rs scripts/verify_M-09.sh`
```
crates/recorder/src/metric_emit.rs:15: "нет естественного триггера в journal.next_seq — пропускается, оракул её не ассертит"
scripts/verify_M-09.sh:125: EVENT_OR_ELSEWHERE=... journal_seq_gaps_total ...  (выведена из-под покрытия)
```

**TD-091** (MINOR (MAJOR при росте окна), phase=Ф2 (из сводной таблицы реестра))
- Суть: Сервер всё ещё принимает WS-соединение без явного WebSocketConfig — действует дефолт tungstenite 64 MiB, а размер снапшота линейно растёт от операторской ручки GATEWAY_WINDOW_MS.
- Рост: дорожает — объём снапшота растёт линейно с окном; при увеличении окна лимит tungstenite будет достигнут без единой правки кода.
- Команда: `grep -n "WebSocketConfig\|accept_hdr_async" crates/gateway-serve/src/lib.rs`
```
389:    // (callback всегда вызывается синхронно внутри accept_hdr_async, ...)
406:    let ws_stream = match tokio_tungstenite::accept_hdr_async(stream, callback).await {
```

**TD-097** (MAJOR, phase=Ф2/Ф4)
- Суть: Стоимость подключения к панели по-прежнему выше цели milestone'а (1.8–3.8 с вместо ≈250 мс) при любом backlog'е; текущая гипотеза причины — открытый сегмент журнала, а не то, что чинил M-54/M-56.
- Рост: дорожает: сегменты растут между ротациями (до ~1 GiB), латентность подключения предположительно растёт вместе с размером открытого сегмента
- Команда: `(проверка требует живого прода/повторных замеров латентности подключения — недоступно из статического чекаута)`
```
(карточка описывает замеры на живом проде 2026-08-03; последняя правка констатирует "часть НЕ ЗАКРЫТА", гипотеза про размер открытого сегмента не подтверждена контрольным экспериментом)
```

**TD-118** (MAJOR, phase=Ф2 (предусловие))
- Суть: Реестр сам подтверждает — починка механизма завершения wsprobe не сделана, есть только компенсация (reap).
- Рост: дорожает: каждый прод-замер до фикса рискует быть загрязнён
- Команда: `grep -n "TD-118" TECH-DEBT.md | head -3`
```
92: TD-118 | ... открыты (а) починка завершения и (в) ревизия чисел ... | Ф2 (предусловие) | MAJOR |
```

**TD-119** (MAJOR, phase=не указана (группа: «Ф0 „Необратимость данных“ — вне пути Ф1–Ф4»))
- Суть: Страж «диск кончился, а не код красный» стоит только в одном verify-скрипте из тридцати; gc_worktrees.sh по-прежнему определяет «занятость» по живому процессу (cwd), а не по lock-файлу владельца — предложенная развязка не реализована.
- Рост: дорожает — неубираемые worktree накапливаются (на момент карточки 37 деревьев, 370 GB), автоматика их почти не трогает.
- Команда: `grep -l "ENOSPC\|No space left" scripts/verify_*.sh | wc -l; ls scripts/verify_*.sh | wc -l; grep -n "PID\|lock" scripts/gc_worktrees.sh | head -3`
```
1
30
63:MY_UID="$(id -u)"
76:# holder_pids <путь> — печатает PID'ы держателей
```

**TD-150** (MINOR сегодня / MAJOR в момент написания спек, phase=не указана (группа: «Процесс и governance»))
- Суть: Механизмы Р-1 (reserve_artifact_id.sh) и Р-4 (деплой через workflow_run по зелёному CI) построены, но замечание №3 из трёх — «шаг push не различает причину отказа» — по-прежнему в коде: любой ненулевой exit трактуется как «номер занят», сетевая/auth-ошибка сожжёт номер так же, как реальный конфликт.
- Рост: дорожает — расхождение задеплоенного SHA и зелёного main растёт с числом коммитов между CI и ручным дозапуском (упомянутый в карточке разрыв в 13 коммитов за день).
- Команда: `sed -n '275,290p' scripts/reserve_artifact_id.sh`
```
if git push "${REMOTE}" "${sha}:refs/reserved/${ID}" >/dev/null 2>&1; then
    ...
    exit 0
  fi
  # Отклонение = имя занято (E1). Прыгаем через ВЕСЬ занятый блок сразу...
```

**TD-152** (MAJOR, phase=не указана (бакет «Research / quant-desk — вне пути Ф1–Ф4»))
- Суть: Пока в окне не появился mid, память растёт пропорционально числу завершённых жизней (×4 на замере) — граница осталась осознанно неисправленной.
- Рост: дорожает: расширение на топ-300 инструментов и многосуточные окна с неликвидными инструментами упрутся именно в этот путь
- Команда: `sed -n '160,177p' crates/research-cli/src/depth_lifetime.rs`
```
"До первого mid в окне завершённые жизни копятся в pending_completed, и расход растёт с их
числом... Граница здесь НЕ достигнута сознательно (решение e0e56a3): милестоун обязывает её
НАЗВАТЬ, а не устранить."
```

**TD-173** (MAJOR, phase=Ф2/Ф4)
- Суть: Оракулы d6a/d6b защищают только счётчик посещённых уровней и число полос — сравнения стоимости каденции 60с vs 1с (что требовала карточка) до сих пор нет.
- Рост: дорожает — чем чаще события и больше журнал, тем дороже отсутствие защиты каденции
- Команда: `grep -n "^fn \|#\[test\]" crates/gateway/tests/red_depth_recompute_cost.rs`
```
180:#[test] fn md_i8_d6a_counter_actually_measures_visited_levels()
228:#[test] fn md_i8_d6b_cost_does_not_multiply_by_number_of_bands()
(нет теста, сравнивающего depth_levels_visited при каденции 60с против 1с)
```

**TD-176** (MAJOR, phase=Ф0/Ф1)
- Суть: Порог алерта всё ещё совпадает 1-в-1 с порогом fail-closed writer'а — оператор узнаёт об остановке записи ровно в момент остановки, а не заранее.
- Рост: дорожает — свободное место убывает по мере роста журнала, а окно предупреждения при неизменном пороге остаётся нулевым
- Команда: `grep -n "expr: journal_disk_free_bytes" deploy/alerts/ops.rules.yml; grep -n "DEFAULT_MIN_FREE_BYTES" crates/journal/src/segments.rs`
```
49:        expr: journal_disk_free_bytes < 10737418240
44:pub const DEFAULT_MIN_FREE_BYTES: u64 = 10 * 1024 * 1024 * 1024; // 10 GiB
```

**TD-193** (MAJOR, phase=M-73/Ф0)
- Суть: Восстановление холодной копии ни разу не проверялось на практике; только объявленная метрика, механизма drill нет.
- Рост: дорожает: копия растёт (85 ГБ, ежечасно), читаемость по-прежнему не доказана
- Команда: `grep -n "backup_restore_drill_ok" docs/fa/ops.md ; find . -iname "*drill*"`
```
84/102: backup_restore_drill_ok | restore-drill (task 3, заблокирован Storage Box) | ...
(find *drill* — ничего)
```

**TD-198** (MAJOR, phase=Ф2/Ф4)
- Суть: validate_selector по-прежнему не ограничивает число полос в запросе клиента — константы MAX_BANDS в коде нет, лимит на количество ничем не задан.
- Рост: дорожает: цена одного злонамеренного запроса растёт вместе с целью DESIGN на 10 000 одновременных сессий
- Команда: `grep -c "pub const MAX_BANDS" crates/gateway/src/lib.rs; sed -n '2731,2733p' crates/gateway/src/lib.rs`
```
0
pub fn validate_selector(sel: &Selector) -> io::Result<()> {
    if sel.timeframe_ms <= 0 || 86_400_000 % sel.timeframe_ms != 0 {
```

**TD-200** (MAJOR, phase=не указана)
- Суть: refresh_heatmap_bucket по-прежнему вызывает book.levels(side) на каждом L2-событии — аллокация пропорциональна глубине книги, дефект предсуществует в коде и не устранён.
- Рост: дорожает: DESIGN §17 предписывает более редкий якорь L2Snapshot, из-за чего дельта-хвосты будут длиннее и событий на тик станет больше
- Команда: `sed -n '1347,1352p' crates/gateway/src/lib.rs`
```
fn refresh_heatmap_bucket(&mut self, time_s: i64) {
    let bids = self.book.levels(Side::Buy);
    let asks = self.book.levels(Side::Sell);
    ...
}
```


### СТАБИЛЕН (не растёт)

**TD-055** (MAJOR, phase=не указана)
- Суть: При отказе Telegram-доставки код только печатает eprintln!, main() всё равно возвращает Ok(()) — exit=0 сохраняется независимо от того, доставлен ли алерт.
- Рост: нет
- Команда: `sed -n '83,109p' crates/ops/src/bin/ops-watchdog.rs`
```
for alert in &outcome.delivered {
    ...
    if let Err(e) = telegram_transport.send(&message) {
        eprintln!("[ops-watchdog] TelegramTransport::send failed: {e}");
    }
}
...
Ok(())
```

**TD-081** (MAJOR, phase=Ф1 (из сводной таблицы реестра))
- Суть: ts_mono_ns по-прежнему считается от epoch, который берётся заново при каждом открытии журнала (обнуляется при рестарте recorder'а), а alpha всё так же судит свежесть сигнала именно по этому полю — устаревший вход после рестарта считается вечно свежим.
- Рост: нет (дефект не накапливается, а срабатывает заново при каждом рестарте)
- Команда: `grep -n "epoch: SystemTime\|SystemTime::now()" crates/journal/src/lib.rs; sed -n '191,192p' crates/alpha/src/lib.rs`
```
crates/journal/src/lib.rs:69:    epoch: SystemTime,
crates/journal/src/lib.rs:111:            epoch: SystemTime::now(),
crates/journal/src/lib.rs:168:            epoch: SystemTime::now(),
191:                let threshold = sample.ts_event_mono_ns.saturating_add(horizon_ns);
192:                let fresh = ev.ts_mono_ns <= threshold;
```

**TD-084** (MAJOR, phase=не указана)
- Суть: harness wsprobe --secret по-прежнему декодирует hex-подобный секрет в байты, а сервер использует сырые ASCII-байты — подпись никогда не сойдётся с типовым hex-секретом.
- Рост: нет
- Команда: `grep -n "is_hex\|from_secret" crates/gateway-serve/src/bin/wsprobe.rs crates/gateway-serve/src/lib.rs`
```
crates/gateway-serve/src/bin/wsprobe.rs:156:    let is_hex =
crates/gateway-serve/src/bin/wsprobe.rs:158:    if is_hex {
crates/gateway-serve/src/lib.rs:2364:        decoding_key: DecodingKey::from_secret(secret.as_bytes()),
```

**TD-093** (MINOR по остатку (MAJOR-часть переехала в TD-097), phase=Ф2)
- Суть: Гонка снапшот↔live закрыта оракулом (а); двойная стоимость на connect и лишний полный проход в resume() без чекпоинта (б/в) остаются нерешёнными — (б) переоценена и переехала в TD-097.
- Рост: нет
- Команда: `grep -n "o3_no_gap_between_snapshot_and_push" crates/gateway/tests/red_connect_cost_single.rs`
```
265:fn o3_no_gap_between_snapshot_and_push()
```

**TD-100** (MAJOR, phase=не указана (группа: «Процесс и governance»))
- Суть: Завышенная в ~1.6 раза цифра «9.8 MiB на сессию» так и не исправлена ни в одном из четырёх документов, где она процитирована.
- Рост: нет
- Команда: `grep -n "9.8 MiB" docs/plans/scale-10k-sessions.md docs/DESIGN.md docs/09-roadmap-v2.md docs/SESSION-HANDOFF.md`
```
docs/plans/scale-10k-sessions.md:34:...≈9.8 MiB...
docs/plans/scale-10k-sessions.md:49:| память | ≈9.8 MiB/сессия ...
docs/SESSION-HANDOFF.md:177:... ≈9.8 MiB на сессию ...
docs/09-roadmap-v2.md:107:...≈9.8 MiB на сессию...
docs/DESIGN.md:582:...цена сессии ≈9.8 MiB...
```

**TD-112** (MAJOR, phase=не указана)
- Суть: Шаблон Done Block по-прежнему предписывает grep | ...; echo exit=$?, что печатает код grep, а не скрипта — точно та конструкция, которую gates.md §3 запрещает поимённо.
- Рост: нет
- Команда: `grep -n 'grep -E "\^(PASS|FAIL|VERDICT)"' .claude/rules/commit-discipline.md`
```
68:    verify: bash scripts/verify_M-NN.sh 2>&1 | grep -E "^(PASS|FAIL|VERDICT)"; echo exit=$?
```

**TD-126** (MAJOR, phase=не указана)
- Суть: в самом свежем verify-скрипте (M-73) шаг проверки по-прежнему прячет весь stdout/stderr упавшей команды (>/dev/null 2>&1) и не проверяет диск/CWD — при красном cargo test причину узнать неоткуда.
- Рост: нет
- Команда: `grep -n "^chk()" -A 4 scripts/verify_M-73.sh`
```
chk() {
  local name; name="$(printf '%s' "$1" | sed -n '1{...}')"
  if ( eval "$1" ) >/dev/null 2>&1; then echo "PASS: ${name}"; else echo "FAIL: ${name}" >&2; FAIL=$((FAIL + 1)); fi
}
```

**TD-130** (MAJOR, phase=не указана)
- Суть: тест, доказывающий, что тёплая сессия не замечает подмену/усечение содержимого закрытого сегмента с тем же именем, всё ещё стоит под #[ignore] — гейт этот сценарий не проверяет.
- Рост: нет
- Команда: `grep -n "#\[ignore\]" -B2 crates/gateway/tests/red_catalog_equivalence.rs | grep -A2 sm11d`
```
1081:/// Пока решения нет, тест помечен `#[ignore]` с явной причиной: держать в обязательном
1088:fn sm11d_content_rewritten_under_unchanged_name_is_invisible_to_warm_session() {
```

**TD-132** (MAJOR, phase=не указана)
- Суть: кеш по-прежнему переоценивает три fail-closed проверки (усечение, подмена fingerprint, битый заголовок) только для файлов из дифа, а не для всех сегментов на каждом такте — понижение частоты осталось как есть, только явно названо.
- Рост: нет
- Команда: `grep -n "validate_like_full_path\|check_first_seq_monotonic" crates/journal/src/segments.rs | head -6`
```
404:        // метод `validate_like_full_path` — единый именованный вызов, чтобы фикс
426:        self.validate_like_full_path(dir, &mut ops)?;
527:    fn validate_like_full_path(&self, dir: &Path, ops: &mut SegmentOps) -> io::Result<()> {
546:        check_first_seq_monotonic(&candidates, |name| {
```

**TD-139** (MAJOR (п. в), MINOR (остальные), phase=не указана)
- Суть: Самый серьёзный пункт (в, коллизия номеров) уже исправлен правкой спеки M-04; пункты (а) и (б) остаются незаведёнными отдельно, как требовала карточка.
- Рост: нет.
- Команда: ``grep -n "Амендмент 2" docs/archive/M-04-research-core.md; ls research/reports/R-001*``
```
docs/archive/M-04-research-core.md:80: Амендмент 2 к задаче 8 (2026-08-14, `TD-139` п.(в)):
номер отчёта БОЛЬШЕ НЕ НАЗВАН — и не будет.
(research/reports/R-001* — не существует)
```

**TD-159** (MAJOR, phase=Ф2/Ф4)
- Суть: одна метка достоверности по-прежнему покрывает всю серию точек, хотя точки внутри неё могут быть посчитаны при разном охвате наблюдения — риск активируется при смене GATEWAY_BANDS.
- Рост: нет
- Команда: `grep -n "pub depth_band_provenance" crates/gateway/src/lib.rs; sed -n '341,353p' crates/gateway/src/lib.rs`
```
pub struct DepthRow {
    pub side: String,
    pub band_pct_e8: i64,
    pub series: Vec<(i64, i64)>,
    pub depth_band_provenance: Option<String>,   // ОДНА метка на ВЕСЬ series
}
```

**TD-175** (MAJOR (условная), phase=не указана)
- Суть: Setup-guard'ы P4/P5 по-прежнему требуют «3 из 3 отказов» — пин конструкции, а не свойства; P6 (соседний guard) уже переделан на свойство, эти два — нет.
- Рост: нет
- Команда: `git show origin/feat/M-71-rev6:crates/gateway/tests/red_egress_cap_paths.rs | grep -n 'const RETRIES\|refusals < RETRIES'`
```
366:    const RETRIES: usize = 3;
384:    if refusals < RETRIES {
505:    const RETRIES: usize = 3;
531:    if refusals < RETRIES {
```

**TD-177** (MAJOR, phase=не указана)
- Суть: при терминальном отказе (cap_terminal) код по-прежнему безусловно снимает подписку и generation, не проверив, что это тот же generation, под которым pump стартовал — отставший pump может снести новую законную подписку, созданную позже под тем же id.
- Рост: нет
- Команда: `sed -n '1386,1401p' crates/gateway-serve/src/lib.rs`
```
let current_gen = inner.gens.get(&id).copied();
let live_keeps = !drained && current_gen == Some(gen_at_pump) && !cap_terminal;
...
if cap_terminal {
    inner.subs.remove(&id);
    inner.gens.remove(&id);
    send_v1_error(...)
}
```

**TD-178** (MAJOR, phase=Ф2)
- Суть: Оракул точки входа для push-цикла v1/legacy с отказом по пределу так и не написан — P6 и W5 по-прежнему проверяют библиотеку и обёртки отдельно, а не реальный подключённый путь.
- Рост: нет
- Команда: `grep -n "gateway_serve|serve::|WebSocket|axum|tokio::spawn|TcpListener" crates/gateway/tests/red_m77_delivery_window.rs`
```
(пусто — оракул работает на уровне библиотеки gateway::LiveReducer напрямую, не через реальный serve-путь)
```

**TD-181** (MAJOR, phase=не указана (бакет «Харнесс: гейты, оракулы, барьеры»))
- Суть: Ветка `pull_request` барьера защищённых артефактов по-прежнему не проверяется ни одним сценарием пробы (0 из 33) — сознательно заморожено до начала работы над TD-164 (которая тоже ещё открыта).
- Рост: нет
- Команда: `grep -oE 'run_barrier "[^"]*" [^ ]+' scripts/tests/red_protected_artifacts.sh | awk '{print $3}' | sort | uniq -c`
```
      1 ""
     32 push
```

**TD-187** (MAJOR, phase=все (глушит защиту ветки))
- Суть: Сейчас `continue-on-error` в ci.yml нет, но и барьера, гарантирующего это впредь, тоже нет — структурный риск остаётся открытым.
- Рост: нет.
- Команда: ``grep -ni "continue-on-error" .github/workflows/*.yml; ls scripts/ | grep -i "aggregate"``
```
(пусто с обеих сторон)
```

**TD-188** (MAJOR, phase=Ф2 (из сводной таблицы реестра))
- Суть: По-прежнему нет ни одного оракула, который бы ловил перестановку «коммит каденс-интервала до применения снимка/дельты» — мутация всё ещё прошла бы весь набор зелёной.
- Рост: нет
- Команда: `grep -rn "MUT-ORDER\|mut_order" crates/gateway*/tests/*.rs`
```
(пусто — ни один тест не найден)
```

**TD-191** (MAJOR, phase=Ф0 (процесс/прод))
- Суть: README всё ещё утверждает, что установка cron — ручной шаг, хотя deploy.yml по-прежнему сам ставит каждый файл из deploy/cron.d на VPS при каждом деплое.
- Рост: нет
- Команда: `grep -n "ОСОЗНАННЫЙ РУЧНОЙ ШАГ" deploy/README.md; grep -n "for src in deploy/cron.d" .github/workflows/deploy.yml`
```
deploy/README.md:126: «ОСОЗНАННЫЙ РУЧНОЙ ШАГ с подписью founder ★, а НЕ авто-действие deploy.yml»
.github/workflows/deploy.yml:288: for src in deploy/cron.d/*; do
```


### НЕ ЗНАЮ / не установлено

**TD-063** (MAJOR, phase=не указана (группа: «Харнесс: гейты, оракулы, барьеры»))
- Суть: Проверки 1/2/5 всё ещё гасятся в INFO при отсутствии нужной структуры в документе — гейт можно выключить, просто отредактировав/удалив проверяемую таблицу; починка сделана только для проверок 6/7.
- Рост: не знаю
- Команда: `sed -n '415,425p' scripts/verify_design_claims.sh (тот же код, что и TD-059)`
```
elif n_table == 0:
        info("1-ЕСТЬ", f"... все вне таблиц статусов ... проверка неприменима")
```

**TD-075** (MAJOR, phase=M-52)
- Суть: Отдельного ops-сервиса для --mode replay-digest в compose так и не завели.
- Рост: не знаю
- Команда: `grep -n "replay-digest" docker-compose.yml ; grep -n "^  [a-z-]*:" docker-compose.yml`
```
(0 совпадений replay-digest)
12:  recorder: / 48: journal-retention: / 91: journal-compaction: / 124: gateway-serve: / 205: gateway-checkpoint:
```

**TD-127** (MAJOR, phase=не указана)
- Суть: Барьер всё ещё тихо пропускает (fail-open) артефакты, недоступные через git, вместо fail-closed.
- Рост: не знаю.
- Команда: ``sed -n '156,183p' scripts/check_artifact_ids.sh``
```
id="$(id_of "$f")" || continue
[ -n "${id}" ] || continue
git cat-file -e "HEAD:${f}" 2>/dev/null || continue
```

**TD-128** (MAJOR, phase=M-61)
- Суть: Оба под-дефекта на месте: нет различения merge-объединения от новой правки, и exit 1 по-прежнему молчаливый.
- Рост: не знаю
- Команда: `sed -n '95,105p' scripts/check_docs_freeze.sh`
```
for c in $(git rev-list "${BASE}..HEAD"); do
  if touches "$c"; then tok "$c" || exit 1; fi
done
exit 0
```

**TD-131** (MAJOR, phase=M-62)
- Суть: Коммит имён сегментов всё ещё в конце is_fresh, мутация перестановки не ловится ничем.
- Рост: не знаю
- Команда: `grep -n "self.file_names = cur_names" crates/journal/src/segments.rs ; grep -n "namescommit" scripts/tests/red_segment_meta_battery.sh`
```
segments.rs:427: self.file_names = cur_names;
battery.sh: Мутанты с ПУСТЫМ kill-set'ом (namescommit, ...)
```

**TD-134** (MAJOR, phase=не указана)
- Суть: Правило про изоляцию `CARGO_TARGET_DIR` между деревьями при мутационном контроле так и не появилось.
- Рост: не знаю.
- Команда: ``grep -rn "CARGO_TARGET_DIR" .claude/rules/*.md``
```
(пусто — совпадений нет)
```

**TD-136** (MAJOR, phase=не указана)
- Суть: Шаги S и N всё ещё глотают stderr — при падении невозможно отличить «инвариант нарушен» от «механизм не отработал».
- Рост: не знаю (зависит от нагрузки хоста, не от объёма данных)
- Команда: `grep -n ">/dev/null 2>&1\|2>/dev/null" scripts/verify_M-61.sh | sed -n '1,3p'`
```
41:  if ( EVENT_NAME=push ... bash "${BARRIER}" >/dev/null 2>&1 ); then
72:    got="$(bash "${ALLOC}" "${CLS}" 2>/dev/null)"
```

**TD-148** (MAJOR, phase=не указана)
- Суть: счётчик segment_meta_ops по-прежнему существует только внутри библиотеки и не логируется и не экспортируется наружу — на проде цену такта в терминах этого счётчика проверить нечем.
- Рост: не знаю
- Команда: `grep -n "events_decoded\|segments_opened\|segment_meta_ops" crates/gateway-serve/src/lib.rs; grep -n "\"/metrics\"" crates/gateway-serve/src/lib.rs`
```
114:    /// - возвращает честные `ReadStats{events_decoded, segments_opened}` — для §8 eyes-on.
1545:            events_decoded = stats.events_decoded,
1546:            segments_opened = stats.segments_opened,
(/metrics — пусто)
```

**TD-168** (MAJOR, phase=не указана)
- Суть: карточка (расширенная 2026-08-29) документирует, что весь корпус тестов слеп к рецидиву дефекта close-семантики (мутация MUT-CLOSE прошла 278/278 тестов и verify PASS) — оракул на этот конкретный регресс всё ещё не написан, долг объявлен, но не закрыт.
- Рост: не знаю
- Команда: `grep -n "flat_map\|bucket_time_s\|distinct_values" crates/gateway/tests/red_depth_cadence.rs | head -6`
```
38://! bucket_time_s (`:824-830`) делит на 1000 целочисленно. Замер:
116:                // полосе — она была КОНСТАНТНОЙ (замер A-025: 120 точек, distinct_values=1),
184:    // ровно «первое событие интервала», против которого и написана. Плюс flat_map схлопывал
192:    // Сравнение идёт ПО СТРОКАМ (side, band_pct_e8); flat_map не воспроизводится.
```

**TD-169** (MAJOR, phase=Ф2/Ф4 (из сводной таблицы реестра))
- Суть: Тест d16 сам объявляет себя вакуумным зелёным пином — чекпоинт на LATEST, хвоста нет, задача каденции ещё не реализована; расхождение warm/full на незакрытом интервале по-прежнему не проверяется.
- Рост: не знаю
- Команда: `sed -n '380,392p' crates/gateway/tests/red_depth_cadence.rs`
```
/// # d16 ЗЕЛЁН сегодня, и это объявлено, а не замаскировано
/// Каденция ещё не реализована, поэтому тёплый и полный пути совпадают ТРИВИАЛЬНО.
/// Это GREEN-ПИН ... Мутацию architect предъявить не смог...
```

**TD-170** (MAJOR, phase=Ф2/Ф4)
- Суть: Гвард проверяет только выравнивание на сутки — 100 и 250 делят 86 400 000 нацело, поэтому подсекундный timeframe_ms по-прежнему проходит молча.
- Рост: не знаю.
- Команда: ``sed -n '2720,2724p' crates/gateway/src/lib.rs` (86400000%100==0, %250==0)`
```
if sel.timeframe_ms <= 0 || 86_400_000 % sel.timeframe_ms != 0 { ... Err ... }
```

**TD-179** (MAJOR, phase=Ф2)
- Суть: Побатчевое продвижение курсора при отказе из середины по-прежнему теряет уже собранные кадры.
- Рост: не знаю.
- Команда: ``sed -n '4441,4443p;4505,4508p' crates/gateway/src/lib.rs``
```
for event in &mut stream {
    let event = event?;
...
    self.cursor = cursor;   // побатчево, внутри цикла
```

**TD-189** (MAJOR, phase=M-68)
- Суть: Обе реализации подсчёта глубины существуют раздельно, равенство не проверяется ничем.
- Рост: не знаю
- Команда: `grep -n "fn depth_from_book\|pub fn depth_within" crates/gateway/src/lib.rs crates/book/src/lib.rs`
```
gateway/src/lib.rs:1191: fn depth_from_book(...)
book/src/lib.rs:199: pub fn depth_within(...)
```

## 3. ПРОТУХШИЕ ЗАПИСИ

### 3.0 Пример из задания (TD-162) — на самом деле НЕ протух

Задание указывало на TD-162 как на образец расхождения таблица/тело. Проверка
командой:

```
$ sed -n '103p;927,932p' TECH-DEBT.md
103:| ~~**TD-162**~~ | ✅ **ЗАКРЫТ 2026-08-24** merge'ем M-69 (`56a0dfc`, `R-128` APPROVE) ... | Ф2 | — (закрыт) |
927:> **`TD-162` ЗАКРЫТ 2026-08-24** (reviewer, вердикт `R-128`, merge M-69 `56a0dfc`, **§8 PASSED**
...
963:> Тело карточки НЕ переносится в архив: это единственный носитель разбора «буквальный R7
> жил здесь, а не там, где его полгода обсуждали» (`A-015` §2.2).
932:- **TD-162** `parse-ошибка-GATEWAY_WINDOW_MS-тихо-даёт-unbounded-буквальный-класс-R7`
```

Таблица помечена закрытой, СРАЗУ под ней стоит явный closed-callout с разбором
(строки 927-963), и только ПОСЛЕ него, СОЗНАТЕЛЬНО, оставлено старое тело
карточки — reviewer прямо объясняет зачем (носитель разбора, не архивируется).
Это не протухшая запись, это документированный дизайн «зачем тело осталось».
Проверено кодом (`crates/gateway-serve/src/lib.rs:2121-2130`, батч 3) — парсинг
`GATEWAY_WINDOW_MS` действительно fail-closed, закрытие реальное.

**Настоящая и гораздо более крупная проблема — другая.**

### 3.1 21 карточка закрыта в коде, но тело НЕ несёт вообще никакой пометки

В отличие от TD-162 (образцовый случай), у 21 карточки НЕТ closed-callout'а —
ни строки, ни абзаца, ничего. Читатель, открывший тело, видит только
формулировку дефекта и ничего о его судьбе. Проверено командой на каждую:

```
$ grep -c "ЗАКРЫТ" cards/TD-NNN.txt   # для каждой из 21 → 0
```

Семь из них (помечены ⚠ТАБЛИЦА ниже) вдобавок ЧИСЛЯТСЯ ОТКРЫТЫМИ в сводной
таблице «В ПЛАНЕ» — то есть двойная ложь: и таблица, и тело говорят «открыт»,
код говорит «закрыт». Это TD-117, TD-138, TD-163, TD-165, TD-166, TD-185,
TD-164 — три из них таблица прямо называет MAJOR и «блокирует Ф2/доставку».

Для каждой из карточек ниже проверено командой: `grep -c "ЗАКРЫТ" cards/TD-NNN.txt` = 0
— в теле карточки НЕТ ни одной пометки о закрытии (ни `✅ ЗАКРЫТ`, ни блока-колбэка),
хотя код на `origin/main` (91a4b84) уже несёт исправление. Семь из них (отмечены ⚠ТАБЛИЦА)
дополнительно числятся ОТКРЫТЫМИ в сводной таблице «В ПЛАНЕ» (строки 82-136).

### TD-054
- Реестр (тело карточки): не содержит закрывающей пометки — выглядит открытым.
- Факт: M-52 ввёл единый бюджет работы (в байтах); при исчерпании скан деградирует в честный Unknown, оракул гоняет НЕ-0x5A мусор — ровно то, что требовалось.
- Команда: `grep -n "READABLE_FLOOR_WORK_BUDGET_BYTES\|BudgetExhausted" crates/journal/src/segments.rs ; grep -n "0x5A\|lcg_garbage" crates/journal/tests/red_floor_work_budget.rs`
```
2610:    let mut budget = WorkBudget::new(READABLE_FLOOR_WORK_BUDGET_BYTES);
2621:        ScanOutcome::BudgetExhausted => return Ok(ReadableFloor::Unknown),
47://! Почему фикстуры здесь — ПСЕВДОСЛУЧАЙНЫЕ, а не 0x5A
118:fn wb_0_pseudorandom_garbage_carries_no_valid_frame() {
171:fn wb_1_floor_scan_work_is_bounded_on_uniform_garbage() {
```

### TD-064
- Реестр (тело карточки): не содержит закрывающей пометки — выглядит открытым.
- Факт: Класс вердиктов исключён из скана битых ссылок — PR #56 (f814347, 2026-08-22) теперь влит в проверяемый HEAD.
- Команда: `grep -n "VERDICT_CLASS_DIRS" scripts/verify_design_claims.sh; git merge-base --is-ancestor f814347 HEAD && echo ancestor-yes`
```
700:VERDICT_CLASS_DIRS = ("research/critiques/", "research/reviews/", "research/arbitration/")
ancestor-yes
```

### TD-068
- Реестр (тело карточки): не содержит закрывающей пометки — выглядит открытым.
- Факт: Дефект был не в коде, а в процессе оценки объёма работ по T1-варианту (карта "по памяти" пропускала examples/bin); практика (грепом + build --workspace в verify) внедрена — воспроизводимость дефекта снижена.
- Команда: `ls crates/journal/src/segments.rs crates/sim/src/exchange.rs crates/recorder/src/lib.rs crates/journal/examples/dump.rs crates/research-cli/src/bin/latency_probe.rs; grep -l "cargo build --workspace" scripts/verify_M-*.sh | wc -l`
```
все 5 файлов существуют (карта из 5 мест подтверждена)
несколько verify-скриптов уже содержат cargo build --workspace
```

### TD-073
- Реестр (тело карточки): не содержит закрывающей пометки — выглядит открытым.
- Факт: Дефект устранён — маркер теперь машинно проверяется. Реестр не отражает закрытие (карточка всё ещё в разделе «ЗАМОРОЖЕНО» как открытая).
- Команда: ``sed -n '1193,1206p' scripts/verify_design_claims.sh``
```
def classify_sha_token(tok, declared, root=None):
    """TD-073 (закрыто 2026-08-03): маркер `<!-- not-a-commit: X -->` БОЛЬШЕ НЕ
    самообслуживаемый. Если X ЯВЛЯЕТСЯ коммитом — FAIL ("LIAR-DECL")."""
    if root is not None and git_commit_exists(root, low):
        return "LIAR-DECL"
```

### TD-076
- Реестр (тело карточки): не содержит закрывающей пометки — выглядит открытым.
- Факт: `recover()` теперь тоже прогоняет guard монотонности `first_seq` — пробел закрыт (устранено вместе с TD-141), тихого беспорядка seq через `recover()` больше нет.
- Команда: `sed -n '462,472p' crates/journal/src/lib.rs; ls crates/journal/tests/red_recover.rs`
```
pub fn recover(dir...) {
    ...
    // JR-I-11 (M-52, TD-030): ... Guard монотонности first_seq идёт ПЕРВЫМ ...
    // До закрытия TD-141 читать инвариант как «держится на всех путях, КРОМЕ recover» — теперь КРОМЕ снято.
    segments::check_monotonic_paths(dir, &segs, &mut ops)?;
crates/journal/tests/red_recover.rs — существует
```

### TD-078
- Реестр (тело карточки): не содержит закрывающей пометки — выглядит открытым.
- Факт: Потолок оракула теперь масштабируется ×6 в debug-сборке (какую и гоняет CI) — риск «зелёный локально, красный на CI» устранён коммитами c6b4f3b/05a1fab.
- Команда: `grep -n "CEILING_SCALE" crates/journal/tests/red_floor_work_budget.rs; git log --oneline -3 -- crates/journal/tests/red_floor_work_budget.rs`
```
98:const CEILING_SCALE: u64 = if cfg!(debug_assertions) { 6 } else { 1 };
99:const CEILING_SECS: u64 = 60 * CEILING_SCALE;
05a1fab fix(TD-078): пустая строка после doc-комментария красила main на clippy — моя регрессия из c6b4f3b
c6b4f3b test(TD-078): потолок wall-clock масштабируется под режим сборки (debug ×6) — оракул мерил CI-машину, а не код
```

### TD-086
- Реестр (тело карточки): не содержит закрывающей пометки — выглядит открытым.
- Факт: Обе половины устранены — деплои сериализованы, пуш тестов не передеплоивает прод.
- Команда: `grep -n "concurrency\|group:\|cancel-in-progress" .github/workflows/deploy.yml ; sed -n '30,45p' .github/workflows/deploy.yml`
```
concurrency:
  group: deploy-main
  cancel-in-progress: false
    paths:
      - 'crates/**'
      - '!crates/*/tests/**'
```

### TD-092
- Реестр (тело карточки): не содержит закрывающей пометки — выглядит открытым.
- Факт: Флаг --full-history добавлен — барьер больше не объявляет legit rename evil merge.
- Команда: `grep -n "full-history" scripts/check_protected_artifacts.sh`
```
318: git log --full-history --diff-filter=DR -M --format='%H' "${base}..HEAD" -- "${path}"
354: git log --full-history --diff-filter=DR -M --format='%H' HEAD -- "${path}" ...
```

### TD-109
- Реестр (тело карточки): не содержит закрывающей пометки — выглядит открытым.
- Факт: Sacred-оракул переписан: теперь явно проверяет, что events_scanned НЕ является алиасом events_decoded (M-57, коммиты b8ddedb/2e48409) — именно тот кандидат, что карточка называла первым.
- Команда: `grep -n "events_scanned\|O-1 (TD-109)" crates/journal/tests/red_tail_scan_bounded.rs; git log -1 --format='%h %ci' b8ddedb`
```
30:  счётчик events_scanned, и только на нём строится главный оракул O-2.
148:/// реализация fn events_scanned(&self) -> u64 { self.events_decoded } — буквальный алиас,
209: "O-1 (TD-109): events_scanned ведёт себя как АЛИАС events_decoded. ...
b8ddedb 2026-08-07 09:34:12 +0000
```

### TD-116
- Реестр (тело карточки): не содержит закрывающей пометки — выглядит открытым.
- Факт: Обе недостающие проверки добавлены — hint.pos теперь валидируется против длины файла и границы кадра (M-62, коммит fc4e18a).
- Команда: `sed -n '1685,1706p' crates/journal/src/segments.rs; git log --oneline -1 -S"условие 6: hint.pos против длины файла"`
```
1690: // M-62 §7, условие 6: hint.pos против длины файла.
1691: if hint.pos > file_len { ... return Ok(header_end); }
1702: // M-62 §7, условие 6b: hint.pos должен указывать на границу кадра.
1704: if !self.probe_frame_boundary(path, hint.pos) { return Ok(header_end); }
fc4e18a feat(M-62): task #4 — guard hint.pos against file length and frame boundary
```

### TD-117 ⚠ТАБЛИЦА (числится открытым и там)
- Реестр (тело карточки): не содержит закрывающей пометки — выглядит открытым.
- Факт: Появился отдельный оракул (red_tick_read_cost.rs, F-036/VB-I-10), меряющий реальное чтение процесса — ловит регресс к полному перескану на тиковом пути.
- Команда: `grep -n "TD-117" crates/gateway/tests/red_tick_read_cost.rs`
```
245: Прямо пиннит то, что TD-117 назвал незапиненным...
```

### TD-137
- Реестр (тело карточки): не содержит закрывающей пометки — выглядит открытым.
- Факт: Ложное утверждение «побайтовое совпадение» в шапке блока прямо переписано с признанием ошибки и точным замером расхождения (29 vs 35 строк) — основной дефект (а) устранён; мёртвая константа TD_PAT в next_artifact_id.sh (пункт б) технически всё ещё не используется, но это уже мелкий остаток, а не вводящее в заблуждение утверждение.
- Команда: `sed -n '29,40p' scripts/next_artifact_id.sh; sed -n '77,88p' scripts/check_artifact_ids.sh; grep -c "TD_PAT" scripts/next_artifact_id.sh`
```
next_artifact_id.sh: "TD-137: прежняя редакция утверждала «там тот же блок, ПОБАЙТОВО».
Это неверно ... замер: 29 строк здесь против 35 там ..."
check_artifact_ids.sh: тот же текст зеркально (35 строк здесь против 29 там)
TD_PAT: 2  (определение + упоминание в комментарии, реально не используется)
```

### TD-138 ⚠ТАБЛИЦА (числится открытым и там)
- Реестр (тело карточки): не содержит закрывающей пометки — выглядит открытым.
- Факт: `docs/fa/journal.md` переписан и теперь верно описывает механизм наследования guard'а после M-62 (три ребра, включая SegmentCatalog), вместо устаревшего утверждения про несуществующий вызов.
- Команда: `sed -n '184,205p' docs/fa/journal.md`
```
"ШЕСТОЙ путь и смена МЕХАНИЗМА наследования (M-62, merge d564617; TD-138)."
таблица трёх рёбер: SegmentCatalog::open / is_fresh→validate_like_full_path / refresh
"Сверяется командой, а не памятью" + grep-инструкция вместо номеров строк
```

### TD-140
- Реестр (тело карточки): не содержит закрывающей пометки — выглядит открытым.
- Факт: Расхождение текста спеки (25 vs 26) устранено явным разложением числа 28.
- Команда: `grep -n "F2 |" milestones/M-61-artifact-ids.md`
```
596: F2 | ... Состав — РАЗЛОЖЕНИЕМ, не итогом (TD-140): 26 мутантов (§4.5) + ... = 28 ...
```

### TD-155
- Реестр (тело карточки): не содержит закрывающей пометки — выглядит открытым.
- Факт: карточка утверждала, что скрипт и джоб объявлены в правилах, но физически отсутствуют — сейчас оба существуют и джоб review-fa входит в блокирующий агрегат All checks passed, расхождение устранено.
- Команда: `ls scripts/check_review_fa.sh; grep -n "^  review-fa:" .github/workflows/ci.yml; grep -n "needs:" .github/workflows/ci.yml | grep status-check`
```
scripts/check_review_fa.sh
480:  review-fa:
needs: [build-test, security, delivery, protected-artifacts, contracts, docs-freeze, artifact-ids, reserve-ids, design-claims, context-budgets, gate-meta, deploy-catchup, review-fa, resource-oracles, rollout-composition]
```

### TD-163 ⚠ТАБЛИЦА (числится открытым и там)
- Реестр (тело карточки): не содержит закрывающей пометки — выглядит открытым.
- Факт: Круг 3 (коммит 38a6df7, PR #75/main 15abd0c) закрыл ложно-красный класс — подтверждено кодом на текущем HEAD; сама карточка уже сообщает об этом.
- Команда: `git log --oneline --all -S"fallback_allowed" -- scripts/check_protected_artifacts.sh | tail -1; git merge-base --is-ancestor 15abd0c HEAD && echo ancestor-yes`
```
38a6df7 fix(harness): TD-163 круг 3 — условие fallback'а задано СОСТОЯНИЮ; закрыты три ложных зелёных
ancestor-yes
```

### TD-164 ⚠ТАБЛИЦА (числится открытым и там)
- Реестр (тело карточки): не содержит закрывающей пометки — выглядит открытым.
- Факт: Кандидат B (авто-воспроизводимость дропа через git merge-tree, функция drop_is_handmade, вызывается из fallback_allowed) реально реализован и вшит в барьер. ВНИМАНИЕ: сводная таблица TECH-DEBT.md (строка 105) до сих пор показывает TD-164 как открытый — это расхождение таблица/код, а не гипотеза карточки.
- Команда: `git log --oneline -3 -S"drop_is_handmade" -- scripts/check_protected_artifacts.sh; git merge-base --is-ancestor 38ec6c0 HEAD && echo ancestor-yes`
```
38ec6c0 fix(harness): TD-164 круг 4 — кандидат B, авто-воспроизводимость дропа; закрыт класс
ancestor-yes
```

### TD-165 ⚠ТАБЛИЦА (числится открытым и там)
- Реестр (тело карточки): не содержит закрывающей пометки — выглядит открытым.
- Факт: Профиль reviewer'а поправлен 25.08 — waiver теперь предписывается класть туда, где его реально читает барьер.
- Команда: `sed -n '78,82p' .claude/agents/reviewer.md`
```
Пробел предъявляется явно: FA-WAIVER ... отдельной строкой в ФАЙЛЕ ВЕРДИКТА research/reviews/R-NNN-*.md.
Поправка 2026-08-25 (TD-165). Здесь стояло «в теле коммита», это было ЛОЖНОЙ нормой...
```

### TD-166 ⚠ТАБЛИЦА (числится открытым и там)
- Реестр (тело карточки): не содержит закрывающей пометки — выглядит открытым.
- Факт: Оракул больше не сравнивает подстрокой по всей строке лога — сверяет цифры сразу после `KEY=`. Реестр не проставил отметку о закрытии.
- Команда: ``sed -n '293,300p' crates/gateway-serve/tests/red_max_subs_config.rs``
```
assert_eq!(
    value_named_with(&on_default, "GATEWAY_MAX_SUBSCRIPTIONS").as_deref(),
    Some(SIGNED_DEFAULT.to_string().as_str()), ...
```

### TD-185 ⚠ТАБЛИЦА (числится открытым и там)
- Реестр (тело карточки): не содержит закрывающей пометки — выглядит открытым.
- Факт: Оба ложных утверждения FA (снимок-only и обрыв цепочки версий на 7→8) исправлены, версия в коде совпадает с документом (9), и семейство MD-I теперь определено в DESIGN.md §22 — окно лжи закрыто приземлением правки документа.
- Команда: `grep -n "GATEWAY_SCHEMA_VERSION" crates/gateway/src/lib.rs; grep -n "7→8" docs/fa/viz-backend.md; grep -n "MD-I" docs/DESIGN.md`
```
crates/gateway/src/lib.rs:98:pub const GATEWAY_SCHEMA_VERSION: u32 = 9;
docs/fa/viz-backend.md:177: VB-I-11 7→8, M-68 8→9 ...
docs/DESIGN.md:925:| MD-I | депт-серия ... заводится M-68: MD-I-8 определён текстом в docs/fa/viz-backend.md §5 ...
```

### TD-194
- Реестр (тело карточки): не содержит закрывающей пометки — выглядит открытым.
- Факт: Компаратор состава перенесён из разового milestone-скрипта в постоянный джоб CI — именно то лечение, что требовала карточка.
- Команда: ``sed -n '520,540p' .github/workflows/ci.yml; grep -n "L2DELTA_CAPTURE_SYMBOLS" scripts/check_rollout_composition.sh``
```
rollout-composition:
  name: Rollout composition (состав записи = подписанному, TD-196)
  run: bash scripts/check_rollout_composition.sh
scripts/check_rollout_composition.sh:89: C_SYMS=...L2DELTA_CAPTURE_SYMBOLS=...
```

### 3.2 Собственная бухгалтерия файла врёт самой себе (уровень заголовков)

Помимо статуса отдельных карточек, у файла есть заголовки-счётчики секций —
и они разошлись с фактическим числом карточек под ними:

```
$ grep -n '^# ' TECH-DEBT.md
51:# В ПЛАНЕ — 48 карточек
1883:# ЗАМОРОЖЕНО до продуктового темпа — 74 карточки

$ awk 'NR>=51 && NR<1883 && /^- (~~)?\*\*TD-[0-9]+\*\*/' TECH-DEBT.md | wc -l
49
$ awk 'NR>=1883 && /^- (~~)?\*\*TD-[0-9]+\*\*/' TECH-DEBT.md | wc -l
78
```

Заголовок «В ПЛАНЕ — 48» при фактических 49 карточках-бюллетах в секции;
«ЗАМОРОЖЕНО — 74» при фактических 78. Отдельно у секции «ЗАМОРОЖЕНО» есть своя
внутренняя таблица групп-навигации, и её сумма — ТРЕТЬЕ число:

```
$ sed -n '1920,1926p' TECH-DEBT.md | grep -oE '\| [0-9]+ \|' 
| 6 |
| 29 |
| 13 |
| 8 |
| 9 |
| 5 |
```

6+29+13+8+9+5 = 70 — не 74 (заголовок) и не 78 (факт). Три источника, три
разных числа для одной и той же секции. TD-156 явно исключён из счёта группами
(закрыт как принятый риск) — это объясняет часть разрыва (78-1=77), но не весь
(70 vs 77 — ещё 7 карточек без группы, включая свежие TD-198-201, заведённые
после того, как таблица групп была построена).

Аналогично таблица «В ПЛАНЕ» (строки 82-136) физически содержит 55 строк, из
них 5 уже помечены закрытыми (`~~TD-NNN~~`):

```
$ sed -n '82,136p' TECH-DEBT.md | grep -c '^|'
55
$ sed -n '82,136p' TECH-DEBT.md | grep -c '~~\*\*TD-'
5
```

50 открытых строк в таблице — а заголовок секции говорит «48 карточек» для
ВСЕЙ секции (таблица + список ниже). Это не единый источник правды, это
минимум четыре независимо считающих себя счётчика (заголовок секции 1,
заголовок секции 2, таблица групп секции 2, физический список бюллетов),
которые никто не пересчитывает при каждой правке.

## 4. ПРЕДЕЛЫ ЭТОГО РАЗБОРА

Названы честно — чего эта работа НЕ проверяла и почему.

1. **5 карточек (TD-002, TD-060, TD-080, TD-123, TD-149) — НЕПРОВЕРЯЕМ дёшево.**
   TD-002 требует операционного действия с секретом (пересоздание SSH-ключа) —
   не свойство кода. TD-060 требует прогона классификатора диффа схемы на
   синтетических примерах (oneOf/breaking), не грепа. TD-080 не называет
   конкретный файл-шаблон milestone-спеки. TD-123 и TD-149 требуют живого
   прод-стенда (N параллельных обрывов сессий; повторный цикл teardown с
   замером `RssAnon`) — из статического чекаута `/tmp/hft-audit-debt` это не
   воспроизвести.

2. **TD-097 и TD-149 (латентность подключения, память сессий) — вердикт ЖИВОЙ
   вынесен по САМООПИСАНИЮ карточки** (последняя запись в её теле говорит
   «часть НЕ ЗАКРЫТА» / «плато, вопрос открыт»), а не по независимому
   прод-замеру этой сессии — такой замер здесь не проводился.

3. **6 параллельных проходов (батчей) работали НЕЗАВИСИМО друг от друга** —
   один и тот же файл/паттерн МОГ быть проверен дважды разными формулировками
   grep без взаимной сверки; расхождений между батчами по итоговому вердикту
   при сведении не найдено, но перекрёстная валидация не проводилась
   намеренно (это ускорило бы работу вдвое ценой риска пропустить
   несогласованность — компромисс сознательный).

4. **Cargo build/test не запускался почти нигде** — по инструкции батчам
   (дорого, workspace большой). Вердикты опираются на статический анализ кода
   (grep/чтение) и `git log`/`git merge-base` для проверки, что названный
   закрывающий коммит реально существует и достижим из `HEAD`. Там, где
   карточка утверждает поведение конкретного теста, проверялось НАЛИЧИЕ и
   СОДЕРЖАНИЕ теста, не факт его прогона на этой сессии.

5. **Оценка «дорожает ли долг»** (столбец роста в §2) — качественная,
   основана на том, что САМА карточка говорит о величине (растущий журнал,
   растущий трафик, растущее число сессий). Для карточек без явного указания
   роста в тексте стоит «не знаю» — это не значит «не растёт», значит «карточка
   не даёт материала для суждения» без отдельного замера, которого здесь не
   делалось.

6. **TD-167 и TD-181 переклассифицированы** из «ОТЛОЖЕНО» (как их изначально
   пометил проверявший батч) в «ЖИВОЙ»: они НЕ про торговый путь (risk/oms/
   killswitch/execution), они про харнесс, сознательно замороженный до
   отдельной подписи founder'а/до другой карточки. Дефект в коде сегодня
   реален — «отложено» здесь про ПРИОРИТЕТ починки, не про существование.

7. **Разбор не трогал `TECH-DEBT.md`** — по прямому запрету задания; это
   read-only аудит, отдельный файл. Приземление найденного (расстановка
   closed-меток, синхронизация счётчиков) — зона reviewer'а.

8. **Совпадение эпохи.** Всё сведено на одном SHA (`91a4b84`, 2026-09-07,
   `git rev-parse HEAD` в `/tmp/hft-audit-debt` подтверждает совпадение с
   заявленным в задании). Проверка не покрывает то, что могло измениться в
   `main` ПОСЛЕ этой ревизии.
