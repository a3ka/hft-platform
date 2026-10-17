<!-- GATE-META
milestone: M-45
audited_repo: a3ka/hft-platform
audited_base: 2e63a37e5bf454da69b0fbd69de28c043b4caf4c
audited_head: d6aca540f91f625aa21e77637e94f8d35199a9d5
verdict: REJECT
-->

# C-199 — M-45 rollout signature, круг 3: closed predecessors; unsigned config expansion still passes

**Вердикт: REJECT.** Оба предмета C-197 закрыты на судимом дереве: 2e63a37 — предок
d6aca54, а актуальная таблица scope-guard разрешает engine-dev объявлять операторские
ручки своих сервисов в корневом docker-compose.yml; check_gate_meta принимает
ALLOW-SUBJECT-CHANGE коммита d6aca54. T10 красный на нынешнем нераскатанном compose,
зелёный на обеих переменных в одном коммите и снова красный при двух коммитах.

Новые блокеры не пересуждают ни П-026, ни закрытые условия C-197: они судят различающую
силу принятого T10/T10b и предъявление фактуры нового supporting-document.

## Закрытия предыдущего круга

### C-197 B-1 — CLOSED

git merge-base --is-ancestor 2e63a37 HEAD вернул 0. На этой ревизии
.claude/rules/scope-guard.md даёт engine-dev корневой docker-compose.yml для операторских
ручек своих сервисов и запрещает менять состав без подписи границы C. Задача 7 назначена
engine-dev; hft-recorder — его сервис. Это требуемая B-1 связка, не ссылка на ещё не
влитую норму.

### C-197 B-2 — CLOSED

EVENT_NAME=pull_request вместе с check_gate_meta завершился 0: subject-lock
scripts/verify_M-45.sh открыт явным ALLOW-SUBJECT-CHANGE в d6aca54.

Выполнением точного блока T10/T10b из scripts/verify_M-45.sh lines 220-285 подтверждено:

- нынешний compose: T10 FAIL, exit 1 — ожидаемо, задача 7 ещё OPEN;
- отдельный detached worktree, один коммит be8b36b: T10/T10b PASS, exit 0;
- отдельный detached worktree, два коммита 5f466dc/1d2f0d2: T10 PASS, T10b FAIL,
  exit 1.

Следовательно, прежняя дыра «задача 7 вообще не судится» закрыта: YAML-boundary и история
одного коммита действительно наблюдаются.

## Новые блокеры

### B-3 — T10 принимает неподписанное расширение состава записи

П-026 подписывает to = BTCUSDT,ETHUSDT на spot и futures и прямо исключает любой
инструмент сверх ETHUSDT. Однако T10 проверяет только подстроки:

~~~python
if "ETHUSDT" not in sym.upper(): ...
if "BTCUSDT" not in sym.upper(): ...
~~~

Он не разбирает список и не сверяет его с подписанным множеством. Поэтому отдельный
detached worktree с одним коммитом 3852d10, содержащим:

~~~yaml
L2DELTA_CAPTURE_SYMBOLS: BTCUSDT,ETHUSDT,SOLUSDT
EPOCH_ID: own-2026-08-m45-eth-sol
~~~

прошёл точный блок T10/T10b с exit 0. Тем самым гейт разрешает записывать SOLUSDT — новое,
неподписанное изменение состава данных границы C. Это не спор о воле founder'а: явная
подпись уже ограничила множество, а оракул не удерживает её предел.

Условие снятия: T10 обязан нормализовать и разобрать L2DELTA_CAPTURE_SYMBOLS так, чтобы
предъявить РОВНО подписанный набор BTCUSDT, ETHUSDT (без дополнительных или
подстрочно-похожих токенов), и иметь RED/мутационную пробу:
BTCUSDT,ETHUSDT,SOLUSDT одним коммитом обязан дать FAIL. Изменять П-026 ради подгонки
оракула нельзя.

### B-4 — scope-check вновь выдаёт пересказы за вывод показанных команд

В docs/plans/scope-check-m45-m70-2026-08-31.md §0 утверждается, что каждое состояние
снято показанной командой. Требуемые diff-сверки опровергают это для четырёх блоков:

1. строки 28-29 не могут быть выводом git ls-remote --heads: команда печатает
   refs/heads/..., которых в показанном выводе нет (к тому же ветка M-45 уже продвинулась);
2. строки 31-32 сокращают реальный grep до двух строк с ..., тогда как команда печатает
   четыре полные строки, включая два doc-comment;
3. строка 44 — синтез :779 / :465 — ..., тогда как grep печатает четыре настоящие строки;
4. строка 46 — человеческое резюме задач, а grep ^| [0-9] печатает девять строк таблицы.

Два соседних блока воспроизводимы: M-45 §3ter lines 130-137 и 140 (оба diff exit 0), а
scope-check lines 34-37 и 48 (оба diff exit 0). Поэтому это не придирка к формату вывода:
документ смешивает реальные transcript-blocks и пересказы внутри одного fenced raw-output
блока. FACTS-маркер задаёт ревизию сбора, но не превращает текст, который команда никогда
не печатала, в её вывод.

Условие снятия: заменить каждый ложный raw-output блок либо байт-в-байт выводом его команды
на названной ревизии (с нужным фильтром в самой команде), либо вынести интерпретацию из
code block и явно пометить её как вывод автора. После этого повторить все diff-пары.

## Полный набор артефактов

T1 не менялся: diff paths 2e63a37..d6aca54 для crates/contracts и contracts пуст.
Зафиксированы T2/T3 функции parse_capture_symbols, should_capture_l2delta,
l2delta_emission_for, SpotSession/FuturesSession и SessionEffect; присутствуют оба RED-набора
allow-list и DET-I-1 fixture.

