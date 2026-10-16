<!-- GATE-META
milestone: TD-236
audited_repo: a3ka/hft-platform
audited_base: 403c29ed1f9ca4cfaf94bd0c5a4f33de6784928f
audited_head: a77a9ab7f154bde1e665e948265031e408792049
verdict: APPROVE
-->

# R-220 — PR-гейт `TD-236` rev3: третий круг по правке комментариев `gateway-serve`

**Роль:** reviewer (PR-time гейт, `gates.md` §4 — UNCONDITIONAL: диапазон трогает
`crates/gateway-serve/src/**`).
**Предмет:** `origin/fix/td-236-comments` → `main`, PR #302. Третий круг после
`R-218` (REJECT) и `R-219` (REJECT, Б-3).
**База аудита:** `403c29e` (= `origin/main`, ветка с ним синхронизирована) · **вершина:** `a77a9ab`.

## §0 — Вершина взята КОМАНДОЙ; ветка НЕ отстаёт

```
$ git fetch origin && git rev-parse origin/fix/td-236-comments
a77a9ab7f154bde1e665e948265031e408792049
$ git merge-base --is-ancestor a77a9ab origin/fix/td-236-comments && echo ancestor-ok
ancestor-ok
$ git log --oneline origin/fix/td-236-comments..origin/main | wc -l
0
$ git log --oneline origin/main..origin/fix/td-236-comments
a77a9ab fix(TD-236): r3 — комментарий успеха legacy-пути без пересказа чужого комментария и без номеров строк (R-219 Б-3) [engine-dev]
4f189b4 Merge remote-tracking branch 'origin/main' into HEAD
38e634c docs(review): R-219 — PR-гейт TD-236 rev2 REJECT (...) [reviewer]
27df701 fix(TD-236): comment-only corrections (R-218) — ...
1869f7f Merge remote-tracking branch 'origin/main' into fix/td-236-comments
d217829 docs(review): R-218 — PR-гейт TD-236 REJECT (...) [reviewer]
05ed925 fix(TD-236): comment-only corrections — OPS-I-11, дельты, имена функций, M-89 архивные ссылки [engine-dev]
```

SHA мандата (`a77a9ab`) = вершина. Ветка содержит `origin/main` целиком ⇒ дерево слияния
совпадает с деревом вершины; условие 2 `R-219` §9 (синхронизация) ВЫПОЛНЕНО.

## §1 — Block-scope: ЧИСТО, comment-only подтверждено своим прогоном

```
$ git diff --numstat origin/main...origin/fix/td-236-comments
19	15	crates/gateway-serve/src/lib.rs
1	2	deploy/cron.d/watchdog
1	2	docker-compose.yml
299	0	research/reviews/R-218-...
338	0	research/reviews/R-219-...

$ git diff origin/main...HEAD -- crates/gateway-serve/src/lib.rs deploy docker-compose.yml \
    | grep -E "^[+-]" | grep -vE "^[+-]{3}" | grep -vcE "^[+-]\s*(//|#)"
0
$ git diff a77a9ab~1 a77a9ab | grep -E "^[+-]" | grep -vE "^[+-]{3}" | grep -vcE "^[+-]\s*//"
0
```

Ноль не-комментарных строк во ВСЁМ диапазоне PR (не только в r3). Зоны: `crates/gateway-serve/src/**`,
`deploy/**`, корневой `docker-compose.yml` — engine-dev (`scope-guard.md`); правки в compose/cron —
только комментарии. Sacred (`*/tests/**`, `crates/contracts/**`, `scripts/{verify_*,check_*}.sh`)
не тронут. Коммит r3 атомарный, subject ссылается на `TD-236` и `R-219 Б-3`, метка
`[engine-dev]`, co-author-трейлера нет.

## §2 — Block-C: НЕПРИМЕНИМ (`crates/contracts/**` не тронут)

## §3 — Block-risk: НЕПРИМЕНИМ

`gateway-serve` — read-only WS-транспорт (`GS-I-1`), не `risk|killswitch|oms|venue-*|contracts`;
risk-critic не требуется (`gates.md` §5).

## §4 — Предъявление FA (M-66)

