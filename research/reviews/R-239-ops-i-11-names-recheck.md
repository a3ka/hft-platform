<!-- GATE-META
milestone: TD-242
audited_repo: a3ka/hft-platform
audited_base: 3753efb5b30305466ca71280d28832cea060a712
audited_head: 9c133ecfb322c7af9da44e8a2a4733960926f824
verdict: APPROVE
-->

# R-239 — перепроверка §9: `OPS-I-11` ссылается на код по именам — APPROVE

**Роль:** architect-recheck (независимый Fable-агент со свежим контекстом, `gates.md` §9; автор
правки — ведущий architect, здесь сторона). **Дата:** 2026-10-04. **Предмет:** ветка
`origin/docs/ops-i-11-names`, один коммит `9c133ec` поверх `origin/main` (`3753efb`), один файл
`docs/fa/ops.md`, +1/−1 — строка `OPS-I-11` (§6, `docs/fa/ops.md:479`). Источник задания —
`R-235` §5 Н-1 (сдвиг `lib.rs:1228` → комментарий после PR #316) и Н-2 (оракула на текст
`unsupported` нет; решение — architect).

## Вердикт

**APPROVE.** Все три замены «файл:строка → имя» указывают на существующие имена; каждая названная
ветка делает то, что о ней утверждает текст (`Unsupported` → `inc_refusals_unsupported`,
нехватка слота `"overloaded"` → `inc_refusals_supported`); `verify_design_claims.sh
--merge-preview origin/main` — `VERDICT: PASS`, exit=0; номеров строк в `docs/fa/ops.md` не
осталось; полномочия — изложение (форма инварианта не менялась); висячих ссылок нет. Решение
из тела коммита (оракула на текст сообщения не будет) считаю допустимым — основание в §4.
Незаблокирующие замечания — §5.

## §0. Предмет — вершина взята командой (`04-workflow.md` §2)

```
$ git fetch origin; git rev-parse origin/docs/ops-i-11-names
9c133ecfb322c7af9da44e8a2a4733960926f824
$ git merge-base origin/main origin/docs/ops-i-11-names
3753efb5b30305466ca71280d28832cea060a712
$ git rev-parse origin/main
3753efb5b30305466ca71280d28832cea060a712
$ git log --oneline origin/main..origin/docs/ops-i-11-names
9c133ec docs(fa/ops): OPS-I-11 — ссылки на код по именам, не по номерам строк (R-235 §5 (1)) [architect]
$ git show --stat --format='' 9c133ec
 docs/fa/ops.md | 2 +-
```

База ветки = вершина `origin/main`, поэтому дерево слияния тождественно дереву ветки; все
проверки ниже сняты на отсоединённом worktree `/tmp/hft-recheck-ops12` на `9c133ec`.

## §1. Каждое утверждение О КОДЕ — командой (`gates.md` §9 (а))

**1.1. `admit()` в `admission.rs`, три проверки `bands` / `symbol` / `timeframe_ms`.**

```
$ grep -n 'pub fn admit' crates/gateway-serve/src/admission.rs
118:pub fn admit(policy: &AdmissionPolicy, sel: &Selector) -> ServingOutcome {
$ sed -n '122p;126,127p;131p' crates/gateway-serve/src/admission.rs
    if !bands_subset(&sel.bands, &policy.canonical_bands) {
    let sym_ok = policy.allowed_symbols.iter().any(|s| s == &sel.symbol);
    if !sym_ok {
    if !profile_allowed(policy, sel) {
$ sed -n '158,160p' crates/gateway-serve/src/admission.rs      # profile_allowed = ось timeframe_ms
    for p in &policy.allowed_profiles {
        if p.timeframe_ms == sel.timeframe_ms {
            return true;
```

Три оси названы верно: третья проверка идёт через `profile_allowed`, которая сравнивает ТОЛЬКО
`timeframe_ms` (комментарий `admission.rs:147-157` говорит это прямо).

