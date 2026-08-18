<!-- GATE-META
milestone: docs/workflow-audit
audited_repo: a3ka/hft-platform
audited_base: 0f4892eeeb3f5a4ae642342d5affe34bd49efee4
audited_head: cca9836
verdict: APPROVED
-->

# R-037 — PR-гейт `docs/workflow-audit`, круг 3 — **APPROVED**

**Роль:** reviewer · **Дата:** 2026-08-05 · **Предмет:** `docs/plans/workflow-audit-2026-08-einhard-vs-hft.md`
**Предыдущие круги:** `R-034` (REJECT, 4 находки) → `R-036` (REJECT, 3 находки + 2 NOTE) → **этот**
**Дельта круга 3:** `f81bf60`, `+74/−13` по одному файлу; из них поверх прежнего tip'а ветки — `+22/−9`.

**Вердикт: APPROVED.** Три находки и два NOTE круга 2 закрыты; каждая закрытая проверена
СОБСТВЕННЫМ замером, не сверкой с заявлением. Целостность моих вердикт-файлов после
force-push подтверждена побайтово. Scope чист — 0 строк кода.

---

## §A — ПРОВЕРКА 1 (первым делом): целостность вердиктов после force-push — **PASS**

Ветка пересобрана `--force-with-lease` от базиса `d7f773c`. Это единственная операция, при
которой мой артефакт может исчезнуть НЕ удалением (барьер `check_protected_artifacts.sh` ловит
удаление в дифе, а не подмену истории). Проверял тремя независимыми способами.

**1. Блобы вердиктов — совпадают с заявленными, до байта:**

```
$ git rev-parse HEAD:research/reviews/R-034-workflow-audit-and-retro.md
5b8011f1879565c95b863e5ad8821cedd1ba955b      # заявлено 5b8011f18795 ✓
$ git rev-parse HEAD:research/reviews/R-036-workflow-audit-rev2.md
b5bc7253af8d7e031c5891fcb6547807cc2be2ab      # заявлено b5bc7253af8d ✓
```

**2. Сообщения моих коммитов не подменены** (архитектор сам сообщил, что первая попытка надела
его сообщение на мой коммит; проверяю результат, а не рассказ об откате):

```
$ diff <(git log -1 --format='%B' 08a021b) <(git log -1 --format='%B' 93f126f) && echo OK
93f126f message == 08a021b OK
$ diff <(git log -1 --format='%B' d7f773c) <(git log -1 --format='%B' cca9836) && echo OK
cca9836 message == d7f773c OK
```

**3. Дерево целиком — единственное отличие старого tip'а от нового есть предмет круга 3:**

```
$ git diff --stat d7f773c cca9836
 .../plans/workflow-audit-2026-08-einhard-vs-hft.md | 31 +++++++++++++++-------
 1 file changed, 22 insertions(+), 9 deletions(-)
```

Ни один другой файл не тронут. Потери при пересборке нет.

**Побочно проверен сам барьер — он НЕ слеп к force-push, как я опасался в `R-036` §E.**
Барьер fail-closed по неустановленному базису:

```
$ EVENT_NAME=push PUSH_BEFORE=d7f773c bash scripts/check_protected_artifacts.sh; echo exit=$?
FAIL  база 'd7f773c' НЕ предок HEAD — история переписана (force-push); что исчезло, недоказуемо
exit=1

$ EVENT_NAME=push PUSH_BEFORE=$(git merge-base origin/main HEAD) bash scripts/check_protected_artifacts.sh; echo exit=$?
OK: защищённые артефакты целы на HEAD (dca889a..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0
```

Уточнение к моей же формулировке круга 2: барьер не «пропускает» force-push — он **отказывается
удостоверять** и требует человека. Роль ручной сверки блобов (выше) он не заменяет, но и молча
зелёным не становится. Долг на это не завожу — механизм ведёт себя правильно.

---

## §B — `F-036-1` (BLOCKER) — **ЗАКРЫТА**, проверено собственным замером

Правка (`:310-317`) утверждает: скрипт **читает** идентичность (`:99`), требует её непустоту в
`--dry-run` (`:104`), а установка снята по `A-003` #27 и это зафиксировано в `:92-96`.
Перемерил всё сам на дереве ветки:

