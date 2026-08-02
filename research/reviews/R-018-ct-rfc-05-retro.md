# R-018 — `docs/ct-rfc-05-retro` @ `30b75eb` — PR-гейт (DOC класс A)

**Date (UTC):** 2026-08-02
**Agent:** reviewer
**Branch:** `origin/docs/ct-rfc-05-retro` @ `30b75eb`
**Worktree:** `/tmp/hft-rev-rfc05` (detached, свой — `branch-hygiene.md` §1)
**Класс гейта:** DOC класс A (`.claude/rules/gates.md` §9) — `docs/rfc/**` (contract-RFC).
**Предыдущие гейты в цепочке:** `C-044` REJECT → `03815dd` фикс → `C-046` PASS.

## Вердикт: **APPROVED**

Каждое утверждение документа о коде и истории перепроверено МНОЮ независимо (не по
вердиктам критика и не по тексту документа). Расхождений не найдено. Мандатный доп-гейт
`verify_design_claims.sh` (ветка `feat/gate-rfc-claims`) — PASS в режиме `--merge-preview
origin/main`, т.е. на MERGE-ЦЕЛИ, а не только на ветке (урок R-013 Б-2/Б-3).

---

## Block-scope

Дифф — docs-only, **только добавления**, ровно в зоне architect'а (`docs/rfc/**`) и
critic'а (`research/critiques/**`). Ни одной правки в `crates/`, `contracts/`, `*/tests/`,
`scripts/`, `milestones/`. Атомарность коммитов соблюдена (4 коммита, каждый — один шаг
цепочки; вердикты критика ЗАКОММИЧЕНЫ на ветку, не остались untracked —
`branch-hygiene.md` §3).

```
$ git log --format='%h %an <%ae> %s' origin/main..HEAD
30b75eb critic <critic@noreply.local> docs(critic): C-046 — CT-RFC-05 retro rev2 PASS; F1/F2 из C-044 закрыты и перепроверены
03815dd architect <architect@noreply.local> docs(rfc): C-044 F1/F2 — SHA проверены по истории, список мест правки дополнен
40a8ff8 critic <critic@noreply.local> docs(critic): C-044 verdict — ретро-документ CT-RFC-05
2b0cbc2 architect <architect@noreply.local> docs(rfc): CT-RFC-05 — ретро-документ на изменение T1, приземлившееся без RFC

$ git diff --stat origin/main...HEAD
 docs/rfc/CT-RFC-05-margin-inventory.md           | 210 +++++++++++++++++++++++
 research/critiques/C-044-ct-rfc-05-retro.md      | 164 ++++++++++++++++++
 research/critiques/C-046-ct-rfc-05-retro-rev2.md | 139 ++++++++++++++
 3 files changed, 513 insertions(+)
```

## Block-C (contract governance)

`crates/contracts/**` в диффе НЕ тронут ⇒ contract-RFC-пакет `05-contract-layer.md` §4 к
самому диффу не применяется; документ РЕТРО-фиксирует уже приземлившееся изменение и явно
это заявляет (§8: «не переигрывает изменение»). Машинный гейт атомарности это подтверждает:

```
$ bash scripts/verify_ct_rfc_atomic.sh; echo "exit=$?"
PASS  crates/contracts/src/** не тронут — атомарность CT-RFC пакета не применима

VERDICT: PASS
exit=0
```

## Block-risk

RISK-BLOCK **не применяется**: дифф не трогает `crates/risk|killswitch|oms|venue-*` и не
трогает `crates/contracts/**` — это ретро-ОПИСАНИЕ уже смёрженного и отревьюенного
изменения. Governance-след самого изменения M-35 полон и проверен мною по истории:
critic `C-024` (REJECT → r2 PASS `a174696`), risk-critic `C-025` PASS, reviewer close-out
`41d3526`. Нового решения документ не предлагает ⇒ founder-подпись сверх уже данной
(2026-07-25) не требуется.

---

## Мои собственные проверки (не доверяя C-044/C-046)

### 1. Каждый цитируемый SHA — существует И входит в `origin/main`

