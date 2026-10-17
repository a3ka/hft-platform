# R-072 — перепроверка §9, круг 2: ветка `docs/architect-postreview-2026-08-14` после фикса `R-070`

**Вердикт: APPROVED с NOTE** — блокер `Б-1` круга 1 закрыт ПО СУЩЕСТВУ (все три ребра
guard'а названы по факту кода и подтверждены каждое своей командой), `Н-2`/`Н-3`/`Н-5`/`Н-6`
закрыты, гейты зелёные обеими формами. Три новых NOTE (`N-1`…`N-3`) — ни один не делает
документ ложным сегодня; блокеров нет, третий круг НЕ обязателен.

- **Дата:** 2026-08-14 · **Роль:** независимый Fable-перепроверщик (§9, свежий контекст,
  круг 2; автор правки — architect, в этом круге сторона)
- **Предмет:** `origin/docs/architect-postreview-2026-08-14`, HEAD `a4e876f`, база `c6c62b8`
  (= `origin/main` = merge-base — дерево ветки ≡ дереву слияния)
- **Модель:** Fable (claude-fable-5) · **Дерево:** `/tmp/hft-recheck2` (detached)
- **Номер `R-072` назначен диспетчером и проверен свободным; аллокатор НЕ вызывался**
  (мандат: параллельно работают пять ролей).
- **Вход круга:** `research/reviews/R-070-architect-postreview.md` (REJECT, `Б-1` + `Н-2`…`Н-6`);
  исправление — `a4e876f`.

---

## Б-1 — ЗАКРЫТ. Все три ребра сверены по коду, каждое своей командой

Новая редакция (`docs/fa/journal.md:177-182`, таблица рёбер; `docs/SESSION-HANDOFF.md` §0bis,
пункт `TD-138`) называет ТРИ ребра. Факт по `crates/journal/src/segments.rs`:

1. **`open` → `segments_counted` → `check_first_seq_monotonic`** — подтверждено:
   `sed -n '197,199p'` → тело `open` зовёт `segments_counted(dir, &mut ops)?` (`:199`);
   хвост `segments_counted` (`:1287`) зовёт `check_first_seq_monotonic(&candidates, …)?`.
2. **`is_fresh` (инкрементальная ветка) → `validate_like_full_path` → guard** — подтверждено:
   `self.validate_like_full_path(dir, &mut ops)?` на `:426`, ДО коммита кеша
   (`self.file_names = cur_names`, `:427`); тело `validate_like_full_path` (`:527-549`)
   завершается `check_first_seq_monotonic(&candidates, …)` (`:546`). Это ЕДИНСТВЕННЫЙ вызов
   `validate_like_full_path` в файле (grep: определение `:527` + один вызов `:426`).
3. **`refresh` → `segments_counted` → guard** — подтверждено: тело `refresh` (`:552-561`)
   зовёт `segments_counted(dir, &mut ops)?` (`:554`); `validate_like_full_path` в его теле
   ОТСУТСТВУЕТ.

Обе прежние ложные формулировки («`refresh` зовёт `validate_like_full_path`») из
`docs/fa/journal.md` и `docs/SESSION-HANDOFF.md` удалены; §0bis теперь называет те же три
ребра, что и FA, слово в слово по механизму. Внутреннее противоречие ветки со спекой M-62
задача 15 снято: обе теперь говорят «инкрементальная ветка».

Воспроизведение (все команды — на дереве слияния, оно ≡ ветке):
```
$ grep -n "validate_like_full_path\|segments_counted" crates/journal/src/segments.rs | grep -vE '^\s*[0-9]+:\s*//'
199:        let segments = segments_counted(dir, &mut ops)?;       # open
426:        self.validate_like_full_path(dir, &mut ops)?;          # is_fresh, единственный вызов
527:    fn validate_like_full_path(...)                            # определение
554:        let segments = segments_counted(dir, &mut ops)?;       # refresh
1260:pub(crate) fn segments_counted(...)                           # определение; хвост :1287 — check_first_seq_monotonic
```

## Н-2 — ЗАКРЫТ. Критерий + таблица; моя сверка перечня сошлась; `recover` — 0/0 подтверждено

Мой греп (`grep -n "pub fn"` по обоим файлам, БЕЗ якоря — см. `N-1`) дал полный список
публичных функций. Все top-level пути, сшивающие каталог, в таблице есть: `read_all`,
`recover` (`lib.rs:440/:464`), `segments()`, `stream`, `stream_from`, `stream_from_at`,
`stream_from_at_with_catalog`, `replay_digest` (`segments.rs:1252/:1829/:1846/:1947/:1984/:2072`).
Делегации таблицы сверены по телам: `stream`→`stream_from` (`:1830`); `stream_from`→
`segments()` (`let all = segments(dir.as_ref())?`); `stream_from_at`→
`stream_from_at_with_catalog`; `replay_digest`→`stream_from`; `read_all`→
`check_monotonic_paths` (`lib.rs:447`). Оракульные метки существуют: `MN-1`
(`red_stitch_monotonic.rs:117`, read_all), `MN-2` (`:151`, зовёт `journal::stream` напрямую —
`:160` и далее), `SM-11`/`SM-11c` (`red_catalog_equivalence.rs:692/:958`).

**`recover` — заявление точно:** в теле (`lib.rs:464-473`) вызовов
`check_monotonic_paths`/`check_first_seq_monotonic` — **0** (только `iter_segments_sorted` +
`read_segment_events`); вхождений `recover` в `crates/journal/tests/red_stitch_monotonic.rs` —
**0** (`grep -c` = 0). Строка таблицы «ОТСУТСТВУЕТ/ОТСУТСТВУЕТ» и оговорка «держится на всех
путях, КРОМЕ `recover`» — правда. Заведение TD-карточки корректно оставлено reviewer'у
(`TECH-DEBT.md` веткой не тронут — `git diff --name-only` подтверждает).

## Номера строк из таблиц убраны — подтверждено, находимость не пострадала

Обе таблицы (`JR-I-11` пути и таблица рёбер) номеров строк не содержат; локатор — имена +
греп-команды. Хвосты в прозе — `N-2` ниже.

---

## Новые NOTE (ни один не делает документ ложным сегодня)

### N-1 (NOTE) — команда сверки таблицы слепа к 4 методам, включая 3 строки САМОЙ таблицы

`docs/fa/journal.md:138-139` обещает: «таблица … сверяется грепом
`grep -n "^pub fn" crates/journal/src/{lib,segments}.rs`». Якорь `^` отсекает методы с
отступом. Проверено исполнением:
```
$ grep -n "^pub fn" crates/journal/src/{lib,segments}.rs | grep -c ""          → 22
$ grep -n "^pub fn" ... | grep -c "fn open\|is_fresh\|refresh"                 → 0
$ grep -nE "^\s+pub fn (open|is_fresh|refresh|segments)\(" crates/journal/src/segments.rs
197/241/552/564 — все четыре метода SegmentCatalog в выводе команды ОТСУТСТВУЮТ
```
То есть строку таблицы `SegmentCatalog::{open,refresh,segments}` (`:150`) названной командой
вывести НЕЛЬЗЯ, а будущий публичный метод внутри `impl` (ровно класс «перечень отстал от
кода», против которого критерий и введён) командой не ловится. Вдобавок `is_fresh` — пуб.
функция и носитель ребра 2 — в строке `SegmentCatalog::{…}` не назван (в таблице рёбер
строкой ниже — назван, поэтому не блокер). Фикс — одна строка: греп без якоря либо
`grep -nE "^\s*pub fn"`, и `is_fresh` в перечень строки `:150`. Править при следующем
касании `docs/fa/journal.md` (любая правка — снова §9), отдельный круг ради этого не нужен.

### N-2 (NOTE) — зачистка номеров строк порвала историческую фразу

`docs/fa/journal.md:192-193`: «Прежняя редакция ссылалась на `segments.rs` и `:771`; обе
цифры уехали» — после удаления `:1354` из ИСТОРИЧЕСКОЙ цитаты «обе цифры» указывает на одну.
Там же остался живой номер строки «`stream_from_at` живёт на `:1947`» — сегодня верный
(проверено: `1947:pub fn stream_from_at(`), но противоречащий собственному принципу абзаца.
Исправление — вернуть `:1354` в цитату (история не протухает) и снять `:1947` (протухнет).
Тот же режим правки, что N-1.

### N-3 (NOTE) — резидуальный носитель класса Н-3, опровергнутый самой веткой

`milestones/M-61-artifact-ids.md:679-686` в настоящем времени: «задача 8 (⏳ OPEN) буквально
называет acceptance `research/reports/R-001*` … Это не гипотеза о будущем — это исполнение
уже написанной спеки». После `c9eb7ce` ЭТОЙ ЖЕ ветки M-04 номер больше не называет
(`grep "R-001\*" milestones/M-04-research-core.md` — только в тексте амендмента «Прежняя
редакция предписывала»). Абзац — историческая оговорка close-out уже смерженного M-61, к
действию не предписывает (требуемое им действие как раз исполнено), поэтому NOTE: дописать
одну строку-постскриптум «снято 2026-08-14, `TD-139` п.(в), M-04 амендмент 2» при следующем
касании. Три носителя, названные `R-070` Н-3 (`M-16:50`, `BACKLOG` ×3, `M-19:85`), — сняты,
проверено грепом; `M-19:45` «R-001-стиля» оставлен законно.

---

## Н-4 круга 1 (нормативные правки без критика) — оценка РИСКА для founder'а

Решение — founder'а; моя задача — риск. **Обе правки приводят текст к уже
смеханизированному факту и норму по существу НЕ меняют; waiver годится.**

- **`d630cc3` (M-61 §6, строка F2).** Требование строки НЕ изменилось: `--battery` → exit 0,
  эталон зелёный, каждая дырявая реализация красная, kill-set = объявленному. Заменена
  ложная константа («25») на разложение `26+1+1=28`. Перепроверено мной: счёт мутантов
  командой из правки → **26**; прогон `bash scripts/tests/red_artifact_ids.sh --battery` →
  **`BATTERY: PASS (28/28)`**, exit=0. Сама батарея и барьер этим коммитом не тронуты
  (`git show d630cc3 --stat` — один файл, M-61).
- **`c9eb7ce` (M-04, задача 8).** Прежний текст предписывал артефакт `research/reports/R-001*`,
  который существующий барьер `check_artifact_ids.sh` красит ЗАКОННО (`gates.md` §12,
  идентификатор занят `research/reviews/R-001-M-49.md`; воспроизведено исполнением в круге 1 —
  `R-070` Done Block, exit=1/exit=0 в обе стороны). Правка меняет ФОРМУ получения номера на
  единственную, совместимую с уже действующей нормой §12 (прошедшей свой критик-круг в M-61);
  существо задачи 8 (прогон OBI, пре-регистрация, risk-critic, founder ★) не тронуто.
  Барьер этим коммитом не изменён; `e6d8425` (единственный коммит ветки, трогающий скрипты) —
  чисто комментарийный: диф вне `#`-строк — **0** (перепроверено).

Остаточный риск waiver'а: прецедент «нормативную секцию можно править без критика, если
правка декларируется выравниванием к факту» — судья декларации в этом случае перепроверщик
§9, а он критика по норме НЕ заменяет. Риск принимается или не принимается founder'ом; для
ДАННЫХ двух правок содержание проверено исполнением и расхождений с фактом нет.

## Н-5, Н-6 — ЗАКРЫТЫ

- **Н-5:** §0bis теперь явно: «закрыта ПОЛОВИНА… пункт (б) НЕ тронут». Факт перепроверен:
  `grep -n TD_PAT scripts/next_artifact_id.sh` → `:33` (комментарий) + `:48` (определение),
  использований 0 — мёртв; карточка `TD-137` в `TECH-DEBT.md` открыта, файл веткой не тронут.
- **Н-6:** формулировка «деградирует» снята; §0 называет три точки (1:57 / 2:34 / 1:38) и
  «тренда НЕТ», пункт очереди сохранён по валидному основанию (таймаут шага `N`).

## (а) Гейты дерева слияния

База = `origin/main` = merge-base = `c6c62b8`; дерево слияния ≡ HEAD ветки.
`verify_design_claims.sh` → `VERDICT: PASS (0 нарушений)`, exit=0;
`--merge-preview origin/main` → `VERDICT: PASS (0 нарушений)`, exit=0. Расхождения нет.

## (б) Полномочия

- **Замок §11:** `EVENT_NAME=push PUSH_BEFORE=$(git rev-parse origin/main)
  bash scripts/check_docs_freeze.sh` → exit=0 — зона замка не тронута.
- **Push-scope:** `git log origin/main..HEAD` — ровно 8 коммитов, один автор (`t`):
  7 × `[architect]` + 1 × `[fable-recheck]` (`69cd54e` — вердикт-артефакт круга 1,
  закоммичен на ветку по `branch-hygiene.md` п.4, законен). Чужих нет.
- **Зона:** тронуты `docs/fa/journal.md`, `docs/SESSION-HANDOFF.md`, `milestones/{BACKLOG,
  M-04,M-16,M-19,M-61,M-62}.md`, `scripts/{check_artifact_ids,next_artifact_id}.sh`
  (карточка `TD-137`: «Зона правки — architect (`scripts/**`)»), `research/reviews/R-070*` —
  всё в заявленной зоне; `TECH-DEBT.md`/`PROJECT-STATE.md` (reviewer-owned) не тронуты.
- **Граница C:** не пересечена — ни промоушенов, ни весов/лимитов, ни фаз, ни состава
  записываемых данных; founder ★ в задаче 8 M-04 сохранён.
- **Барьер идентификаторов на диапазоне ветки:** `OK: ни один коммит диапазона
  c6c62b8..HEAD не ввёл второй носитель…`, exit=0.

## (в) Связность

`R-069` (`research/reviews/R-069-td135-probe-hermeticity.md`), `R-070`, `A-004`
(`research/arbitration/A-004-td083-measure.md`) — существуют; `TD-135`…`TD-140` — по одной
заводящей карточке (`grep -c` = 1 ×6); `MN-1`/`MN-2`/`SM-11`/`SM-11c` — в оракулах по
названным файлам; `gates.md` §12 (ссылка амендмента M-04) — существует; ссылки `R-070` из
новых текстов разрешаются на самой ветке. Висячих не найдено.

## Done Block (сырой вывод; зелёное агрегировано, красное отсутствует)

```
$ git merge-base origin/main HEAD | cut -c1-7; git rev-parse --short origin/main
c6c62b8
c6c62b8

$ git log origin/main..HEAD --format='%an' | sort -u; git log origin/main..HEAD --oneline | wc -l
t
8

$ sed -n '199p;426p;554p' crates/journal/src/segments.rs
        let segments = segments_counted(dir, &mut ops)?;
        self.validate_like_full_path(dir, &mut ops)?;
        let segments = segments_counted(dir, &mut ops)?;
$ sed -n '546p' crates/journal/src/segments.rs            # хвост validate_like_full_path
        check_first_seq_monotonic(&candidates, |name| {
$ sed -n '1287p' crates/journal/src/segments.rs           # хвост segments_counted
    check_first_seq_monotonic(&candidates, |name| {

$ grep -c 'recover' crates/journal/tests/red_stitch_monotonic.rs
0
$ sed -n '464,473p' crates/journal/src/lib.rs | grep -c "check_monotonic_paths\|check_first_seq_monotonic"
0

$ grep -n "^pub fn" crates/journal/src/{lib,segments}.rs | grep -c ""   # N-1: команда из FA
22
$ grep -n "^pub fn" crates/journal/src/{lib,segments}.rs | grep -c "fn open\|is_fresh\|refresh"
0

$ bash scripts/verify_design_claims.sh 2>&1 | tail -1; echo exit=$?
VERDICT: PASS (0 нарушений)
exit=0
$ bash scripts/verify_design_claims.sh --merge-preview origin/main 2>&1 | tail -1; echo exit=$?
VERDICT: PASS (0 нарушений)
exit=0

$ EVENT_NAME=push PUSH_BEFORE=$(git rev-parse origin/main) bash scripts/check_docs_freeze.sh; echo exit=$?
exit=0
$ EVENT_NAME=push PUSH_BEFORE=$(git rev-parse origin/main) bash scripts/check_artifact_ids.sh; echo exit=$?
OK: ни один коммит диапазона c6c62b8..HEAD не ввёл второй носитель под занятым идентификатором
exit=0

$ grep -oE "emit(_h)? [a-z0-9]+-check\.sh" scripts/tests/mk_ref_artifact_ids.sh | grep -cv ref-check
26
$ bash scripts/tests/red_artifact_ids.sh --battery 2>&1 | tail -1; echo exit=$?
BATTERY: PASS (28/28)
exit=0

$ git show e6d8425 --format='' | grep -E '^[+-]' | grep -vE '^[+-]{3}' | grep -vE '^[+-]\s*#' | wc -l
0

$ grep -rn "R-001" milestones/M-16-historical-import.md milestones/M-19-frontend-cockpit.md | grep -v "стиля"
{пусто}
$ grep -c "R-001" milestones/BACKLOG.md
0
```

## Условие APPROVED — выполнено этим кругом; остаток

1. `Б-1` — закрыт, оба файла (см. выше). 2. `Н-2` — закрыт критерием + таблицей, `recover`
названа оговоркой, TD — reviewer'у. 3. `Н-3`/`Н-5`/`Н-6` — закрыты; `Н-4` — оценка риска
дана, решение founder'а. Остаток (`N-1`…`N-3`) — правки одной-двух строк при следующем
касании соответствующих файлов, своим кругом §9; вход ветки в `main` они НЕ блокируют.
