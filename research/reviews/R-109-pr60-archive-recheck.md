<!-- GATE-META
milestone: PR-60
audited_repo: a3ka/hft-platform
audited_base: 40c7cce30ff10eb787caaafccb4b809794c503ee
audited_head: d788abf8570c51cb324b1261eeffffc9aaa26059
verdict: REJECT
-->

# R-109 — перепроверка §9 PR #60: переезд не состоялся — 52 копии без единого удаления; барьер IDs краснит и это дерево, и настоящий переезд

**Роль:** независимый Fable-клон architect'а со свежим контекстом (`gates.md` §9).
**Предмет:** ветка `harness/archive-id-universe`, вершина `d788abf` (снята сам:
`git rev-parse HEAD` → `d788abf8570c51cb324b1261eeffffc9aaa26059`), база `origin/main` =
`40c7cce` = merge-base ⇒ дерево ветки = дереву слияния. Два коммита: `1b3a12f`
(предусловие 1 нормы Р-2) и `d788abf` («вынос 26 гейтов»).

**Вердикт: REJECT — три блокера, каждый воспроизводим командой.** Предмет опасный
(сокращение активного корпуса), и падает он не на потере — потерь как раз нет, — а на
том, что заявленное сокращение НЕ ПРОИЗОШЛО, и на том, что механизм, построенный ему в
предусловие, не пропускает его ни в каком исполнении.

---

## Б-1 (блокер). `d788abf` — копирование, а не переезд: 0 удалений, 0 переименований

Заявление коммита: «гейтов в scripts/: 45 → 19; спек в milestones/: 54 → 28». Факт:

```
$ git show d788abf --name-status --format='' | awk '{print $1}' | sort | uniq -c
     52 A
     10 M
$ git diff -M --name-status origin/main..HEAD | grep -c '^R100'
0
$ ls scripts/verify_M-*.sh | wc -l && ls milestones/M-*.md | wc -l
45
54
$ wc -l scripts/verify_M-*.sh | tail -1
  6183 total
```

Ни одного `D`, ни одного `R`. Все 26 гейтов и 26 спек живы по СТАРЫМ путям; в
`docs/archive/` легли их копии. «45 → 19», «6 183 → 2 983», «54 → 28» — ложь о дереве;
формулировка тела «файлы остаются в дереве по новому пути» описывает переезд, которого в
коммите нет. Активный корпус не сократился ни на строку — единственная цель работы не
достигнута, при этом переписанные комментарии тестов («гейт сдан в архив… больше не
исполняется», `crates/gateway/tests/red_hint_pos_guard.rs`) на ЭТОМ дереве стали ложью:
`scripts/verify_M-62.sh` существует.

**Гипотеза механизма (гипотеза, не факт):** `.githooks/pre-commit` отвергает `git commit`
без явных путей (проверено на себе в scratch-прогоне ниже — отказ с перечнем индекса);
`git commit -- <только новые пути>` сужает коммит до названного, и 52 удаления остались
в индексе незакоммиченными, а затем были сброшены. Симптоматика совпадает, транскриптом
автора не проверялась.

## Б-2 (блокер). CI ветки красный: `artifact-ids` FAIL — дубль M-4

Локально, CI-формой (та же проводка, что джоб: `EVENT_NAME`/`PR_BASE_SHA`):

```
$ EVENT_NAME=pull_request PR_BASE_SHA=40c7cce3… bash scripts/check_artifact_ids.sh; echo exit=$?
FAIL  M-4: второй носитель «docs/archive/M-04-research-core.md» под идентификатором,
      занятым «milestones/M-04-research-core.md»
exit=1
```

GitHub подтверждает (сырые строки `gh pr checks 60`):

```
All checks passed                                  fail  4s
Artifact IDs (механический аллокатор, TD-111)      fail  25s
```

Это прямое следствие Б-1 (копия = второй носитель под занятым номером) — барьер,
поправленный `1b3a12f`, честно ловит дубль, который создал `d788abf`. PR физически не
вливается.

## Б-3 (блокер конструкции). И НАСТОЯЩИЙ переезд барьер не пропустит — проверено исполнением