Проверены ВСЕ 12 hex-токенов документа, не только те 3, что чинил `03815dd`:

```
$ grep -oE '\b[0-9a-f]{7,40}\b' docs/rfc/CT-RFC-05-margin-inventory.md | sort -u | while read s; do ... git cat-file -e / git merge-base --is-ancestor ... done
0999929 EXISTS ANCESTOR-MAIN : fix(M-35): C-024 REJECT — proxy-collector reframe ... [architect]
1f342b8 EXISTS ANCESTOR-MAIN : merge(M-35): task 2e — spawn run_margin_inventory в recorder ...
239e796 EXISTS ANCESTOR-MAIN : docs(critic): C-024 verdict — reject M-35 CT-RFC-05
41d3526 EXISTS ANCESTOR-MAIN : docs(M-35): reviewer close-out — margin-inventory MERGED 1f342b8, §8 GREEN ...
557be33 EXISTS ANCESTOR-MAIN : feat(contracts): verify_ct_rfc_atomic.sh — машинная атомарность изменения T1 (класс CT-RFC-05)
988afff EXISTS ANCESTOR-MAIN : feat(M-35): task 2d — md_kind_label MarginInventory (recorder) [engine-dev]
a174696 EXISTS ANCESTOR-MAIN : docs(critic): C-024 r2 pass M-35 CT-RFC-05
ab6e222 EXISTS ANCESTOR-MAIN : feat(M-35): task 2c — exhaustive-match: MdPayload::MarginInventory { .. } => continue в latency_probe ...
b3a5a95 EXISTS ANCESTOR-MAIN : fix(M-35): red_rfc04 epoch-tripwire 3→4 (CT-RFC-05 bump ...) [architect]
b3b42d2 EXISTS ANCESTOR-MAIN : ci(contracts): подключить verify_contracts.sh + verify_ct_rfc_atomic.sh + diff_contract_schema.sh отдельным джобом
ba61c62 EXISTS ANCESTOR-MAIN : merge(M-35): margin-inventory available-collector — CT-RFC-05 MarginInventory (дискр.7, schema→4) ...
e06e48a EXISTS ANCESTOR-MAIN : feat(M-35): CT-RFC-05 — MdPayload::MarginInventory (дискр.7, schema→4) + schema regen + RED ...
f2d1edb EXISTS ANCESTOR-MAIN : feat(M-35): task 2b — exhaustive-match MarginInventory (journal/sim) [engine-dev]
```

**0 фантомных SHA. 0 орфанов** (дефект `C-044` F1 не воспроизводится). Даты в §STATUS тоже
сверены: `e06e48a` 2026-07-25T15:22:10Z, `ba61c62` 2026-07-25T19:50:10Z — документ цитирует
второе дословно, «8 дней без RFC-файла» до 2026-08-02 арифметически верно.

### 2. Полнота списка мест правки — мой греп, включая `examples/` и `src/bin/`

```
$ grep -rn "MarginInventory" --include=*.rs crates/   # 31 вхождение, по файлам:
  6 crates/venue-binance/tests/red_margin_inventory.rs
  4 crates/venue-binance/src/lib.rs          <- ПРОИЗВОДИТЕЛЬ (эмиссия), не match
  3 crates/recorder/src/main.rs              <- комментарии + spawn (task 2e), не match
  2 crates/research-cli/src/bin/latency_probe.rs
  2 crates/recorder/src/lib.rs
  2 crates/contracts/src/lib.rs              <- сам тип
  1 crates/sim/src/exchange.rs
  1 crates/journal/src/segments.rs
  1 crates/journal/examples/dump.rs
```

Отсеяв производителя (`venue-binance/src`), spawn-точку (`recorder/src/main.rs`) и сам
тип, получаю ровно **ПЯТЬ** exhaustive-`match`-мест: `journal/src/segments.rs:2577`,
`sim/src/exchange.rs:283`, `journal/examples/dump.rs:39`, `research-cli/src/bin/
latency_probe.rs:134`, `recorder/src/lib.rs:78`. Дословно совпадает с §4 документа,
включая `examples/**` и `src/bin/**` — то, что было пропущено в `C-044` F2. Проверено, что
у каждого из пяти `match` НЕТ `_ =>` (перечислены все 8 вариантов явно) — т.е. это
действительно компилятор-принудительные места, а не выбор автора.

