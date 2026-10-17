<!-- GATE-META
milestone: M-85
audited_repo: a3ka/hft-platform
audited_base: 95d2422cb175010fb6ba8f9302c8aa4a84bda71c
audited_head: 5e0b4dbb01abc2659b28505eab9e2864d7b044b6
verdict: APPROVE
-->

# R-183 — M-85 «кадр `frames_since` строится ОТ КНИГИ» — PR-гейт: **APPROVE**

Дата: 2026-09-17 UTC · Роль: reviewer · Ветка: `feat/M-85-frames-book-continuity`
База = `origin/main` (`95d2422`); ветка ровно на 15 коммитов впереди, `0` позади
(`git rev-list --left-right --count origin/main...HEAD` → `0 15`) ⇒ **дерево слияния
совпадает с веткой**, отдельный merge-preview по `gates.md` §8 не требуется и не
подменяется словом: расхождение замерено, а не предположено.

**Живые инварианты тронутого модуля (`crates/gateway`, `crates/gateway-serve` → `docs/fa/viz-backend.md`):**
`VB-I-2` («live == replay», `:199`) — предмет милестоуна; `VB-I-10` («bounded-window
snapshot: память `snapshot`/`frames_since` ограничена ОКНОМ, не длиной истории», `:207`) —
граница, за которую развязка не смела выйти.

---

## Block-scope — PASS

`git diff --stat 95d2422..HEAD` — десять файлов, каждый в разрешённой зоне
(`M-85` §2):

| файл | строк | зона |
|---|---|---|
| `crates/gateway/src/lib.rs` | +46 | engine-dev ✔ |
| `crates/gateway-serve/src/lib.rs` | +3/−1 | engine-dev, исключение §2 (док-комментарий, ОТДЕЛЬНЫЙ коммит, потолок 8 строк) ✔ |
| `crates/gateway/tests/red_m85_*.rs` | +695 | architect (sacred) ✔ |
| `milestones/M-85-*.md`, `scripts/verify_M-85.sh` | +470 | architect ✔ |
| `research/{critiques/C-223,C-224,C-225, arbitration/A-034}` | +842 | артефакты гейтов ✔ |

Вне зоны — НИЧЕГО: `crates/contracts`, `docs/rfc`, `crates/journal`, `crates/venue-*`
не тронуты (шаг `C` гейта, три проверки, все PASS). `TECH-DEBT.md`/`PROJECT-STATE.md`/
`docs/ROADMAP.md` (reviewer-owned) в диапазоне отсутствуют.

**Разделение ролей внутри диапазона проверено поимённо, а не по метке в subject'е:**
`git log --name-only 95d2422..HEAD -- 'crates/*/tests/*'` даёт ЧЕТЫРЕ коммита, и все
четыре — `[architect]` (`f0205be`, `302d6c9`, `2b6c5c4`, `c657bdc`). Оба коммита
`[engine-dev]` (`6f17d12`, `5e0b4db`) трогают ТОЛЬКО `*/src/**`. Sacred-зона оракулов
dev'ом не тронута — `scope-guard.md` §«Тесты — sacred» соблюдён.

## Block-C (contracts) — НЕ ПРИМЕНИМ, и это замерено

`git diff --name-only 95d2422..HEAD -- crates/contracts docs/rfc` — пусто. Форма T1 не
менялась, `SCHEMA_VERSION` не бампался, contract-RFC не требуется.

## Block-risk — НЕ ТРИГГЕРИТСЯ (`gates.md` §5)

Диапазон не трогает `crates/risk/**`, `crates/killswitch/**`, `crates/oms/**`,
`crates/venue-*/**`. `gateway`/`gateway-serve` — read-only консюмер журнала (`VB-I-3`),
order-egress отсутствует. Вердикт `risk-critic` не требуется; милестоун объявил это сам
(§7), и заявление подтверждено дифом, а не принято на слово.

## Block-DoneBlock — PASS, гейт прогнан РЕВЬЮЕРОМ заново

Отчёт tester'а принят НЕ пересказом: `scripts/verify_M-85.sh`, `cargo test --workspace`
и доказательство RED прогнаны в собственном чистом дереве `/tmp/hft-reviewer-m85`
(detached `5e0b4db`). Цифры совпали с заявленными (26 PASS/0 FAIL, 976/0) — сырой вывод
ниже.

## RED-first — предъявлен ПРЯМЫМ прогоном на пред-фиксной ревизии, а не мутацией