Универсум занятости в `check_artifact_ids.sh` строится по ВСЕМ refs
(`refs_all()` — `refs/remotes/origin` ∪ `refs/heads`, `scripts/check_artifact_ids.sh:146`),
а «введённое» — по диапазону с `--no-renames` (`:178`, осознанно: переезд = D+A). До
merge старый носитель ЖИВ в дереве `origin/main` ⇒ любая ветка переезда даёт «второй
носитель» независимо от того, удалены ли старые пути на ветке. Scratch-эксперимент
(временный коммит поверх `d788abf` с 52 настоящими удалениями; сброшен сразу после):

```
$ git rm -q scripts/verify_M-{04,…,62}.sh milestones/M-…md   # 52 пути
$ git commit --no-verify -m 'tmp scratch'
$ EVENT_NAME=pull_request PR_BASE_SHA=40c7cce3… bash scripts/check_artifact_ids.sh; echo exit=$?
FAIL  M-4: второй носитель «docs/archive/M-04-research-core.md» под идентификатором,
      занятым «milestones/M-04-research-core.md»
exit=1
$ git reset --hard d788abf && git status --porcelain | wc -l
0
```

То есть предусловие `1b3a12f` в текущей конструкции делает исполнение самой нормы Р-2
невозможным через CI: пути «зелёного переезда» в барьере не существует, и проба
(`red_artifact_ids.sh` — 48/48 PASS на ветке) его отсутствия не видит, потому что
сценария «переезд одного предмета проходит» в ней нет. Чинить это — конструкция барьера
(зона architect, харнесс-трек); я дефект называю, фикс не проектирую (мандат).

---

## Что ПОДТВЕРЖДЕНО (не переделывать в следующем круге)

1. **Потерь нет.** Все 52 файла в `docs/archive/` побайтово равны источникам
   (`cmp` по каждой паре: `pairs-differing=0`).
2. **Состав 26 верен.** `ls docs/archive/verify_M-*.sh` (минус umbrella) = ровно
   {04 07 17 18 20 22 23 24 29 30 32 33 34 37 38a 40 41 47 50 51 53 58 59 60a 61 62} =
   30 из `R-098` §3.2 − {35, 45, 48, 49}. Спеки = гейты (diff множеств пуст; ловушка
   «M-17 без .sh» не сработала).
3. **Отложенные — ровно те.** Ссылки в `crates/*/src` + `deploy/` несут ТОЛЬКО
   verify_M-35 (×2), -45 (×2), -48 (×6), -49 (×1) из 30 кандидатов (плюс M-09/28/38b/52
   из списка ДЕРЖАТЬ); у 26 вынесенных таких ссылок нет.
4. **Токены.** `ALLOW-ARTIFACT-DELETE` ×1, `ARCHIVED-VERDICT` ×26; пути токенов =
   ровно 26 вынесенных гейтов (diff списков пуст). В ЭТОМ диапазоне оба класса токенов
   инертны: удалений нет, `check_gate_meta.sh` судил 0 файлов («вердиктов проверено: 0»)
   — `docs/archive/verify_*` его предметом не является.
5. **Остальные барьеры на ветке зелёные** (CI-формой, exit-коды сняты отдельно от
   `tail`): protected-artifacts 0 · gate-meta 0 · context-budgets 0 ·
   `verify_design_claims.sh --merge-preview origin/main` 0 (1 NOTE H-FACTS, не о
   предмете) · проба `red_artifact_ids.sh` 48/48.
6. **check5 НЕ ослаблен.** Милестоун, отсутствующий и в `milestones/`, и в
   `docs/archive/`, по-прежнему даёт FAIL (`verify_design_claims.sh:995-998`: `mnum not
   in milestone_status` → fail); архивный со STATUS из OPEN-множества тоже ловится.
   Фаза не может цитировать несуществующий милестоун.
7. **Регекс аллокатора.** Подстановкой: `M-33-depth-band-3060` → 33 (жадность не
   вернулась), `M-38a` → 38 (буква не отделяется), `M-60-umbrella` → 60, чужие классы
   не захвачены. Ветка: `next_artifact_id.sh M` → `M-70`; main-версия → `M-70` (замер
   ниже) — регресса выдачи нет. Замечание N-3 ниже.