### 3. Форма T1 и порядок вариантов (утверждение §1/§2 — «вставлен строго в конец»)

```
$ awk '/pub enum MdPayload/,/^}$/' crates/contracts/src/lib.rs | grep -nE '^\s{4}[A-Za-z0-9]+( \{|,|\()'
2:    Trade {        8:    L2Snapshot {   13:    Funding {     19:    OpenInterest {
25:    Liquidation {  33:    MarginRate {   55:    L2Delta {     73:    MarginInventory {
$ grep -n "SCHEMA_VERSION: u32" crates/contracts/src/lib.rs
26:pub const SCHEMA_VERSION: u32 = 4;
$ grep -n "MarginInventory {" crates/contracts/src/lib.rs
311:    MarginInventory {
```

Порядок 0..7 совпадает с §1 дословно; `MarginInventory` — последний; ссылки на строки
(`lib.rs:24-26`, `lib.rs:311-315`) точны. Комментарий-история версий в `lib.rs:12-26`
подтверждает обоснование бампа §3 (эпоха сегмента, а не wire-совместимость; TD-031 →
provenance-изоляция void в контейнере без `git`) — документ пересказывает его верно, без
добавления мотивации.

### 4. Артефакты §5 существуют и содержат заявленное

```
$ grep -n "^fn mi_i\|fn mi_i" crates/contracts/tests/ct_rfc05.rs
25:fn mi_i_1_margin_inventory_roundtrip()   47:fn mi_i_1_pre_rfc05_funding_still_decodes()   71:fn mi_i_1_schema_version_is_4()
$ grep -n "MarginInventory" crates/contracts/schema/event.schema.json | head -3
350/353 — объект MarginInventory, required: available_e8 + ts_exch_ms, оба integer/int64
$ ls crates/contracts/fixtures/{valid,invalid}/ | grep -i margin
event-margin-inventory.json ; event-margin-inventory-missing-ts.json
$ grep -n "CT-RFC-05" crates/contracts/CHANGELOG.md
6:## schema_version 3 → 4 — CT-RFC-05 «MarginInventory» (2026-07-25)
```

### 5. §6 «мотивация не выдумана» — сверено с источниками дословно

```
$ grep -n "Founder-решение\|маржин по" milestones/M-35-margin-inventory.md
6:...Founder-решение (2026-07-25): «маржин по usdt/usdc собирать». Источник задокументирован
$ grep -n "^## §9" research/data-quality/margin-source-survey.md
411:## §9. RE-PROBE С READ-ONLY КЛЮЧОМ (architect, 2026-07-25) — proxy достижим; ledger НЕТ (§8 стоит)
```

Граница интерпретации («сырой supply-пул, НЕ ledger») присутствует ДОСЛОВНО в трёх местах
одновременно: milestone «Мотивация», doc-comment типа `lib.rs:306-310`, JSON Schema
`description`. Документ ничего не добавил от себя — §8 честно ограничивает себя этими
источниками. **Выдуманной мотивации не обнаружено.**

### 6. §7 «почему документа не было» — проверяемо, не догадка

```
$ grep -n "Д2" docs/plans/contracts-current-state.md
156:### Д2 — `CT-RFC-05` в коде, RFC-документа нет — RFC-дисциплина нарушена на реальном T1-бампе
315:| CT-RFC-05 без документа | §1.3 и C-040 оба фиксируют факт | `ls docs/rfc/` — по-прежнему только 01-04 | Без изменений — дыра Д2 подтверждена заново |
$ sed -n '269,272p' research/critiques/C-040-design-migration-plan.md
**CT-RFC-05 в коде без документа — ПОДТВЕРЖДЕНО:** `docs/rfc/` содержит только `CT-RFC-01..04-*.md`; ...
$ grep -n "docs/rfc" scripts/verify_ct_rfc_atomic.sh
115-117: "RFC-документ (docs/rfc/CT-RFC-NNN-*.md)" | '^docs/rfc/CT-RFC-[0-9]+-.*\.md$' | "нет ... (класс CT-RFC-05, ... Д2)"
```