```
$ wc -l .claude/wrappers/pi-dev.sh
197 .claude/wrappers/pi-dev.sh

$ grep -n '' .claude/wrappers/pi-dev.sh | sed -n '92,104p'
92:# Личность НЕ переустанавливается (branch-hygiene.md п.6, commit-discipline).
93:# Автор коммитов — владелец репозитория; роль указывается меткой в конце subject'а:
94:#   feat(M-NN): task #k — <...> [${ROLE}]
95:# Прежний блок ставил ролевой user.name/email per-worktree; замером установлено, что как
96:# признак роли git-личность не работает (все worktree несли подпись предыдущей роли).
97:[ -d "$PROJECT_DIR/.githooks" ] && git config core.hooksPath .githooks
99:IDENT_EMAIL="$(git config user.email)"
104:  [ -n "$IDENT_EMAIL" ] || { echo "DRY-RUN FAIL: git identity не настроена"; FAIL=1; }

$ grep -nE 'git config[[:space:]]+(--[a-z]+[[:space:]]+)?user\.(name|email)[[:space:]]+' .claude/wrappers/pi-dev.sh; echo exit=$?
exit=1                                  # установки идентичности в скрипте НЕТ
```

Атрибуция `A-003` #27 тоже проверена, а не принята на слово — `research/arbitration/A-003-rules-vs-workflow.md:74`:
пункт 27 (`branch-hygiene` п.6 identity через `--worktree`/config) — вердикт **УДАЛИТЬ**,
заменить «единая подпись владельца + метка роли в конце subject». Ссылка точна.

Согласованность с самим документом: `§7-2` («Ролевая git-личность + identity-hook… Не
реанимировать») существует и говорит ровно то, на что ссылается новый текст `:328-329`.

**Тиражирование дефекта устранено.** `R-036` отметил, что клауза уже скопирована в спеку M-60.
Проверено на ветке-приёмнике:

```
$ git show --numstat --format='' 878c6a5
7	2	milestones/M-60-mechanisms.md
$ git show origin/feat/M-60-mechanisms:milestones/M-60-mechanisms.md | sed -n '82,89p'
**Что носитель уже делает:** свежий worktree от `origin/$BRANCH` (`pi-dev.sh:75-76`),
ЧИТАЕТ git-идентичность и требует её непустоту в `--dry-run` (:99, :104), …
**Идентичность он НЕ устанавливает:** `pi-dev.sh:92-96` фиксирует, что прежний блок …

$ git grep -n 'ставит идентичность' origin/main -- .; echo exit=$?
exit=1                                  # в main ложного утверждения нет и не будет
```

Остаточные вхождения строки «ставит идентичность» — только в `R-036` (мой же вердикт) и в
абзаце-исправлении как ЦИТАТА снятого. Это аудит-след, а не рецидив.

**Прочие номера строк того же абзаца перемерены заодно** (раз абзац переписан — проверяю
целиком, а не только спорную клаузу):

| утверждение документа | замер | итог |
|---|---|---|
| worktree от `origin/$BRANCH` — `:75-76` | `:75` `worktree add … "origin/$BRANCH"`, `:76` fallback `--detach` | ✅ |
| инжект `dispatch-mandate.md` — `:119-126`, `:176` | `:119-126` чтение файла + предупреждение при отсутствии; `:176` `--append-system-prompt "$AGENT_CONTENT$SYSTEM_IDENTITY$MANDATE_CONTENT"` | ✅ |
| W6 (auto-push при выходе) ОТСУТСТВУЕТ | `grep -n push .claude/wrappers/pi-dev.sh` → **exit=1** | ✅ |

---

## §C — `F-036-2` (MAJOR) — **ЗАКРЫТА**

Было (обрубок, снятого утверждения в «цитате» нет):
> `Прежняя формулировка: субагент — субагент получает текстовый мандат.`

Стало (`:323-324`):
> `Прежняя формулировка (СНЯТА): «У нас носителя-лаунчера нет — субагент получает текстовый`
> `мандат». Её оговорка остаётся в силе: наш харнес ИМЕЕТ исполняемые крючья…`

Оба требования `R-036` выполнены: цитата приведена целиком (аудит-след правки восстановлен) и
антецедент следующей фразы больше не повисает — «Её оговорка» отсылает к явно приведённой
формулировке, а не к тезису, вычеркнутому из текста.

---

## §D — `F-036-3` (BLOCKER) — **ЗАКРЫТА в коммите architect'а**

```
$ git log -1 --format='%B' f81bf60 | grep -c '^Co-authored-by:'
0
$ git log -1 --format='%B' f81bf60 | grep -ci 'co-authored-by'
1
```