8. **Полномочия.** Тронутые пути — `scripts/check_*/next_*/verify_design_claims.sh`
   (харнесс, правки СУЩЕСТВУЮЩИХ барьеров — `П-017` A2 соблюдён), `crates/*/tests/**`
   (sacred architect), `docs/rfc/**`, `docs/archive/**` (docs). Замок §11 не задет ни
   одним путём диапазона. Прогон `check_docs_freeze` в CI — pass.

## Не-блокирующие находки

- **N-1.** `docs/rfc/CT-RFC-04-l2delta.md:133` — «`verify_M-18.sh` гейтит замыкание
  списка…» — настоящее время о выносимом гейте НЕ переписано (в файле правлена только
  строка 6). Класс `R-098` §4.3: после настоящего переезда инвариант остаётся с
  named-сторожем, которого нет. В следующем круге — та же правка, что сделана шести
  файлам тестов.
- **N-2.** Переписанные комментарии тестов и RFC истинны только ПОСЛЕ настоящего
  переезда; на текущем дереве ложны (следствие Б-1, отдельной правки не требует).
- **N-3.** Новый регекс/glob класса M матчит и вложенные пути
  (`docs/archive/M-10-obi-killscreen-retired-2026-07/README.md` → «10», проверено
  подстановкой): `.*`/`*` кроют `/`. Для max+1 и уникальности это консервативно
  (ретированный номер считается занятым — верно по духу §12), вреда не нашёл; называю,
  чтобы следующий круг не открыл это как «дефект» заново.

## Done Block

```
$ git rev-parse HEAD                                         # d788abf85…059
$ git show d788abf --name-status --format='' | awk …         # 52 A / 10 M / 0 D
$ ls scripts/verify_M-*.sh | wc -l                           # 45
$ ls milestones/M-*.md | wc -l                               # 54
$ EVENT_NAME=pull_request PR_BASE_SHA=40c7cce… check_artifact_ids   # exit=1 (FAIL M-4)
$ gh pr checks 60 | head -2                                  # All checks passed: fail; Artifact IDs: fail
$ scratch: 52 удаления + тот же барьер                       # exit=1 (FAIL M-4) — Б-3
$ git reset --hard d788abf; git status --porcelain | wc -l   # 0 (scratch снят)
$ EVENT_NAME=pull_request PR_BASE_SHA=40c7cce… check_protected_artifacts  # exit=0
$ EVENT_NAME=pull_request PR_BASE_SHA=40c7cce… check_gate_meta            # exit=0 (0 вердиктов)
$ bash scripts/check_context_budgets.sh                      # exit=0 (VERDICT: PASS)
$ bash scripts/verify_design_claims.sh --merge-preview origin/main   # exit=0 (PASS, 1 NOTE)
$ bash scripts/tests/red_artifact_ids.sh                     # VERDICT: PASS (48/48)
$ bash scripts/next_artifact_id.sh M                         # M-70 (ветка)
$ bash <main:next_artifact_id.sh> M                          # M-70 (до)
$ cmp по 52 парам архив↔источник                             # pairs-differing=0
```

FA-предъявление (M-66): диф трогает `crates/gateway/tests/**` и `crates/journal/tests/**`
— живой инвариант тронутого модуля: `JR-I-11` (`docs/fa/journal.md`, floor-scan bounded
budget — ровно предмет `red_floor_scan_prodscale.rs`, чей комментарий диапазон переписал).

## Условие следующего круга (описание, не дизайн)

REJECT снимается, когда: (1) старые пути 52 файлов действительно исчезают из дерева
ветки тем же диапазоном; (2) существует механический путь, которым такой диапазон
проходит `artifact-ids` (сегодня его нет — Б-3), с пробой на этот сценарий; (3)
`CT-RFC-04:133` переписан тем же классом правки, что шесть тестов. Пункты 1 и 3 — прямое
исполнение уже написанной нормы; пункт 2 — харнесс-трек architect'а.
