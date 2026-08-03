# M-46 — engine-dev отчёт: `wsprobe` (задачи #1, #4, #5a)

**Дата (UTC):** 2026-08-03 · **Milestone:** `M-46-read-path-probe` · **Роль:** engine-dev
**Worktree:** `/tmp/hft-dev-m46` (detached, `origin/main`) · общий чекаут не трогался.

## 1. Что сделано

### Задача #1 — `wsprobe` WS read-path harness
`crates/gateway-serve/src/bin/wsprobe.rs` — read-only WS-клиент:
- `--url ws://HOST:PORT` (дефолт `ws://127.0.0.1:8080`), `--token <JWT>` ИЛИ `--secret <hex|str>`
  (тогда сам подписывает HS256, claims `{sub, exp}` — форма `gateway_serve::auth::Claims`);
- подключается, принимает первый `ServeMsg::Snapshot`, затем до `--frames N` (дефолт 20) кадров
  ИЛИ до `--seconds S` (дефолт 10) — что раньше; отсутствие кадров в пределах дедлайна НЕ ошибка
  (тихий рынок / self-test без новых событий после старта);
- пишет в `--out <dir>`: `snapshot.json` (сырой wire-JSON первого сообщения), `frames.jsonl`
  (по кадру на строку, сырой wire-JSON), `summary.json` (`schema_version`, `cursor.upto_seq`,
  `history_start_seq`, `history_truncated`, `latency_first_snapshot_ms`, `frames_received`,
  длины всех 10 серий `SeriesBundle`);
- печатает короткую сводку + ASCII-панель в stdout (не весь дамп);
- `--self-test`: без сети — сам строит эфемерный журнал-фикстуру (`tempfile::tempdir()`,
  `L2Snapshot` + мульти-филл `Trade` + асимметричные `L2Delta` по обе стороны границы UTC-суток,
  та же дисциплина «фикстура счастливого пути — дефект оракула», что у sacred-тестов M-46, но
  независимая копия — не читает/не импортирует `tests/`), поднимает `gateway_serve::server` на
  ephemeral-порту, сам подписывает токен, сам подключается. Это то, что зовёт гейт:
  `wsprobe --self-test --out <dir>`.

### Задача #4 — рендер «для глаз» без дизайна
- **ASCII-панель** (≤100 столбцов): heatmap-сетка 60×14 density-символами (` .:-=+*#%@`, price×time
  бакеты), VWAP/CVD спарклайны (блочные символы, последнее значение + знак у CVD), топ-5 COB по
  каждой стороне.
- **`panel.html`** — один автономный файл: инлайн CSS, инлайн JSON-данные (`const DATA = {...}`),
  инлайн JS (canvas, без внешних библиотек/CDN). Секции: **Heatmap** (главная, bid=зелёный/
  ask=красный по интенсивности size), **Candles + VWAP** (свечи + линия VWAP поверх), **CVD**
  (знак/цвет), **Volume Profile** (горизонтальные бары сбоку, POC подсвечен), **COB** (серверный
  HTML-список, top-10 каждая сторона). Содержит буквальные слова `heatmap`/`vwap`/`cvd` (T9) +
  реальные данные (не только разметку) — подтверждено визуально headless-Chrome скриншотом на
  `--self-test` фикстуре (все 5 секций показывают ненулевые данные).

### Задача #5a (добавлена architect'ом в ходе работы — находка: `wsprobe` не попадал в прод-образ)
`Dockerfile`: `--bin wsprobe` добавлен в `RUN cargo build --release ...` (builder stage) +
`COPY --from=builder .../wsprobe /usr/local/bin/wsprobe` в runtime-слой. `ENTRYPOINT` не тронут
(остаётся `recorder`, D2) — `wsprobe` вызывается явно через `docker run`, никогда не как
entrypoint. Проверено: `docker build -t hft-m46-check .` — успех; бинарь внутри контейнера
запущен и отработал (`docker run --rm --entrypoint /usr/local/bin/wsprobe hft-m46-check
--self-test --out /tmp/out` → все 4 артефакта сгенерированы).