Единственное вхождение — без якоря начала строки, внутри прозы («из тела убран трейлер
`Co-authored-by:`»). Это объяснение правки, а не трейлер: `git interpret-trailers` его не
видит, `git log --format='%(trailers)'` — тоже. Дефекта нет.

Состояние ветки целиком:

| коммит | автор-роль | трейлеров (якорь) |
|---|---|---|
| `f81bf60` | architect | **0** ✅ |
| `93f126f` | reviewer (мой) | 1 ⚠ |
| `cca9836` | reviewer (мой) | 1 ⚠ |

Мои два коммита трейлер несут и НЕ переписаны — и это правильное решение architect'а: их
пересборка ради косметики второй раз подвергла бы риску ровно тот аудит-след, который §A только
что удостоверил побайтово. Причина трейлера — инфраструктурная (см. §G), решение — за
founder'ом; долг по прямому указанию не завожу.

---

## §E — NOTE-1 и NOTE-2 — **закрыты**

**NOTE-1 (порядок §6).** Проверено грепом заголовков:

```
$ grep -n '^\*\*6-[0-9]' docs/plans/workflow-audit-2026-08-einhard-vs-hft.md
354:6-1  370:6-2  382:6-3  391:6-4  401:6-5  411:6-6  422:6-7  430:6-8  435:6-9
```

Монотонно 6-1…6-9. Выбран вариант «переставить», а не «оговорить ранг в шапке».

**NOTE-2 (двойной учёт выгоды 6-5/6-9).** Внесён абзац `:448-452`: «6-5 делает push-статус в §D
проверяемым ПРЕДИКАТОМ — лечит ложное УТВЕРЖДЕНИЕ о push'е; 6-9 добавляет auto-push при выходе —
лечит сам факт незапушенной работы». Разграничение верно по текстам обоих пунктов (`:401-409`
— предикат `AHEAD=$(git log origin/<branch>..HEAD | wc -l)` в `handoff-block.md`; `:435-447` —
механика в `pi-dev.sh`). Двойного счёта больше нет.

---

## §F — Block-scope + новые утверждения о коде

```
$ git diff --stat origin/main...HEAD
 .../plans/workflow-audit-2026-08-einhard-vs-hft.md | 503 +++++++++++++++
 research/reviews/R-036-workflow-audit-rev2.md      | 416 +++++++++++++
 2 files changed, 919 insertions(+)
```

`docs/plans/**` (зона architect/auditor) + `research/reviews/**` (моя зона). **Ноль строк кода,
ноль касаний `crates/**`, `contracts/**`, `*/tests/`, `scripts/`.** RISK-BLOCK (`gates.md` §5)
не применим — safety-путь не затронут ни кодом, ни текстом (`docs/fa/{risk,killswitch,oms}.md`,
`RK-I-*`, `INTG-I-*`, анти-оверфит гейт §6 — не в дифе). Contract Block-C не применим.
Critic-гейтинг: `gates.md` §9 — правка `docs/plans/**`, форма архитектуры (инварианты, границы,
фазы) не меняется; SKIP законен и заявлен в §D handoff'а явно.

**Новых непроверенных утверждений о коде в дельте круга 3 нет.** Дельта `+22/−9` содержит
ровно пять утверждений, ссылающихся на код или артефакты, — все пять перемерены в §B/§E.

---

## §G — Причина `F-036-3` установлена и она НЕ та, что в handoff'е — факт для founder'а

Architect вынес вопрос founder'у как «правило BINDING против атрибуции на GitHub, три недели
нарушалось». Первая половина верна, вторая — нет, и точность здесь меняет цену решения.

**Замер:**

```
$ stat -c 'mtime=%y atime=%x' .git/hooks/prepare-commit-msg
mtime=2026-07-26 23:55:25   atime=2026-08-05 00:18:06

$ git log --all --format='%H' | while read c; do \
    git log -1 --format='%B' $c | grep -q '^Co-authored-by:' && git log -1 --format='%ad' --date=short $c; \
  done | sort | uniq -c
     36 2026-08-05

$ # из них уже в origin/main: 12
```

Хук лежит с **2026-07-26**, но **ни один коммит до 2026-08-05 трейлера не несёт**. Все 36 —
сегодняшние. Объяснение даёт сам аудируемый документ, `§1.3`:

> `git config --show-origin core.hooksPath` → `file:.git/config  .githooks`, при этом каталога
> `.githooks` НЕТ… это ловушка: **любой будущий хук, положенный в `.git/hooks`, будет молча
> игнорироваться** (config разделяется всеми worktree).
> **Статус (architect, 2026-08-05).** Настройка снята: `git config --unset core.hooksPath`.

`prepare-commit-msg` и был тем самым молча игнорируемым хуком — девять дней. Снятие мёртвого
`core.hooksPath` (пункт **6-4** этого же аудита, исполненный 2026-08-05) его **разбудило**;
atime хука `2026-08-05 00:18` совпадает с окном.

**Что из этого следует для решения founder'а:**

1. **Правило не «нарушалось три недели» — оно нарушается один день, 36 коммитами** (12 из них
   уже в `main`). Масштаб отката, если выбирается «правило важнее», на порядок меньше
   заявленного.
2. **Атрибуция на GitHub все девять дней не работала.** Хук был инертен — зелёные квадраты за
   этот период трейлер не давал. То есть выбор founder'а не «отнять работающую атрибуцию», а
   «включать ли её впервые с 26 июля».
3. **Это побочный эффект исполнения пункта аудита.** Свидетельство в пользу самого аудита:
   §1.3 предсказала ровно этот класс («будущий хук будет молча игнорироваться») — и первым же
   разбуженным хуком оказался тот, что бьёт по BINDING-правилу. Механическая заметка для
   пункта 6-4 при внедрении: снятие `hooksPath` обязано сопровождаться ревизией `.git/hooks`,
   иначе «уборка мёртвой настройки» тихо включает неизвестный набор хуков.

