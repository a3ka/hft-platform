<!-- GATE-META
milestone: M-60b
audited_repo: a3ka/hft-platform
audited_base: 92f6e65a90382506eb3fcc4ab09d4744ffeaade4
audited_head: ee89de088e78aaf9bf7ccd594458849372ce791a
verdict: PASS
-->

# C-135 — адверсарий круга 7, PR #65: оба блокера `C-134` сняты по существу, GM-40/GM-41 честны в обе стороны и не вакуумны, матрица «6 компонентов × 2 потребителя» закрыта вся — 17 мутантов и 4 сломанных guard'а красны каждый на своём сценарии, выживших нет. Предмет ГОТОВ к merge как починка сломанного барьера

**Предмет:** PR #65, ветка `harness/gate-meta-base-drift`, вершина `ee89de0` — исполнение
условия снятия `C-134` (два сценария `GM-40`/`GM-41`, барьер не тронут: `git show ee89de0
--numstat` → один файл, `scripts/tests/red_gate_meta.sh`, 61/1).
**Роль:** адверсарий харнесс-трека, круг 7 (последний санкционированный), СВЕЖИЙ контекст,
Fable (названа в мандате). Не автор правки и не автор вердиктов кругов 1–6.
**Маршрут:** `docs/workflow/harness-track.md` §3/§5. Прогоны — в worktree `/tmp/hft-adv-c135`
(detached на `ee89de0`; ветка занята чужим worktree — `branch-hygiene.md` п.2, запасной
путь). Мутанты — только в ОТДЕЛЬНЫХ копиях (`/tmp/adv-c135/mut/`), предмет read-only.

## Вердикт: PASS

**Ответ на итоговый вопрос мандата — прямо: предмет ГОТОВ к merge'у как починка сломанного
барьера.** Конкретного ложного срабатывания на прод-форме, предъявленного исполнением, НЕ
ОСТАЛОСЬ: я построил 17 мутантов барьера (два новых компонента, вся шестёрка поимённо, вся
подводка `own_rev_args`→потребители, батарея прежних блоков) и 4 сломанных setup-guard'а —
каждый красен, и каждый на СВОЁМ сценарии; оба направления обоих блокеров `C-134`
предъявлены прод-формой на ручных фикстурах, построенных мной независимо от пробы.

## 1. Оба блокера `C-134` сняты по существу — исполнением, включая прод-форму

**Б-1 (`^ah` в `own_bodies`, ложное ЗЕЛЁНОЕ).** Мой mutAHB (`check_gate_meta.sh:237` —
`own_rev_args ""` вместо `own_rev_args "${1:-}"`, та же форма, что у круга 5):

```
BARRIER=mutAHB.sh bash scripts/tests/red_gate_meta.sh → FAIL GM-40; VERDICT: FAIL (1); exit=1
```

Ровно GM-40, соседи молчат. Прод-форма — РУЧНАЯ фикстура (не из пробы): B0 → круг-1 правка
гейта с законным `ALLOW-SUBJECT-CHANGE` → вердикт APPROVE (`ah` = круг-1) → правка гейта
ПОСЛЕ APPROVE без токена:

```
честный ee89de0: FAIL subject-lock ... тронут класс «гейт»; VERDICT: FAIL (1); exit=1  ← корректный красный
mutAHB:          NOTE ... открыт явным ALLOW-SUBJECT-CHANGE; VERDICT: PASS; exit=0     ← ложное зелёное
```

**Б-2 (`BASE..` в `own_rev_args`, ложный КРАСНЫЙ).** Мой mutBASE (`check_gate_meta.sh:219` —
`printf '%s\n' "${TOUCH_TIP}"`, нижняя граница потеряна):

```
BARRIER=mutBASE.sh bash scripts/tests/red_gate_meta.sh → FAIL GM-41; VERDICT: FAIL (1); exit=1
```

Ровно GM-41. Прод-форма — ручная фикстура: чужой влитый PR правит `scripts/check_m99.sh` в
main → merge ветки с вердиктом (`ah`=B0) → push, `PUSH_BEFORE` = main до merge:

```
честный ee89de0: VERDICT: PASS — вердиктов проверено: 1; exit=0                        ← корректно, и НЕ вакуумно
mutBASE:         FAIL subject-lock ... тронут класс «гейт»: scripts/check_m99.sh; exit=1 ← ложный красный на чужой истории
```

Строка «вердиктов проверено: 1» снята мною с полного stdout честного барьера на этой
фикстуре: GM-41 не судит пустоту — вердикт входит в суд, PASS выносится по существу.

## 2. GM-40/GM-41 честны — оба направления, все guard'ы кричат

