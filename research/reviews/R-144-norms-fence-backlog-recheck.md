<!-- GATE-META
milestone: M-67
audited_repo: a3ka/hft-platform
audited_base: c4cfb8564fb5549060762c7056485065557afee0
audited_head: e4566618cf20298a77d5343bfd005b8e7be316a3
verdict: APPROVE
-->

# R-144 — перепроверка §9: фенс GATE-META + reading-map GW-I (078c18d) · BACKLOG M-67/M-64/заморозка (e456661)

**Роль:** независимый architect-клон (Fable, свежий контекст), перепроверка по `gates.md` §9.
**Номер выдан механизмом:** `bash scripts/reserve_artifact_id.sh R` →
`reserve: попытка 1/8 — R-144 ← 68a2693d8ee9e71f23149feba66658b66c07ab1f` / `R-144` /
`резерв R-144 взят`.

**Один вердикт-файл на два предмета, копия на каждой ветке несёт `audited_head` СВОЕГО
предмета** — это вынужденно механизмом, а не вольностью: `check_gate_meta.sh:371` требует
`audited_head` предком HEAD ветки, а предметы живут на разных ветках. Обе вершины полностью:
- предмет 1: `078c18d2f006a51e2fbbd4e74f6a0a3ae170066f` (`docs/norms-gatemeta-fence-readingmap`) — **REJECT** (одна находка Б-1, правка тривиальна);
- предмет 2: `e4566618cf20298a77d5343bfd005b8e7be316a3` (`docs/backlog-m67-m64-frozen`) — **APPROVE**.

---

## §1 — Предмет 1: `078c18d` (gates.md фенс + reading-map GW-I 12→13)

### Проверено командами (все — merge-preview / дерево слияния, база `c4cfb85`)

```
$ bash scripts/verify_design_claims.sh --merge-preview origin/main   # на детаче 078c18d
VERDICT: PASS (0 нарушений)
exit=0

$ grep -rhoE '\bGW-I-[0-9]+\b' crates/ | sort -uV | wc -l
13        # GW-I-1..12, GW-I-14 — уникальных ID тринадцать, автор заявляет 13 — ВЕРНО

$ grep -n 'GW-I' docs/DESIGN.md | grep -i 'заявлено\|оракул'
924:| GW-I | gateway-serve | 0 | 13 | обратный дрейф: оракулы есть, докс-семейство не заведено |
          # §22 согласован: тоже 13

$ grep -n 'I-4' research/arbitration/A-010-nine-disputes-2026-08-18.md | head -1
735: | I-4 | architect | Починить шаблон GATE-META в gates.md: сегодня он HTML-комментарий
     и вырезается при впрыске ... Показать его в ```-фенсе. Зона §11 ⇒ токен founder'а. |
          # фенс — ДОСЛОВНО предписанная A-010 §I-4 форма; строка 760 даёт токен на §I-4

$ bash scripts/check_context_budgets.sh      # на дереве предмета
OK    .claude/rules/gates.md   47864 B / 47900 B (запас 36 B)
VERDICT: PASS — 7 файлов, 111955 B из 114900 B бюджета (запас 2945 B); exit=0
```

Побочных потребителей фенс не ломает: `GATE-META` из `gates.md` не парсит ни один скрипт —
`check_gate_meta.sh` читает шапки вердикт-файлов `research/{critiques,reviews,arbitration}`,
`gates.md` упоминает только в справочном сообщении (`check_gate_meta.sh:443`).

### Полномочия (§11) — мутация барьера в ИЗОЛИРОВАННОМ клоне (`git clone`, своя `.git`)

```
$ git log -1 --format='%B' 078c18d | grep FOUNDER-APPROVED
FOUNDER-APPROVED: founder дал прямое указание 2026-08-28 «7. делай» — довести   # ≥12 симв.

