<!-- GATE-META
milestone: TD-220
audited_repo: a3ka/hft-platform
audited_base: 32574deddfd273a925a9ae9ca146a4417af6490e
audited_head: c0b94f2472979fa1b24b878d1aebd86fffa0db42
verdict: REJECT
-->

# R-230 — перепроверка `gates.md` §9: ветка `docs/td-220-241` (TD-241 sacred-шапки, TD-220 решение в FA)

**Роль:** независимая перепроверка §9, свежий контекст, модель Fable (`[architect-recheck]`); автор
правки — ведущий architect, здесь он сторона. **Предмет — ветка** `origin/docs/td-220-241`;
вершина на момент прогона `c0b94f2`, база = `origin/main` = `32574de` (ветка сидит ровно на
`main`, merge-preview тождествен дереву ветки). Два коммита: `7000e59` (TD-241, два
sacred-оракула, только комментарии/тексты assert'ов) и `c0b94f2` (TD-220, `docs/fa/ops.md`).

**Вердикт: REJECT** — две блокирующие находки, обе дешёвые (две строки в двух файлах), обе — класс
«утверждение документа о коде расходится с кодом», ради которого §9 и существует. Решение по
существу (TD-220: `refusals_unsupported` тревогой не судится) — **верно и полномочно**, см. §Б.

**Чем грепал ярус C:** `TECH-DEBT.md` — `TD-220`, `TD-241`; `R-226` — `TD-220`,
`refusals_unsupported`, `по бюджету`; `R-224` — `Н-2`, `Б-1`. Ярус B — `docs/fa/ops.md` целиком
(523 строки), оба тронутых оракула, `crates/ops/src/watchdog.rs` (сторож), `crates/gateway-serve/src/
{lib.rs,admission.rs,metrics.rs,main.rs}` (продюсер).

**Живые инварианты тронутого модуля (`docs/fa/ops.md` на `c0b94f2`):** `OPS-I-11` (`:479`, тишина
ВЫДАЧИ, правило на дельтах) и `OPS-I-8` (`:476`, тишина потока MD) — оба существуют в файле и оба
являются предметом правки.

---

## Б — блокирующие

### Б-1. `verify_design_claims.sh --merge-preview origin/main` → **FAIL**, и виновата ветка

```
$ bash scripts/verify_design_claims.sh --merge-preview origin/main 2>&1 | grep -E '^(FAIL|VERDICT)'; echo exit=$?
FAIL  [2-ПОКРЫТИЕ] §22: семейство OPS-I — документ заявляет 'в оракулах'=8, реальный замер (анти-плацебо) strict=9, loose (любое упоминание)=9
VERDICT: FAIL (1 нарушений)
exit_vdc=1
```

Атрибуция — тем же скриптом на `origin/main` (отдельный detached-worktree `32574de`):

```
$ bash scripts/verify_design_claims.sh 2>&1 | grep -E 'OPS-I|^VERDICT'
PASS  [2-ПОКРЫТИЕ] §22: OPS-I — заявлено=10, в оракулах=8 — подтверждено замером (loose=8)
VERDICT: PASS (0 нарушений)
exit_vdc_main=0
```

Причина — коммит `7000e59`: он ввёл идентификатор `OPS-I-11` в `*/tests/`, которого там прежде не
было, и множество упомянутых в оракулах `OPS-I-*` выросло с 8 до 9:

```
$ git grep -ohE 'OPS-I-[0-9]+' origin/main -- 'crates/*/tests/*.rs' | sort -u | tr '\n' ' '
OPS-I-1 OPS-I-10 OPS-I-4 OPS-I-5 OPS-I-6 OPS-I-7 OPS-I-8 OPS-I-9
$ git grep -ohE 'OPS-I-[0-9]+' HEAD -- 'crates/*/tests/*.rs' | sort -u | tr '\n' ' '
OPS-I-1 OPS-I-10 OPS-I-11 OPS-I-4 OPS-I-5 OPS-I-6 OPS-I-7 OPS-I-8 OPS-I-9
```

Таблица покрытия `docs/DESIGN.md:921` при этом не тронута:

```
$ sed -n '921p' docs/DESIGN.md
| OPS-I | ops/алерты | 10 | 8 | [ЧАСТИЧНО] |
```

Скрипт входит в CI (`.github/workflows/ci.yml:323-324`), то есть PR с этой веткой будет красным
независимо от моего вердикта — `gates.md` §8: «расхождение прогонов — блокер, не примечание».

**Что сделать (architect, та же ветка):** `docs/DESIGN.md:921` → `| OPS-I | ops/алерты | 11 | 9 |
[ЧАСТИЧНО] |`. Колонка «Заявлено» скриптом не сверяется, но `10` — та же протухшая шапка, что ветка
только что исправила в `ops.md:5` (`OPS-I-1..9` → `..11`); чинить обе за один заход. Правка —
счётная колонка, изложение по §9; критика не требует. После правки — повторный прогон
`--merge-preview` в Done Block.

### Б-2. FA называет оси отказа `Unsupported` не те, что судит код

`docs/fa/ops.md:479` (новый текст): «растёт, когда селектор клиента вне политики допуска
(**`band`/`timeframe`/`window`**; `crates/gateway-serve/src/lib.rs`, ветка `ServingOutcome::Unsupported`)».

Код — `admit()` в `crates/gateway-serve/src/admission.rs` — судит ТРИ вещи, и `window` среди них нет:

```
$ grep -n 'bands_subset(&sel\|sym_ok\|profile_allowed(policy, sel)\|^fn profile_allowed\|p.timeframe_ms == sel.timeframe_ms' crates/gateway-serve/src/admission.rs
122:    if !bands_subset(&sel.bands, &policy.canonical_bands) {
126:    let sym_ok = policy.allowed_symbols.iter().any(|s| s == &sel.symbol);
127:    if !sym_ok {
131:    if !profile_allowed(policy, sel) {
146:fn profile_allowed(policy: &AdmissionPolicy, sel: &Selector) -> bool {
159:        if p.timeframe_ms == sel.timeframe_ms {
```

Комментарий внутри `profile_allowed` (`admission.rs:147-156`) говорит это прямо: «Проверяем только
`timeframe_ms` (остальные поля профиля — `window_ms`/`depth_cadence_ms` — … проверяются на следующем
слое)». Значит: `window` в FA — **ложный** триггер счётчика, `symbol` — **пропущенный**. Источник
ошибки виден: FA скопировала пользовательскую строку ошибки `lib.rs:1180`
(`"selector вне политики допуска (band/timeframe/window)"`), которая сама неточна. Строка кода — зона
engine-dev и не предмет ветки; FA — нормативный документ и обязана совпадать с кодом, а не с его
сообщением.

**Что сделать:** `(bands/symbol/timeframe_ms — три проверки `admit()`,
`crates/gateway-serve/src/admission.rs:122-131`)`. Неточность строки `lib.rs:1180` — карточкой долга
через reviewer (не моя зона и не зона этой ветки).

---

## А — утверждения о коде, подтверждённые командой

| Утверждение ветки | Команда / факт | Итог |
|---|---|---|
| единственный вызов `inc_refusals_unsupported` в прод-пути — `lib.rs:1178` | `grep -rn inc_refusals_unsupported crates/ --include=*.rs` → определения `metrics.rs:102,125,157` (+ внутренняя диспетчеризация `:159-160`) и ОДИН вызов `lib.rs:1178` | **верно** |
| вызов стоит в ветке `ServingOutcome::Unsupported` v1-пути | `lib.rs:1177-1178`; функция-носитель — `handle_v1_message` (`:1055`), `run_authorized_session` начинается с `:2068` | **верно** |
| legacy-путь счётчик не двигает | в `run_authorized_session` единственный инкремент отказов — `inc_refusals_supported` (`lib.rs:2151`), комментарий `:2149` называет это явно | **верно** |
| сторож поле читает, правила не имеет | `watchdog.rs:168` — поле `ServingHeartbeatSample`; `check_serving_silence` (`:629-636`) считает только `attempts`/`refusals_supported`/`successes`; других вхождений `refusals_unsupported` в `crates/ops/src` нет | **верно** |
| логика оракулов не изменилась | `git diff --numstat 32574de..HEAD` → `3/3` и `1/1` в тестах; диф — только `//!`-шапки и строковые литералы `assert_eq!` | **верно** |
| прод-путь всегда с политикой (условие, при котором «`Unsupported` = ошибка клиента» вообще верно) | `main.rs:49` `bind_with_policy`; `bind` без политики — только `src/bin/wsprobe.rs:289` и тесты | **верно, с оговоркой Н-2** |

Прогоны (worktree `/tmp/hft-recheck-td220`, `HEAD=c0b94f2`):

```
$ cargo test -p ops --test red_m89_serving_silence 2>&1 | tail -3
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
exit_ops=0
$ cargo test -p gateway-serve --test red_m91_legacy_counters 2>&1 | grep -E '^test result'
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.07s
exit_gs=0
$ cargo test -p gateway-serve --features testing --test red_m91_legacy_counters 2>&1 | grep -E '^test |^test result'
test l2_legacy_not_ready_counts_attempt_and_supported_refusal ... ok
test l1_legacy_success_counts_one_attempt_and_one_success ... ok
test l4_legacy_warming_is_a_supported_refusal ... ok
test l3_two_legacy_sessions_count_exactly_two ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.11s
exit_l4=0
$ cargo fmt --all -- --check; echo exit=$?
exit_fmt=0
```

Третий прогон добавлен мной: `l4` (его assert-строка в диффе, `red_m91_legacy_counters.rs:338-341`)
стоит за `#[cfg(feature = "testing")]` (`:306`), и мандатная команда без фичи его **не компилирует и не
гоняет** («running 3 tests»). Без `--features testing` утверждение «тесты зелёные» не покрывало бы
ровно одну из четырёх правленых строк.

---

## Б — полномочия

**Форма или изложение?** Изложение/уточнение границы, **критик по §9 не требуется.** Основание:
правило `OPS-I-11` — формула `Δattempts > 0 ∧ Δrefusals_supported > 0 ∧ Δsuccesses == 0` — не
изменилось ни в FA, ни в коде (`watchdog.rs:636`), ни в оракулах (диф `*/tests/` — только текст);
`refusals_unsupported` в формулу не входил с `M-89`. Новый абзац **описывает существующее**
свойство правила и даёт ему основание; инвариант не ослабляется — множество состояний, на которых он
обязан звенеть, то же самое. Открытый вопрос `R-226` §3 («решения architect'а “не нужно” нет ни в FA…»)
закрывается ровно той записью, которой реviewer требовал. **risk-critic** не требуется: `docs/fa/ops.md`
не в перечне safety-путей §9 (`risk`/`killswitch`/`oms`, `RK-I-*`/`INTG-I-*`). **Граница C** не задета:
нет промоушена, весов, лимитов, состава записываемых данных, фаз.

**Шапка `OPS-I-1..11`** (`ops.md:5`) — счётная строка, изложение; закрывает `R-224` Н-2. Но см. Б-1:
та же протухшая цифра осталась в `DESIGN.md:921`, и теперь она ещё и роняет гейт.

---

## В — связность ссылок (новый текст)

`TD-220` — `TECH-DEBT.md:199` ✓ · `TD-241` — `:217` ✓ · `TD-237` — закрыта, упомянута как история ✓ ·
`R-224` Б-1 — файл есть, `Б-1` в нём 3 раза ✓ · `C-269` — `research/critiques/C-269-m91-legacy-serving-counters.md` ✓ ·
`П-003` — 3 вхождения в `PENDING-SIGNATURE.md` ✓ · `crates/gateway-serve/src/lib.rs` ветка
`ServingOutcome::Unsupported` — `:1177` ✓ · `crates/ops/src/watchdog.rs` `check_serving_silence` — `:629` ✓.
Висячих нет; содержательная неточность одной ссылки — Б-2.

---

## Н — не блокирует

- **Н-1. «Голодание, при котором отвергаются ПОДДЕРЖАННЫЕ запросы, ловит правило выше».** Верно для
  ПОЛНОГО голодания (`Δsuccesses == 0`); частичный случай из `R-226` §3 («часть клиентов стабильно
  отвергается») правило не ловит по построению. Фраза защитима (частичное — деградация, не голодание),
  но reviewer при закрытии `TD-220` не должен читать её как покрытие частичного случая. Отдельно и в
  пользу решения: «отказы ПО БЮДЖЕТУ» из `R-226` — это слот/`overloaded`, и в коде они идут в
  `refusals_supported` (`lib.rs:1228-1233`), то есть **уже внутри правила**; одна фраза об этом в FA
  закрыла бы формулировку `R-226` буквально.
- **Н-2. Предел рассуждения «`Unsupported` = ошибка клиента».** Сервер без политики (`bind`, `None`)
  отвечает `Unsupported` на ВСЁ (`lib.rs:339-340, 393-394`) — там счётчик растёт при исправных
  клиентах и правило слепо. На проде недостижимо (`main.rs:49` — только `bind_with_policy`), поэтому
  решение верно для прод-формы; назвать это пределом в FA — по желанию автора.
- **Н-3.** `red_m89_serving_silence.rs:2` стала длиннее 100 знаков; `rustfmt` doc-комментарии не
  переносит, `fmt --check` зелёный — косметика.
- **Н-4.** Мандат следующему проверяющему `red_m91_legacy_counters` обязан называть
  `--features testing`, иначе `l4` молча выпадает из прогона.
- **Н-5.** `docs/DESIGN.md:297,306` — «`OPS-I-1..8`»; до ветки, вне диффа; та же протухшая шапка,
  починится вместе с Б-1 или отдельно.
- **Н-6.** `scripts/next_artifact_id.sh R` на 128 ref'ах форкает `sed` на КАЖДЫЙ файл каждого дерева и
  идёт минутами (класс `R`/`M`/`C`/`A`); `reserve_artifact_id.sh R` за 60 с не уложился вовсе. Гонка
  §12 за это время сработала на мне: первый выпуск дал `R-229`, барьер `check_artifact_ids.sh` до push
  поймал второго носителя (`origin/fix/TD-241-watchdog-comments` — там reviewer закрыл engine-dev-часть
  `TD-241`, комментарии `watchdog.rs:54,214,624`); перевыпуск после `fetch` — `R-230`. Эшелон
  «аллокатор впереди, барьер сзади» отработал, но цена вызова — предмет для харнесс-трека.

---

## Done Block

```
$ pwd; git rev-parse --short HEAD; git branch --show-current
/tmp/hft-recheck-td220
c0b94f2
docs/td-220-241
$ git merge-base origin/main origin/docs/td-220-241; git rev-parse origin/main
32574deddfd273a925a9ae9ca146a4417af6490e
32574deddfd273a925a9ae9ca146a4417af6490e
$ git diff --numstat 32574de..HEAD
3	3	crates/gateway-serve/tests/red_m91_legacy_counters.rs
1	1	crates/ops/tests/red_m89_serving_silence.rs
2	2	docs/fa/ops.md
$ cargo test -p ops --test red_m89_serving_silence                      → 7 passed; exit=0
$ cargo test -p gateway-serve --test red_m91_legacy_counters            → 3 passed; exit=0
$ cargo test -p gateway-serve --features testing --test red_m91_legacy_counters → 4 passed (l1-l4); exit=0
$ cargo fmt --all -- --check                                            → exit=0
$ bash scripts/verify_design_claims.sh --merge-preview origin/main      → VERDICT: FAIL (1 нарушений); exit=1   ← Б-1
$ (worktree origin/main 32574de) bash scripts/verify_design_claims.sh   → VERDICT: PASS (0 нарушений); exit=0
$ grep -rn 'inc_refusals_unsupported' --include=*.rs crates/ | grep -c 'lib.rs'
1
$ grep -n 'refusals_unsupported' crates/ops/src/watchdog.rs
168:    pub refusals_unsupported: u64,
$ sed -n '636p' crates/ops/src/watchdog.rs
    if d_attempts > 0 && d_refusals_supported > 0 && d_successes == 0 {
```

**Условие APPROVE:** Б-1 (`DESIGN.md:921` → `11 | 9`, зелёный `--merge-preview` в Done Block) и Б-2
(оси в `ops.md:479` → `bands/symbol/timeframe_ms`) на той же ветке; повторный круг — тем же маршрутом
§9, свежий Fable-агент. Критик не требуется (см. §Б).
