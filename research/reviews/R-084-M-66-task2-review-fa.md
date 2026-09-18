> **Врезка: перенумерация и перенос на ветку merge'а (architect, 2026-08-15).**
> Этот вердикт был вынесен reviewer'ом 2026-08-14 под идентификатором `R-079` и лёг на ветку
> `docs/M-66-protocol-attestation` (`1f3c6e2`). С ним случились две вещи, обе поймал следующий
> круг гейта (`R-081` Б-2/Б-3):
>
> 1. **Коллизия идентификатора.** Под `R-079` в `origin/main` уже лежит другой файл —
>    `research/reviews/R-079-td141-fa-sync-merge.md`. Два разных носителя под одним
>    идентификатором запрещены (`gates.md` §12), и барьер `check_artifact_ids.sh` краснеет.
>    По действующему промежуточному правилу founder'а (14.08) номер удерживает тот, чей
>    носитель УЖЕ в `origin/main`, а пришедший позже берёт следующий свободный. Поэтому здесь
>    `R-084`, выданный механизмом (`scripts/next_artifact_id.sh R`, exit=0). Прецедент —
>    `R-038` → `R-082` на M-59.
> 2. **Вердикт лежал вне ветки, идущей в merge.** Работа круга 2 ушла на `feat/M-66`, а
>    `1f3c6e2` её предком не является (проверено `git merge-base --is-ancestor`, exit=1). По
>    `gates.md` §4 merge без файла вердикта, называющего milestone, — нарушение независимо от
>    очевидности дифа. Milestone, который сам механизирует это правило, обязан ему следовать
>    в первую очередь.
>
> Текст ниже — ДОСЛОВНЫЙ, суждение reviewer'а не редактировалось; изменён только
> идентификатор в заголовке и добавлена эта врезка.

---

# R-084 — M-66 protocol-attestation, задача 2 (`scripts/check_review_fa.sh`)

**Вердикт: REJECT (CHANGES REQUESTED).**
**Дата:** 2026-08-14 · **Роль:** reviewer (PR-гейт, `gates.md` §4 UNCONDITIONAL)
**Предмет:** `origin/docs/M-66-protocol-attestation`, HEAD `b6fc6af`
**Предшествующие гейты:** `C-082` REJECT → `C-085` REJECT → `C-087` APPROVE (plan-time) · tester PASS (34/35)
**Merge в `main` НЕ выполнен.** `PROJECT-STATE.md` / `TECH-DEBT.md` НЕ обновлены — close-out §7.6 не наступил.

Номер выдан механизмом: `scripts/next_artifact_id.sh R` → `R-079` (exit=0), `gates.md` §12.

---

## 1. Что прогнано МНОЙ (не перенесено из handoff'а)

Все числа ниже сняты в собственном worktree `/tmp/hft-rev-m66` (detached `b6fc6af`) и в
герметичной пробе `/tmp/m66probe`. Вердикт tester'а прочитан, но **не засчитан как факт**:
`gates.md` §8 — «отчёт агента — гипотеза, состояние git — факт».

Совпало с tester'ом: RED-проба 34/35 (единственный FAIL — `W8OK`), батарея 12/12,
H1 `d564617` exit=1, H2 `710b1ad` exit=0, scope чист, `fmt`/`clippy` exit=0.

**Не совпало:** я прогнал барьер в **прод-форме** — против РЕАЛЬНОЙ `docs/fa/journal.md`
(13 живых `JR-I-*`), а не против синтетической фикстуры пробы (1 живой ID). Проба этого
не делает, и именно там лежат обе находки ниже.

## 2. Block-scope — PASS

```
$ git diff --name-status $(git merge-base origin/main HEAD) HEAD
A	milestones/M-66-protocol-attestation.md
A	research/critiques/C-082-M-66-protocol-attestation.md
A	research/critiques/C-085-M-66-rev2.md
A	research/critiques/C-087-M-66-rev3.md
A	scripts/check_review_fa.sh
A	scripts/tests/red_review_fa.sh
A	scripts/verify_M-66.sh
```

