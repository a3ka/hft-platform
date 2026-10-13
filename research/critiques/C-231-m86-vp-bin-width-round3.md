<!-- GATE-META
milestone: M-86
audited_repo: a3ka/hft-platform
audited_base: 0f55ea7577386c08c22b4cedd428b354308db52e
audited_head: ba3d6ed6c931ef01fbe927d1608abdab88654e73
verdict: REJECT
-->

# C-231 — M-86 fixed VP bin width, круг 3 — REJECT

## Предмет и полнота набора

Проверен committed artifact set `0f55ea7..ba3d6ed` на
`feat/M-86-vp-bin-width`, а не одна milestone-спека: сама M-86, пять RED-файлов,
`verify_M-86.sh`, FA, `docs/ROADMAP.md` и оба предыдущих verdict-файла. T1/`contracts/**`
не меняются; contract-RFC и ревизия workspace-`match` не требуются. Новых trait-signature
нет; M-86 объявляет T2 API (`DEFAULT_VP_BIN_WIDTH_E8`, getter, setter) для будущей
engine-dev реализации.

Живые инварианты предмета: **VP-I-4** (`docs/fa/viz-backend.md:295`) — корзина есть
диапазон и пустая корзина не изобретается; **VB-I-10** — whole-session eviction bounded
window; **VB-I-2** — `live == replay`.

## Закрытие C-230

### B1 — CLOSED

`VP-I-4` теперь определяет диапазон `[k·W, (k+1)·W)`, ключ через `div_euclid`, нулевой
якорь, отсутствие пустых корзин, сохранение объёма и принадлежность POC/VAH/VAL отданным
корзинам. Это не ослабляет `VB-I-8`: он сохранён дословно; изменена единица ряда с точной
торгованной цены на диапазон, а диапазон без сделки так же не появляется
(`docs/fa/viz-backend.md:297-302`). Шаг G зелёный на audited head без dev-правки.

### B2 — RED-оракул CLOSED; его mutation control — нет

V1 действительно несёт 500 сделок прошлой UTC-сессии, получает cursor её последней
сделки и setup-стражом доказывает одну видимую строку до продолжения журнала
(`red_vp_bin_width_size.rs:160-178`). На LATEST он требует ровно текущую строку
(`:191-203`). В изолированной минимальной реализации baseline был GREEN; удаление
`self.vp.bins.remove(&sid)` воспроизводимо дало ожидаемое падение V1 с двумя строками
профиля (см. Done Block). Сам RED-тест поэтому пиннит `VB-I-10`.

### NOTE — CLOSED

§7.2bis исправлен на 2 043. Независимый пересчёт по ценам с нулевым якорем даёт
20 417 корзин для W и 2 043 для незаконного `10·W`; названная причина (крайняя корзина
при `LOW_E8`, не кратном `10·W`) соответствует геометрии фикстуры.

## Вердикт: REJECT

### B3 — H/H2 выдают PASS по любому сбою, поэтому мутацию фактически не доказывают

`H2` считает любой non-zero от `cargo test` доказательством эвикционного мутанта
(`scripts/verify_M-86.sh:155-163`). На самом audited RED-head setup мутации проходит:
целевая строка существует и удаляется. Но V1 затем не компилируется из-за отсутствующего
`DEFAULT_VP_BIN_WIDTH_E8`, поэтому V1 до ассерта `«в кадре 2 строк профиля»` не доходит.
Несмотря на это, H2 печатает `PASS: ... уронила V1`. Это ложное положительное mutation
control, запрещённое `testing.md` «Целостность гейта», свойство 3: non-zero от
несостоявшегося/постороннего setup не является доказательством инварианта.

Шаг H имеет тот же дефект формы (`:124-134`): он не требует baseline GREEN и принимает
любой non-zero от каждого полного бинаря. Кроме того, H обещает, что V6 остаётся зелёным,
но никогда не запускает именованный `v6_grid_aligned_input_passes_through_unchanged`:
агрегированный non-zero не говорит, упал ли V2/V1 по мутации или V6 по иной причине.

Это и есть ещё один экземпляр причины, корректно названной в §0ter: ожидаемый RED не
может служить фоном, на котором неполный или несостоявшийся architect-gate выглядит
успешной проверкой. §0ter верно объясняет, почему B1 нельзя было отложить; но набор всё
ещё имеет этот же класс дефекта в H/H2. Других architect-артефактов B1-класса не найдено:
E предъявляет именованный состав, G отдельно предъявляет FA, а проверка startup-набора
считает десять тестов. Проблема именно в положительных заявлениях mutation controls.

