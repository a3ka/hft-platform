<!-- GATE-META
milestone: M-96
audited_repo: a3ka/hft-platform
audited_base: ad20a7e7b844bf3882d846ebb66d3b049753c74b
audited_head: ee8ffa1d55abf20b6bc3d212e0b35563b2993af1
verdict: APPROVE
-->

# R-252 — PR-гейт `M-96` (окно поиска быстрого пути в обе стороны, `TD-250`), задача 1

**Вердикт: APPROVE — задача 1.** Задача 2 (§8 на проде) — моя, после merge'а и деплоя;
она НЕ закрыта этим вердиктом, и милестоун этим вердиктом НЕ закрывается.

Предмет — ветка `feat/M-96-tail-seek-window`. Вершина взята командой
`git fetch origin && git rev-parse origin/feat/M-96-tail-seek-window` →
`ee8ffa1d55abf20b6bc3d212e0b35563b2993af1`; совпала со справкой мандата tester'а (`ee8ffa1d`).
Повторный `fetch` перед записью — та же вершина. База — `origin/main` `ad20a7e`
(merge-base). За время гейта `main` ушёл `ad20a7e → b3e9d7b` (PR #333, `M-95`) — дерево слияния
проверено отдельно (§Merge-preview).

Ярус C — грепом, не целиком: `TECH-DEBT.md` — `TD-250` (карточка `:5403`, OPEN, MAJOR);
`PROJECT-STATE.md` — `M-96`, `MS-STATE: M-9` (раздела `M-96` нет); `docs/ROADMAP.md` — `TD-250`, `M-96`.

## Block-scope

`git diff --stat origin/main...origin/feat/M-96-tail-seek-window` → 10 файлов, +1503/−13.

| коммит | роль | файлы | зона по §10 |
|---|---|---|---|
| `6c405908` `ed4f2742` `bf07c513` | architect | `crates/journal/tests/red_m96_tail_seek_window.rs` | architect ✔ |
| `295e5347` | architect | `scripts/verify_M-96.sh`, `scripts/tests/red_verify_M-96_ci_map.sh` | architect ✔ |
| `e387e47b` `481c646e` `7ec51988` `aef2c1e1` | architect | `milestones/M-96-tail-seek-window.md` | architect ✔ |
| `a8d0751b` | architect | `docs/ROADMAP.md` (строка `TD-250 → M-96`) | architect, §10 ✔ |
| `18d3c97b` | architect | `research/critiques/C-290-td250-w1-adversary.md` | исключение §10 (founder 2026-10-09) ✔ |
| `86ec3c30` `b35f04d1` `ba5b7435` | critic | `research/critiques/C-291`, `C-294`, `C-295` | critic ✔ |
| `ee8ffa1d` | engine-dev | `crates/journal/src/segments.rs` | §10: ТОЛЬКО `seek_back_from_tail` и описание ✔ |

Ханки `ee8ffa1d` (`git diff -U0 … | grep '^@@'`): `-3612…-3722` → `+3612…+3735`; функция
начинается на `:3631`, следующая (`cheap_tail_last_seq`) — на `:3760`. Все ханки — в описании
и теле `seek_back_from_tail`. Соседние условия `resolve_active_start_offset` (1–6), гард
«пустой хвост» (`last_seq < after_seq + 1 ⇒ Ok(None)`), `seek_fallbacks`, формат кадра,
`TailHint` — не тронуты (запретный список §6 соблюдён).

Block-C: `crates/contracts/**` не тронут. RISK-BLOCK: `risk`/`killswitch`/`oms`/`venue-*` не
тронуты — risk-critic не требуется. Plan-time critic пройден: `C-291` REJECT → `C-294` REJECT →
`C-295` ESCALATE(→approved).

**RED-first.** `crates/journal/tests/**` и `scripts/verify_*` тронуты только коммитами
architect'а, все — РАНЬШЕ `ee8ffa1d`; коммит dev'а тестов не касается
(`git log --format='%h %s' ad20a7e..HEAD -- 'crates/*/tests/*' 'scripts/verify_*' 'scripts/tests/*'`
→ 4 коммита, все `[architect]`). Атомарность: одна задача — один коммит, `fix(M-96): task #1 …
[engine-dev]`. Трейлеров co-author нет.

## Block-FA (M-66)

Тронут `crates/journal/**` ⇒ `docs/fa/journal.md`. Живой инвариант: **`JR-I-2`**
(`docs/fa/journal.md:112`: «`seq` строго монотонен без дыр; разрыв при чтении → abort»).
Правка опирается на него буквально: кадры с `seq < after+1` пропускаются, `seq == after+1` —
найден, `seq > after+1` — `Ok(None)`; это корректно ровно потому, что `seq` в сегменте монотонен.
Сдвиг меняет только СТОИМОСТЬ, а не выдачу — для пути выдачи `docs/fa/viz-backend.md`
**`VB-I-2`** (live == replay): все `t*` сверяют выдачу РОВНО `after+1 ..= last`.

## Корректность — разбор реализации

- `win_start = approx_pos.saturating_sub(16 КиБ).max(header_end)`; читается 32 КиБ. Цель на
  расстоянии до 16 КиБ назад и до 16 КиБ вперёд от оценки попадает в окно — требование §3 п.1
  («до 8 КиБ в обе стороны») выполнено с запасом, предел стоимости §3 п.2 — замером (ниже).
- Позиция найденного кадра пересчитана от начала окна (`pos = win_start + i`) — ключевая
  строка правки; моя мутация её возврата роняет 5 оракулов (ниже).
- Начало окна может прийтись на середину кадра — ресинк побайтный с CRC и `postcard`, как был.
  Вероятность ложного совпадения CRC+декод+`seq == after+1` выросла пропорционально окну (×4),
  остаётся пренебрежимой, и найденный hint повторно валидируется `resolve_active_start_offset`.
- Окно у начала файла обрезается `header_end`, у конца — коротким `read` (`buf.truncate(n)`).

## Done Block (дерево `ee8ffa1d`, `/tmp/hft-reviewer-m96`, сырой вывод)

```
$ git fetch origin && git rev-parse origin/feat/M-96-tail-seek-window
ee8ffa1d55abf20b6bc3d212e0b35563b2993af1
$ git status --porcelain
(пусто)
$ bash scripts/verify_M-96.sh 2>&1 | grep -E '^(PASS|FAIL|SKIP|VERDICT)'; echo exit=$?
PASS  task1: red_m96_tail_seek_window (t1…t4)
PASS  I-2: journal red_m89_seek_contract (j1…j9)
PASS  I-2: journal red_m89_seek_junction
PASS  I-2: journal red_m89_seek_fallback_observed
PASS  task1 / I-3: red_m89_warm_resume_seek w1 — 10 из 10 прогонов зелёные
PASS  ci-map: проба карты CI-паритета (red_verify_M-96_ci_map.sh; число миров печатает проба)
PASS  ci-parity: cargo fmt --all -- --check
PASS  ci-parity: cargo clippy --all-targets --all-features -- -D warnings
PASS  ci-parity: cargo test --all
PASS  ci-parity: cargo audit
… (ещё 44 шага ci-parity — все PASS; 6 SKIP по карте исключений; 1 SKIP check_review_fa —
   вердикта в диапазоне ещё не было, этот файл его и вносит)
PASS  ci-parity: учтено шагов 61 из 61 (исполнено 54, исключено по карте 6)
SKIP  task2: §8-замер на проде — … снимает reviewer
VERDICT: PASS
exit=0
```

## Мутационный контроль (свой, не повтор tester'а; оба отката проверены `git status --porcelain` → пусто)

| мутация `seek_back_from_tail` | оракул | исход |
|---|---|---|
| `WIN_HALF` 16 КиБ → 4 КиБ (окно назад меньше требуемых 8 КиБ) | `red_m96_tail_seek_window` | `t5_overshoot_between_4_and_8_kib_needs_8_kib_back --- FAILED`; 5 passed; 1 failed; exit=101 |
| `pos = win_start + i` → `pos = approx_pos + i` (не пересчитано смещение позиции) | `red_m96_tail_seek_window` | `t1`, `t2`, `t3`, `t5`, `t6` FAILED (5 failed), exit≠0 |
| то же | `red_m89_seek_contract` | 5 passed; 5 failed |

Tester (мутация «а» — окно только вперёд) — `t1`/`t3`/`t5` FAILED; совпадает с §8 спеки
(«`ad20a7e` — `t1`, `t3`, `t5` FAILED»).

## Merge-preview (`gates.md` §8, `strict: false`)

`main` ушёл на `b3e9d7b` (PR #333, `M-95`: `crates/gateway/src/lib.rs`,
`crates/gateway-serve/src/lib.rs` — тот же путь подписки, на котором живёт `w1`).
`git merge-tree --write-tree origin/main ee8ffa1d` — без конфликтов. На дереве слияния
(временный коммит, не пушился):

```
journal red_m96_tail_seek_window exit=0
journal red_m89_seek_contract exit=0
journal red_m89_seek_junction exit=0
journal red_m89_seek_fallback_observed exit=0
gateway red_segment_meta_bound exit=0
gateway red_m95_cold_resume_provenance exit=0
gateway-serve red_m95_catalog_once exit=0
gateway-serve red_m95_provenance_fresh exit=0
w1 x10 on merge-preview: fail=0
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 164.03s
VERDICT: PASS (0 нарушений)
(verify_design_claims.sh — строка VERDICT выше; exit взят у tail, решение — по VERDICT)
```

## Находки (не блокируют)

**N-1 — `i = frame_end` не пиннится оракулом (сообщение tester'а, §E п.1).** Замена на `i += 1`
оставляет `t1…t6` зелёными: после принятого кадра с `seq < after+1` сканер идёт побайтно через
его тело, CRC отбрасывает мусор, следующий заголовок всё равно находится. Это **не дефект
предмета**: строка `i = frame_end` существовала до `M-96` и правкой не тронута (в диффе
меняется только комментарий над ней); выдача и I/O не меняются, CPU ограничен окном 32 КиБ
(в худшем случае — CRC на каждой позиции окна). Оракул меряет то, что обещает спека, —
`rchar`, а не CPU; требования к CPU в §3 нет. Карточку долга НЕ завожу: ограниченная цена при
нулевом вреде выдачи. Если architect сочтёт, что «кадровый» шаг стоит пиннить, — это его
решение о форме защиты (`gates.md` §4). Оформление tester'а как `SCOPE VIOLATION REQUEST` —
не по формату: правки он не просил, это наблюдение.

**N-2 — устаревшее число в спеке.** `milestones/M-96-tail-seek-window.md` §4 строка `I-1` и §8
строка `t1` говорят «`хвост + 256 КиБ`», а оракул (`red_m96_tail_seek_window.rs:90`,
`FAST_PATH_OVERHEAD = 172 * 1024`) и §3 п.2 — 172 КиБ для `t1…t3`, `t5`, `t6`. Оракул строже
текста; вред — читатель спеки получит неверный предел. Зона — architect, при close-out.

## Условие закрытия милестоуна

Задача 2 — после деплоя: (а) push-прогон CI на merge-коммите `main` и ближайшие прогоны —
`w1` не флакует; (б) Deploy success, прод healthy. `TD-250` закрывается reviewer'ом ПОСЛЕ
наблюдения (а), не этим вердиктом.

## Handoff §D

APPROVE ⇒ merge через PR (`gh pr create` → `gh pr checks --watch` → `gh pr merge --merge
--delete-branch`), затем §8-деплой-гейт и задача 2 — исполняет reviewer сам. Находки N-1/N-2 —
к сведению `architect` (не REJECT, разбор не обязателен для merge'а).