### Вне моего мандата в этой сессии
Задача #5 (реальный sidecar-прогон против прода, `docker run --network container:hft-gateway-serve
hft-gateway-serve wsprobe ...` на VPS) — явно НЕ входила в инвокацию этой сессии («Твоя часть —
задачи 1 и 4»); требует доступа к прод-VPS и живого `gateway-serve`-контейнера. Задачи #2/#3
(сверка с реплеем / применение кадров) были green ДО начала этой сессии — они покрыты
существующим кодом `gateway`/`gateway-serve` библиотек (M-22/M-28/M-38b), не моей работой в этой
сессии; статус в milestone-таблице для них не трогал (не мой вклад — не буду присваивать).

## 2. Находки (не правил — только описываю, `gates.md` §4)

### Находка 1 — `verify_M-28.sh` (architect-owned, НЕ CI-гейт) стал красным на 2 из 6 проверок
`scripts/verify_M-28.sh` (не входит в `.github/workflows/ci.yml` — проверено грепом, только
`verify_delivery_M-08.sh`/`verify_contracts.sh`/`verify_ct_rfc_atomic.sh` там есть; и не входит в
`verify_M-46.sh`) содержит grep-канарейку GS-I-3 («нет journal-writer в `crates/gateway-serve/
src/**`») и позитивную канарейку «использует `gateway::`». Обе сканируют ВЕСЬ каталог `src/**`,
а не только `lib.rs`/`main.rs`. `wsprobe.rs` (по прямому требованию milestone'а — `--self-test`
строит эфемерную фикстуру-журнал) легитимно использует `Journal::open_with`/`WriterConfig`/
`.append(`/`.flush(` — но ТОЛЬКО для тестового харнесса, никогда для прод-журнала (единственный
писатель остаётся `recorder`, JR-I-1 не нарушен). Канарейка не различает «сервер пишет журнал»
(что и проверяет GS-I-3 по замыслу) от «тестовый инструмент строит СВОЙ tempdir-журнал» — ловит
буквальное совпадение строк, не намерение.

Второй провал того же прогона («gateway-serve uses gateway:: library») — не регрессия по сути, а
воспроизведение SIGPIPE-флейка, который сам `lib.rs:634-644` предупреждает документацией
(`_GW_USES_GATEWAY` sentinel): `sed ... | grep -qE 'gateway::'` под `set -o pipefail` ломается,
если совпадение находится РАНЬШЕ последней строки sed-вывода (grep закрывает пайп на первом
совпадении → sed получает SIGPIPE → ненулевой код перехватывается `pipefail`). `wsprobe.rs`
легитимно использует `gateway::Snapshot`/`gateway::SeriesBundle`/`gateway::CobLevel` как типы —
и, судя по всему, `find crates/gateway-serve/src -name '*.rs'` кладёт `bin/wsprobe.rs` в поток
sed РАНЬШЕ `lib.rs` (порядок обхода каталога), из-за чего grep находит совпадение до
sentinel-строки и триггерит тот самый флейк.

**Раздельный от гейта M-46 факт: `verify_M-28.sh` НЕ входит ни в CI, ни в `verify_M-46.sh` —
не блокирует эту задачу.** Но architect стоит решить (не я — `scripts/verify_*.sh` sacred):
сузить ли grep-скоуп GS-I-3/позитивной канарейки до `lib.rs`+`main.rs` (тонкие оболочки сервера),
или явно исключить `src/bin/wsprobe.rs` как «клиентский инструмент, не сервер».

Сырой вывод (воспроизводимо `bash scripts/verify_M-28.sh`):
```
FAIL no journal-writer in gateway-serve/src (GS-I-3 read-only)
  ↳ 919:    use journal::{Journal, WriterConfig};
929:    let cfg = WriterConfig { ... }; 936: Journal::open_with(dir, cfg)?; 938/949/962/975/987: j.append(...); 1000: j.flush()
FAIL gateway-serve uses gateway:: library (thin shell, not re-implementing reducers)
VERDICT: FAIL (2 failed)
```

### Находка 2 — URL без пути ломает HTTP request-line (не инвариант, баг харнесса, уже исправлен мной)
Не архитектурная находка, а собственный баг на этапе разработки (задокументирован для памяти):
`ws://host:port` без завершающего `/` перед `?token=` даёт `GET ?token=... HTTP/1.1` — невалидную
request-line, сервер (`httparse` внутри `tungstenite`) отвергает с `HTTP format error: invalid
format`, клиент видит `Handshake not finished`. Пофикшено нормализацией пути в `wsprobe.rs`
(добавляет `/` если после authority нет `/`) — уже в закоммиченной версии.

### Находка 3 (СРОЧНАЯ, НЕ моя, обнаружена ПОСЛЕ push моих коммитов) — `main` красный на `cargo clippy --workspace`

После `git rebase origin/main` для push'а close-out коммита обнаружилось, что
`cargo clippy --workspace --all-targets -- -D warnings` (T2 в `verify_M-46.sh`, и, судя по
`ci.yml`, тот же гейт в CI) стал **FAIL** — но НЕ из-за моих файлов. Причина —
`c6b4f3b test(TD-078): потолок wall-clock масштабируется под режим сборки` (автор
`architect`, приземлился на `main` МЕЖДУ моими пушами task#5a и close-out-доксом):

```
error: empty line after doc comment
  --> crates/journal/tests/red_floor_work_budget.rs:83:1
   |
83 | / /// отличал бы «ограничено» от «неограниченно» (прод — 158 сегментов, не один).
84 | |
   | |_^
...
   = note: `-D clippy::empty-line-after-doc-comments` implied by `-D warnings`
error: could not compile `journal` (test "red_floor_work_budget") due to 1 previous error
```

`crates/journal/tests/**` — sacred, architect-owned, вне моей зоны и вне зоны engine-dev в
принципе (`crates/journal/src/**` — моя зона по мандату, `tests/**` — никогда). Подтверждено:
на моих трёх коммитах (`57a5b08`/`69d63a5`/`691ff77`, ДО `c6b4f3b`) `cargo clippy --workspace`
был чист (см. Done Block §4 — тот прогон делался ДО этого ребейза). Это отдельная от M-46
регрессия чужого коммита. Не правил, не буду — сообщаю явно, т.к. она красит `main` для ВСЕХ,
включая CI по моим же пушам, которые в момент этого отчёта ещё `in_progress`
(`gh run list --branch main`).

## 3. Как запускать

```bash
# Прод/staging, готовый токен:
wsprobe --url ws://127.0.0.1:8080 --token <JWT> --frames 20 --seconds 10 --out ./out

# Прод/staging, свой секрет (подпишет сам, HS256, sub=wsprobe, exp=+1h):
wsprobe --url ws://127.0.0.1:8080 --secret <hex|str> --out ./out

# Без сети, для проверки рендера (это же зовёт verify_M-46.sh T9):
wsprobe --self-test --out ./out

# Sidecar против прода (задача #5, ВНЕ этой сессии — образ теперь содержит бинарь):
docker run --rm --network container:hft-gateway-serve hft-platform-recorder:local \
  wsprobe --url ws://127.0.0.1:8080 --secret <GATEWAY_JWT_SECRET из .env на VPS> \
  --out /tmp/wsprobe-prod
```

## 4. Done Block

```
$ git status --porcelain
(пусто)

$ git log --oneline -4
691ff77 feat(M-46): task #5a — Dockerfile собирает и копирует бинарь wsprobe
69d63a5 feat(M-46): task #4 — wsprobe render "для глаз" без дизайна (ASCII + panel.html)
57a5b08 feat(M-46): task #1 — wsprobe WS read-path harness (connect, snapshot+frames dump)
51dafe9 docs(process): находка — push тестов передеплоивает прод ...

$ cargo fmt --all -- --check; echo exit=$?
exit=0

$ cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3; echo exit=$?
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.47s
exit=0

$ cargo test -p gateway-serve -p gateway 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" (блоков: "NR")"}'
passed=122 failed=0 (блоков: 36)

$ bash scripts/verify_M-46.sh 2>&1 | grep -E "^(PASS|FAIL|VERDICT)"
PASS  T0 оракул присутствует: crates/gateway-serve/tests/red_ws_series_vs_replay.rs
PASS  T0 оракул присутствует: crates/gateway-serve/tests/red_ws_protocol.rs
PASS  T0 оракул присутствует: crates/gateway-serve/tests/red_ws_honesty_sessions.rs
PASS  T1 cargo build --workspace
PASS  T2 cargo clippy --workspace --all-targets -D warnings
PASS  T2b cargo fmt --all --check (совпадает с ci.yml:20)
PASS  T3 сверка WS↔реплей GREEN (5 тестов)
PASS  T4 фикстура O-1 содержит события книги (L2Snapshot+L2Delta)
PASS  T4 парный vantage на месте (Trade-only ⇒ книжные серии пусты)
PASS  T5 протокольные оракулы GREEN
PASS  T6 честность/сессии GREEN
PASS  T7 crates/contracts/** не тронут
PASS  T8 бинарь wsprobe собирается
PASS  T9 рендер даёт непустую панель с сериями (7730 байт)
VERDICT: PASS
exit=0

$ docker build -t hft-m46-check . 2>&1 | tail -3; echo exit=$?
#18 naming to docker.io/library/hft-m46-check 0.0s done
exit=0

$ docker run --rm --entrypoint /usr/local/bin/wsprobe hft-m46-check --self-test --out /tmp/out 2>&1 | tail -3
wrote /tmp/out (snapshot.json, frames.jsonl, summary.json, panel.html)
```

## 5. Push-статус

Три атомарных коммита, все запушены напрямую в `origin/main` (milestone уже вёлся без
отдельной feat-ветки — architect/critic/предыдущие dev-сессии тоже коммитили прямо в main
для M-46, см. `git log`; RISK-BLOCK неприменим — read-only, `crates/contracts/**` не тронут):

- `57a5b08` — task #1 (T8 → PASS)
- `69d63a5` — task #4 (T9 → PASS)
- `691ff77` — task #5a (Dockerfile)

Push-scope перед каждым push проверен (`git log origin/main..HEAD` — только мои коммиты).
Каждый push триггерит CI+Deploy на VPS (`.github/workflows/{ci,deploy}.yml`, path-фильтр
широкий). На момент этого отчёта все три прогона (`gh run list --branch main`) были
`in_progress` — **§8 post-merge деплой-гейт (дождаться success + ssh-проверка VPS) мной НЕ
пройден**: моя инвокация оканчивалась хендоффом к `tester`, не закрытием milestone'а; по
`.claude/rules/gates.md` §8 текст относит финальный деплой-гейт к «агенту, сделавшему push...
в конце milestone-цикла» (reviewer). Reviewer при закрытии M-46 ОБЯЗАН дождаться зелёного
CI+Deploy для всех трёх коммитов и сделать eyes-on VPS, прежде чем считать M-46 закрытым.
