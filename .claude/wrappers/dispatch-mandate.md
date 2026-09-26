# DISPATCH MANDATE — обязательный контракт каждого pi-агента hft-platform

Ты работаешь во fresh-worktree, который подготовила обвязка (идентичность роли уже
установлена). Правила ниже — BINDING; полные версии в `.claude/rules/` (обвязка
запускает pi без автозагрузки файлов — этот мандат и есть твой контракт).

## SACRED — НЕ ТРОГАТЬ (нарушение = авто-reject reviewer'ом)

- `*/tests/**` и inline `#[cfg(test)]`-модули с пометкой SACRED — тесты пишет ТОЛЬКО
  architect; тест = спецификация. Тест кажется ошибочным → СТОП + блок
  `!!! SCOPE VIOLATION REQUEST !!!` (agent/task/файл/что/почему) в финальном ответе.
- `crates/contracts/**` (T1 — только contract-RFC), `crates/journal/**` (DET-I-1),
  `crates/risk/**`, `crates/killswitch/**`, `scripts/verify_*.sh`, `.claude/**`,
  `milestones/**`, `docs/**` — вне твоей зоны, если milestone-задача явно не дала carve-out.
- Пиши ТОЛЬКО в зону своей роли (scope-guard: квант-агенты — `crates/signals/`+`research/`;
  engine-dev — `crates/{journal,book,oms,sim,runner,alpha,portfolio,strategy}/src`;
  venue-dev — `crates/venue-*/src`; research-dev — `crates/research-cli/src`).

## TDD: RED → GREEN

Реализуешь до зелёного СУЩЕСТВУЮЩИЕ RED-тесты. Тест, зелёный против no-op заглушки, —
дефект. Нет RED-теста и нет FAIL-скрипта → НЕ реализуй, подними вопрос.

## Гейт перед «готово» (все команды — сырой вывод в Done Block)

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings   # 0 warnings
cargo test --workspace                                   # 0 FAILED
bash scripts/verify_M-NN.sh; echo "exit=$?"              # если у milestone есть гейт
```

## Коммиты

- Атомарно: одна задача = ≥1 коммит, `type(M-NN): task #k — <суть> [<твоя-роль>]`.
  Бандл на 5 задач = авто-reject. Коммить только при зелёном clippy.
- **НЕ пушь.** Push делает reviewer после APPROVED (post-merge деплой-гейт — его зона).
  Твои коммиты остаются на сессионной ветке worktree — обвязка сохранит её и напечатает
  путь для следующего в цепочке.

## Done Block (обязателен в финальном ответе; пересказ без сырого stdout = провал)

```
## Done Block
$ git status --porcelain        → пусто
$ git log --oneline <N последних твоих коммитов>
$ cargo test -p <крейты> | tail  → сырой хвост
$ cargo clippy ... | tail        → сырой хвост
$ bash scripts/verify_M-NN.sh; echo "exit=$?"  → VERDICT + exit
```

После Done Block — краткий Handoff: §B что сделал · §C артефакты/exit-коды ·
§E риски/что осталось. Секреты в вывод не вставлять.

## STOP-правила

СТОП + доклад (не импровизация), если: нужного файла/milestone нет в чекауте (возможно,
не та ветка — сообщи); тест противоречит спеке; нужен файл вне зоны; verify-скрипт
выглядит багованным (не обходи его — доложи).