Ключевая проверка гейта: оракул обязан быть КРАСЕН против кода БЕЗ развязки. Вместо того
чтобы принять мутационный контроль dev'а/tester'а на веру (и вместо того, чтобы писать в
чужую зону самому), reviewer завёл второе дерево на `3b6a16c` — последнем коммите
architect'а, где оракулы УЖЕ есть, а `seed_book` ЕЩЁ нет
(`grep -c seed_book crates/gateway/src/lib.rs` → `0`):

```
test result: FAILED. 3 passed; 1 failed
m85_1 …: реплей 4 точки, клиент 3 + фантомный (…013, 0)
exit=101
```

Расхождение ПОБУКВЕННО совпало с тем, что заявляли спека §0.3, dev в теле `6f17d12` и
tester в мутационном прогоне. Три сторожа (`m85_2`, `m85_3`, `m85_6`) на той же ревизии
ЗЕЛЕНЫ ⇒ красное принадлежит предмету, а не фикстуре. **Это сильнее мутации:** мутация
проверяет «фикс — причина зелёного», прогон на пред-фиксной ревизии проверяет то же самое
БЕЗ единой записи в `crates/**/src/`.

## Существо развязки — прочитано, а не принято по Done Block'у

`seed_book` (`crates/gateway/src/lib.rs:1099-1134`) применяет к `self.book` ТОЛЬКО
`MdPayload::L2Snapshot`/`L2Delta`, отобранные `selector.matches`, и зовётся РОВНО из
одной точки — seed-ветки `reduce_event_stream` (`:2291-2300`), рядом с `seed_vwap`.
Четыре проверки, каждая — открытием кода:

1. **Фильтр тот же, что у `apply`.** `apply` (`:1259-1267`) отбирает события тем же
   `selector.matches(md)`; ранний выход `bucket_time_s` → `None` возможен только при
   `timeframe_ms <= 0` (`:1064-1071`), который отсекает `validate_selector`. Асимметрии
   «затравка применила то, чего не применил бы реплей» нет.
2. **Буфер наблюдений НЕ включается.** `capture_book_observation` в `seed_book` не
   зовётся; `set_capture_book_observations(true)` стоит ровно в двух местах (`:4281`,
   `:4353`) — оба на живом пути `LiveReducer`. Запрет `§2.1` («включать захват вне
   `LiveReducer::resume` нельзя — буфер `O(журнал)`») держится не прозой, а замером
   `m85_4` (аллокации: отношение при 4× истории — под потолком 2.0).
3. **Книга не участвует в оконной эвикции ни на одном пути.** `evict_window_state`
   (`:1003-1035`) чистит `heatmap/ohlcv/depth/vp/cvd`, `self.book` не трогает — значит
   затравка не создаёт состояния, которое реплей бы выбросил. `VB-I-10` не ослаблен.
4. **Публичная форма не тронута.** Шаг `СИГНАТУРА` гейта сравнивает `git show
   95d2422:…` с деревом по обеим функциям — PASS. Seek-вариант
   `frames_since_with_stats` не изменён и остаётся вне предмета (§2bis.2); шаг гейта
   наблюдает ОТСУТСТВИЕ его вызовов в `crates/*/src/**` — PASS.

## Атомарность коммитов — PASS

`6f17d12` — задача 1 (46 строк, `crates/gateway/src/lib.rs`); `5e0b4db` — задача 5
(4 строки, `crates/gateway-serve/src/lib.rs`), отдельным коммитом, как предписывает §2.
Оба тела ссылаются на `milestones/M-85-*.md §…`; `Co-Authored-By` отсутствует. Числа
`git show --numstat` соответствуют заявленному в телах (`branch-hygiene.md` §9).

## Цепочка гейтов — предъявлена ФАЙЛАМИ

| круг | артефакт | вердикт |
|---|---|---|
| plan-time 1 | `research/critiques/C-223-M-85-frames-book-continuity.md` | REJECT |
| plan-time 2 | `research/critiques/C-224-M-85-round2.md` | REJECT |
| арбитраж (`gates.md` §0, два REJECT подряд) | `research/arbitration/A-034-m85-frames-topology.md` | DECISION |
| plan-time 3 | `research/critiques/C-225-M-85-round3.md` | APPROVE (ограниченная проверка закрытия) |
| PR-time | **этот файл** | APPROVE |

---

## Находки — NOTE (ни одна не блокирует merge)