# клон /tmp/hft-freeze-clone, checkout subj1, база = merge-base c4cfb85:
$ EVENT_NAME=pull_request PR_BASE_SHA=c4cfb85... bash scripts/check_docs_freeze.sh
с токеном exit=0
# git commit --amend, строка FOUNDER-APPROVED снята (grep -c → 0):
без токена exit=1        # барьер токен ЧИТАЕТ, не декорация
```

### Находка Б-1 — БЛОКЕР: неэкранированные пайпы ломают GFM-рендер добавленной команды

`docs/workflow/reading-map.md:82` (редакция ветки): в ячейку таблицы добавлен код-спан
`` `grep -rhoE '\bGW-I-[0-9]+\b' crates/ | sort -uV | wc -l` `` с ДВУМЯ неэкранированными
`|`. По спецификации GFM пайп внутри код-спана В ТАБЛИЦЕ остаётся разделителем ячеек
(экранируется только `\|`). Строка рассыпается: команда в отрендеренном виде обрывается на
`crates/`, хвост `sort -uV | wc -l` уходит в лишние ячейки и ОТБРАСЫВАЕТСЯ рендером
(лишние ячейки сверх ширины шапки GFM игнорирует). Читатель, копирующий замер со страницы,
получает НЕПОЛНУЮ команду.

Это не орфография, а тот же класс, который коммит сам чинит: заявленное основание правки (а)
— «Markdown его не отображает, и роль, читающая норму, шаблона НЕ ВИДИТ». Вводить тем же
коммитом строку, которую Markdown отображает ЛОЖНО, — внутреннее противоречие круга. Автор
правило знает: в соседнем предмете `e456661` пайпы в таблице экранированы
(`ls research/arbitration/ \| grep -i 'abc\|branch'` — `milestones/BACKLOG.md`, строка
таблицы «Заморожено»).

**Воспроизведение:** `git show origin/docs/norms-gatemeta-fence-readingmap:docs/workflow/reading-map.md | sed -n '82p'`
— два голых `|` внутри код-спана ячейки.
**Условие снятия:** экранировать оба пайпа (`\|`) в `reading-map.md:82`. Больше условий нет —
все остальные проверки предмета 1 зелёные.

**N-1 (не блокер, к сведению):** множество ID — `GW-I-1..12, GW-I-14`, номер 13 пропущен.
«13 оракулов» как СЧЁТ верно; читатель, ищущий `GW-I-13`, его не найдёт. Фиксить нечего,
но при заведении докс-семейства `GW-I` дыру в нумерации назвать.

**Вердикт предмета 1: REJECT** (единственная находка Б-1, исправление — два символа).

---

## §2 — Предмет 2: `e456661` (BACKLOG: M-67 в «Заморожено», M-64 → К1.1, таблица к факту)

### (а) Каждое утверждение — командой

```
$ bash scripts/verify_design_claims.sh --merge-preview origin/main   # на детаче e456661
VERDICT: PASS (0 нарушений); exit=0

$ for b in feat/harness-verdict-gate docs/plan-abc-2026-08-20 docs/M-67-rev2 \
           feat/M-64-export-contract feat/harness-milestone-shape feat/harness-doc-integrity; do
    printf '%-34s ' "$b"; git ls-remote --heads origin "$b" | grep -q . && echo ЖИВА || echo НЕТ; done
feat/harness-verdict-gate          НЕТ     # таблица говорит «ветки в origin НЕТ» — ВЕРНО
docs/plan-abc-2026-08-20           НЕТ     # ВЕРНО
docs/M-67-rev2                     ЖИВА    # ВЕРНО
feat/M-64-export-contract          ЖИВА    # ВЕРНО («НЕ УДАЛЯТЬ» осмысленно)
feat/harness-milestone-shape       ЖИВА    # ВЕРНО (круг 2 существует)
feat/harness-doc-integrity         ЖИВА    # ВЕРНО

$ git ls-remote origin 'refs/salvage/2026-08-28-cleanup/*'
... feat-harness-verdict-gate · docs-plan-abc-2026-08-20 · feat-M-64-export-contract ·
    docs-M-67-rev2 — все четыре названных спас-рефа СУЩЕСТВУЮТ

$ git show origin/feat/M-64-export-contract:milestones/M-64-export-contract.md | head -8
Статус: BLOCKED (rev3, 2026-08-11) — ждёт миграции форм в crates/contracts (К1/К6b ...)
Решение founder'а 2026-08-11: сперва миграция, гейт строится сразу на конечном месте.
          # блокер ДОСЛОВНО таков, как цитирует врезка ⇒ сведение M-64 в К1.1 — дедуп
          # одного предмета, а не тихое закрытие милестоуна

$ git ls-tree --name-only origin/feat/M-64-export-contract research/critiques/ | grep C-07
C-074-M-64.md · C-075-M-64-rev3.md · C-076-M-64-rev3-b1m1.md   # «три оплаченных круга» — ФАКТ

$ git ls-tree -r --name-only origin/docs/M-67-rev2 | grep -E 'M-67|red_md|C-091'
crates/gateway/tests/red_md_i6_journal_first.rs · red_md_i7_band_lock.rs ·
crates/journal/tests/red_md_i2_hot_window.rs · docs/plans/M-67-capacity-2026-08-16.md ·
milestones/M-67-market-layer.md · research/critiques/C-091-M-67-market-layer.md ·
scripts/verify_M-67.sh            # «на ветке ЛЕЖИТ РАБОТА» — ФАКТ, состав совпадает

$ git show origin/docs/M-67-rev2:research/critiques/C-091-M-67-market-layer.md | grep -m2 -E 'verdict|ARCHITECT'
verdict: REJECT / **Verdict: REJECT — NOT REVIEWED — ARCHITECT ARTIFACTS INCOMPLETE.**   # цитата точна

