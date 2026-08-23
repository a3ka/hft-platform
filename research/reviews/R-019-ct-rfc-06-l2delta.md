# R-019 — `docs/ct-rfc-06-l2delta` @ `2852fae` — PR-гейт (DOC класс A)

**Date (UTC):** 2026-08-02
**Agent:** reviewer
**Branch:** `origin/docs/ct-rfc-06-l2delta` @ `2852fae`
**Worktree:** `/tmp/hft-rev-rfc06` (detached, свой — `branch-hygiene.md` §1)
**Класс гейта:** DOC класс A (`.claude/rules/gates.md` §9) — `docs/rfc/**`.
**Предыдущий гейт:** `C-045` PASS (critic).

## Вердикт: **CHANGES REQUESTED (не мержу)**

**Опровержение посылки — ПОДТВЕРЖДЕНО моей независимой проверкой, и это главное.** Вариант
`L2Delta` действительно уже в T1 с `CT-RFC-04`/M-18; вводить нечего; M-45 действительно
сводится к расширению allow-list без contract-пакета и без RISK-BLOCK. Карта из **ПЯТИ**
`match` подтверждена моим собственным грепом (мандатные «три» — занижение). §8 стоит на
проверенной земле. Содержательных дефектов рассуждения я не нашёл.

**Блокирует другое:** документ **не проходит мандатный доп-гейт документов**
`verify_design_claims.sh` (ветка `feat/gate-rfc-claims`) — **5 нарушений, exit=1**. Два из
них — ровно тот класс, ради которого гейт был построен неделю назад после `C-044` F1:
**пруф-якорь, живущий на несмёрженной ветке**. Документ уходит в `main` как принятая
governance-спека, а половина его доказательной базы (`research/measurements/**`) в `main`
не существует. Это не формальность: RFC, чьи пруфы нельзя пройти по репозиторию,
воспроизводит ровно ту дыру, которую закрывает соседний CT-RFC-05.

Правки объёмом на один коммит. Проектировать фикс — не моя зона (`gates.md` §4, граница
reviewer↔architect); ниже — что именно не сходится и как я это померил.

---

## Block-scope — ЧИСТО

Дифф docs-only, только добавления, ровно в зонах architect'а и critic'а. Ни `crates/`, ни
`contracts/`, ни `*/tests/`, ни `scripts/`, ни `milestones/`. Коммиты атомарны (4 шага
документа + вердикт критика), вердикт критика ЗАКОММИЧЕН на ветку (`branch-hygiene.md` §3).

```
$ git log --format='%h %an <%ae> %s' origin/main..HEAD
2852fae critic <critic@noreply.local> docs(critic): C-045 — CT-RFC-06 L2Delta PASS; опровержение посылки подтверждено, 5 match-мест подтверждены
cdd2dd6 architect <architect@noreply.local> docs(CT-RFC-06): §6-§9 — TD-053 CLOSED (M-50/JR-I-9), объёмы по замерам, план с картой 5 match (не 3), решения founder'а
bcc071c architect <architect@noreply.local> docs(CT-RFC-06): §3-§5 — эпохи через epoch_id (анти-E-001), инвариант JR-I-10 якорной достаточности, DET-I-1 на смешанном журнале
3ff4355 architect <architect@noreply.local> docs(CT-RFC-06): §0-§2 — проверка посылки (вариант уже в T1, CT-RFC-04/M-18), ратификация формы, класс изменения без бампа SCHEMA_VERSION
49fc8c3 architect <architect@noreply.local> docs(CT-RFC-06): скелет contract-RFC L2Delta — разделы 0-9, STATUS: PROPOSED

$ git diff --stat origin/main...HEAD
 docs/rfc/CT-RFC-06-l2delta.md                 | 366 ++++++++++++++++++++++++++
 research/critiques/C-045-ct-rfc-06-l2delta.md | 182 +++++++++++++
 2 files changed, 548 insertions(+)
```

## Block-risk — RISK-BLOCK корректно НЕ применён, и это проверено, а не принято на веру