**N-1. Бухгалтерия спеки отстала от факта.** В `§Tasks` на `HEAD` задачи 1, 2, 2b, 4, 5
стоят `⏳ OPEN`, хотя все пять исполнены и предъявлены гейтом. Колонка `Status` — carve-out
dev-агента (`scope-guard.md` §Milestone-файлы); reviewer её не правит (`milestones/**`
закрыт мне безусловно). Долг бухгалтерии, не кода: спека, читаемая как «работа не
начата», — тот же класс ложного документа, что `TD-138`, только в обратную сторону.
Адресат — architect на close-out.

**N-2. Мутационный контроль tester'а сделан ЗАПИСЬЮ в `crates/gateway/src/lib.rs`.**
Tester назвал это сам (§E своего отчёта) и откатил полностью — `git status --porcelain`
пуст, строка `:2300` на месте, эффект на репозиторий ноль. Нарушением зоны я это не
числю: правки в истории нет, а альтернативой был бы вердикт `INVALID` на 12 команд из 13.
Но способ был лишним: **пред-фиксная ревизия `3b6a16c` даёт то же доказательство без
единой записи** — и именно им гейт закрыт выше. Для будущих прогонов это штатный путь, а
временная мутация в дереве read-only роли — нет.

**N-3. `GS-I-5`, названный новым док-комментарием, в `docs/fa/viz-backend.md` НЕ объявлен.**
`grep -cE '^\| GS-I-' docs/fa/viz-backend.md` → `0`. Инвариант живой (`milestones/M-28-gateway-serve.md:53`,
оракулы `crates/gateway-serve/tests/red_serve_passthrough.rs`), и сам FA числит этот
пробел долгом во врезке §5. Правка `5e0b4db` его не создаёт и не усугубляет — ссылка
корректна по существу. Фиксирую, чтобы пробел не растворился.

**N-4. Долг «`frames_since` остаётся дорогим» обещан спекой §6 и подлежит ЗАПИСИ.**
Путь читает журнал с головы и теперь на каждом затравочном `L2` ещё и применяет событие к
книге: работа линейна по ДЛИНЕ ИСТОРИИ (память — нет, `m85_4`). Это не регресс `M-85` —
свойство было и до него, — но спека прямо требует записать его долгом, а не выдать за
решённое. Карточку завожу close-out'ом; `TECH-DEBT.md` — моя зона.

**N-5. Наследуется от `C-225`:** ссылка на `A-028` живёт на невлитой
`docs/M-73-closeout-architect`. Вне предмета `M-85` (`A-034` §4), остаётся NOTE.

---

## Subject-lock `A-034` — открыт ОСМОТРОМ, а не обходом

`scripts/check_gate_meta.sh` в PR-режиме даёт одно красное, и оно законно:

```
FAIL  research/arbitration/A-034-…md: subject-lock — после проходного вердикта (DECISION)
      тронут класс «гейт»: scripts/verify_M-85.sh
```

Лок существует, чтобы заставить ПОСМОТРЕТЬ, что ветка сделала с гейтом после проходного
вердикта. Посмотрел:

```
$ git diff de43b61 3b6a16c -- scripts/verify_M-85.sh
-chk "! grep -rn 'frames_since_with_stats' crates/gateway-serve/src/ crates/recorder/src/ crates/ops/src/ …"
+chk "! grep -rn 'frames_since_with_stats' crates/*/src/ | grep -v '///' | grep -v 'pub fn …' | …"
```

Единственная правка — **расширение** SEEK-стража с трёх каталогов на весь `crates/*/src/`
(17 каталогов), предписанное `A-034` §7 п.3-6 и перепроверенное `C-225` п.1. Гейт стал
СТРОЖЕ, а не мягче; ослабления, ради ловли которого лок построен, здесь нет.
Поэтому коммит этого вердикта несёт `ALLOW-SUBJECT-CHANGE` с названной причиной. Токен —
аудит-след, а не доказательство (`gates.md` §11): доказательство — диф выше и вердикт
`C-225`, аудировавший ровно эту ревизию.

---

## Done Block

