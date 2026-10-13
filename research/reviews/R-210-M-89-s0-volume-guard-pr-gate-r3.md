<!-- GATE-META
milestone: M-89
audited_repo: a3ka/hft-platform
audited_base: 3809085c4053c6e031ecebfe55bcdfc2745aeaac
audited_head: 58c766ae84183e08ae5ab0fc19d8584945477475
verdict: APPROVE
-->

# R-210 — M-89 «остаток S0: объём одной выдачи структурно + видно снаружи», PR-гейт, круг 3

**Вердикт: APPROVED.** Все три блокирующих условия `R-209` §7 (п. 1 `Б-1`, п. 2 `Н-1`,
п. 3 `Н-2`) закрыты, и каждое я предъявил СВОЕЙ мутацией, а не отчётом стороны. Приёмка
зелена на моём чистом чекауте (83 PASS / 0 FAIL / 2 названных SKIP, `exit=0`), корпус
1177/0 в 268 блоках, merge-preview против `origin/main` — PASS.

Найдено НОВОЕ, не блокирующее: **утверждение architect'а о коде, вынесенное мне на
суждение, ложно по замеру** (§3). Оно не меняет поведения поставки — инвариант держат
другие гарды, — но меняет то, что о поставке сказано. Заводится `TD-226`.

## 0. Что проверено и чем