Запретный список §9 не тронут ни одним путём (`.claude/**`, `CLAUDE.md`,
`docs/04-workflow.md`, `docs/fa/**`, `research/reviews/**`, `crates/**`, `contracts/**`,
`TECH-DEBT.md`, `PROJECT-STATE.md` — грепом по диапазону: пусто). Замок §11 не задет —
подтверждено независимо guard'ом `G` самого verify (`PASS  G locked process files не тронуты`).

Коммит задачи 2 атомарен и адресен: `b6fc6af` → `git show --numstat` = ровно
`267 0 scripts/check_review_fa.sh`. Заявленное в subject'е соответствует дифу
(`branch-hygiene.md` п.9, симметрия «`--porcelain` до / `--numstat` после»). Co-author и
`Generated with` трейлеров в диапазоне нет.

## 3. Block-C (контракты) — N/A, проверено фактом

`crates/contracts/**` в дифе отсутствует; contract-RFC не требуется (спека §6 подтверждена
замером, а не цитатой).

## 4. Block-risk — N/A, проверено фактом

`crates/risk/**`, `crates/killswitch/**`, `crates/oms/**`, `crates/venue-*/**` в дифе
отсутствуют. `gates.md` §5 RISK-BLOCK не срабатывает; risk-critic в цепочке не требуется.

## 5. Находки

### Б-1 (BLOCKER) — барьер зелен на МЁРТВОМ ID: `grep -qF` матчит подстроку, не ID

`scripts/check_review_fa.sh:251` — предикат эха:

```bash
if [ -f "${f}" ] && grep -qF "${id}" "${f}" 2>/dev/null; then
```

`-F` — фиксированная строка **без границы слова**. Живой `JR-I-9` является подстрокой
мёртвого `JR-I-999`; живой `JR-I-1` — подстрокой мёртвого `JR-I-14`. Барьер засчитывает эхо,
которого в вердикте нет.

**Воспроизведение (герметичное, прод-форма вызова).** Репозиторий с РЕАЛЬНОЙ
`docs/fa/journal.md` (живые `JR-I-1..13`), диф трогает `crates/journal/src/lib.rs`,
вердикт несёт РОВНО один ID:

```
$ grep -o 'JR-I-[0-9]*' research/reviews/R-998-offbyone.md
JR-I-14
$ EVENT_NAME=push PUSH_BEFORE="$BASE" bash scripts/check_review_fa.sh; echo "exit=$?"
research/reviews/R-998-offbyone.md: JR-I-1
exit=0
```

Ожидание по §4 kill-set (`B4DEAD`) и по инварианту §2 — **FAIL**. Получено — **PASS**.
Второй прогон, с `JR-I-999`, даёт то же (`exit=0`, печать `JR-I-9`).

**Почему это блокер, а не примечание.** Свойство, ради которого milestone существует
(«вердикт несёт ЖИВОЙ инвариант-ID тронутого модуля»), в прод-форме не обеспечено.
Живые `JR-I-1..13` покрывают префиксами практически всё пространство `JR-I-<цифры>`:
мимо проходит и экзотический `JR-I-999`, и самый вероятный реальный случай — соседний
номер `JR-I-14`, который пишут по памяти или который УДАЛИЛИ из FA. Это ровно тот
`TD-138`-класс (FA разошлась с кодом), от которого barrier должен был защищать.

**Почему это не поймали три круга критика, tester и батарея.** Синтетическая FA пробы —
`scripts/tests/red_review_fa.sh:148`:

```bash
printf '# journal FA\n\nJR-I-1\nDET-I-1\n' > docs/fa/journal.md
```

