# C-084 — process-decisions-2026-08-14

**Дата (UTC):** 2026-08-14
**Роль:** critic
**Модель:** codex/gpt-5, сильная модель назначена явно
**Предмет:** `origin/docs/process-decisions-2026-08-14` @ `b7785c2`
**Файл:** `docs/plans/process-decisions-2026-08-14.md`

## Verdict

**NOTE.** Блокеров уровня REJECT не нашёл: несущий замер Р-1 воспроизведён и подтверждает
no-op-дыру одинакового SHA; предложенный принцип `commit-tree` с уникальным сообщением делает
этот класс недостижимым при условии реально уникального nonce; документ не меняет §11-зону
напрямую; `deploy.yml` по действующей букве правил относится к зоне §9-харнесса, а не к §11.

NOTE не означает "без замечаний": ниже зафиксированы advisory-пункты, которые должны быть
сохранены как вход для будущих milestone'ов. Я не предлагаю альтернативную развязку, а сужу
предъявленную.

## Scope / authority

- Дифф предмета: один новый файл `docs/plans/process-decisions-2026-08-14.md` на 344 строки.
- Прямых правок `.claude/rules/**`, `.claude/agents/**`, `.claude/wrappers/**`, `CLAUDE.md`,
  `docs/04-workflow.md` нет.
- `check_docs_freeze.sh` в форме мандата прошёл: `EVENT_NAME=push PUSH_BEFORE=$(git rev-parse origin/main) bash scripts/check_docs_freeze.sh` -> `exit=0`.
- Утверждение "deploy.yml -- зона §9, не §11" подтверждено буквой `gates.md`: §9 включает
  `.github/workflows/**` в харнесс Fable-перепроверки, §11 перечисляет только `.claude/rules/**`,
  `.claude/agents/**`, `.claude/wrappers/**`, `CLAUDE.md`, `docs/04-workflow.md`.
- T-contracts / trait signatures / RED tests / verify script / milestone file как plan-time
  dev artifact set здесь неприменимы: предъявленный предмет -- процессный decision-документ,
  не milestone на реализацию. Для будущих механизмов сам документ требует отдельные RED-first
  milestone'ы.

## Checks

### Р-1 — Git-race замер

Воспроизвёл локально на bare-origin + два клона:

```text
E1 different SHA, plain push:
A exit=0, new reference refs/reserved/R-100
B exit=1, rejected/fetch first

E2 same SHA, plain push:
A exit=0, new reference refs/reserved/R-101
B exit=0, Everything up-to-date

E3 same SHA, --force-with-lease expecting absent:
A exit=0, new reference refs/reserved/R-102
B exit=0, Everything up-to-date

E5 simultaneous different SHA:
A exit=0
B exit=1, remote: cannot lock ref 'refs/reserved/R-105': reference already exists

E6 cleanup:
R-100/R-101/R-102/R-105 delete exit=0; remaining reserved refs: empty
```

Итог: исходный кандидат "push занятого ref отвергается" ложен для одинакового SHA; `--force-with-lease`
не закрывает no-op; разные SHA и одновременность ведут себя как документ утверждает.

Отдельная проверка `commit-tree`:

```text
100 commit-tree с уникальными сообщениями: total=100 unique=100 duplicates=0
два commit-tree с тем же деревом и тем же сообщением: SHA совпал
```

Следствие: конструкция корректна только если будущая обёртка действительно гарантирует уникальное
сообщение/nonce. Это не опровергает решение, но должно остаться обязательным RED-пунктом.

### Р-1 — видимость refs/reserved текущими скриптами

Факты на дереве предмета:

```text
git config --get-all remote.origin.fetch
+refs/heads/*:refs/remotes/origin/*

scripts/next_artifact_id.sh: refs_all() = refs/remotes/origin refs/heads
scripts/check_artifact_ids.sh: refs_all() = refs/remotes/origin refs/heads
git for-each-ref refs/remotes/origin refs/heads refs/reserved | rg reserved
<empty>
```

Утверждение документа "аллокатор и барьер резервов не видят" подтверждено.

### Р-2 — узкий retro-§9 по M-61

`R-068` существует в `origin/main`:

```text
9601f42 docs(review): R-068 -- M-61 круг 5-fix, независимая перепроверка R-065 [reviewer]
```

По существу документ оценивает `R-068` корректно: он покрывает исполнением существенные пункты
(мутация реального барьера, контрфакт на `origin/main`, battery), но не называет модель Fable
как требование §9 и не предъявляет пункт (в) "связность/висячие ссылки" отдельным пунктом.
Решение "узкий круг только по непокрытой дельте" не является лазейкой "проверим после merge",
если сохраняется заявленная граница: это разовый retro-круг из-за диспетчерской коллизии
живых reviewer-сессий, а не новая норма.

### Р-4 — deploy model

Факты по workflow подтверждены:

```text
deploy.yml:
on.push.branches = [main]
paths include crates/**, Cargo.*, Dockerfile, docker-compose.yml, .github/workflows/deploy.yml
paths exclude !crates/*/tests/**
concurrency.group = deploy-main
cancel-in-progress = false
permissions.actions = read

gh run list --workflow=deploy.yml:
2026-08-14T12:13Z workflow_dispatch success c6c62b8
2026-08-13T23:19Z push              failure d564617
2026-08-08T23:08Z workflow_dispatch success 650f22d
2026-08-07T15:21Z push              failure 710b1ad
2026-08-03T13:50Z workflow_dispatch success e8d039e
2026-08-03T09:03-09:16 push         failure x4
```

Диагноз "пуш, делающий CI зелёным вне кодовых путей, не обязан триггерить deploy.yml" подтверждён:
`scripts/tests/**` не входит в deploy paths, а CI запускается на каждый push в `main`.

Открытая founder-развилка `П-008` п.3 не пересечена: текущая норма §8 уже означает, что `main`
должен доехать до VPS; будущий catch-up может быть переопределён на promoted SHA, если founder
выберет явный promotion.

## Notes

**N-1 — `Резерв необязателен` оставляет измеренную гонку в нерезервированной форме.**
Документ честно называет это эшелонированием: барьер остаётся postfactum-бэкстопом. Но это
означает, что решение не устраняет класс для двух ролей, которые продолжают брать номер без
обёртки; оно уменьшает цену для compliant-пути. Это допустимо как NOTE, не как claim "гонка
закрыта полностью".

**N-2 — источник nonce и предел попыток пока только названы, не специфицированы.**
Замер `commit-tree` показывает: одинаковое сообщение даёт одинаковый SHA. Поэтому future RED
для `reserve_artifact_id.sh` обязан пиннить не только мутанта "SHA вершины", но и невозможность
повторить одно и то же reserve-сообщение в параллельном запуске. Пункт 4 говорит "попыток --
ограниченное число, исчерпание -- FAIL"; самого числа и поведения при N одновременных ролях
в decision-документе нет. Для плана это приемлемо, для milestone-спеки будет обязательным.

**N-3 — протухшие `refs/reserved/*` не ломают аллокатор, но имеют неограниченный operational tail.**
Пункт 6 сознательно выбирает "не удалять автоматически" и платит пропущенным номером. Это
согласуется с §12 ("разрывы не занимаются"), но `ls-remote 'refs/reserved/<класс>-*'` в будущей
обёртке будет линейно зависеть от числа протухших резервов, а документ предъявляет цену только
на короткой пробе. Не блокер решения, но это должно быть явно проверено в acceptance будущего
milestone'а, иначе refs-reserved может стать скрытой свалкой.

**N-4 — ручная норма Р-4 остаётся cognitive-only до catch-up-механизма.**
Критерий "агент, чей коммит сделал CI зелёным" исполним через `gh run list`/run watch по head SHA,
и с `concurrency: deploy-main` прямого конфликта нет. Но красное состояние "main зелёный в CI,
но не доехал до VPS" сегодня наблюдается только агентом close-out'а; постоянного watcher'а нет.
Документ это признаёт и назначает milestone catch-up. До него правило не механический барьер.

## Done Block

```text
$ git show --stat --oneline HEAD
b7785c2 docs(plans): процессные развязки 14.08 -- резерв номеров CAS-пушем, узкий ретро-§9, добор деплоя [architect-clone]
 docs/plans/process-decisions-2026-08-14.md | 344 +++++++++++++++++++++++++++++

$ git diff --name-status origin/main...HEAD
A       docs/plans/process-decisions-2026-08-14.md

$ EVENT_NAME=push PUSH_BEFORE=$(git rev-parse origin/main) bash scripts/check_docs_freeze.sh; echo exit=$?
exit=0

$ gh run list --workflow=deploy.yml --limit 20 ...
2026-08-14T12:13:25Z workflow_dispatch completed success c6c62b8
2026-08-13T23:19:59Z push              completed failure d564617
2026-08-08T23:08:43Z workflow_dispatch completed success 650f22d
2026-08-07T15:21:16Z push              completed failure 710b1ad
2026-08-03T09:03-09:16Z push           completed failure x4

$ local git-race sandbox
E1: different SHA -> exit 0 / exit 1
E2: same SHA -> exit 0 / exit 0 Everything up-to-date
E3: same SHA + --force-with-lease -> exit 0 / exit 0 Everything up-to-date
E5: simultaneous different SHA -> exit 0 / exit 1 cannot lock ref
E6: delete refs -> exit 0, no reserved refs remaining

$ local commit-tree uniqueness probe
unique_messages total=100 unique=100 duplicates=0
same_message_equal=0  # test exit 0 means SHA1 == SHA2 for identical message
```