Вывод «contract-пакет не нужен ⇒ risk-critic не нужен» держится ЦЕЛИКОМ на опровержении
посылки. Я проверил опровержение первым, до всего остального (см. ниже) — оно верно, значит
основание пропустить risk-critic законно: дифф не трогает `crates/contracts/**` и вообще ни
строки кода. **Оговорка для следующего агента:** это верно для ДОКУМЕНТА. Сам M-45 при
реализации трогает `crates/venue-*/src/**` — MD-only carve-out (`gates.md` §5) там
применим только если правка остаётся read-only-MD (константа allow-list), без order-egress;
подтверждать это будет reviewer M-45 в Block-scope, а не этот вердикт.

---

## ЧАСТЬ 1. Опровержение посылки — моя независимая проверка (ПОДТВЕРЖДЕНО)

### 1.1 Вариант `L2Delta` уже в T1

```
$ grep -n "L2Delta {" crates/contracts/src/lib.rs
293:    L2Delta {
$ sed -n '293,300p' crates/contracts/src/lib.rs
    L2Delta { bids: Vec<Level>, asks: Vec<Level>, first_update_id: u64,
              final_update_id: u64, prev_final_update_id: Option<u64>, ts_exch_ms: i64 }
$ git log --oneline -1 -S "L2Delta {" -- crates/contracts/src/lib.rs
6af0aef feat(M-18): CT-RFC-04 — MdPayload::L2Delta T1 (сырые book-дельты)
$ git cat-file -e f635bd2^{commit}; git merge-base --is-ancestor f635bd2 origin/main; echo ok
f635bd2 merge(M-18): CT-RFC-04 L2Delta — сырые book-дельты (BTC-only spot+perp) [reviewer APPROVED]  — ANCESTOR-MAIN
```

Форма поле-в-поле совпадает с §1 документа. Порядок вариантов: `Trade`(0) … `L2Delta`(6),
`MarginInventory`(7) — дискриминант 6 у `L2Delta` подтверждён.

```
$ grep -n "L2DELTA_CAPTURE_SYMBOLS" crates/venue-binance/src/lib.rs crates/venue-binance-futures/src/lib.rs
crates/venue-binance/src/lib.rs:485:const L2DELTA_CAPTURE_SYMBOLS: &[&str] = &["BTCUSDT"];
crates/venue-binance-futures/src/lib.rs:460:const L2DELTA_CAPTURE_SYMBOLS: &[&str] = &["BTCUSDT"];
```

Строки, названные в §0.2/§8.1, точны до номера. **Вывод: посылка мандата («новый вариант»)
действительно разошлась с кодом. Опровержение верно. M-45 = расширение константы, T1-форма
не меняется ⇒ contract-пакет `05-contract-layer.md` §4 и бамп `SCHEMA_VERSION` не нужны.**

### 1.2 Сколько `match` — мой греп, не текст документа: **ПЯТЬ**

Перечислил ВСЕ `match` по `payload` во всём `crates/` (включая `examples/**`, `src/bin/**`,
исключая `*/tests/`), затем вручную проверил каждый на наличие `_ =>`:

```
$ grep -rn --include=*.rs -E "match [&a-z_.]*payload" crates/ | grep -v "/tests/"
crates/research-cli/examples/depth_lifetime.rs:55    -> есть `_ => {}`   (не exhaustive)
crates/research-cli/src/bin/latency_probe.rs:120     -> 8 вариантов, БЕЗ wildcard  ✅
crates/signals/src/obi.rs:84                         -> есть `_ =>`      (не exhaustive)
crates/research-cli/src/export_io.rs:247             -> есть `_ =>`      (не exhaustive)
crates/recorder/src/recon_loop.rs:57                 -> `_ => return`    (не exhaustive)
crates/book/src/lib.rs:323                           -> есть `_ => {}`   (не exhaustive)
crates/journal/examples/dump.rs:18                   -> 8 вариантов, БЕЗ wildcard  ✅
crates/journal/examples/dump.rs:51                   -> есть `_ =>`      (не exhaustive)
crates/gateway/src/lib.rs:858 и :866                 -> `_ => return` / `_ => {}` (не exhaustive)
crates/sim/src/exchange.rs:227                       -> 8 вариантов, БЕЗ wildcard  ✅
crates/journal/src/segments.rs:2569                  -> 8 вариантов, БЕЗ wildcard  ✅
crates/recorder/src/lib.rs:70 (md_kind_label)        -> 8 вариантов, БЕЗ wildcard  ✅
```