**Условие снятия.** До принятия mutation-result скрипт обязан доказать, что НЕмутированный
именованный тест GREEN; после внесения мутации — что он стал RED именно на требуемом
диагнозе, а не на compile/setup-ошибке. Для H2 это диагностическая строка V1 о двух
строках профиля и прошлой UTC-сессии. Для H нужны отдельные результаты: V1/V2 RED на
мутанте и `v6_grid_aligned_input_passes_through_unchanged` GREEN. Пока baseline ещё
plan-time RED, H/H2 должны честно завершаться FAIL «mutation result is not yet judgeable»,
а не PASS. После исправления предъявить raw output baseline + mutant; implementation не
писать до решения следующего гейта.

## Done Block

```text
$ git rev-parse HEAD; git merge-base HEAD origin/main
ba3d6ed6c931ef01fbe927d1608abdab88654e73
0f55ea7577386c08c22b4cedd428b354308db52e
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [1-ЕСТЬ] все 3 маркеров [ЕСТЬ] в таблицах статусов сопровождены существующим пруфом
PASS  [2-ПОКРЫТИЕ] §22: VB-I — заявлено=11, в оракулах=8 — подтверждено замером
PASS  [7-RFC-PATH] путей-кандидатов ... все проверенные существуют в дереве репозитория
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/verify_M-86.sh | grep -E '^(===|PASS:|FAIL:|VERDICT:)'; echo exit=${PIPESTATUS[0]}
PASS: docs/fa/viz-backend.md вообще называет VP-I-4 ...
PASS: VP-I-4 в FA описывает корзину профиля как ДИАПАЗОН, а не как точку
PASS: мутация (эвикция отключена) уронила V1, как и обязана (VB-I-10)
VERDICT: FAIL (10)
exit=1
# H2 PASS above is invalid: its test failed to compile on the missing T2 API, not on V1.

$ cargo test -p gateway --test red_vp_bin_width_size --quiet  # isolated minimal M-86 probe
test result: ok. 1 passed; 0 failed; finished in 31.53s
baseline_exit=0

$ cargo test -p gateway --test red_vp_bin_width_size --quiet  # same probe; self.vp.bins.remove(&sid) deleted
V1 НАРУШЕН (`VB-I-10`): в кадре 2 строк профиля вместо одной — прошлая UTC-сессия
(session_id=20276) не эвиктнута оконным правилом `session_max_time_s < lo_time_s` ...
test result: FAILED. 0 passed; 1 failed; finished in 32.05s
mutant_exit=101

$ awk '<prices, zero-anchored canonical W versus 10·W>'
canonical_W_bins=20417
illegal_10W_bins=2043
exit=0

$ git diff --check 0f55ea7..ba3d6ed
exit=0

$ bash scripts/next_artifact_id.sh C
C-231
exit=0
```

=== HANDOFF: CRITIC → ARBITER ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-09-19T18:18Z
- Milestone: M-86-vp-bin-width
- Статус: BLOCKED — REJECT, круг 3
- HEAD: ba3d6ed — test(M-86): V1 предъявляет эвикцию прошлой сессии; мутант без неё краснеет [architect]

## §B — Что я сделал
- Аудировал committed M-86 artifact set и C-229/C-230 на merge-preview, включая T1/T2 границу, FA, RED-набор и real-gate verify.
- Подтвердил B1, B2 RED и NOTE; самостоятельно воспроизвёл B2-мутант против минимальной реализации.
- Нашёл B3: H/H2 принимают посторонний non-zero за доказанную мутацию; H2 уже печатает ложный PASS на этом RED-head.

## §C — Артефакты / результаты
- `research/critiques/C-231-m86-vp-bin-width-round3.md`
- Done Block: merge-preview exit=0; plan-time verify exit=1 (ожидаемый RED), но H2 false-PASS; isolated baseline exit=0, eviction-mutant exit=101 с требуемым диагнозом.

## §D — Следующий агент + инвокация
- **Следующий агент:** `arbiter` (сильная модель, свежий контекст)
- **Paste-ready промпт:**
  ```
  Ты независимый арбитр M-86, свежий контекст. Прочти M-86, C-229, C-230, C-231 и указанные ревизии 0f55ea7..ba3d6ed. Реши B3 измерением: достаточно ли H/H2 считать любой non-zero mutation success, когда на plan-time head V1 ещё compile-RED; обязаны ли они сначала установить baseline GREEN, требовать V1-диагноз о 2 строках и отдельно предъявить V6 GREEN. Вынеси обязательное решение в research/arbitration/A-NNN-<topic>.md, закоммить и запушь на feat/M-86-vp-bin-width. Не проектируй implementation и не отправляй предмет в круг 4.
  ```
- Push-статус: verdict подлежит push на `origin/feat/M-86-vp-bin-width` этим кругом.
- Кэш: после push critic удалит `target/` обоих созданных изолированных worktree.

## §E — Риски / открытые вопросы
- Это третий круг по одному предмету. По `gates.md` §0 п.2 следующий маршрут — АРБИТР; четвёртый critic-round запрещён до решения арбитра.
- Граница C не затронута: вопрос только о достоверности RED/mutation gate, а не о выборе продуктовой формы.

=== END HANDOFF ===