Долг не завожу (прямое указание handoff'а). Изложено как факт-корректировка к вопросу,
вынесенному founder'у.

---

## §H — Остаточные NOTE (на merge не влияют, автору на усмотрение)

- **NOTE-3.** Шапка §6 — «ранжировано: закрываемая цена ошибки ÷ цена внедрения». После
  перестановки по NOTE-1 порядок стал числовым, и 6-9 (кандидат с пересчитанной, высокой
  ценностью) стоит после условного «НЕ сейчас» 6-8. Ранг и номер теперь совпадают ценой
  ослабления ранжирования. Выбор был мой (я предложил два варианта) — фиксирую как след, не
  как требование.
- **NOTE-4.** Тело `f81bf60` в своей части круга 2 сохраняет прежнюю формулировку «…носитель
  есть и работает (worktree :75-76, **идентичность :99**…)», а опровергает её ниже, в секции
  «Круг 3 (R-036)» того же сообщения. Внутри одного сообщения это читается как история правки;
  переписывать ради этого коммит третий раз — хуже, чем оставить.

---

## §I — Done Block

```
$ pwd; git rev-parse --short HEAD
/tmp/hft-rev-wfaudit3
cca9836

$ git rev-parse HEAD:research/reviews/R-034-workflow-audit-and-retro.md
5b8011f1879565c95b863e5ad8821cedd1ba955b
$ git rev-parse HEAD:research/reviews/R-036-workflow-audit-rev2.md
b5bc7253af8d7e031c5891fcb6547807cc2be2ab

$ git diff --stat d7f773c cca9836
 .../plans/workflow-audit-2026-08-einhard-vs-hft.md | 31 +++++++++++++++-------
 1 file changed, 22 insertions(+), 9 deletions(-)

$ git log -1 --format='%B' f81bf60 | grep -c '^Co-authored-by:'
0

$ grep -nE 'git config[[:space:]]+(--[a-z]+[[:space:]]+)?user\.(name|email)[[:space:]]+' .claude/wrappers/pi-dev.sh; echo exit=$?
exit=1

$ grep -n push .claude/wrappers/pi-dev.sh; echo exit=$?
exit=1

$ git grep -n 'ставит идентичность' origin/main -- .; echo exit=$?
exit=1

$ bash scripts/verify_design_claims.sh >/dev/null 2>&1; echo exit=$?
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main >/dev/null 2>&1; echo exit=$?
VERDICT: PASS (0 нарушений)
exit=0

$ EVENT_NAME=push PUSH_BEFORE=$(git merge-base origin/main HEAD) bash scripts/check_protected_artifacts.sh; echo exit=$?
OK: защищённые артефакты целы на HEAD (dca889a..HEAD; проверка по РЕЗУЛЬТАТУ, не по способу)
exit=0

$ git diff --stat origin/main...HEAD
 2 files changed, 919 insertions(+)     # docs/plans/** + research/reviews/**, 0 строк кода
```

---

## §J — Вердикт и условия

**APPROVED.** Мержу `docs/workflow-audit` → `main` (`--no-ff`), обновляю `PROJECT-STATE.md` /
`TECH-DEBT.md`, деплой-гейт `gates.md` §8, `gc_worktrees.sh`. Merge разблокирует **M-60**
(`C-064` F-064-1).

Итог трёх кругов: `R-034` — 4 находки (одна несущая: «лаунчеров нет» при существующем
`pi-dev.sh`); `R-036` — 3 находки, включая регрессию того же класса в абзаце-исправлении;
`R-037` — чисто. Документ уходит в `main` с корпусом замеров, каждый пункт которого предъявлен
командой и exit-кодом.

---

## §K — Close-out: merge, деплой-гейт `gates.md` §8, GC — **выполнено**

**Merge и ledger'ы:**

```
$ git log --oneline -3 origin/main
a00c8de docs(state): close-out docs/workflow-audit — аудит в main (R-037 APPROVED), TD-106 …
60aef9e merge(docs/workflow-audit): аудит воркфлоу einhard vs hft-platform — APPROVED R-037 …
0f4892e docs(debt): TD-106 — симптом закрыт (cf24aac), корень OPEN; main зелёный …

$ git show --numstat --format='' a00c8de
32	13	PROJECT-STATE.md
8	0	TECH-DEBT.md
```

**CI — оба коммита зелёные до терминального статуса:**

```
$ gh run watch 31056122871 --exit-status; echo exit=$?   # merge-коммит
exit=0
$ gh run watch 31056233111 --exit-status; echo exit=$?   # close-out ledger'ов
exit=0

$ gh run list --limit 3
completed  success  docs(state): close-out docs/workflow-audit …   CI  main  push  31056233111  10m43s
completed  success  merge(docs/workflow-audit): аудит воркфлоу …   CI  main  push  31056122871  10m52s
completed  success  docs(debt): TD-106 — симптом закрыт …          CI  main  push  31054572965   7m47s
```

**Deploy НЕ запускался — и это корректно, а не пропуск гейта.** `deploy.yml` триггерится
«пуш в main **по путям кода**»; оба коммита — `docs/plans/**`, `research/reviews/**`,
`PROJECT-STATE.md`, `TECH-DEBT.md`. Развёртываемого не менялось, прод остаётся на `dca889a`
(последний Deploy — `31052175517`, success).

**Прод глазами (§8 п.2) — два замера с интервалом ~11 минут:**

```
$ ssh … 'docker ps --format "{{.Names}} {{.Status}}"; cat …/recorder.heartbeat'
hft-gateway-serve Up About an hour (healthy)
hft-recorder      Up About an hour (healthy)

t1: {"events":235097,"next_seq":176510415,"segment_index":189,"ts_wall_ms":1785972257499,"writable":true}
t2: {"events":279825,"next_seq":176555208,"segment_index":189,"ts_wall_ms":1785972907500,"writable":true}
     Δt = 650.0 s · Δevents = +44 728 · Δseq = +44 793 — журнал растёт, heartbeat отстаёт на 9 s

$ for p in $(pgrep -f recorder); do grep RssAnon /proc/$p/status; done
RssAnon:  19616 kB          # норма; замер по RssAnon, не docker stats (TD-021)
$ du -sh …/journal-data/_data → 37G · free_bytes 67.3 GB при min_free 10.7 GB
```

Содержательный sanity свежих событий не требуется: деплой не выполнялся, парсеры/форматы
merge'ем не затронуты (0 строк кода).

**Worktree-GC:**

```
$ bash scripts/gc_worktrees.sh; echo exit=$?
REMOVED  hft-arch-wfaudit          # ветка предмета влита
REMOVED  hft-rev-wfaudit3          # мой ревью-worktree
REMOVED  hft-research-dev-1785969863
worktree'ов осталось: 15
VERDICT: GC DONE
exit=0
```

Осталось у architect'а: удалить страховочную ветку `safety-r036` (локальная, на прежнем
`d7f773c`) — её назначение исчерпано, целостность подтверждена §A.

**Что merge разблокировал:** **M-60** (`C-064` F-064-1 — спека больше не опирается на замеры
вне проверяемой цепочки) и пункт **6-3** как спроектированное лечение корня `TD-106`/`TD-062`.
