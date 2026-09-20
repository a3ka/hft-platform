# R-023 — PR-гейт (круг 2): `feat/gate-rfc-claims` @ `e52ccd3` — устранение `R-020` B-1/N-1/N-2

- **Дата (UTC):** 2026-08-02
- **Роль:** reviewer
- **Ветка:** `feat/gate-rfc-claims` @ `e52ccd3` (5 коммитов architect'а поверх моего вердикта `1057694`)
- **База сравнения:** `origin/main` @ `3e78a80`
- **Вердикт: APPROVED** — блокер **B-1 закрыт по существу**, N-1/N-2/N-3/N-4 закрыты.
  Два новых остаточных долга (**TD-073**, **TD-074**), оба MINOR, оба НЕ блокируют.
- **Merge выполнен** (`--no-ff` в `main`), см. §7.

---

## 1. Block-scope — PASS

```
$ git diff --name-status origin/main...HEAD
A	research/notes/N-gate-rfc-b1-fix.md
A	research/reviews/R-020-gate-rfc-claims.md      (мой вердикт круга 1)
M	scripts/tests/red_verify_design_claims.sh
M	scripts/verify_design_claims.sh

$ git diff --name-only origin/main...HEAD | grep -E '^(docs/|\.github/|crates/|contracts/|milestones/)'
{пусто}   grep_rc=1
```

Диф строго в `scripts/**` + `research/notes/**`. **`docs/**` не тронут вовсе** — ключевая
проверка мандата: architect НЕ правил RFC/DESIGN ради зелёного гейта. `.github/workflows/**`
не тронут (N-6 остаётся отдельной работой, долг TD-062). `crates/`, `contracts/`,
`milestones/` — не тронуты.

Авторство всех пяти коммитов — `architect <architect@noreply.local>`, conventional-формат,
ссылка на `R-020` и предметную область (`docs-gate`) в subject, без co-author трейлеров.
Атомарность: три коммита оракулов + один коммит реализации + отчёт — раздельно.

Risk-блок **не применяется** (нет `risk`/`killswitch`/`oms`/`venue-*`/`contracts`).
Block-C не применяется.

## 2. B-1 — ЗАКРЫТ ПО СУЩЕСТВУ (воспроизведено моим же репро из `R-020`)

Требование `R-020`: «остаток обязан быть виден — каждый hex-токен SHA-формы вне фенсов либо
проверен, либо явно перечислен с причиной; "не нашёл цитат" не печатается там, где токены есть».

### 2.1 Моё репро круга 1 — синтетический RFC, форма «исправлением», без слова-маркера

Тот же файл, что в `R-020` (два выдуманных SHA, второй — в СОСЕДНЕМ параграфе):

```
$ cat $D/docs/rfc/CT-RFC-99-probe.md
# CT-RFC-99 — проба формы «подтверждено исправлением»
Это подтверждено отдельным исправлением `0000000deadbee` («fix(M-99): нечто»),
которого в репозитории не существует вовсе.
Здесь же close-out ревьюера (`1111111`) — тоже выдуманный.

$ bash scripts/verify_design_claims.sh $D | grep -E 'RFC-SHA|VERDICT'          # НОВЫЙ движок
FAIL  [6-RFC-SHA] docs/rfc/CT-RFC-99-probe.md:3: цитируется коммит `0000000deadbee` — не найден в git-объектах репозитория вовсе
FAIL  [6-RFC-SHA] docs/rfc/CT-RFC-99-probe.md:6: цитируется коммит `1111111` — не найден в git-объектах репозитория вовсе
INFO  [6-RFC-SHA] SHA-подобных токенов (docs/DESIGN.md + docs/rfc/**.md): всего=5 проверено=5 пропущено=0 — из них 5 нарушений
VERDICT: FAIL (36 нарушений)
exit=1

$ bash /tmp/rev-old-engine.sh $D | grep -E 'RFC-SHA|VERDICT'                    # СТАРЫЙ движок 1057694
FAIL  [6-RFC-SHA] docs/DESIGN.md:274: ... `cc5197c` ...
FAIL  [6-RFC-SHA] docs/DESIGN.md:882: ... `cc5197c` ...
VERDICT: FAIL (33 нарушений)
```

Старый движок обе подделки **пропускал молча**; новый ловит обе. Дополнительно новый ловит
`docs/DESIGN.md:318` — ровно третий из трёх токенов, которые я перечислил в `R-020` как
`SKIP-NOCTX`. «Проверка неприменима» на документе с токенами больше не печатается
(допустима только при `всего=0` — проверено кодом и оракулом `RFC-SHA-no-inapplicable`).

### 2.2 Баланс сходится на РЕАЛЬНОМ корпусе (замер мой, не перенос)

```
$ bash scripts/verify_design_claims.sh --merge-preview origin/main; echo exit=$?
PASS  [6-RFC-SHA]  всего=30 проверено=30 пропущено=0 — все 30 существуют И входят в историю HEAD/MERGE_HEAD
PASS  [7-RFC-PATH] всего=188 проверено=124 пропущено=64 — все 124 проверенных существуют
VERDICT: PASS (0 нарушений)
exit=0
```

`K+M==N` в обеих проверках. Проверка 6: пропусков **ноль** — весь корпус под гейтом.
Проверка 7: все 64 пропуска напечатаны построчно с файлом/строкой/токеном и причиной из
ЗАКРЫТОГО списка; разбивка (мой подсчёт по сырому выводу):
`SKIP-PROSE 39` · `SKIP-GLOB 18` · `SKIP-NOTREPO 6` · `SKIP-ABS 1`.

Я перечитал все 64 поимённо: настоящих ссылок на файлы среди них нет — одиночные `/`,
фрагменты прозы между inline-code, glob'ы (`crates/contracts/**`, `crates/venue-*`),
имена веток/ref'ов (`feat/M-08`, `origin/main`, `origin/research/m-45-impact`),
перечисления через слэш (`Ord/Risk/Ctl`, `DET-I-1/2/3`), эндпоинт `/sapi/v1/...`.

## 3. Отклонение от мандата (`SKIP-DIGITS` отвергнут) — проверено, обоснование подтверждается

**(а) Даёт ли `0000000`-подобный токен FAIL — ДА.**

```
$ cat $D/docs/rfc/sub/CT-RFC-98-nested.md
Влито коммитом `0000000` — семь нулей, канонический выдуманный SHA.
$ bash scripts/verify_design_claims.sh $D | grep 6-RFC-SHA
FAIL  [6-RFC-SHA] docs/rfc/sub/CT-RFC-98-nested.md:3: цитируется коммит `0000000` — не найден в git-объектах
```

Плюс `1111111` из §2.1. Под правилом `SKIP-DIGITS` обе формы ушли бы из-под гейта —
отклонение architect'а по существу верно, оно закрывает, а не открывает дыру.

**(б) Ложные FAIL от fail-closed на цифрах — НЕТ.** Замер мой, на дереве слияния:
из 30 SHA-токенов чисто цифровой **ровно один** — `0999929`
(`docs/rfc/CT-RFC-05-margin-inventory.md:139`), и он проверен без нарушения (настоящий
коммит). Fixed-point констант в backtick'ах на корпусе нет. `VERDICT: PASS`, exit=0.

N-3 закрыт двусторонне: цифровой токен без маркера → FAIL (fail-closed), он же с явным
`<!-- not-a-commit: <token> -->` → печатается строкой `SKIP-DECLARED` и входит в баланс
(проверено вручную: `всего=4 проверено=3 пропущено=1`).

## 4. N-1 / N-2 — закрыты

**N-1 (пути).** Формы, которые старый гейт пропускал молча, теперь резолвятся, а их
«битые» близнецы дают FAIL (прогон движка на реальном дереве):

```
'contracts/src/lib.rs:46'        → ПРОВЕРЕН, крейт-относительно (crates/<name>/...)
'recorder/src/main.rs:58'        → ПРОВЕРЕН, крейт-относительно
'journal/src/segments.rs'        → ПРОВЕРЕН, крейт-относительно
'tests/red_schema.rs'            → ПРОВЕРЕН, как суффикс существующего пути дерева
'journal/src/NETU.rs'            → НАРУШЕНИЕ (якорен, не резолвится) → FAIL
'contracts/src/NETU.rs'          → НАРУШЕНИЕ → FAIL
```

Чем резолвятся 124 проверенных на merge-цели (мой замер): `104` прямо от корня,
`17` крейт-относительно, `3` только суффиксно (`tests/red_schema.rs`,
`fixtures/invalid/event-l2delta-missing-final-id.json`, `examples`). Суффиксный резолв —
единственное расширяющее (ослабляющее) правило, и его фактический охват — 3 токена из 188.

**N-2 (рекурсия).** RFC в подкаталоге проверяется обеими проверками:

```
FAIL  [6-RFC-SHA]  docs/rfc/sub/CT-RFC-98-nested.md:3  `0000000`
FAIL  [7-RFC-PATH] docs/rfc/sub/CT-RFC-98-nested.md:4  `crates/journal/src/net-takogo-fajla.rs`
# тот же вход, старый движок:
INFO  [7-RFC-PATH] в docs/rfc/**.md путей ... не найдено — проверка неприменима
```

## 5. N-4 (RED-first) — соблюдён; проверено воспроизведением истории, не словами отчёта

```
$ git log --format='%h %ad %an %s' --date=iso 1057694..HEAD | tac
40be250 2026-08-02 13:02:10 architect test(docs-gate): RED — остаток SHA/путей обязан быть виден
13e8482 2026-08-02 13:04:03 architect test(docs-gate): RED-правка — цифровой ... SKIP-DIGITS
a6c5157 2026-08-02 13:08:34 architect test(docs-gate): RED-правка — цифровой токен fail-closed
12f6e41 2026-08-02 13:11:40 architect fix(docs-gate): остаток SHA/путей больше не исчезает молча
e52ccd3 2026-08-02 13:14:23 architect docs(architect): отчёт ...
```

Все три редакции оракулов — ДО реализации. Каждая редакция была КРАСНОЙ против
дореализационного движка (`git show 1057694:scripts/verify_design_claims.sh`):

```
оракул 40be250 vs старый движок: FAIL-сценариев=12 | VERDICT: FAIL (12 нарушений)
оракул 13e8482 vs старый движок: FAIL-сценариев=13 | VERDICT: FAIL (13 нарушений)
оракул a6c5157 vs старый движок: FAIL-сценариев=13 | VERDICT: FAIL (13 нарушений)
```

Финальный набор против финальной реализации — зелёный; промежуточные редакции против неё
FAIL(2)/FAIL(3), что и подтверждает: спека менялась ДО кода, а не подгонялась под него
(`13e8482` требовал `SKIP-DIGITS`, `a6c5157` переиграл требование на fail-closed — оба
раньше `12f6e41`).

Сценарий `RFC-SHA-no-context`, закреплявший дефект как норму, **удалён**; на его месте —
`RFC-SHA-fake-nomarker`, `RFC-SHA-orphan-nomarker`, `RFC-SHA-no-inapplicable` (+ два
подсценария) и `RFC-SHA-balance`.

## 6. Анти-плацебо и мутационный контроль — пройдены (прогоны мои)

**Прямой (новые оракулы против СТАРОЙ реализации `1057694`) — 13 сценариев краснеют:**

```
$ BARRIER=/tmp/rev-old-engine.sh bash scripts/tests/red_verify_design_claims.sh | grep '^FAIL  сценарий'
FAIL  сценарий RFC-SHA-fake-nomarker (B-1)          FAIL  сценарий RFC-SHA-no-inapplicable
FAIL  сценарий RFC-SHA-orphan-nomarker (B-1)        FAIL  сценарий RFC-SHA-no-inapplicable/второй токен
FAIL  сценарий RFC-SHA-digits-declared (N-3)        FAIL  сценарий RFC-SHA-no-inapplicable/цифровой выдуманный
FAIL  сценарий RFC-SHA-len64                        FAIL  сценарий RFC-SHA-balance
FAIL  сценарий RFC-SHA-declared                     FAIL  сценарий RFC-SUBDIR (N-2)
FAIL  сценарий RFC-PATH-crate-rel-missing (N-1)     FAIL  сценарий RFC-PATH-crate-rel-real (N-1)
FAIL  сценарий RFC-PATH-balance
VERDICT: FAIL (13 нарушений)   exit=1
```

**Обратный (ломаю новые проверки на КОПИЯХ скрипта; реальный файл не трогался):**

| Мутация | Что сломано | Self-test |
|---|---|---|
| M1 | воссоздано отвергнутое `SKIP-DIGITS` (цифровой токен не проверяется) | **FAIL 4** (`RFC-SHA-fake`, `digits-failclosed`, `no-inapplicable/цифровой`, `RFC-SUBDIR`) |
| M2 | печать строк `SKIP-*` выключена (остаток снова молчит) | **FAIL 3** (`digits-declared`, `len64`, `declared`) |
| M3 | баланс врёт: `проверено=всего пропущено=0` | **FAIL 1** (`RFC-SHA-balance`) |
| M4 | крейт-относительный резолв выключен | **PASS** — см. ниже |
| M5 | рекурсия → `os.listdir` (возврат к N-2) | **FAIL 1** (`RFC-SUBDIR`) |
| M6 | суффиксный резолв выключен | **PASS** — см. ниже |
| M4b | выключены ОБА расширяющих резолва (точный возврат к N-1) | **FAIL 1** (`RFC-PATH-crate-rel-real`) |
| M7 | якорение путей выключено (`path_token_is_anchored` → False) | **FAIL 3** (`RFC-SUBDIR`, `RFC-PATH-fake`, `crate-rel-missing`) |

M4/M6 давшие PASS — **не дефект оракула**: замером подтверждено, что крейт-относительный и
суффиксный резолвы взаимно избыточны для этих форм (`'journal/src/segments.rs' in
suffix_index → True`), поэтому выключение одного не меняет наблюдаемого поведения.
Оракул проверяет требуемое СВОЙСТВО («форма резолвится И токен засчитан в баланс»), а не
конкретный механизм; выключение обоих механизмов (M4b) он ловит.

```
$ git status --porcelain      # мутанты удалены, дерево чистое
{пусто}
```

## 7. Ложные срабатывания и форма гейта

- `--merge-preview origin/main` → `VERDICT: PASS (0 нарушений)`, exit=0 (§2.2). Новая
  строгость на реальном корпусе не даёт ни одного ложного FAIL.
- Документы под зелёный гейт **не правились**: `docs/**` вне диффа (§1); заявление
  architect'а §8 отчёта («находок в документах — ни одной») подтверждается тем, что гейт
  зелёный БЕЗ единой правки docs.
- Голое дерево ветки: `VERDICT: FAIL (2 нарушений)`, exit=1 — обе на проверке 4
  (`docs/ORCHESTRATION-STATE.md:223/:245` → `docs/rfc/CT-RFC-06-l2delta.md`), приходят из
  базы ветки; на merge-цели файл существует, проверка 4 зелёная. Это `R-020` N-5, не диф.
- Запрещённой конструкции `cmd && echo PASS || echo FAIL` в скриптах нет (только упоминание
  в комментарии-шапке). Агрегация/exit-код — внутри движка; `VERDICT` соответствует exit.
- `grep -n "verify_design_claims" .github/workflows/*.yml` → пусто (TD-062 остаётся открыт).

## 8. Новые долги (заведены, НЕ блокируют)

- **TD-073** — маркер `<!-- not-a-commit: <token> -->` даёт автору документа право вывести
  ЛЮБОЙ токен из-под проверки 6 одной строкой в том же файле; гейт не проверяет
  обоснованность объявления. Отличие от B-1 принципиальное и делает это долгом, а не
  блокером: там обход происходил СЛУЧАЙНО и НЕВИДИМО (обычный синоним), здесь — намеренно и
  ГРОМКО (строка `SKIP-DECLARED` в выводе + HTML-комментарий в дифе). Severity: MINOR.
- **TD-074** — кандидатом проверки 6 является только токен **в backtick'ах**; `подтверждено
  коммитом b3a5a95` без обратных кавычек гейту не виден и в баланс не попадает. Замер мой
  на merge-цели: SHA-подобных токенов вне backtick'ов — **5**, все на
  `docs/rfc/CT-RFC-04-l2delta.md:164-165` и все являются числовыми литералами
  (`6500050000000`, `1752000000123`, …), ни один не в контексте цитаты коммита; достижимых
  форм лжи сегодня — 0. Severity: MINOR (латентный остаток класса B-1).

Остаются открытыми и этим PR не затронуты: **TD-062** (гейт не в CI — N-6 из `R-020`;
препятствий больше нет), **TD-063** (INFO «неприменимо» вместо FAIL — закрыто ТОЛЬКО для
проверок 6/7, для 1/2/5 остаётся), **TD-064** (гейт краснеет на цитату битой ссылки).

## 9. Done Block (прогоны reviewer'а, worktree `/tmp/hft-rev-gate2`)

```
$ git log --format='%h %an <%ae> %s' -5
e52ccd3 architect <architect@noreply.local> docs(architect): отчёт по устранению R-020 B-1/N-1/N-2 + отклонение SKIP-DIGITS
12f6e41 architect <architect@noreply.local> fix(docs-gate): остаток SHA/путей больше не исчезает молча (R-020 B-1, N-1, N-2)
a6c5157 architect <architect@noreply.local> test(docs-gate): RED-правка — цифровой токен fail-closed, N-3 закрывается маркером not-a-commit
13e8482 architect <architect@noreply.local> test(docs-gate): RED-правка — цифровой выдуманный SHA идёт в SKIP-DIGITS, не в FAIL
40be250 architect <architect@noreply.local> test(docs-gate): RED — остаток SHA/путей обязан быть виден (R-020 B-1/N-1/N-2)

$ git status --porcelain
{пусто}

$ bash scripts/tests/red_verify_design_claims.sh > /tmp/rev-selftest.txt 2>&1; echo exit=$?
exit=0
$ grep -cE '^(PASS|FAIL)  сценарий' /tmp/rev-selftest.txt ; grep -E '^(FAIL|VERDICT)' /tmp/rev-selftest.txt
41
VERDICT: PASS

$ bash scripts/verify_design_claims.sh --merge-preview origin/main; echo exit=$?
PASS  [6-RFC-SHA] всего=30 проверено=30 пропущено=0
PASS  [7-RFC-PATH] всего=188 проверено=124 пропущено=64
VERDICT: PASS (0 нарушений)
exit=0

$ BARRIER=<старый движок 1057694> bash scripts/tests/red_verify_design_claims.sh | grep -E '^VERDICT'
VERDICT: FAIL (13 нарушений)

$ bash scripts/verify_design_claims.sh; echo exit=$?          # голое дерево ветки (N-5, база)
FAIL  [4-МЁРТВЫЕ-ФАЙЛЫ] docs/ORCHESTRATION-STATE.md:223 ...
FAIL  [4-МЁРТВЫЕ-ФАЙЛЫ] docs/ORCHESTRATION-STATE.md:245 ...
VERDICT: FAIL (2 нарушений)
exit=1
```

## 10. Итог

**APPROVED.** B-1 закрыт по существу — подтверждаю: остаток обеих проверок виден,
баланс сходится, «неприменимо» невозможно при непустом корпусе, моё репро круга 1 даёт FAIL
на обеих формах, старый движок на том же входе молчал. N-1/N-2/N-3/N-4 закрыты.
Отклонение `SKIP-DIGITS` проверено независимо и поддержано замером.

## 11. Merge и §8 деплой-гейт (`gates.md` §8)

`main` двинулся под ревью (`3e78a80` → `1c040e0`, параллельная сессия). Гейт перепрогнан на
НОВОЙ merge-цели до merge'а: `VERDICT: PASS (0 нарушений)` exit=0, числа те же (30/30/0 и
188/124/64). Merge `--no-ff` без конфликтов; на СМЁРЖЕННОМ дереве гейт и self-test прогнаны
повторно (PASS/exit=0, 41/41), в том числе ПОСЛЕ моих правок `PROJECT-STATE.md`/`TECH-DEBT.md`.

```
$ git log --format='%h %an <%ae> %s' -3       # состояние main после push
ae9e049 reviewer <reviewer@noreply.local> docs(reviewer): R-023 close-out — проверки 6/7 гейта документа в main; TD-073/TD-074 заведены, TD-062/TD-063 уточнены
4b762ef reviewer <reviewer@noreply.local> merge(docs-gate): R-023 APPROVED — гейт docs/rfc SHA/путей rev2 (B-1 закрыт: остаток виден, баланс сходится; N-1/N-2/N-3/N-4)
08cfdb1 reviewer <reviewer@noreply.local> docs(reviewer): R-023 APPROVED — гейт docs/rfc SHA/путей rev2 ...

$ git push origin HEAD:main
   1c040e0..ae9e049  HEAD -> main

$ gh run watch 30750098092 --exit-status ; echo watch_rc=$?
✓ All checks passed in 4s (ID 91503059411)
watch_rc=0

$ gh run list --limit 2
completed  success  docs(reviewer): R-023 close-out — проверки 6/7 гейта документа в main…  CI  main  push  30750098092  6m0s  2026-08-02T13:31:06Z
completed  success  merge(docs): R-021 — синхронизация номеров TD-071/TD-072 в вердикте (…  CI  main  push  30749918288  5m49s  2026-08-02T13:26:09Z
```

**Deploy НЕ триггерился — и это корректно, проверено фактом, а не предположением.**
`.github/workflows/deploy.yml` слушает `paths: crates/** · Cargo.toml · Cargo.lock ·
Dockerfile · docker-compose.yml · .github/workflows/deploy.yml`; содержимое push'а —
`PROJECT-STATE.md`, `TECH-DEBT.md`, `research/notes/**`, `research/reviews/**`,
`scripts/verify_design_claims.sh`, `scripts/tests/red_verify_design_claims.sh`. Ни одного
пути из триггера. Прод в этом заходе не пересобирался.

**Eyes-on на VPS (обязателен независимо от того, был ли деплой):**

```
$ ssh -i /home/nous/.ssh/hft_deploy -o IdentitiesOnly=yes root@167.233.192.131 \
    'docker ps --format "{{.Names}} {{.Status}}"; cat /var/lib/docker/volumes/hft-platform_journal-data/_data/recorder.heartbeat'
hft-gateway-serve Up 3 hours (healthy)
hft-recorder Up 3 hours (healthy)
{"events":831593,"free_bytes":85134544896,"min_free_bytes":10737418240,"next_seq":149371571,
 "segment_index":158,"ts_wall_ms":1785677835571,"writable":true}
```

Оба контейнера `(healthy)`; heartbeat свежий — `ts_wall_ms=1785677835571` против локального
времени замера `1785677838414` (лаг ≈ 2.8 с); `writable=true`, `next_seq=149371571` растёт,
свободного места 85 GiB при пороге 10 GiB. Прод здоров, милестоун закрывается не поверх
красного.