U фикстуры = `{JR-I-1}`, и `JR-I-1` **не** является подстрокой `JR-I-999` — сценарий
`B4DEAD` проходит по случайности состава фикстуры, а не потому, что барьер различает
живое и мёртвое. Прод-U = `{JR-I-1..13}` суффикс-свободным не является. Это именной класс
проекта: `testing.md` §«Форма прода снимается ЗАМЕРОМ, а не воображается» и урок 1
`SESSION-HANDOFF` §0bis («гейт, проверенный не тем вызовом, каким его зовёт прод, — не
проверен»).

**Следствие для батареи (анти-плацебо не сработал).** Мутант `synonly` («проверяет
синтаксис, а не живость») объявлен пойманным сценарием `B4DEAD` — 12/12 зелёные. Но
ЭТАЛОННЫЙ барьер против прод-данных ведёт себя как `synonly`. Батарея на оси 4 —
плацебо самой себя: она отличает мутанта от эталона только на фикстуре, где разницы
между ними по этой оси нет.

### Б-2 (BLOCKER) — молчаливый вакуумный PASS при непустом FA-крейте

`scripts/check_review_fa.sh:245-257`:

```bash
ECHO_OK=1
if [ "${#U[@]}" -eq 0 ]; then
  ECHO_OK=0            # ← вакуум: предикат считается выполненным
```

Спека §3.1 шаг 6 отводит `U = ∅` РОВНО одному случаю — «тронуты ТОЛЬКО NO-FA крейты», и
требует печати `B вакуумен; пробел предъявлен: <список waived>`. В коде `U = ∅`
достигается и вторым путём: FA-файл из таблицы **существует**, но не несёт ни одного ID
СВОЕГО префикса. Тогда `NOFA_CRA` пуст, шаг 5 ничего не требует, шаг 6 вакуумен, и барьер
отдаёт `exit=0` **не напечатав ни одной строки** — ни `WAIVED`, ни диагностики.

**Воспроизведение.** `docs/fa/ops.md` существует, живых `OPS-I-*` — ноль; диф трогает
`crates/ops/src/lib.rs`; вердикт без единого ID:

```
$ EVENT_NAME=push PUSH_BEFORE="$BASE" bash scripts/check_review_fa.sh; echo "exit=$?"
exit=0
```

Вывод пуст. Инвариант §2 требует FAIL.

**Почему блокер, хотя сегодня не срабатывает.** Замер всех 17 строк таблицы §3.2 на HEAD
ветки: каждый FA-файл несёт живые ID своего префикса, минимум — **`gateway-serve` → `GS`,
ровно 1** (`GS-I-4`). То есть одна правка `docs/fa/viz-backend.md`, снимающая или
переименовывающая единственный `GS-I-*`, тихо гасит весь `gateway-serve`-путь барьера, и
наблюдать это нечем: путь молчит. Это прямое нарушение принципа, который сам барьер
декларирует строкой выше (§3.1 шаг 3: «отсутствие файла ≠ пустое множество — fail-closed») —
соседний случай «файл есть, множество пусто» оставлен fail-open. `testing.md` §«Целостность
гейта» св-во 4: гейт обязан наблюдать ОТСУТСТВИЕ, а не только сбой.

### N-1 (NOTE, известное) — задача 3 не сделана: механизм построен и не подключён

`verify_M-66.sh` T3 красный, сценарий `W8OK` красный:

```
FAIL  T3 review-fa job / status-check wiring не готов — задача 3 OPEN
      ↳ job review-fa отсутствует
      ↳ status-check.needs не содержит review-fa
      ↳ status-check не проверяет needs.review-fa.result
```

Это не дефект задачи 2 и tester'ом отмечено честно. Фиксирую как состояние гейта: пока
джоба нет, барьер — код, который прод не исполняет (`gates.md` §4, DoD «Механизм на пути»:
merge допустим только с подключением, доказанным оракулом точки входа, либо с TD-записью
`built-not-wired` MAJOR). Поскольку merge не производится, TD-запись не заводится —
предмет остаётся на ветке.