- **Направления:** зелены против честного `ee89de0` (в составе 48/48 базового прогона);
  красны каждый против своего мутанта (§1).
- **Setup-guard'ы — ломал КАЖДЫЙ по отдельности в копии пробы, все четыре кричат, тихой
  зелени нет** (прогоны против честного барьера):
  - g40a (токен убран из ДО-вердиктного коммита, `red_gate_meta.sh:942`) →
    `FAIL GM-40 SETUP НЕ СОСТОЯЛСЯ`; VERDICT: FAIL (1); exit=1;
  - g40b (токен добавлен в ПОСЛЕ-вердиктный коммит, `:945`) → тот же крик; exit=1;
  - g41a (чужая правка ниже `PUSH_BEFORE` уведена с `scripts/check_m99.sh` на
    `docs/DESIGN.md`, `:970`) → `FAIL GM-41 SETUP НЕ СОСТОЯЛСЯ`; exit=1;
  - g41b (правка гейта добавлена ВНУТРЬ push-диапазона, после `:973`) → тот же крик; exit=1.
- **GM-41 — не дубль ни одного push-сценария:** mutBASE красен ТОЛЬКО на GM-41 (FAILs=1 из
  48) — ни один прежний сценарий нижнюю границу не держит, что согласуется с выживанием
  этого же мутанта 46/46 у круга 5. Утверждение автора «первым разводит BASE и ah» по
  существу верно и предъявлено исполнением: несущий элемент фикстуры — правка гейта в окне
  `ah..PUSH_BEFORE` (у всех прежних push-сценариев это окно пусто, `ah` = база фикстуры).

## 3. «Все шесть несущие» — проверено ПОИМЁННО, мои мутанты, мои прогоны

| компонент | мутант (моя копия, строка барьера) | красный сценарий | заявка коммита |
|---|---|---|---|
| `BASE..` | mutBASE (`:219`) | **GM-41** (1) | GM-41 ✓ |
| `TOUCH_TIP` | mutTIP (`:189` HEAD^2→HEAD) | **GM-37** (1) | GM-37 ✓ |
| `EXCL_MAIN` | mutEXCL (`:182` → `()`) | **GM-38+GM-39** (2) | GM-38,GM-39 ✓ |
| `^ah` в `own_touched` | mutAHT (`:385` → `""`) → **GM-37**; mutAHX (`:385` → `"^HEAD"`) → **GM-10, GM-16c** (+9) | обе стороны | GM-10/GM-16c ✓ (см. N-1) |
| `^ah` в `own_bodies` | mutAHB (`:237`) | **GM-40** (1) | GM-40 ✓ |
| фильтр вердиктов | mutFILTER (`:303` → `:`) | **GM-16h** (1) | GM-16h ✓ |

Дополнительно вся ПОДВОДКА, через которую компоненты доезжают до потребителей: mut232
(`own_touched` теряет `$1`, `:232`) → GM-37; mutEXP (цикл `EXCL_MAIN` выброшен из
`own_rev_args`, `:222`) → GM-38+GM-39. Выживших мутантов НЕТ — ни одного PASS 48/48 среди
всех семнадцати.

## 4. Прежние 46 не сломаны — мутационный контроль по блокам

Базовый прогон: `VERDICT: PASS (48/48)`, exit=0; мой счёт `grep -c '^PASS'` → 48. Батарея
прежних блоков (формы круга 5), каждый прогон мой:

| мутант | круг 5 (на 539085f) | мой прогон на ee89de0 |
|---|---|---|
| mutB2 (токен из `ah..HEAD`) | GM-16e+GM-39 (2) | GM-16e+GM-39 (2) ✓ |
| mutPV (лок на все вердикты) | GM-11+GM-16h (2) | GM-11+GM-16h (2) ✓ |
| mutABS (absence-петля) | GM-17+GM-21 (2) | GM-17+GM-21 (2) ✓ |
| mutCC (`cc`→`--no-merges`) | GM-16f (1) | GM-16f (1) ✓ |
| mutGC (`is_gate_class`→нет) | 10 | **11** = те же 10 + GM-40 ✓ согласованный прирост |
| mutANC (предок-проверка) | GM-6 (1) | GM-6 (1) ✓ |
| mutORIG (сверка origin) | GM-3 (1) | GM-3 (1) ✓ |
| mutAHT | GM-37 (1) | GM-37 (1) ✓ |
| mutBODIES (весь `BASE..HEAD` у тел) | GM-39 (1) | **GM-39+GM-40** (2) ✓ GM-40 добавился согласованно |

## 5. Прод-форма — свои прогоны целиком