verify_M-45.sh прогнал T0-T9 зелёно: workspace build, clippy, fmt, обе RED-suite, O-8
на реальной entry point и смешанный DET-I-1. Его итог закономерно красный только по
открытой раскатке T10. Живые FA-инварианты, проверенные на этой ревизии: **VN-I-3**
(venue-specific branching остаётся в адаптерах) и **BK-I-2** (на gap книга синхронно
становится Stale до следующего события).

## Done Block

~~~text
$ git rev-parse HEAD origin/main
d6aca540f91f625aa21e77637e94f8d35199a9d5
2e63a37e5bf454da69b0fbd69de28c043b4caf4c
exit=0

$ git merge-base --is-ancestor 2e63a37 HEAD
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=2e63a37e5bf454da69b0fbd69de28c043b4caf4c bash scripts/check_gate_meta.sh
── GATE-META: диапазон 2e63a37e..HEAD, origin=a3ka/hft-platform
NOTE  research/reviews/R-162-decisions-recheck-r4.md: subject-lock открыт явным ALLOW-SUBJECT-CHANGE (аудит-след, НЕ доказательство — F-064-6): scripts/verify_M-45.sh
VERDICT: PASS — вердиктов проверено: 6, до-нормативных приземлений: 0, merge'ей с milestone в subject'е: 2
exit=0

$ bash scripts/verify_M-45.sh 2>&1 | grep -E '^(PASS|FAIL|VERDICT)'
PASS  T0 оракул присутствует: crates/venue-binance/tests/red_l2delta_allowlist.rs
PASS  T0 оракул присутствует: crates/venue-binance-futures/tests/red_l2delta_allowlist.rs
PASS  T1 cargo build --workspace
PASS  T2 cargo clippy --workspace --all-targets -D warnings
PASS  T2b cargo fmt --all --check (совпадает с ci.yml)
PASS  T3 venue-binance: без конфигурации состав эмиссии = ["BTCUSDT"]
PASS  T4 venue-binance: allow-list оракул GREEN (23 тестов)
PASS  T4 venue-binance-futures: allow-list оракул GREEN (21 тестов)
PASS  T5b venue-binance: O-8 GREEN (6 тестов через реальную точку входа)
PASS  T5b venue-binance-futures: O-8 GREEN (6 тестов через реальную точку входа)
PASS  T8 DET-I-1 GREEN на смешанном журнале (снапшот+дельта)
FAIL  T10 задача 7 НЕ исполнена — ОТСУТСТВУЮТ на сервисе recorder: L2DELTA_CAPTURE_SYMBOLS, EPOCH_ID
VERDICT: FAIL (1 нарушений)
exit=1

$ exact T10/T10b block on detached worktrees
current: FAIL T10: L2DELTA_CAPTURE_SYMBOLS, EPOCH_ID absent; exit=1
atomic be8b36b: PASS T10; PASS T10b, one commit; exit=0
split 5f466dc/1d2f0d2: PASS T10; FAIL T10b, different commits; exit=1
extra 3852d10: PASS T10 with BTCUSDT,ETHUSDT,SOLUSDT; PASS T10b, one commit; exit=0

$ required diff pairs
M-45 §3ter lines 130-137: exit=0
M-45 §3ter line 140: exit=0
scope-check lines 34-37: exit=0
scope-check line 48: exit=0
scope-check lines 28-29: exit=1
scope-check lines 31-32: exit=1
scope-check line 44: exit=1
scope-check line 46: exit=1

$ bash scripts/next_artifact_id.sh C
C-199
exit=0

$ git diff --check 2e63a37..d6aca54
exit=0
~~~

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-08-31T23:12Z
- Milestone: M-45-persist-l2delta
- Статус: BLOCKED
- HEAD: d6aca54 — spec(M-45): C-197 B-1/B-2 — норма влита в ветку, задача 7 назначена engine-dev [architect]

## §B — Что я сделал
- Судил только закрытие C-197 B-1/B-2 и новые классы; П-026 и предмет инверсии не пересуждал.
- Воспроизвёл T10/T10b на нынешней, атомарной, split и extra-symbol конфигурациях, а также все требуемые diff-пары.

## §C — Артефакты / результаты
- research/critiques/C-199-m45-rollout-signature-r3.md
- Done Block: check_gate_meta exit=0; current verify_M-45.sh exit=1 только по OPEN T10; atomic T10/T10b exit=0; split exit=1; unsigned-extra mutation exit=0 (дефект); 4 scope-check diff exit=1.

## §D — Следующий агент + инвокация
- **Следующий агент:** architect
- **Paste-ready промпт:**
  ~~~
  Исправь REJECT C-199 на ветке docs/M-45-rollout-signature, не меняя решение П-026.
  B-3: усили T10 так, чтобы он принимал только точный подписанный набор
  BTCUSDT,ETHUSDT и краснел на BTCUSDT,ETHUSDT,SOLUSDT даже при одном коммите; предъяви
  RED/мутационную пробу. B-4: в docs/plans/scope-check-m45-m70-2026-08-31.md замени
  пересказы внутри raw command blocks на точный вывод их команд либо вынеси интерпретации
  из blocks; повтори diff-пары. Запушь новый subject head и запроси новый critic-круг.
  ~~~
- Push-статус: ⏸ verdict commit follows this handoff; push target is origin/docs/M-45-rollout-signature.
- Кэш: ✅ кэши временных test-worktree будут удалены после push.

## §E — Риски / открытые вопросы
- Не диспетчеризовать engine-dev на задачу 7: текущий T10 допускает неподписанный новый символ.
- Текущий T10 FAIL на рабочей ветке сам по себе ожидаем: реальная раскатка ещё не выполнена; блокер — его ложное зелёное на extra-symbol fixture.

=== END HANDOFF ===
