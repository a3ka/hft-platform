<!-- GATE-META
milestone: TD-220
audited_repo: a3ka/hft-platform
audited_base: e4cab872344282dcc5c8ade0c6394095f1d049b4
audited_head: e4cab872344282dcc5c8ade0c6394095f1d049b4
verdict: NOTE
-->

# R-233 — закрытие `TD-241` и `TD-220`, заведение `TD-242` (реестры к факту после PR #311/#312)

**Роль:** reviewer (владелец `TECH-DEBT.md` / `PROJECT-STATE.md`; колонка «Состояние» `docs/ROADMAP.md`).
**Дата:** 2026-10-04. **Ветка:** `docs/td-220-241-closeout` от `e4cab87`.
**Предмет:** состояние `origin/main` после merge PR #312. Вершину взял командой:

```
$ git fetch origin && git rev-parse origin/main
e4cab872344282dcc5c8ade0c6394095f1d049b4
$ gh pr view 312 --json state,mergeCommit --jq '.state+" "+.mergeCommit.oid'
MERGED e4cab872344282dcc5c8ade0c6394095f1d049b4
```

Кода диапазон не трогает: это приведение реестров к факту. Поэтому вердикт — **NOTE**, а не APPROVE.
Живой инвариант предмета — **`OPS-I-11`** (`docs/fa/ops.md:479`, тишина ВЫДАЧИ), соседний —
**`OPS-I-8`** (`:476`, тишина MD-потока).

## 1. `TD-241` — ЗАКРЫТА целиком

Условие карточки: `OPS-I-8` → `OPS-I-11` в трёх doc-комментариях `watchdog.rs` (engine-dev) и в
четырёх местах двух sacred-оракулов (architect).

- Часть engine-dev — PR #311 `44821e8`, вердикт `R-229` APPROVE.
- Sacred-часть — PR #312 `e4cab87` (перепроверка `R-232` APPROVE).

Проверка КЛАССА, а не названных мест:

```
$ git grep -n 'OPS-I-8' origin/main -- 'crates/*/tests/*'
crates/ops/tests/red_m89_serving_silence.rs:2:  …(`I-6`, `TD-220`, `OPS-I-11` — тишина выдачи; до `TD-237` подавалось как «`OPS-I-8` на выдачу», `TD-241`…
crates/ops/tests/red_ops_metrics.rs:148:        /// OPS-I-8: возраст последнего MD-события выше порога → тишина (алерт P1, TD-011/TD-014).
crates/recorder/tests/red_metrics_emission.rs:239: …(возраст с последнего приёма; OPS-I-8 silence растёт при тишине)…
crates/recorder/tests/red_metrics_emission.rs:251: "md_event_age_ms НЕ несёт labeled SAMPLE `{{venue}}` — silence-метрика (OPS-I-8) мертва"
```

- `red_m89_serving_silence.rs:2`: атрибуция теперь `OPS-I-11`, а `OPS-I-8` упомянут как
  исторический след с отсылкой к `TD-241`. Это верно.
- Остальные три попадания относятся к `md_event_age_ms`, то есть к MD-потоку. Это верно.
- В `red_m91_legacy_counters.rs` попаданий нет.
- В `crates/*/src/` остались только ссылки на MD-поток (`R-229`).

## 2. `TD-220` — ЗАКРЫТА

Критерий закрытия назван в самой карточке (`R-226`): «"не нужно" закрывает карточку записью решения;
"нужно" — RED + engine-dev». Решение «не нужно» записано в FA на `origin/main`:

```
$ git show origin/main:docs/fa/ops.md | sed -n 479p | grep -oE '[^.]*(refusals_unsupported|overloaded|частичн)[^.]*\.'
 **Отказы `refusals_unsupported` правилом НЕ судятся — решение (`TD-220`, architect 2026-10-04):** этот счётчик растёт, когда селектор клиента вне политики допуска (три проверки `admit()` — `bands`, `symbol`, `timeframe_ms`, `crates/gateway-serve/src/admission.…
 Голодание, при котором отвергаются ПОДДЕРЖАННЫЕ запросы, ловит правило выше; отказы по нехватке слота выдачи (`overloaded`, `crates/gateway-serve/src/lib.…
 Предел: правило ловит ПОЛНОЕ голодание (`Δsuccesses == 0`); частичное — часть поддержанных запросов стабильно отвергается при ненулевых успехах — им не ловится.
```

Решение прошло перепроверку §9 (FA — уставная зона):

```
$ head -7 research/reviews/R-230-td-220-241-recheck.md | grep verdict   →  verdict: REJECT  (head c0b94f2)
$ head -7 research/reviews/R-232-td-220-241-recheck-2.md | grep verdict →  verdict: APPROVE (head 46a85ec)
$ git merge-base --is-ancestor 46a85ec46a7334ca946d9237a7bbec1435d647b3 origin/main && echo "R-232 head в main"
R-232 head в main
```

Доставку и исполнение потребителя закрыл ещё `R-222`. Канал наружу (`П-003`) — дело founder'а.
Карточка сама оговаривала, что это не её предмет. Значит, все условия исполнены.

## 3. `TD-242` — заведена (поручение `R-232` Н-3, находка `R-230` Б-2)

В `R-232` Н-3 записано: «напоминание reviewer'у при закрытии `TD-220`». В реестре такой карточки не было
(`grep -n 'band/timeframe/window' TECH-DEBT.md` → 0). Замер:

```
$ git grep -n 'band/timeframe/window' origin/main -- crates/
crates/gateway-serve/src/lib.rs:1180:  "selector вне политики допуска (band/timeframe/window)".to_string();
```

`admit()` (`crates/gateway-serve/src/admission.rs:122-131`) судит `bands`, `symbol`, `timeframe_ms`.
В сообщении ось `window` указана ложно, а `symbol` пропущен. Severity MINOR: правило допуска работает
верно, неверен только текст отказа. Зона — engine-dev. Номер выдан механизмом:
`scripts/next_artifact_id.sh TD` → `TD-242`.

## 4. Счёт «В ПЛАНЕ»: 75 → 74

```
$ awk 'NR>51 && NR<2100 && /^\| \*\*TD-[0-9]+\*\*/' TECH-DEBT.md | wc -l     # до правки, origin/main e4cab87
75
$ awk 'NR>51 && NR<2101 && /^\| \*\*TD-[0-9]+\*\*/' TECH-DEBT.md | wc -l     # после правки
74
```

Арифметика: 75 − 2 (`TD-220`, `TD-241`) + 1 (`TD-242`) = 74. Тела закрытых карточек перенесены в
`docs/archive/TECH-DEBT-closed-2026-08-16.md` с меткой `✅ CLOSED` и выводом reviewer'а — по образцу `TD-237`/`R-226`.

## 5. Что ещё правлено

- `docs/ROADMAP.md`, строка 9-A-bis, колонка «Состояние» (зона reviewer'а, `04-workflow.md` §2):
  оговорка «правило на отказы по бюджету — `TD-220`, architect» заменена на «решено».
- `PROJECT-STATE.md`: дополнение к блоку `M-93`/`R-226` с итогом и прод-фактом после PR #311
  (`44821e80` на VPS, оба контейнера healthy, деплой success; предъявлено в close-out `R-229`).

## Что искал грепом в ярусе C

- `TECH-DEBT.md`: `TD-220`, `TD-241`, `TD-237`, `band/timeframe/window`.
- `PROJECT-STATE.md`: `TD-220`, `TD-241`.
- `docs/ROADMAP.md`: `TD-220`, `TD-241`.
- `research/reviews/`: `R-230`, `R-232`.