**1.2. `handle_v1_message`, ветка `ServingOutcome::Unsupported` → `inc_refusals_unsupported`.**

```
$ grep -n 'fn handle_v1_message' crates/gateway-serve/src/lib.rs
1055:    async fn handle_v1_message<S>(
$ awk 'NR>1055 && /^    }$/ {print NR; exit}' crates/gateway-serve/src/lib.rs   # конец функции
1685
$ grep -n 'ServingOutcome::Unsupported\|inc_refusals_unsupported' crates/gateway-serve/src/lib.rs
1177:                        ServingOutcome::Unsupported => {
1178:                            metrics::inc_refusals_unsupported(inner.counters.as_ref());
```

Единственная ветка `Unsupported` и единственный вызов `inc_refusals_unsupported` во всём
`lib.rs` — оба внутри `handle_v1_message` (1055..1685). Отсюда же подтверждается соседнее
утверждение строки «legacy-путь … этот счётчик не двигает»: второго места вызова нет.

**1.3. `handle_v1_message`, ветка нехватки слота с кодом `"overloaded"` → `inc_refusals_supported`.**

```
$ grep -n '"overloaded"' crates/gateway-serve/src/lib.rs
1238:                                    "overloaded",
1580:                            "overloaded"
$ sed -n '1233,1239p' crates/gateway-serve/src/lib.rs
                            None => {
                                metrics::inc_refusals_supported(inner.counters.as_ref());
                                send_v1_error(
                                    sink,
                                    Some(id),
                                    "overloaded",
                                    "no serving slot available",
```

Кодов `"overloaded"` в функции ДВА (`:1238` и `:1580`). Квалификатор «нехватки слота» в новом
тексте разрешает неоднозначность в пользу `:1233-1239` (`None` от получения `SlotGuard`,
сообщение `no serving slot available`); второй (`:1573-1580`) — отказ `resume` по пределу объёма
`PL-I-5`, не слот. Ссылка по имени точна — см. §5 Н-2 о том, что при этом НЕ сказано.

**1.4. Номеров строк в FA не осталось; `verify_design_claims` на дереве слияния.**

```
$ grep -cE '\.rs:[0-9]+' docs/fa/ops.md; echo exit=$?
0
exit=1
$ bash scripts/verify_design_claims.sh --merge-preview origin/main 2>&1 | grep -E '^(FAIL|VERDICT)'; echo exit=$?
VERDICT: PASS (0 нарушений)
exit=0
```

(`grep -c` при нуле совпадений печатает `0` и выходит `1` — показано обеими строками, `R-097` N-7.)

## §2. Полномочия (`gates.md` §9 (б))

Диф не меняет ни условие тревоги, ни пределы, ни продюсера/потребителя `OPS-I-11` — только
способ адресации трёх мест в коде. Это «изложение», а не «форма»: критик по §9 не требуется,
маршрут — PR автора с этой перепроверкой. `docs/fa/ops.md` — не safety-путь (§9: risk-critic
только для `fa/risk|killswitch|oms`, `RK-I-*`/`INTG-I-*`). Зона `docs-freeze` (§11) файл не
накрывает (`check_docs_freeze.sh:63`: `.claude/rules|agents|wrappers`, `CLAUDE.md`,
`docs/04-workflow.md`) — трейлер `FOUNDER-APPROVED` не нужен. Граница C не затронута. Автор
— architect, владелец `docs/fa/**` (профиль, Writes). Полномочия в порядке.

## §3. Связность и висячие ссылки (`gates.md` §9 (в))

Имена, введённые дифом: `admit` (`admission.rs:118`), `handle_v1_message` (`lib.rs:1055`),
`ServingOutcome::Unsupported` (`lib.rs:1177`), код `"overloaded"` (`lib.rs:1238`) — все
существуют (§1). Не тронутые дифом, но соседние ссылки строки проверены на существование:

```
$ grep -n 'fn check_serving_silence\|fn check_serving_heartbeat_missing\|ServingHeartbeatStale,\|pub serving_heartbeat_warn_ms\|pub serving_heartbeat_crit_ms' crates/ops/src/watchdog.rs
53:    ServingHeartbeatStale,
218:    pub serving_heartbeat_warn_ms: i64,
219:    pub serving_heartbeat_crit_ms: i64,
575:pub fn check_serving_heartbeat_missing(hb: &ServingHeartbeat) -> Option<Alert> {
629:pub fn check_serving_silence(
$ ls crates/ops/tests/red_m89_serving_silence.rs crates/gateway-serve/tests/red_m89_counters_instance.rs crates/gateway-serve/tests/red_m91_legacy_counters.rs crates/gateway-serve/tests/red_m89_heartbeat_entrypoint.rs
(все четыре существуют)
$ gh pr view 316 --json state,mergedAt -q '.state+" "+.mergedAt'
MERGED 2026-10-04T22:11:40Z
```

Висячих ссылок нет.

## §4. Решение в теле коммита — оракула на текст `unsupported` не будет

Текст коммита: «текст — подсказка клиенту, не контракт; оси решения пиннят оракулы `admit()`.
Остаточный риск — расхождение текста с осями при добавлении новой оси; ловит ревью правки
`admit()`».

Проверка состояния, на которое решение опирается:

```
$ grep -rn 'вне политики допуска' crates/*/tests; echo exit=$?
exit=1
$ grep -n 'вне политики допуска' crates/gateway-serve/src/lib.rs
1185:                            let msg = "selector вне политики допуска (bands/symbol/timeframe_ms)"
```