**Итого ровно 5 компилятор-принудительных мест** — совпадает с таблицей §8.2 дословно,
включая `examples/**` и `src/bin/**`. Мандатные «три» — занижение (пропущены ровно те два,
что не ловятся узким `cargo test -p`).

**Независимое эмпирическое подтверждение:** когда в T1 РЕАЛЬНО добавляли 8-й вариант
(`MarginInventory`, M-35), компилятор потребовал армы ровно в этих пяти файлах — три
коммита, `git show --stat`: `f2d1edb` (`segments.rs` + `sim/exchange.rs` +
`journal/examples/dump.rs`), `ab6e222` (`latency_probe.rs`), `988afff`
(`recorder/src/lib.rs`). 3+1+1 = 5. Это не совпадение текстов двух документов — это
проверенный компилятором факт на реальном прецеденте.

### 1.3 `epoch_id` (§3) — механизм СУЩЕСТВУЕТ, не предлагается

```
$ sed -n '97,110p' crates/contracts/src/lib.rs
pub struct SegmentHeader { pub schema_version: u32, pub source: DataSource,
    pub provenance: String, pub epoch_id: String, ... }     # T1-поле есть
$ sed -n '2001,2005p' crates/journal/src/segments.rs
        if header.source == cfg.source
            && header.provenance == cfg.provenance
            && header.epoch_id == cfg.epoch_id
            && header.schema_version == contracts::SCHEMA_VERSION
$ grep -n "EPOCH_ID" crates/recorder/src/main.rs
481:    let epoch_id = std::env::var("EPOCH_ID").unwrap_or_else(|_| default_epoch_id_now());
```

Все три звена (T1-поле → reuse-условие `decide_open_segment` → операторский env-рычаг)
существуют в `main`. §3 не выдаёт проектируемое за существующее — проверено.

**Решает ли это класс E-001?** Частично, и документ этого не оговаривает — см. F4 ниже.

### 1.4 `JR-I-10` (§4) — сформулирован исполнимо

Формулировка квантифицирована (∀ seq S, ∀ символ с дельтами в эпохе S → ∃ достижимый якорь
`L2Snapshot` с `seq ≤ S` либо покрывающий чекпоинт) и снабжена конструкцией RED-оракула с
явным анти-плацебо («оракул падает на реализации, прунящей по одному лишь возрасту») — это
исполнимо, а не декларативно. Опорные механизмы существуют, проверил:

```
$ grep -n "checkpoint_covered_through_seq\|allow_prune_without_checkpoint" crates/journal/src/segments.rs | head -4
2117: pub checkpoint_covered_through_seq: Option<u64>,
2121: pub allow_prune_without_checkpoint: bool,
```

Оговорка — F5 ниже (термин «читаемое хранилище» машинно не определён).

### 1.5 `DET-I-1` на смешанном журнале (§5) — M-51 НЕ ломается

```
$ git cat-file -e d896b98^{commit} && git log -1 --format=%s d896b98
merge(M-51): DET-I-1/2/3 — бит-идентичный реплей потока, проекций и редьюсеров (TD-007)   — ANCESTOR-MAIN
$ for f in $(grep -rln "DET-I-" crates/*/tests/); do echo "$f L2Delta=$(grep -c L2Delta $f) L2Snapshot=$(grep -c L2Snapshot $f)"; done
crates/book/tests/red_det_projection.rs        L2Delta=1 L2Snapshot=1     <- M-51 (9d940e5), СМЕШАННЫЙ вход
crates/gateway/tests/red_checkpoint_byte_identity.rs L2Delta=4 L2Snapshot=3  <- M-38b, СМЕШАННЫЙ вход
crates/journal/tests/red_det_replay_digest.rs  L2Delta=0 L2Snapshot=0
crates/journal/tests/red_det_prodscale.rs      L2Delta=0 L2Snapshot=1
...
```

Смешанный журнал действительно уже под оракулом (`red_det_projection.rs` — из самого M-51,
`9d940e5` «test(M-51): DET-I-2 — проекция == пересборка реплеем»). Аргумент §5 «DET-I-1 —
свойство редьюсера, не однородности входа» корректен: `replay_digest` — потоковая свёртка
над фреймами, тип payload'а в неё не входит. **Разрушения M-51 нет.** Уточнение — F6.