```text
$ pwd
/tmp/hft-reviewer-m85
$ git rev-parse HEAD
5e0b4dbb01abc2659b28505eab9e2864d7b044b6
$ git status --porcelain
?? research/reviews/R-183-M-85-frames-book-continuity.md      # этот вердикт, до коммита
$ git rev-list --left-right --count origin/main...HEAD
0	15

$ bash scripts/next_artifact_id.sh R
R-183
alloc_exit=0

# ── PR-гейт: acceptance прогнан РЕВЬЮЕРОМ (не пересказ tester'а)
$ bash scripts/verify_M-85.sh 2>&1 | grep -cE '^PASS'
26
$ bash scripts/verify_M-85.sh 2>&1 | grep -cE '^FAIL'
0
$ grep -E '^(VERDICT|exit=)' /tmp/m85_verify.log
VERDICT: PASS
exit=0

  из них (паритет с CI):
  PASS: cargo fmt --all -- --check
  PASS: cargo clippy --all-targets --all-features -- -D warnings
  PASS: cargo test --all --quiet
  PASS: оракул m85_1 (VB-I-2 на пути frames_since) (исполнено тестов: 1)
  PASS: сторож m85_2 (норма не куплена ценой фикса) (исполнено тестов: 1)
  PASS: страж m85_3 (фикстура строит предмет) (исполнено тестов: 1)
  PASS: сторож m85_6 (запрет §2.1 получил сторожа, а не прозу) (исполнено тестов: 1)
  PASS: оракул m85_4 + страж оси m85_5 (VB-I-10 на пути frames_since) (исполнено тестов: 2)
  PASS: оракулы M-77 (непрерывность книги на пути pump) (исполнено тестов: 6)
  PASS: оракул M-77 (цена pump не выросла) (исполнено тестов: 1)
  PASS: ! grep -rn 'frames_since_with_stats' crates/*/src/ … | grep -q .
  PASS: diff <(git show 95d2422:…lib.rs | grep -A7 '^pub fn frames_since(') <(…)
  PASS: [ $(git diff --numstat 95d2422..HEAD -- crates/gateway-serve/src/lib.rs | …) -le 8 ]

$ cargo test --workspace 2>&1 | awk '/^test result/ {p+=$4; f+=$6; n++} END {print "passed="p" failed="f" (блоков: "n")"}'
passed=976 failed=0 (блоков: 230)
$ grep -cE 'FAILED' /tmp/m85_ws.log
0
$ grep -E '^exit=' /tmp/m85_ws.log
exit=0

# ── RED предъявлен ПРЯМЫМ прогоном на пред-фиксной ревизии (без записи в чужую зону)
$ git worktree add /tmp/hft-reviewer-m85-red 3b6a16c --detach
HEAD is now at 3b6a16c spec(M-85): A-034 §7 п.3-6 … [architect]
$ grep -c 'seed_book' crates/gateway/src/lib.rs        # в дереве 3b6a16c
0
$ cargo test -p gateway --test red_m85_frames_book_continuity
thread 'm85_1_…' panicked at crates/gateway/tests/red_m85_frames_book_continuity.rs:275:9:
assertion `left == right` failed: VB-I-2 НАРУШЕН на пути `frames_since` …
  реплей: [(1752000010, 500000000), (1752000011, 500000000), (1752000012, 700000000), (1752000013, 700000000)]
  клиент: [(1752000010, 500000000), (1752000012, 700000000), (1752000013, 0)]
test result: FAILED. 3 passed; 1 failed; 0 ignored
exit=101

# ── Scope: роли по файлам, а не по метке в subject'е
$ git log --format='%h %s' --name-only 95d2422..HEAD -- 'crates/*/tests/*' | grep -c 'engine-dev'
0
$ git show --numstat --format='' 6f17d12
46	0	crates/gateway/src/lib.rs
$ git show --numstat --format='' 5e0b4db
3	1	crates/gateway-serve/src/lib.rs
$ git diff --name-only 95d2422..HEAD -- crates/contracts docs/rfc crates/journal crates/venue-* crates/risk crates/killswitch crates/oms
{пусто}
```

## VERDICT: **APPROVE** — merge через PR (`gates.md` §8)

Условие APPROVE выполнено целиком: гейт зелёный в дереве reviewer'а, RED предъявлен
исполнением, зона соблюдена, sacred-оракулы dev'ом не тронуты, границы `C` не пересечены,
`risk-critic` не требуется по замеру, а не по заявлению. Пять NOTE выше merge не держат:
две — бухгалтерия (`N-1`, `N-4`, закрываются close-out'ом), одна — способ работы tester'а
(`N-2`, эффект на репо ноль), две — унаследованные пробелы чужих документов (`N-3`, `N-5`).