### N-2 (NOTE) — печать барьера утверждает неправду

Оба воспроизведения Б-1 печатают строку вида `research/reviews/R-998-offbyone.md: JR-I-1`
для файла, в котором `JR-I-1` отсутствует. Строка вывода — это аудит-след, попадающий в
лог CI; сегодня он способен зафиксировать эхо, которого не было. Чинится вместе с Б-1, но
называю отдельно: дефект предиката и ложность его протокола — разные свойства.

### N-3 (NOTE, известный FAIL — НЕ дефект M-66) — `cargo test --all` красный по `TD-151`

CI-паритет verify упал: `cargo test --all` exit=101, единственный упавший тест —
`disk_guard_halts_writes_explicitly_when_free_space_is_low`
(`crates/journal/tests/red_retention.rs:63`). Перечисляю явно, а не прячу за общим `tail`
(`commit-discipline.md`: известные FAIL называются с обоснованием).

Это **`TD-151`**, уже заведённый открытый долг: «`disk_guard` берёт порог `free_bytes+1` и
проигрывает гонку с хостом; развязка обязана убрать хост из уравнения». Оракул меряет
ОКРУЖЕНИЕ, а не свой инвариант (`testing.md` §«Целостность гейта» св-во 2). Триггер сегодня
предъявлен: `df -h /` → **90 %** занято.

Атрибуция снята замером, а не рассуждением: диапазон M-66 трогает `crates/` РОВНО в нуле
файлов (`git diff --name-only … | grep -c '^crates/'` → `0`). Ветвь красноту не вносила и
починить её не может — зона `crates/**/tests/` sacred и в §9 milestone'а запрещена.
На вердикт по задаче 2 не влияет; блокером merge является и без него `VERDICT: FAIL`
самого verify по T2/T3.

### N-4 (NOTE, направление правки — за architect'ом)

`gates.md` §4, граница reviewer↔architect: я описываю дефект, но не проектирую фикс.
Отмечу лишь требование `testing.md`: «исправление по вердикту тоже требует оракула» —
правка Б-1/Б-2 обязана прийти с RED, который краснеет на САМОЙ находке, и фикстура этого
RED обязана нести прод-форму U (несколько ID, суффикс-непустой набор), иначе следующий
круг поймает тот же класс в новой одежде.

## 6. Условие APPROVED

1. Б-1 закрыт: предикат эха различает `JR-I-1` и `JR-I-14`/`JR-I-999`; RED на находку
   краснеет против текущего `b6fc6af`.
2. Б-2 закрыт: `U = ∅` при непустом `LIVE_CRA` — FAIL с диагностикой, а не молчаливый ноль.
3. Фикстура FA в пробе приведена к прод-форме (набор живых ID, а не один), и мутант
   `synonly` продолжает ловиться уже НЕ по случайности состава.
4. Задача 3 (джоб `review-fa` + обе рукописные строки агрегата `status-check`) — по
   §3.3/§8.5; `W8OK` зелёный.
5. `bash scripts/verify_M-66.sh` → `VERDICT: PASS`, exit=0 (кроме задач 5/7, вынесенных
   спекой в отдельные заходы — их FAIL остаётся легитимным и должен быть назван).
   **Оговорка:** пункт 5 недостижим, пока открыт `TD-151` (N-3) — CI-паритет verify гоняет
   `cargo test --all`, а тот красен по чужому host-зависимому оракулу. Это НЕ повод
   ослаблять verify обходом: развязка — закрытие `TD-151` в своей зоне (architect), либо
   явное, названное в вердикте следующего круга исключение. Молча зеленить нельзя.

## 7. Done Block (сырой вывод, мой прогон)