### 1.6 Числа §6/§7 — сверены с источниками, все сходятся

`READABLE_SCAN_MAX_CARRY = 64 KiB` (`segments.rs:1568`), `FRAME_LEN_SANITY_CAP = 64 MiB`
(`segments.rs:431`) — точно. TD-053 CLOSED (`TECH-DEBT.md`, M-50, merge `163939a` —
существует, ANCESTOR-MAIN). Числа замера сверены с самим замером (он на ветке — см. F2):
`L2Delta` max 32 799 B payload / 32 807 B фрейм = 50.1%; синтетика 3236 уровней/сторону =
71 237 B; `L2Snapshot` bucket-cap 3000 = 66 032 B = 100.8%; выборка 4 614 851 событие — всё
дословно. Объёмы §7 сверены с `docs/06` §2 (8.83 GB/сут, ~96% байт `L2Snapshot`, 20–31 KB
на снапшот) и §5.1 (2.5–3× `[verify-at-impl]`) — совпадают, и §7 честно помечает, где
вывод авторский.

**Итог части 1: опровержение подтверждено полностью. Ни одного ложного утверждения о коде
я не нашёл.**

---

## ЧАСТЬ 2. Почему всё равно не мержу — 5 нарушений мандатного гейта

```
$ bash scripts/verify_design_claims.sh   # версия с ветки feat/gate-rfc-claims, прогнана внутри репо
FAIL  [6-RFC-SHA] docs/rfc/CT-RFC-06-l2delta.md:28: цитируется коммит `6122fce` — существует как git-объект, но НЕ входит в историю HEAD (орфан/несмёрженная ветка)
FAIL  [6-RFC-SHA] docs/rfc/CT-RFC-06-l2delta.md:364: цитируется коммит `6122fce` — то же
FAIL  [7-RFC-PATH] docs/rfc/CT-RFC-06-l2delta.md:15: путь `docs/07` — не существует в дереве репозитория
FAIL  [7-RFC-PATH] docs/rfc/CT-RFC-06-l2delta.md:27: путь `research/measurements/m-45-l2delta-impact.md` — не существует
FAIL  [7-RFC-PATH] docs/rfc/CT-RFC-06-l2delta.md:256: путь `research/measurements/td-053-event-size.md` — не существует
VERDICT: FAIL (5 нарушений)
exit=1
```

Проверил каждое нарушение руками — **ложных срабатываний нет**:

### F1 (БЛОКЕР) — пруф-якорь на несмёрженной ветке: `6122fce` / `m-45-l2delta-impact.md`

```
$ git cat-file -e 6122fce^{commit}   -> OK (объект есть)
$ git merge-base --is-ancestor 6122fce origin/main   -> НЕ ancestor
$ ls research/measurements/          -> No such file or directory  (каталога нет в main вовсе)
```

Документ ЧЕСТНО пишет «ветка `origin/research/m-45-impact`» — обмана нет, и это отличает
случай от `C-044` F1. Но гейт fail-closed не зря: **после merge в `main` принятая
governance-спека будет ссылаться на артефакт, которого в репозитории нет**, а ветка может
быть удалена/переписана — тогда центральный пруф карты влияния (§0.2, §8.2) станет
непроверяемым навсегда. Ровно эту дыру закрывает соседний CT-RFC-05, смёрженный сегодня же.
Ссылка на ветку — не эквивалент коммита в `main`.

### F2 (БЛОКЕР) — то же, но БЕЗ раскрытия: `research/measurements/td-053-event-size.md`

§6 берёт из этого файла ЧЕТЫРЕ числа (32 799 / 3236 / 71 237 / 66 032) и цитирует его как
обычный репо-путь, **не оговаривая, что файла в `main` нет** — в отличие от §0.2, где
ветка названа. Нашёл его сам:

```
$ for b in $(git branch -r --format='%(refname:short)'); do git ls-tree -r "$b" --name-only | grep -q "measurements/td-053-event-size.md" && echo "FOUND in $b"; done
FOUND in origin/research/td-053-event-size
```

Числа я сверил с этим файлом — **они верны** (см. §1.6). Дефект не в числах, а в том, что
пруф недостижим из `main`. Асимметрия с §0.2 (там ветка названа, здесь нет) — читатель
`main` решит, что путь просто битый.