Тронут `crates/gateway-serve/**` ⇒ FA `docs/fa/viz-backend.md`, префикс `GS`
(`check_review_fa.sh:195-199`). Живой инвариант по предмету: **`GS-I-4`**
(`docs/fa/viz-backend.md:289`, внутри `VB-I-6`) — `gateway-serve` сериализует
`Snapshot`/`Frame` целиком; ровно на этом пути стоит правимый комментарий (сдача снимка в
сокет → инкремент успеха). Также `OPS-I-11` (`docs/fa/ops.md`, в `main`) — правило тишины
выдачи, на которое ссылается комментарий `:2086-2088`. `crates/gateway-serve` своей FA не
имеет (`M-87` §17) — долг назван прежде, здесь не переоткрываю.

## §5 — Б-3 `R-219` СНЯТ: каждое утверждение нового комментария проверено против кода

Новый текст `crates/gateway-serve/src/lib.rs:2374-2383`. Почленно:

| утверждение | проверка | итог |
|---|---|---|
| v1 ADD и SWITCH: `inc_successes` сразу после `sink.send(...).await`, вне `spawn_blocking` | `:1475-1477` (SWITCH), `:1622-1625` (ADD); замыкания `spawn_blocking` `:1376`/`:1497` закрыты раньше | ВЕРНО |
| внутри `spawn_blocking` v1 после `resume` растёт ДРУГОЙ счётчик — `add_journal_payload_bytes` | `:1405`, `:1555` | ВЕРНО |
| запрет повторного учёта в точке отправки в v1 относится ТОЛЬКО к счётчику байт | `sed -n '1615,1621p'` — субъект «счётчик прочитанных байт», слова `successes` нет | ВЕРНО — ложная атрибуция Б-3 снята |
| удаление строки «как дубля» возвращает `TD-228` | `TD-228` (`TECH-DEBT.md:201`) — ноль успехов legacy при реальном трафике | ВЕРНО |
| `send`→`Ok` = кадр сдан в сокет ОС, не «клиент прочёл» | `SinkExt::send` | ВЕРНО (TD-236 (б)) |
| признак доставки — `frames_received` у зонда | `crates/gateway-serve/src/bin/wsprobe.rs:337,474` | ВЕРНО |

Номера строк из комментария успеха и отказов убраны — источник Б-1/Б-3 двух прошлых кругов.
Оставшиеся адреса в комментарии попытки (`:2081-2082`: `:1172`, `:1293`) сверены на
вершине: `grep -n inc_attempts` → `1172`, `1293`, `2089` — точны.

Пункты карточки `TD-236`: (а) ложная история — снята (`05ed925`); (б) переобещание
доставки — снято; (в) сдвинутые ссылки — сняты/исправлены; (г) правило тишины на дельтах —
`OPS-I-11`, `Δ`-форма совпадает с `crates/ops/src/watchdog.rs:633-635`
(`d_attempts`, `d_refusals_supported`, `d_successes`). Все четыре закрыты.

## §6 — N-1 (замечание, НЕ блокер): «единственный учёт успеха legacy-сессии» — абсолют шире кода

**Место:** `:2379`: «Этот `inc_successes` — единственный учёт успеха legacy-сессии».

**Факт по коду.** Legacy-сессия после снимка принимает v1-сообщения:
`:2478-2490` → `Server::parse_and_dispatch_v1_message` → `:534` `handle_v1_message` с
`v1_session_inner.policy: None` (`:2441`). Клиент, приславший `subscribe` после grace-окна
(`CT-RFC-09` §2.8, сценарий M-65 — описан в doc-comment `:516-521`), проходит ветку без
политики (`:1293` `inc_attempts`) и ADD (`:1625` `inc_successes`). То есть в ОДНОМ
соединении legacy-сессии успех может учитываться не только здесь.

**Почему НЕ блокер — по тому же критерию, по которому `R-219` Б-3 был блокером
(место и последствие), а не по снисхождению к третьему кругу:**

1. **Есть верное прочтение.** «Успех legacy-сессии» = успех legacy-выдачи снимка; v1-подписки
   внутри соединения — успехи v1-подписок, каждая считается в своей точке. В этом прочтении
   утверждение истинно; ложно оно только в прочтении «все успехи соединения».