Обе ссылки §7 точны (номера строк `C-040` — попадание в цель), машинный гейт действительно
существует, действительно требует `docs/rfc/CT-RFC-NNN-*.md` и действительно подключён к CI
(`b3b42d2`). Формулировка «правило держалось словом, не проверкой» — подтверждается кодом.

### 7. Честность негативных утверждений

§4 прямо пишет: «Прямого RED-теста именно на `MarginInventory`-эпоху reuse-барьера в диффе
CT-RFC-05 **не обнаружено**», и §8 фиксирует это как открытый вопрос, а не как факт в любую
сторону. Проверил — так и есть: reuse-барьер живёт в общем `decide_open_segment` (введён
TD-031/CT-RFC-04), отдельного теста на дискриминант 7 нет. Документ не выдаёт косвенную
гарантию за прямую — это ровно та дисциплина, за отсутствие которой били C-041/C-042/R-004.

### 8. Мандатный доп-гейт документов — на MERGE-ЦЕЛИ

Скрипт взят из ветки `feat/gate-rfc-claims` (в `main` ещё нет) и прогнан ВНУТРИ репозитория;
в коммит НЕ включён (зона architect'а — reviewer его не приносит в `main` своей рукой).

```
$ bash scripts/verify_design_claims.sh --merge-preview origin/main
...
PASS  [6-RFC-SHA] все 17 цитат коммитов (docs/DESIGN.md + docs/rfc/**.md) существуют И входят в историю HEAD/MERGE_HEAD
PASS  [7-RFC-PATH] все 67 путей, процитированных в docs/rfc/**.md, существуют в дереве репозитория
VERDICT: PASS (0 нарушений)
exit=0
```

Прогон именно в `--merge-preview` — принципиально: R-013 Б-2/Б-3 показал, что зелёный гейт
НА ВЕТКЕ становится ложью в момент слияния, если `main` ушёл вперёд.

---

## Находки (NOTE, не блокирующие)

- **N1 (косметика).** §7, строка «critic C-024 — doc-гейт ..., **but** проверял CHANGELOG/
  фикстуры» — англоязычное `but` посреди русской фразы. Опечатка в прозе (класс B по
  `gates.md` §9), смысла не меняет; правится попутно при следующем касании файла.
- **N2 (не дефект документа).** §4 корректно фиксирует ОТСУТСТВИЕ прямого reuse-теста на
  эпоху 3→4. Это реальный пробел в оракулах (не в документе): гарантия «сегмент schema-3 не
  reuse'ится schema-4 бинарём» держится на общем коде `decide_open_segment` + tripwire-тесте
  предыдущей эпохи. Заведено в `TECH-DEBT.md` — проектирование оракула зона architect'а
  (`gates.md` §4, граница reviewer↔architect: я описываю, не проектирую).

## Что я НЕ проверял

- `crates/venue-binance/tests/red_margin_inventory.rs` построчно — документ явно выводит его
  из своей зоны (§8), и с ним согласен: файл вне `crates/contracts/**`.
- `cargo test --workspace` не гонял: дифф docs-only, ноль строк кода. Зелёность кода
  подтверждается post-merge CI (§8 ниже).

## Done Block

```
$ git -C /tmp/hft-rev-rfc05 status --porcelain
{пусто — до создания этого файла}

$ git log --format='%h %an %s' origin/main..HEAD | wc -l
4

$ bash scripts/verify_ct_rfc_atomic.sh; echo "exit=$?"
VERDICT: PASS
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main; echo "exit=$?"
VERDICT: PASS (0 нарушений)
exit=0
```

Post-merge §8 (CI + прод) — дописывается ПРУФОМ ниже после push в `main`.
