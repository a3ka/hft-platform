<!-- GATE-META
milestone: M-99
audited_repo: a3ka/hft-platform
audited_base: cde51bf845a54d043030e271cf913e5e6196d9d9
audited_head: 037ce0a92ffce5e77390ebb3a9544363740fbfe2
verdict: APPROVE
-->

# R-267 — M-99 restore-drill automation, PR-гейт круг 3: APPROVE

FA-WAIVER: crates/recorder — FA для крейта recorder не существует (reading-map §2); опора — OPS-I-10 из docs/fa/ops.md

## §0. Предмет

- Ветка `origin/feat/M-99-restore-drill-automation`, вершина взята командой после `git fetch`:
  `037ce0a9` (= справке tester'а; `merge-base --is-ancestor` → 0). Повторный fetch перед
  записью вердикта — без сдвига.
- База аудита — `merge-base origin/main HEAD` = `cde51bf8` = `origin/main` (ветка не отстаёт).
- Круг 3 — последний PR-круг с добором оракулов (`A-056` §2.3). Судится: суть 2c (`R-266` Б-1)
  и попутные тексты (`R-266` Н-2/Н-3), коммиты `f2cebe8e`, `35114443`, `47c82e5d`, `037ce0a9`
  поверх набора architect'а `d98a2c89`/`011aa29a`/`40fde15f` (`C-317` NOTE).
- Прочитано по предмету: спека `M-99` целиком; `R-266` (мой круг 2); `A-056` §1–§4;
  `C-317`; `M-74` §«Сигнатура читателя»; `docs/fa/journal.md` `JR-I-6`; `docs/fa/ops.md`
  `OPS-I-3`, `OPS-I-10`. Ярус C — грепом: `M-99` в `PROJECT-STATE.md`, `TD-193` в `TECH-DEBT.md`.
- Инварианты тронутых модулей: **`OPS-I-3`** (холодная копия периодически восстанавливается и
  читается), **`OPS-I-10`** (объявлена ⟹ эмитится), **`JR-I-6`** (старый журнал читается
  новым кодом навсегда).

## §1. Вердикт

**APPROVE.** `R-266` Б-1 закрыт в реализации и запиннен оракулом, который краснеет на
нейтрализованных воротах (§2). Новой дыры класса «обещание спеки без оракула / писатель нарушает
T2» (`A-056` §2.3) не найдено: краевые формы §3 Н-1 T2 не нарушают и укладываются в предикат
ворот, объявленный арбитром. Всё прочее — не блокеры.

## §2. Закрытие `R-266` Б-1 — предъявлено исполнением

`deploy/bin/journal-restore-drill-cron.sh:298-319` — ворота протокола успеха ровно по `M-99`
§4 стр.11 / `A-056` §4.1: `ok=1` только при `rc=0` И `digest` 64 строчных hex И `events_read`
≥ 1 И `intra_segment_continuous=true`; иначе `ok=0`, `digest=""`,
`reader-protocol-violation`, код 7, состояние записано. Ветки `missing-digest-on-success` нет.
`intra=false` остаётся `intra-segment-gap`/4; отсутствие поля в `intra-segment-gap` больше не
превращается (`:292`).

- Проба: `RP0`, `RP` (10 форм), `RP-M` (9 мутантов) — PASS, `VERDICT: PASS (36/36)`.
- **Мутационный контроль (мой):** условие ворот `:315` → `if false` ⇒ проба
  `VERDICT: FAIL (1 из 36)`, красный — только `RP`. Файл возвращён, дерево чистое.

## §3. Не блокеры

- **Н-1 — ворота разбирают вывод `sed`'ом по полям, а не как JSON.** Сверх десяти форм пробы
  исполнено 12 форм прод-обёрткой (§Done Block). `ok=1` получают: (а) оборванный JSON с тремя
  полями; (б) не-JSON-текст с тремя полями; (в) две строки, где `true` идёт раньше `false`
  (берётся первая); (г) `"intra_segment_continuous":truex`. Остальные (65 hex, числа строкой,
  отрицательные, `"true"` строкой, `1e30`) дают `ok=0`/7 — верно.
  **Почему не блокер и не СТОП по `A-056` §2.3:** (1) T2 не нарушен — во всех четырёх случаях
  `ok=1` пишется с 64-hex `digest`, взятым у читателя; (2) реализация совпадает с предикатом,
  объявленным арбитром (`A-056` §4.1 «полный протокол = три условия»), а форма «stdout не JSON»
  в его замере §3 — вывод без полей, и она покрыта; (3) прод-читатель эти формы не выдаёт:
  одна строка `println!` фиксированного формата, `intra` — литерал `true`/`false`
  (`journal-drill-read.rs:264-276`), при обрыве вывода код ≠ 0. Расхождение с ТЕКСТОМ спеки
  есть: §4 стр.11 перечисляет «stdout не JSON» среди отказов без оговорки о полях. Решение
  (сузить текст или добавить проверку формы JSON) — architect'а; заведено в `TECH-DEBT.md`.
- **Н-2 — runbook (`deploy/README.md`, «Разбор вердикта») неполон в двух местах.** (а) Обещает
  «ровно те причины, что пишет обёртка», но пропускает общий вид `reader-rc-<N>` — его обёртка
  пишет при любом коде читателя кроме 0/4/5/6 (исполнено: `rc=1/125/127` ⇒ `reader-rc-1/125/127`).
  Это ровно исход первого живого прогона, если в образе нет бинаря или образа. (б) «После
  починки: `rm …journal-restore-drill.alert`» — при отказе читателя (`reader-rc-*`) обёртка
  маркер тревоги не создаёт (ветка `:272-283` без `alert`). Спека маркер не обещает — наблюдение
  идёт через файл состояния и `WD-RESTORE-DRILL`, — так что это неточность текста, не дыра.
- **Н-3 — нет общего предела времени у транспорта** (`R-266` Н-4, `M-99` §11). Заводится в
  `TECH-DEBT.md`, как предписывает §11.
- **Н-4 — тесты engine-dev в `src/`** (`R-266` Н-5) — без изменений, отмечено ранее.

`R-266` Н-2 закрыт полностью (`ops-watchdog.rs` путь, обёртка «каталог ПУСТ» → «доставленные
байты», `Dockerfile` «`hft-recorder`» → `image:` сервиса). `R-266` Н-3 закрыт в названных трёх
пунктах (лишние причины убраны, `reader-protocol-violation`/7 добавлена, перечитывание через
`docker run … --entrypoint`); остаток — Н-2 выше.

## §4. Что проверено и в порядке

- **Scope:** после `R-266` engine-dev тронул ровно `deploy/bin/journal-restore-drill-cron.sh`,
  `crates/ops/src/bin/ops-watchdog.rs`, `Dockerfile`, `deploy/README.md` — все в §6 Allowed.
  Граница §7 (путь удаления, `crates/journal/src/**` кроме нового бинаря, `RETENTION`) — шаг
  «граница» гейта PASS.
- **RED-first / sacred:** RED (`d98a2c89`) и гейт (`011aa29a`) — коммиты `[architect]` ДО
  реализации (`f2cebe8e`); ни один коммит `[engine-dev]` не трогает `*/tests/**`, `scripts/**`.
- **Атомарность:** суть 2c и три текстовые правки — четыре коммита, метки ролей, ссылки на
  задачу; `Co-Authored-By` в диапазоне — 0.
- **Merge `6cfc134d`** (синхронизация с `main`, `rustls` 0.23.46): `--remerge-diff` пуст —
  ручных разрешений нет.
- **Block-C:** `crates/contracts/**` не тронут. **Risk-блок:** `risk`/`killswitch`/`oms`/
  `venue-*` не тронуты — `risk-critic` не требуется.
- **Цепочка:** `R-266` REJECT → `A-056` DECISION → `C-317` NOTE → engine-dev → tester PASS → этот
  вердикт. Арбитр повторно не нужен: развилки «решён ли пункт» нет.
- **Дерево слияния:** `verify_design_claims.sh --merge-preview origin/main` PASS; `main` зелёный.

## §5. Условие и дальнейшее

Merge через PR. После деплоя — задача 8 (первый живой прогон `TRANSPORT=remote` на VPS),
отдельным артефактом close-out'а.

## Done Block

```
$ git fetch origin && git rev-parse origin/feat/M-99-restore-drill-automation
037ce0a92ffce5e77390ebb3a9544363740fbfe2
$ git merge-base --is-ancestor 037ce0a9… origin/feat/M-99-restore-drill-automation; echo $?
0
$ git rev-parse origin/main; git merge-base origin/main origin/feat/M-99-restore-drill-automation
cde51bf845a54d043030e271cf913e5e6196d9d9
cde51bf845a54d043030e271cf913e5e6196d9d9

$ bash scripts/verify_M-99.sh   (своё дерево /tmp/hft-reviewer-m99-r3, real 32m41s)
PASS-строк: 90
PASS: grep -q 'reader-protocol-violation' deploy/bin/journal-restore-drill-cron.sh && ! grep -q 'missing-digest-on-success' …
PASS: ci-parity: cargo fmt --all -- --check
PASS: ci-parity: cargo clippy --all-targets --all-features -- -D warnings
PASS: ci-parity: cargo test --all
PASS: ci-parity: cargo audit
PASS: ci-parity: учтено шагов 65 из 65 (исполнено 59, исключено по карте 6)
SKIP: task #8 — первый живой прогон на VPS TRANSPORT=remote (A-032 §2.4) — снимает reviewer
VERDICT: PASS
verify_exit=0

$ bash scripts/tests/red_restore_drill.sh | grep -E '^(PASS|FAIL)  (RP|RP0|RP-M)\b|VERDICT'
PASS  RP0 полный протокол читателя-заглушки ⇒ ok=1, код 0, отпечаток взят у читателя побайтно
PASS  RP 10 форм неверного вывода читателя при rc=0 ⇒ ok=0, без отпечатка, причина reader-protocol-violation, код 7, состояние записано
PASS  RP-M судья протокола принял допустимый и отверг 9 мутантов (…)
VERDICT: PASS (36/36) — копия читается прод-читателем, исходы различимы
probe_exit=0

$ (мутация) условие ворот :315 → `if false`; bash scripts/tests/red_restore_drill.sh
FAIL  RP обёртка на протокольно-неверном выводе читателя (rc=0) нарушает M-99 §4 стр.11 / §7:
VERDICT: FAIL (1 из 36)
$ git checkout -- deploy/bin/journal-restore-drill-cron.sh; git status --porcelain
(пусто)

$ (Н-1/Н-2) прод-обёртка, TRANSPORT=local, читатель-заглушка, rc=0 если не указано
полный протокол                  rc=0  ok=1 digest=0123…cdef
обрыв JSON (без })               rc=0  ok=1 digest=0123…cdef
не-JSON с полями                 rc=0  ok=1 digest=0123…cdef
digest 65 hex                    rc=7  ok=0 reader-protocol-violation
events_read строкой              rc=7  ok=0 reader-protocol-violation
events_read отрицат.             rc=7  ok=0 reader-protocol-violation
events_read 1e30                 rc=7  ok=0 reader-protocol-violation
intra строкой "true"             rc=7  ok=0 reader-protocol-violation
intra=trueX                      rc=0  ok=1 digest=0123…cdef
две строки: false потом true     rc=4  ok=0 intra-segment-gap
две строки: true потом false     rc=0  ok=1 digest=0123…cdef
digest дважды: пустой+hex        rc=0  ok=1 digest=0123…cdef
rc=0 + stderr-шум                rc=0  ok=1 digest=0123…cdef
reader rc=1   → wrapper rc=1   reason "reader-rc-1"
reader rc=125 → wrapper rc=125 reason "reader-rc-125"
reader rc=127 → wrapper rc=127 reason "reader-rc-127"

$ git show --remerge-diff --format='%h %s' 6cfc134d
6cfc134d merge: sync main into restore-drill branch
(диф пуст)
$ git log --format='%B' cde51bf8..HEAD | grep -ci 'co-authored'
0
$ bash scripts/verify_design_claims.sh --merge-preview origin/main | tail -1
VERDICT: PASS (0 нарушений)
$ bash scripts/check_branch_health.sh | grep -E 'M-99|VERDICT'
feat/M-99-restore-drill-automation            0     50     35 —        0 сут
VERDICT: PASS — наблюдение состоялось
$ gh run list --branch main --limit 2
completed success Deploy to VPS … 2026-10-14T15:38:53Z
completed success Merge pull request #353 … CI … 2026-10-14T15:21
$ bash scripts/reserve_artifact_id.sh R
R-267
```

## Handoff §D

APPROVE ⇒ merge через PR — reviewer. После деплоя — задача 8 (первый живой прогон), затем
close-out (`PROJECT-STATE.md`, `TECH-DEBT.md`). Architect — уведомление о закрытии и Н-1/Н-2
(текст спеки §4 стр.11 и runbook) как долг.