2. **Действие, которое фраза мотивирует, верно.** Операционная часть — «не удалять как
   дубль, иначе `TD-228`» — истинна: для snapshot-only сессий (`wsprobe`, единственный
   реальный клиент) это единственный инкремент. Б-3 был блокером потому, что ложная
   атрибуция могла мотивировать удаление ВЕРНОГО кода; здесь обратное — неточность
   защищает верный код и ни к какому вредному изменению не толкает.
3. **Цена — интерпретация метрики, не поведение.** Максимум, к чему ведёт буквальное
   прочтение, — допущение «≤1 успех на legacy-соединение» при анализе сердцебиения; правило
   тишины (`OPS-I-11`) на дельтах от этого не зависит.

Новую карточку не завожу: это не долг поведения и не ложь об истории/контракте, а
неточность квантора с верным прочтением. Рекомендация для следующего касания строки
(формулировку выбирает исполнитель): «единственный учёт успеха legacy-СНИМКА; v1-подписки,
присланные в той же сессии после grace-окна, считаются на ADD/SWITCH v1-пути».

## §7 — Маршрут кругов (`gates.md` §0)

Это третий круг по предмету (`R-218`, `R-219`, `R-220`). Арбитр по §0 п.2 созывается для
разрешения тупика; тупика нет — круг закрывается APPROVE, спорных пунктов между сторонами не
осталось. `R-219` §7 вернул круг `architect`'у; исполнитель r3 — engine-dev (`[engine-dev]`
в subject), что допустимо: профиль разрешает architect'у диспетчеризовать dev на impl-правку
через founder'а. Класс «комментарий пересказывает чужой комментарий/номера строк» закрыт
по классу, а не по месту: r3 убрал номера строк и пересказ, а не поправил очередной адрес.

## §8 — Done Block (вершина `a77a9ab` = дерево слияния)

```
$ cargo fmt --all -- --check; echo fmt exit=$?
fmt exit=0
$ cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -1
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.74s
clippy exit=0
$ cargo test -p gateway-serve  → exit=0
gateway-serve passed=203 failed=0 (блоков: 46)
$ cargo test -p ops            → exit=0
ops passed=172 failed=0 (блоков: 19)
$ grep -c FAILED gs.log ops.log
ops.log:0
gs.log:0
$ bash scripts/check_archived_refs.sh
PASS  НОВЫХ висячих ссылок нет (проверено 68 артефактов)
PASS  базовая линия актуальна (унаследованных ссылок: 21)
VERDICT: PASS
archived exit=0

$ gh pr checks 302   (на a77a9ab, до записи вердикта)
fmt + clippy + test	pass	16m12s
fmt + clippy + test (ветка)	pass	10m21s
cargo audit	pass · Secret material	pass · Rollout composition	pass
```

Гейты зелёные ожидаемо — правка comment-only, компилятор к истинности комментария слеп;
§5/§6 получены ЧТЕНИЕМ против файла.

**Ярус C грепом (`reading-map.md` §2), предметы поиска:** `TECH-DEBT.md` — `TD-236`
(`:210`, карточка `:5556`), `TD-228` (`:201`, `:5151`), `TD-220` (`:195`); `PROJECT-STATE.md` —
`TD-236`, `M-91` (`:3028-3116`). Целиком не читал.

## §9 — ВЕРДИКТ

**APPROVE.** Условия `R-219` §9 выполнены: (1) Б-3 устранён — атрибуция запрета `:1615-1621`
счётчику байт исправлена, утверждение «v1 запрещает `successes` в точке отправки» из файла
ушло; (2) ветка синхронизирована с `origin/main`. Все четыре пункта `TD-236` закрыты (§5).
N-1 (§6) — замечание без карточки.

Реестры обновлены этим же PR (до merge, одним диапазоном): `TD-236` → ЗАКРЫТ в `TECH-DEBT.md`, отметка в `PROJECT-STATE.md` (раздел `M-91`), `TD-237` дополнен фактом снятия ссылок `OPS-I-8` из прод-кода.