$ grep -H audited_head docs/archive/abandoned-gate-verdicts-2026-08-24/C-11{0,1,2}*.md
все три: audited_head: 2dcad4882ac72ac11d505e22f17521e1988ba5b9   # «ОДИН audited_head» — ФАКТ

$ ls research/arbitration/ | grep -i 'abc\|branch'; echo exit=$?
exit=1    # арбитра по плану ABC нет — «АРБИТР НЕ СОЗВАН» — ФАКТ

$ git ls-tree --name-only origin/feat/harness-doc-integrity research/critiques/ | grep C-101
C-101-harness-milestone-shape.md   # «круг 1 ... там C-101» — ФАКТ

# заявление «проба 24/24, батарея 4/4» ВОСПРОИЗВЕДЕНО мной, не принято на слово:
$ git checkout --detach origin/feat/harness-milestone-shape && bash scripts/tests/red_milestone_shape.sh
батарея ослаблений: поймано 4 из 4
PASS=24 FAIL=0 (сценариев: 24) · VERDICT: PASS · exit=0

$ sed -n '147p' docs/workflow/harness-track.md    # ссылка «§5 п.3»
3. вердикт адверсария — ФАЙЛОМ, закоммиченным на ветку;    # ссылка ВЕРНА (§5 «Гейт трека»)

$ grep -n 'не заводи' milestones/BACKLOG.md
251: «не заводи новые задачи — расчистить всё, что есть сначала»   # цитата founder'а 24.08
     уже закреплена в этом же документе — врезка ссылается на существующую запись

$ grep -oE 'Н-7\.[0-9]' <(git show origin/docs/backlog-m67-m64-frozen:milestones/BACKLOG.md) | sort -u
Н-7.1 Н-7.2 Н-7.3 Н-7.4 Н-7.5      # переносимые находки существуют в файле
```

### (б) Полномочия

- `milestones/BACKLOG.md` в §11-замок НЕ входит (`gates.md` §11: `.claude/**`, `CLAUDE.md`,
  `docs/04-workflow.md`); токен `FOUNDER-APPROVED: founder 2026-08-28 велел завести хвосты B и C
  одним кругом.` в теле есть всё равно — избыточен, не вреден.
- Граница C соблюдена ЯВНО: строка `M-67` заканчивается «**решение founder'а о месте в
  очереди**» — правка фиксирует состояние и НЕ двигает приоритеты сама; исполняемая очередь
  (`M-68` → К-ветка) не тронута.
- Кандидат-барьер профиля (не-статусная правка `milestones/M-NN-*.md` без `C-*`) не задет:
  `BACKLOG.md` под маску `M-NN-*.md` не подпадает.

### (в) Связность

Все названные в диффе носители существуют (ветки/спас-рефы/архив
`docs/archive/abandoned-gate-verdicts-2026-08-24/` — проверено выше); `R-124` Б-3 и `П-017`
упоминаются как снятые/пережитые основания — противоречий с действующим текстом не найдено.
Пайпы в таблицах экранированы корректно (`\|`).

**Вердикт предмета 2: APPROVE.** Находок нет.

---

## §3 — Известный предел: запас 36 B у `.claude/rules/gates.md`

Замер на дереве предмета 1: `47 864 / 47 900 B`, `check_context_budgets.sh` PASS. Оценка:

1. **Эти два круга НЕ блокирует** — предмет 1 проходит, предмет 2 `gates.md` не трогает.
2. **Следующую содержательную правку `gates.md` — блокирует почти наверняка:** 36 B — меньше
   одной строки текста. Тот круг обязан либо ужать текст на размер вставки, либо поднять
   бюджет в `scripts/check_context_budgets.sh` ЯВНЫМ коммитом (харнесс-трек; при этом общий
   запас ядра — 2 945 B, поднимать потолок файла можно только внутри него, иначе поднимать
   и общий — а это уже осознанное решение о размере впрыска, его молча не делают).
3. **Поднимать бюджет ЭТИМ кругом не следует:** бюджет — намеренный тормоз роста впрыска;
   запас-в-обрез сам по себе не дефект, а сигнал, что `gates.md` пора не наращивать, а
   выносить (прецедент — `reading-map.md` вне ядра). Решение о подъёме — вместе с той
   правкой, которой он понадобится, не впрок.

## §4 — Итог

| предмет | вершина | вердикт | условие |
|---|---|---|---|
| `docs/norms-gatemeta-fence-readingmap` | `078c18d` | **REJECT** | Б-1: экранировать два пайпа `\|` в `docs/workflow/reading-map.md:82`; после правки — повторный круг НЕ нужен сверх штатной перепроверки диффа-в-два-символа |
| `docs/backlog-m67-m64-frozen` | `e456661` | **APPROVE** | — |