### F3 (мелкий) — усечённый путь `docs/07`

Реальный файл — `docs/07-cockpit-backend-roadmap.md`; секция §14 в нём существует и
содержит именно то, на что ссылается §0.1 («Order Flow Intelligence — 4 индикатора +
L2Delta fidelity»). Т.е. смысл верен, форма пути — нет. По `gates.md` §9 путь — класс A
именно потому, что следующий агент по нему ходит.

### F4 (NOTE) — §3 не оговаривает, ЧЕГО механизм эпох не решает

`epoch_id` закрывает **намеренный** режимный сдвиг: маркер выставляется ДО раскатки. E-001
же был **незамеченным дефектом** (инвертированные стороны HL) — для него нельзя выставить
маркер заранее, потому что о нём не знали. §3 пишет «зеркальная противоположность E-001»,
не называя остаточный класс «семантический сдвиг, которого никто не заметил» — он остаётся
открытым. Плюс само выставление `EPOCH_ID` — операторский шаг: забыли выставить → сегмент
переиспользуется и эпохи молча смешиваются. §8.1 ловит это только post-hoc глазами (§8
eyes-on), машинного fail-closed нет. Для STATUS: PROPOSED приемлемо, но должно быть НАЗВАНО,
а не умолчано.

### F5 (NOTE) — `JR-I-10` опирается на машинно неопределённый термин

«достижимый якорь … в **читаемом хранилище (HOT/WARM/COLD)**»: COLD-читаемость зависит от
смонтированного Storage Box, а R1 ещё не сделан (§8.3 сам ставит R1 первым). В нынешней
формулировке инвариант выполним ВАКУУМНО, если COLD считать читаемым независимо от факта
монтирования. Оракул из §4 п.2 этого не ловит — он про retention_plan, не про доступность
tier'а.

### F6 (NOTE) — «M-51 закрепил DET-I-* поверх именно такого журнала» шире факта

Смешанный вход есть у DET-I-2 (`red_det_projection.rs`). У оракула DET-I-1
(`red_det_replay_digest.rs`) фикстур с `L2Delta` — ноль. Вывод §5 верен по существу
(digest тип-агностичен), но формулировка «DET-I-*» как семейства обещает покрытие, которого
у DET-I-1 фактически нет. Это ровно класс «фикстура счастливого пути» из
`.claude/rules/testing.md` — там же сказано, что документ обязан называть, что НЕ покрыто.

---

## Что требуется для APPROVED (не проектирую фикс — фиксирую условие гейта)

1. F1+F2: пруф-артефакты `research/measurements/**`, на которых стоят §0.2/§6/§8.2, должны
   быть достижимы из `main` — либо приземлены, либо не использоваться как якоря
   утверждений. Как именно — решает architect.
2. F3: путь `docs/07` → полное имя файла.
3. F4/F5/F6: назвать остаточные классы явно (что механизм НЕ решает, при каком условии
   инвариант не вакуумен, какой оракул чем НЕ покрыт).
4. Повторный прогон `verify_design_claims.sh --merge-preview origin/main` → `VERDICT: PASS`,
   exit=0. Гейт должен быть зелёным на MERGE-ЦЕЛИ, не только на ветке (R-013 Б-2/Б-3).

Повторный critic не требуется: содержательная часть (опровержение, карта, инварианты)
проверена дважды независимо — критиком и мной; правки не меняют выводов документа.

## Done Block

```
$ git -C /tmp/hft-rev-rfc06 log --format='%h %an %s' origin/main..HEAD | wc -l
5

$ grep -rn --include=*.rs -E "match [&a-z_.]*payload" crates/ | grep -v "/tests/" | wc -l
13   # из них exhaustive (без `_ =>`): 5

$ bash scripts/verify_design_claims.sh; echo "exit=$?"
VERDICT: FAIL (5 нарушений)
exit=1

$ bash scripts/verify_design_claims.sh --merge-preview origin/main; echo "exit=$?"
VERDICT: FAIL (5 нарушений)
exit=1
```

**Merge НЕ выполнен.** Ветка остаётся `docs/ct-rfc-06-l2delta`; `main` не тронут.