Оракула на текст по-прежнему нет (`R-235` Н-2 воспроизведён). **Считаю решение допустимым**, и
основание не в «текст неважен», а в том, что оракул, который можно написать, НЕ закрывает
названный риск: тест вида «`msg` содержит `bands`, `symbol`, `timeframe_ms`» пиннит нынешнюю
строку, но при добавлении четвёртой оси в `admit()` остаётся зелёным — ось в тексте
отсутствует, а тест об этом не знает. Оракул соответствия «оси `admit()` ↔ слова в `msg`»
требует перечня осей, независимого от `admit()`; такого перечня в коде нет, а завести его —
значит породить второй источник правды об осях (`testing.md`: зависимый эталон мутация ловит
плохо). Протокольный контракт (`ServingOutcome::Unsupported`, код `"unsupported"`) пиннится
`red_m87_admission.rs` — он и есть граница, за которую клиент вправе цепляться. Остаточный
риск назван в коммите честно и совпадает с тем, что я вижу. Норма `testing.md` «исправление
по вердикту тоже требует оракула» здесь упирается в то, что оракула на САМ риск не существует
по построению; принятие риска записью — предусмотренная развязка (`binding-requires-mechanism`:
«механизируй либо признай остаточный риск записью»). Запись есть — тело коммита `9c133ec`; при
close-out `TD-242` reviewer'у стоит сослаться на этот SHA, чтобы решение не жило только в
`git log` (это не требование к строке, а подсказка close-out'у).

## §5. Замечания (не блокируют)

**Н-1 — удвоение в тексте.** Новая формулировка читается: «счётчик — в ветке
`ServingOutcome::Unsupported`, `crates/gateway-serve/src/lib.rs`, `handle_v1_message`, ветка
`ServingOutcome::Unsupported`» — ветка названа дважды в одной скобке. Смысл не страдает,
чтение — слегка. Правка по желанию автора при следующем касании строки; отдельного круга не
стоит.

**Н-2 — наблюдение ВНЕ объёма, не требование (мандат: новых требований к строке не выдвигать).**
Второй код `"overloaded"` в той же функции (`lib.rs:1573-1580`, отказ `resume` по `PL-I-5`)
НЕ двигает ни `refusals_supported`, ни `successes` (между `:1540` и `:1600` единственный вызов
`metrics::` — `add_journal_payload_bytes`). Попытка при этом уже засчитана (`:1172`). Для правила
`Δattempts > 0 ∧ Δrefusals_supported > 0 ∧ Δsuccesses == 0` такой исход невидим: голодание,
при котором КАЖДЫЙ поддержанный запрос отвергается пределом объёма, тревоги не даст, потому что
`Δrefusals_supported == 0`. Строка `OPS-I-11` этого не утверждает и не отрицает — её перечень
«не-`Ready` исходов» (`not_ready`, `warming`, нехватка слота) полон для того, что она называет;
но «Предел, названный честно» этот случай не называет. Дефект (если это дефект) — в коде/правиле,
а не в правке ссылок; он существовал до `9c133ec` и этой правкой не менялся. Передаю architect'у
как кандидата в `TD` — решать ему, не этому гейту.

**Н-3 — комментарий в коде всё ещё несёт номер строки.** `lib.rs:1179`: «`admit()`
(`crates/gateway-serve/src/admission.rs:122-131`)» — тот же класс, что чинила правка, но в
`crates/**/src`, зоне dev, не FA. Сейчас диапазон верен (проверки стоят на `:122-133`).
Вне предмета.

## §6. Что искал грепом в ярусе C

`TECH-DEBT.md`: `TD-242` (`:219` таблица, `:5552` карточка). `research/reviews/`: `R-235` §5 —
открыт на `origin/main` целиком (§5 прочитан, не пересказан). `docs/fa/ops.md`: `^## §6`
(`:465`), `OPS-I-11` (`:479`), `\.rs:[0-9]+` (0 совпадений). `scripts/check_docs_freeze.sh`:
состав зоны (`:63`). `scripts/verify_design_claims.sh`: `fa/ops.md` — 0 упоминаний (документ
проверяется общими правилами, не адресно).

## §7. Предъявление FA

Диапазон не трогает `crates/**` (один файл `docs/fa/ops.md`) — `check_review_fa.sh` даст `SKIP`,
требование когнитивное. Живой инвариант тронутого модуля назван: **`OPS-I-11`** (`docs/fa/ops.md:479`
на `9c133ec`), соседний — `OPS-I-8` (`:476`), от которого строка себя отделяет.

## Done Block

```
$ pwd; git -C /tmp/hft-recheck-ops12 rev-parse HEAD
/tmp/hft-recheck-ops12
9c133ecfb322c7af9da44e8a2a4733960926f824
$ bash scripts/verify_design_claims.sh --merge-preview origin/main 2>&1 | grep -E '^(FAIL|VERDICT)'; echo exit=$?
VERDICT: PASS (0 нарушений)
exit=0
$ grep -cE '\.rs:[0-9]+' docs/fa/ops.md; echo exit=$?
0
exit=1
$ grep -rn 'вне политики допуска' crates/*/tests; echo exit=$?
exit=1
$ bash scripts/next_artifact_id.sh R
R-239
$ bash scripts/reserve_artifact_id.sh R | tail -3
R-239
reserve: резерв R-239 взят; снять после приземления носителя:
reserve:   bash scripts/reserve_artifact_id.sh --release R-239
$ bash scripts/check_branch_health.sh 2>&1 | tail -2
веток кроме main: 15; замечаний: 0
VERDICT: PASS — наблюдение состоялось (NOTE не блокируют: это наблюдатель, не барьер)
```

Пост-коммитные проверки (`check_gate_meta.sh`, `check_review_fa.sh`, `check_artifact_ids.sh` на
диапазоне `origin/main..HEAD`) — в отчёте агента, после коммита этого файла; их исход
обязан быть зелёным до push.

## §8. Маршрут

APPROVE → автор открывает PR `docs/ops-i-11-names` → `main` с этим вердиктом в диапазоне →
зелёный `All checks passed` → merge → удалить ветку. Резерв `R-239` снят самим
перепроверяющим сразу после push носителя (`reserve_artifact_id.sh --release R-239`; номер
дальше держит сам файл на ветке). §8-деплой не нужен: диф — только
`docs/fa/ops.md`, прод-поведение не менялось.