```
$ git -C /tmp/hft-rev-m66 log -1 --oneline
b6fc6af feat(M-66): task #2 — scripts/check_review_fa.sh по §3.1–3.2 [engine-dev]

$ bash scripts/tests/red_review_fa.sh 2>&1 | tail -6
PASS  МАНИФЕСТ ⇄ исполнение: 35 сценариев, состав совпал
SPEC_ROWS=35
PASS  СПЕКА⇄МАНИФЕСТ: 35 строк §4 совпали в обе стороны
VERDICT: FAIL (1 нарушений; 34/35 сценариев прошли)
PROBE_EXIT=1

$ bash scripts/tests/red_review_fa.sh --battery 2>&1 | tail -3
PASS  anywaiver → пойман сценарием W7WRONG (exit=1)
PASS  echoexcuse → пойман сценарием W7MIXGAP (exit=1)
BATTERY: PASS (12/12)
BATTERY_EXIT=0

$ bash scripts/verify_M-66.sh   # ключевые строки
PASS  T2 scripts/check_review_fa.sh существует и парсится
FAIL  T2 RED-проба против реального барьера (exit=1)
FAIL  T3 review-fa job / status-check wiring не готов — задача 3 OPEN
PASS  T4 scripts/verify_M-66.sh существует и парсится
PASS  H1 M-62 обязан краснеть: journal+gateway без JR/VB echo (ожидаемый FAIL, exit=1)
PASS  H2 M-57 обязан проходить: R-040 несёт JR-I-1/2/11 (exit=0)
PASS  T4 мутационная батарея red_review_fa.sh --battery (exit=0)
PASS  T5 база профилей подтверждена: 9/9 имеют Startup reading
FAIL  T5 founder-approved строка предъявления отсутствует (0/9) — задача 5 ждёт founder
FAIL  T6 TECH-DEBT.md не изменён в диапазоне M-66 — close-out задача 6 OPEN
FAIL  T7 docs/fa/derive.md и/или docs/fa/recorder.md отсутствуют — follow-up задача 7 OPEN
PASS  G locked process files не тронуты текущим диапазоном
PASS  G docs/fa/** не тронуты в core-задачах 1-4
PASS  cargo fmt --all -- --check (exit=0)
PASS  cargo clippy --all-targets --all-features -- -D warnings (exit=0)
FAIL  cargo test --all (exit=101)
      ↳ thread 'disk_guard_halts_writes_explicitly_when_free_space_is_low' panicked at
        crates/journal/tests/red_retention.rs:63:18
      ↳ test result: FAILED. 2 passed; 1 failed; 0 ignored
VERDICT: FAIL
VERIFY_EXIT=1

$ # прод-форма Б-1: живые JR-I-1..13, вердикт несёт только JR-I-14
$ EVENT_NAME=push PUSH_BEFORE=$BASE bash scripts/check_review_fa.sh; echo exit=$?
research/reviews/R-998-offbyone.md: JR-I-1
exit=0

$ # прод-форма Б-1bis: вердикт несёт только JR-I-999
$ EVENT_NAME=push PUSH_BEFORE=$BASE bash scripts/check_review_fa.sh; echo exit=$?
research/reviews/R-999-fake.md: JR-I-9
exit=0

$ # Б-2: docs/fa/ops.md существует, живых OPS-I-* ноль, вердикт без эха
$ EVENT_NAME=push PUSH_BEFORE=$BASE bash scripts/check_review_fa.sh; echo exit=$?
exit=0
```

## 8. Что я НЕ проверял — названо явно

- **Прод (`gates.md` §8)** — деплой-гейт не применялся: merge не производился, в `main`
  ничего не уехало.
- **Задачи 5 и 7** — вынесены спекой в отдельные заходы (founder-подпись / follow-up
  architect'а); их красное в verify легитимно и в вердикт как дефект не идёт.
- **Полнота таблицы §3.2 против будущих крейтов** — проверена только на сегодняшнем
  составе `crates/` (файлов непосредственно под `crates/` нет, ложного красного по этому
  пути не возникает).
</content>
</invoke>