| блок | исход |
|---|---|
| Block-scope | `git diff --name-only 3809085 HEAD` — 38 файлов; `crates/{risk,killswitch,oms,contracts,venue-*,book,recorder}/**` НЕ тронуты (грепом по списку — пусто). RISK-BLOCK (`gates.md` §5) неприменим, ордерного пути нет |
| Block-C | `crates/contracts/**` в дифе отсутствует ⇒ contract-RFC не требуется; `SCHEMA_VERSION` в дифе `crates/journal/src/` не менялся |
| sacred-зоны | `*/tests/**`, `scripts/verify_M-89.sh`, `scripts/tests/red_verify_M-89_*.sh` правил ТОЛЬКО architect (`git log --format='%h|%s' 3809085..HEAD -- '*/tests/*'` — все с меткой `[architect]`, кроме merge-коммита синхронизации `5969486` и двух безметочных, по содержанию тоже architect'ских). Ни одной dev-правки теста |
| Done Block | прогнан МНОЙ заново на чистом чекауте (§5), не принят пересказом tester'а |
| мутационный контроль | проведён МНОЙ по всем трём условиям (§1.1, §2, §3) — сырой вывод, не рассуждение |
| FA (M-66) | живые на `58c766a`: `JR-I-2` (`docs/fa/journal.md:112`), `JR-I-11` (`:133`), `OPS-I-8` (`docs/fa/ops.md:476`), `OPS-I-10` (`:478`), `VB-I-2` (`docs/fa/viz-backend.md:205`), `VB-I-10` (`:213`), `VB-I-11` (`:214`) |
| ярус C | `TECH-DEBT.md` грепом по `TD-219`/`TD-220`/`TD-221`/`TD-224`/`TD-225`/`TD-124`; `PROJECT-STATE.md` грепом по `M-87`/`M-89`. Оба читаны на `origin/main` (`git show origin/main:<файл>`) — на ветке они отстают |
| ярус S | `gh run list --branch main` · `gh run list --workflow=deploy.yml` · `ssh rev-parse HEAD` + контейнеры + heartbeat + отсутствие watchdog на VPS — §4 |
| §8 strict:false | main поверх базы добавил РОВНО один файл — `docs/SESSION-HANDOFF.md` (`git diff --name-only $(git merge-base HEAD origin/main) origin/main`). Кода в нём нет ⇒ дерево слияния по Rust/scripts ТОЖДЕСТВЕННО ветке, и мой зелёный прогон на ветке ЕСТЬ прогон на дереве слияния. Документную половину закрывает `verify_design_claims.sh --merge-preview origin/main` — PASS |

## 1. Условие 1 (`R-209` §7 п. 1, `Б-1`) — ЗАКРЫТО, предъявлено моей мутацией

`c4533e3` перевёл оракул `v` (`crates/gateway-serve/tests/red_m89_read_volume_truth.rs`) с
процессного `serving_counters()` на ручку ЭКЗЕМПЛЯРА `Server::counters_handle()`
(`:258-262`, `:281`, `:304`). Эталон независим: `replica()` считает дельту тем же
читателем журнала, минуя транспорт, — зависимого эталона (`testing.md`) здесь нет.

### 1.1. Мутация (сырой вывод, прогон reviewer'а)

Три точки per-pump push возвращены к `add_journal_payload_bytes_pub(...)` — воспроизведён
РОВНО дефект `R-208` Б-1:

```
$ git diff --stat
 crates/gateway-serve/src/lib.rs | 12 +++---------
 1 file changed, 3 insertions(+), 9 deletions(-)

$ grep -n 'add_journal_payload_bytes_pub(pump_stats' crates/gateway-serve/src/lib.rs
1832:                            metrics::add_journal_payload_bytes_pub(pump_stats.payload_bytes_read);
2178:                metrics::add_journal_payload_bytes_pub(pump_stats.payload_bytes_read);
2459:                            metrics::add_journal_payload_bytes_pub(pump_stats.payload_bytes_read);

$ cargo test -p gateway-serve --all-features --test red_m89_read_volume_truth --test red_m89_counters_instance 2>&1 | grep -E "^test |^test result|panicked|I-4 /"
test i2_instance_handle_is_fed_by_the_real_serving_path ... ok
test i1_requests_to_one_server_do_not_move_counters_of_another ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.55s
test v_counter_includes_first_pump_and_equals_the_reader_delta ... FAILED
thread 'v_counter_includes_first_pump_and_equals_the_reader_delta' panicked at crates/gateway-serve/tests/red_m89_read_volume_truth.rs:338:5:
I-4 / задача 15 M-87: счётчик выдачи объявил 154563 Б, а обслуживание обязано было прочитать слепок (154563 Б) И хвост (24462 Б) = 179025 Б. ...
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.16s

$ git status --porcelain   # после восстановления
(пусто)
$ git diff --stat
(пусто)
```

Против `R-209` §2.1 («3 зелёных из 3») теперь `v` КРАСЕН на том же мутанте, и сообщение
называет ровно расхождение. Условие выполнено.

**Остаточное, не блокирующее:** `i1`/`i2` на этом мутанте остаются зелёными — то есть имя
`i2_instance_handle_is_fed_by_the_real_serving_path` по-прежнему обещает больше, чем тест
проверяет (он удовлетворяется одним `resume`, не требуя, чтобы байты pump'ов дошли до
ручки). Architect назвал это сам в §D handoff'а. Инвариант закрыт (`v`), но имя оракула
лжёт — это входит в `TD-226`.

## 2. Условие 2 (`R-209` §7 п. 2, `Н-1`) — ЗАКРЫТО, предъявлено моим прогоном

`c807f48` добавил в `scripts/verify_M-89.sh:344-348` сравнение НОМЕРОВ строк
(присваивание против строки расписания) и пробу `p10-cron-order`
(`scripts/tests/red_verify_M-89_scan.sh:131`).

```
$ grep -nE '^WATCHDOG_SERVING_HEARTBEAT_PATH=|scripts/watchdog_cron\.sh' deploy/cron.d/watchdog
33:WATCHDOG_SERVING_HEARTBEAT_PATH=/var/lib/docker/volumes/hft-platform_gateway-state/_data/gateway-serve.heartbeat
34:*/5 * * * * root flock -n /var/lock/hft-watchdog.lock /root/hft-platform/scripts/watchdog_cron.sh

$ VERIFY_M89_MODE=task11 VERIFY_M89_COMPOSE=docker-compose.yml VERIFY_M89_CRON=deploy/cron.d/watchdog bash scripts/verify_M-89.sh
PASS  task11: композиция путей сходится: /var/lib/docker/volumes/hft-platform_gateway-state/_data/gateway-serve.heartbeat (compose ⇒ хост) == cron; файл, не каталог
VERDICT: PASS
exit=0

# МУТАЦИЯ: копия с переставленными строками 33↔34 (присваивание НИЖЕ расписания)
$ VERIFY_M89_MODE=task11 ... VERIFY_M89_CRON=/tmp/m89-cron-swapped bash scripts/verify_M-89.sh
FAIL  task11: в /tmp/m89-cron-swapped WATCHDOG_SERVING_HEARTBEAT_PATH= стоит НИЖЕ строки расписания — cron.d применяет присваивание только к строкам после него; ops-watchdog его не увидит
VERDICT: FAIL (провалов: 1)
exit=1
```

`R-209` §3 замерил на этой же переставленной копии `exit=0`. Теперь `exit=1` с названной
причиной. Условие выполнено: гейт судит ПОВЕДЕНИЕ cron.d, а не наличие текста.

## 3. Условие 3 (`R-209` §7 п. 3, `Н-2`) — ЗАКРЫТО; и НОВАЯ находка внутри него

`58c766a` несёт в теле сырой вывод мутационного контроля по 25 мутантам задач 1 и 10
(`M1`…`M15` с вариантами): у каждого — `git diff --stat`, точная команда `cargo test` с
фильтром по именам, строки `^test`/`^test result`, и `git checkout -- <файл>` +
`git status --porcelain` как доказательство побайтного возврата. Это сырой вывод, а не
пересказ, которым был `501ff12`. Четыре мутанта стыка, требуемые `A-042` §5, присутствуют
поимённо: (в) удалена → `n1`/`n3` FAILED (`M9a`); (в) пропущена при непринятом преемнике →
`n4` FAILED (`M9c`); ожидание переносится через стык → `n5` FAILED (`M9d`). Условие
выполнено.

**Пятый — (г) — НЕ сработал, и dev сказал это ПРЯМО** (раздел «НАХОДКА ОРАКУЛА» в теле
коммита): одиночная нейтрализация гарда (г) оставляет `n2` зелёным. Честность отчёта —
принимается без оговорок, это ровно то поведение, которого норма требует.

### 3.1. Объяснение architect'а проверено и оказалось ЛОЖНЫМ

§D handoff'а предъявил мне для суждения: «проверка левого края ДУБЛИРУЕТСЯ блоком ожидания
`header.first_seq` (`segments.rs` ~2327-2368); **удаление обоих роняет `n2`**. Оракул цел,
код избыточен, но безвреден».

Первая половина верна, вторая — нет. Замер:

```
# мутация 1 — гард (г) нейтрализован ОДИН (:2328 `if false && self.first_decoded_in_segment`)
$ cargo test -p journal --all-features --test red_m89_seek_junction -- n2_first_frame_of_successor_cut_is_refused_at_left_edge
test n2_first_frame_of_successor_cut_is_refused_at_left_edge ... ok
test result: ok. 1 passed; 0 failed; ...                      # воспроизведён M9b

# мутация 2 — СВЕРХУ: (а/б) больше НЕ предпочитает expected_first_seq
#   (:2350 `if let Some(first) = self.expected_first_seq.filter(|_| false)`)
#   то есть левый край не сторожит НИ ОДИН из двух блоков
$ git diff --stat
 crates/journal/src/segments.rs | 4 ++--
 1 file changed, 2 insertions(+), 2 deletions(-)
$ cargo test -p journal --all-features --test red_m89_seek_junction --test red_m89_seek_fallback_observed
test f1b_gap_mid_tail_yields_prefix_then_aborts ... ok
test f1_gap_at_after_plus_one_aborts_and_fallback_is_observable ... ok
test f2_cursor_beyond_journal_is_a_fallback ... ok
test f3_active_segment_seek_succeeds_with_zero_fallbacks ... ok
test f4_closed_segment_seek_succeeds_with_zero_fallbacks ... ok
test f5_compacted_segment_is_a_named_limit_not_a_fallback ... ok
test result: ok. 6 passed; 0 failed; ...
test n2_first_frame_of_successor_cut_is_refused_at_left_edge ... ok          <<< ВСЁ РАВНО ЗЕЛЁН
test n1_removed_middle_segment_is_refused_at_seg0_right_edge ... ok
test n3_removed_middle_segment_after_zst_seg0_is_refused_too ... ok
test n4_truncated_tail_of_accepted_before_excluded_is_refused ... ok
test n5_positive_control_legit_filtered_projection_passes_with_all_as_pair ... FAILED
test result: FAILED. 4 passed; 1 failed; ...
$ git status --porcelain   # после восстановления
(пусто)
```

`n2` зелён даже когда левый край не сторожит НИКТО. Причина видна в раскладке `n2`
(`A-042` §5): seg0 = 0..86, `after=82`, seg1 объявляет `first_seq=87`, но кадр 87 вырезан.
Последний выданный из seg0 — 86, следующий декодированный — 88. То есть дыра видна ПРОСТОЙ
проверкой `prev + 1` (гард «б»), и `header.first_seq` для её обнаружения не нужен вовсе.
**`n2` пиннит гард (б), а не (г)** — атрибуция в `A-042` §5 («дыру видит только левый край»)
неверна. Упавший `n5` — это другой мутант (`M9d`, «перенос ожидания через стык»), и он
подтверждает, что предпочтение `expected_first_seq` в (а/б) пиннится именно `n5`.

### 3.2. Почему это НЕ блокер — и что остаётся долгом

Поведение поставки не страдает, и это проверено, а не предположено:

- **Инвариант `JR-I-2` на левом крае ДЕРЖИТСЯ.** Любой разрыв на стыке даёт
  `Err(InvalidData)` минимум от одного живого гарда: `n1`…`n5` + `f1`/`f1b` зелены на
  `58c766a`, и каждый из них краснеет на своём мутанте (`M8a`, `M8b`, `M9a`, `M9c`, `M9d`
  — сырой вывод в теле `58c766a`, я его воспроизвёл выборочно выше).
- **Гард (г) fail-closed-избыточен.** Он способен только ДОБАВИТЬ `Err` там, где (а/б) уже
  даёт `Err`; пропустить дыру он не может. Замер, показывающий это: он стоит ПОСЛЕ фильтра
  `if ev.seq <= after { continue; }` (`segments.rs:2299-2303`) и потому видит первое
  ВЫДАННОЕ событие, а не первое декодированное.
- **Зона `stream()`/`stream_from` не тронута** — вопреки риску, который я проверял отдельно:
  `expected_first_seq` выставляется только под `if self.containing_seg_idx.is_some() &&
  self.after_seq.is_some()` (`:2097`). Запрет §10 («менять семантику `stream`/`stream_from`/
  `read_all`») соблюдён, отложенное в `M-81` (`A-042` §6) не втянуто.

Остаётся ровно то, что нельзя приземлить молча: **блок кода на sacred-пути чтения журнала,
который не пиннит ни один оракул, и три текста, описывающие его иначе, чем он работает.**
`testing.md` называет это прямо: «Не упал → оракул ничего не пиннит». Заводится `TD-226`
(MINOR, зона architect — `crates/journal/src/**` в части гарда и `*/tests/**` + `A-042` §5
в части атрибуции).

## 4. Условие 4 (`R-209` §7 п. 4-5, `Н-3`) — закрыто; два НОВЫХ дрейфа комментария

Принято: `03f1ba4` снял `journal::tail_bytes_for_dir` (`grep -rn 'tail_bytes_for_dir'
crates/ --include=*.rs` даёт только два комментария, определения и ре-экспорта нет);
`0c3cf02` привёл к коду два комментария `gateway-serve` (`:1330-1336`, `:2271`); шапка
`red_m89_read_volume_truth.rs:7` помечена «(история, до `M-89`)».

**NOTE (не блокер, тот же класс `Н-3`, подметается любым следующим касанием):**

1. `crates/gateway-serve/src/lib.rs:1830-1831` — «тест `v` (process-global) остаётся
   зелёным». После `c4533e3` `v` читает ручку ЭКЗЕМПЛЯРА; скобка стала ложной РОВНО тем
   коммитом, который закрывал условие 1.
2. `crates/gateway/src/lib.rs:5074`, `:5084` — ссылаются на `journal::tail_bytes_for_dir`,
   удалённый `03f1ba4`. Текст прошедшего времени и утверждение «здесь вызова НЕТ» верны по
   смыслу, но символа в дереве больше нет: читатель грепнет и не найдёт.
3. `crates/journal/src/segments.rs:2321-2326` — комментарий гарда (г) говорит «на ПЕРВОМ
   ДЕКОДИРОВАННОМ (НЕ yield'нутом)» и «используем `events_scanned`». Код использует
   `first_decoded_in_segment` и стоит ПОСЛЕ фильтра `seq <= after` — то есть проверяет
   первое ВЫДАННОЕ. Входит в `TD-226`.

## 5. Состояние мира (ярус S) — проверено, блокеров нет

```
$ gh run list --branch main --limit 5
completed  success  Deploy to VPS  main  workflow_run  36711574207  12s  2026-09-30T11:56:00Z
completed  success  Merge pull request #283 ...  CI  main  push  36710465989  10m44s  2026-09-30T11:45:12Z

$ ssh ... 'cd /root/hft-platform && git rev-parse --short HEAD; docker ps --format "{{.Names}} {{.Status}}"; cat .../recorder.heartbeat; date +%s'
843894f
hft-gateway-serve Up 4 days (healthy)
hft-recorder Up 4 days (healthy)
{"events":43073671,...,"next_seq":750297280,"segment_index":887,"ts_wall_ms":1790806326573,"writable":true}1790806331
```

- heartbeat свежий: `1790806331 − 1790806326.573 ≈ 4.4 с`; журнал растёт (`segment_index` 887).
- **Отставание прода БЕЗВРЕДНО и это замер, а не допущение:** прод `843894f`, 36 коммитов
  позади `origin/main`; `git diff --name-only 843894f origin/main | grep -E 'crates/.*\.rs$' |
  grep -v '/tests/'` — **пусто**. Рантайм-исходников в отставании нет, `deploy.yml`
  фильтрован по `paths:` и законно не срабатывал. **После merge M-89 это перестаёт быть
  верным** — диф несёт `crates/{journal,gateway,gateway-serve,ops}/src/**` и
  `docker-compose.yml`, значит деплой ОБЯЗАН сработать; §8-гейт снимается отдельно.
- **Задача 11 на VPS не установлена** (ожидаемо, §11 п. 5 — ручной шаг founder ★):
  `/etc/cron.d/hft-watchdog` — нет, `/var/lib/hft/watchdog.last-success` — нет.
  ⇒ тревога `OPS-I-8` на выдачу ПОСТРОЕНА и проверена оракулами (`g1`…`g5`), но на проде
  МОЛЧИТ. Это `TD-220` ЧАСТИЧНО, не полностью, и так и будет записано.
- **Задача 12 (прод-замер ДО/ПОСЛЕ)** физически не снимается до деплоя — «ПОСЛЕ» ещё не
  существует. Снимается на §8-гейте по форме `docs/plans/m89-warm-resume-measure-2026-09-27.md`.

## 6. Done Block — прогон reviewer'а на ЧИСТОМ чекауте

Дерево `/tmp/hft-reviewer-m89-r3`, `git worktree add --detach 58c766a`.

```
$ git status --porcelain
(пусто)

$ git log -1 --oneline
58c766a chore(M-89): tasks #1, #10 — мутационный контроль СЫРЫМ выводом (R-209 Н-2; заменяет пересказ 501ff12) [engine-dev]

$ cargo build --workspace --tests 2>&1 | tail -2
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 32.92s
BUILD_EXIT=0

$ cargo fmt --all -- --check; echo fmt_exit=$?
fmt_exit=0

$ cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -2
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.60s
clippy_exit=0

$ cargo test --all --all-features 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" блоков="NR}'
passed=1177 failed=0 блоков=268

$ grep -E '^failures:|FAILED|panicked at' <полный лог>
(пусто)
TEST_EXIT=0

$ bash scripts/verify_M-89.sh; echo VERIFY_EXIT=$?
...
SKIP  task11: установка deploy/cron.d/watchdog в /etc/cron.d и сборка ops-watchdog на VPS — ручной шаг founder ★
SKIP  task12: rchar/время первой выдачи новой подписки на проде ДО/ПОСЛЕ ... — снимает reviewer на §8-гейте
VERDICT: PASS
VERIFY_EXIT=0
$ grep -cE '^PASS' /tmp/m89-rev-verify.log
83

$ bash scripts/verify_design_claims.sh --merge-preview origin/main; echo exit=$?
PASS  [H-FACTS-SHA] маркеров `FACTS:` проверено 22 — все ревизии сбора существуют и входят в историю HEAD/MERGE_HEAD
PASS  [6-RFC-SHA] SHA-подобных токенов: всего=38 проверено=38 — все входят в историю HEAD/MERGE_HEAD
VERDICT: PASS (0 нарушений)
exit=0
```

Оба `SKIP` — названные пределы поставки (§11 п. 5, п. 7), не замаскированные провалы.
Числа совпали с прогоном tester'а (`/tmp/m89-r3-verify2.log`) — но приняты они по МОЕМУ
прогону, а не по его.

**Предел этого Done Block назван честно, как и в `R-209`:** он предъявляет, что набор
ЗЕЛЁН. Что набор ЛОВИТ — предъявляют мутации §1.1, §2, §3.1, и в круге 3 они КРАСНЫЕ там,
где обязаны быть красными. Ровно поэтому вердикт сменился с REJECT на APPROVE, а не потому,
что приёмка позеленела: она была зелена и на `25c9ff4`.

## 7. Что записывается в долг

- **`TD-226`** (новая, MINOR) — оракулы `M-89`, чьё ИМЯ или АТРИБУЦИЯ обещают больше, чем
  они пиннят: (а) гард (г) `segments.rs:2327-2342` не пиннится ни одним оракулом — ни
  одиночная нейтрализация, ни снятие предпочтения `expected_first_seq` в (а/б) не роняют
  `n2`, потому что раскладка `n2` ловится простым `prev + 1`; (б) `A-042` §5 приписывает
  `n2` проверку левого края — атрибуция неверна; (в) комментарий гарда (г) утверждает
  «первое ДЕКОДИРОВАННОЕ» и `events_scanned`, а код стоит после фильтра `seq <= after` и
  видит первое ВЫДАННОЕ; (г) `i2_instance_handle_is_fed_by_the_real_serving_path` зелен,
  когда байты pump'ов идут мимо ручки. Зона — architect (sacred).
- **`TD-219`** — закрывается: структурная граница объёма одной выдачи поставлена
  (`I-3`, `s`/`s1`/`s2`, бюджет-плацебо удалён).
- **`TD-220`** — **ЧАСТИЧНО**: счётчики наружу процесса построены и проверены (`h1`, `v`,
  `i1`/`i2`, `g1`…`g5`), но cron-watchdog на VPS НЕ установлен — тревога молчит.
- **`TD-224`** — закрывается: счётчики по экземпляру построены (`Server::counters_handle`).
- **`TD-225`** — закрывается: сторож сериализации механизирован (`red_m89_serial_guard`,
  `m1`…`m6`, шаг `verify_M-89.sh`).
- **`TD-221`** — остаётся MINOR без изменений (правдивость обоснования остатка барьером
  не проверяется; шаг `task-status` зелен на 1 открытой задаче).
- **`TD-124`** — работы не требует: карточка УЖЕ снята (грепом на `origin/main`
  заводящей строки ``| **TD-124** |`` нет, упоминание — единственное, в тексте `TECH-DEBT.md:2583`
  как «снятая»). Просьба `R-209`/`gates.md` §11 «закрыть при ближайшем close-out» исполнена
  раньше меня; заново я её не «закрываю».

## 8. Прежние MINOR/NOTE, перенесённые без изменений

- `Н-4`: восемь коммитов диапазона без ролевой метки (`c505e81`, `e457598`, `9faa54d`,
  `56b858e`, `cf951c4`, `32ceb1b`, `cec5fda`, `9aa3bfd`) — все шесть коммитов круга 3
  метку несут; не блокер.
- `Н-6`: `docs/DESIGN.md` вне §12 Allowed paths (правка — счётчик покрытия `JR-I` 7 → 8,
  фактически верный, покрыт `C-264`); зона названа, блокером не считаю.
- `ALLOW-SUBJECT-CHANGE` в `c807f48` — законный след: правка предмета после вердикта
  гейта, причина названа в теле.

**Маршрут:** merge через PR → §8-гейт (CI + Deploy + ssh + задача 12) → close-out
(`PROJECT-STATE.md`, `TECH-DEBT.md`) → architect (перенос спеки и гейта `M-89`, а также
`M-87`, в `docs/archive/`).