- `bash scripts/tests/red_gate_meta.sh` → `VERDICT: PASS (48/48)`, exit=0.
- `bash scripts/verify_M-60b.sh` ЦЕЛИКОМ → PASS-строк **60** (мой `grep -c '^PASS'`),
  FAIL **0**, `VERDICT: PASS`, exit=0; строка verify: «red_gate_meta: зелёная, счёт сошёлся
  (48/48 по факту файла)».
- declared: `grep -cE '^[[:space:]]*(if )?run_barrier '` → **48** = executed 48.
- Семь блокирующих барьеров CI на дереве `ee89de0` (`EVENT_NAME=pull_request
  PR_BASE_SHA=92f6e65…`): protected_artifacts · docs_freeze · artifact_ids ·
  context_budgets · gate_meta · resource_oracles · review_fa — **все exit=0**.
- Фикстуры убраны: `ls -d /tmp/red-gatemeta-* | wc -l` → **0** после всех прогонов.

## 6. Числа тела коммита — каждое своим прогоном

48/48 ✓ · mutAHB → FAIL GM-40, FAIL (1) ✓ · mutBASE → FAIL GM-41, FAIL (1) ✓ ·
verify PASS=60 FAIL=0 exit=0 ✓ · declared/executed 48/48 ✓ · семь барьеров exit=0 ✓ ·
numstat 61/1, один файл ✓ (`git show ee89de0 --numstat`). Ложных само-измерений класса
кругов 1–5 в этом теле НЕ НАЙДЕНО.

## N — не блокирует

- **N-1.** Атрибуция клетки «`^ah` в `own_touched` → GM-10/GM-16c» в таблице тела коммита
  верна для направления ПЕРЕ-исключения (mutAHX `"^HEAD"` → false green, красны GM-10,
  GM-16c и ещё 9); направление СНЯТИЯ исключения (mutAHT → false red) роняет GM-37 — как и
  замерил круг 5. Клетка запиннена в обе стороны; таблица не ложна, но называет лишь одну
  из них. Точность — на будущее, предмету ничего не должно.

## Done Block

```
$ git worktree add /tmp/hft-adv-c135 --detach origin/harness/gate-meta-base-drift && git log -1 --format='%H'
ee89de088e78aaf9bf7ccd594458849372ce791a                                   exit=0
$ git merge-base HEAD origin/main → 92f6e65a90382506eb3fcc4ab09d4744ffeaade4
$ git show ee89de0 --numstat --format='' → 61  1  scripts/tests/red_gate_meta.sh (один файл)

$ bash scripts/tests/red_gate_meta.sh → VERDICT: PASS (48/48); exit=0; grep -c '^PASS' → 48
$ BARRIER=mutAHB.sh  → FAIL GM-40 (1); exit=1        # C-134 Б-1 снят, ровно свой сценарий
$ BARRIER=mutBASE.sh → FAIL GM-41 (1); exit=1        # C-134 Б-2 снят, ровно свой сценарий
$ BARRIER=mutTIP.sh → GM-37 (1) · mutEXCL → GM-38+39 (2) · mutAHT → GM-37 (1)
$ BARRIER=mutAHX.sh → 11 FAIL, в т.ч. GM-10+GM-16c   # обе стороны клетки ^ah×own_touched
$ BARRIER=mutFILTER.sh → GM-16h (1) · mut232 → GM-37 · mutEXP → GM-38+39   # подводка вся
$ батарея: mutB2→16e+39 mutPV→11+16h mutABS→17+21 mutCC→16f mutGC→11(=10+GM-40)
           mutANC→6 mutORIG→3 mutBODIES→39+40        # все красны, атрибуции сходятся с C-134
$ g40a/g40b/g41a/g41b (сломанные setup'ы, честный барьер) → каждый:
  FAIL GM-4x SETUP НЕ СОСТОЯЛСЯ; VERDICT: FAIL (1); exit=1                 # guard'ы кричат
$ прод-форма Б-1 (ручная фикстура): честный FAIL(1) exit=1 | mutAHB NOTE+PASS exit=0
$ прод-форма Б-2 (ручная фикстура): честный PASS «вердиктов проверено: 1» exit=0
                                    | mutBASE FAIL(1) exit=1
$ bash scripts/verify_M-60b.sh → PASS=60 FAIL=0; VERDICT: PASS; exit=0     # целиком, фон завершён
$ 7 барьеров CI (PR_BASE_SHA=92f6e65) → все exit=0
$ git ls-remote origin refs/reserved/C-135 → 27fd6b90…                     # номер зарезервирован
$ ls -d /tmp/red-gatemeta-* 2>/dev/null | wc -l → 0                        # фикстуры убраны
```
