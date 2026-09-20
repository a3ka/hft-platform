<!-- GATE-META
milestone: M-45
audited_repo: a3ka/hft-platform
audited_base: 2e63a37e5bf454da69b0fbd69de28c043b4caf4c
audited_head: 6ea5ce238d3ca1bad9ce0d49ba51e0888468ccd6
verdict: APPROVE
-->

# R-168 — M-45 (раскатка L2Delta на ETHUSDT), PR-гейт круг 2: **APPROVED**

**Роль:** reviewer (`gates.md` §4 — UNCONDITIONAL) · **Дата (UTC):** 2026-09-01T18:35Z
**Предмет:** `2e63a37..6ea5ce2` на `docs/M-45-rollout-signature` (45 коммитов, 23 файла,
+5159/−50 против merge-base).
**Мандат:** tester `PASS` (24 PASS, exit=0, пять мутаций).
**Предыдущий круг:** `R-167` — REJECT на `5fd83cb` (Б-1/Б-2/Б-3).

Прогон tester'а воспроизведён полностью и сошёлся по каждому числу. Три блокера `R-167`
закрыты механизмом, а не обещанием, и хватка механизма предъявлена ДВУМЯ мутациями,
которых tester не делал.

---

## Block-scope — ПРОЙДЕН

`crates/**` не тронут ВООБЩЕ: дифф — `docker-compose.yml` (+2), докс, артефакты гейтов,
харнесс (`scripts/verify_M-45.sh`, `scripts/lib/rollout_symbols_check.py`).

```
$ git diff --name-only 2e63a37..HEAD | grep -c '^crates/'
0
$ git diff --name-only 2e63a37..HEAD | grep -c '^crates/contracts/'
0
```

- **MD-only carve-out подтверждён** (`gates.md` §5): order-egress (submit/cancel/auth
  торговли) отсутствует как класс — кода в диффе нет вовсе. **risk-critic не требуется.**
- **Block-C ПРОЙДЕН:** `crates/contracts/**` не тронут (подтверждено и шагом `T7`);
  `SCHEMA_VERSION` не бампался; contract-RFC не требуется (`M-45` §0).
- **Block-risk НЕ ПРИМЕНИМ:** `crates/risk/**`, `crates/killswitch/**`, `crates/oms/**`,
  `crates/venue-*/**` не тронуты; `docs/fa/{risk,killswitch,oms}.md` и `RK-I-*`/`INTG-I-*`
  не тронуты (`gates.md` §9 risk-триггер по докам не сработал).

**Предъявление FA (M-66).** Барьер `check_review_fa.sh` даёт `SKIP` — диапазон не трогает
`crates/**`, то есть здесь требование КОГНИТИВНОЕ, и я его исполняю явно, а не молчанием:
живой инвариант предмета — **`VN-I-3`** «Core-крейт `venues` не содержит ветвления по
конкретному `venue_id`» (`docs/fa/venues.md:176`, подтверждён грепом на судимой ревизии).
Он и есть причина, по которой раскатка законно живёт в `docker-compose.yml`, а не в коде:
состав задаётся конфигурацией на границе, ветвления по площадке в core не появляется.
`FA-WAIVER` не требуется.

## Соответствие подписи `П-026` — ТОЧНОЕ, включая фьючерсы

`П-026` подписана 2026-08-31, дословно «СПОТ И ФЬЮЧЕРСЫ». `R-159` в своё время отклонил
раскатку именно за то, что фьючерсы подпись задевает, а запись исключает. На судимой
ревизии расхождения нет:

```
$ grep -nE 'L2DELTA_CAPTURE_SYMBOLS|EPOCH_ID|BINANCE_FUTURES_SYMBOLS' docker-compose.yml
29:      BINANCE_FUTURES_SYMBOLS: ${BINANCE_FUTURES_SYMBOLS:-BTCUSDT,ETHUSDT}
31:      L2DELTA_CAPTURE_SYMBOLS: BTCUSDT,ETHUSDT
32:      EPOCH_ID: own-2026-09-m45-ethusdt
```

Обе переменные стоят на ЕДИНСТВЕННОМ сервисе `recorder` (`container_name: hft-recorder`),
который ведёт и спот, и фьючерсы в одном процессе — поэтому один литерал покрывает обе
площадки. Что обе стороны реально под оракулом, подтверждают `T0`/`T3` поимённо для
`venue-binance` И `venue-binance-futures`. Состав — литерал, не подстановка (`§3quinquies`):
`${VAR:-default}` в этих двух строках отсутствует, значит операторская среда не может
расширить границу C молча.

## Закрытие блокеров `R-167` — по условию, дословно

Условие `R-167`: «Б-1 закрыт записью значения; Б-2 и Б-3 закрыты оракулом ЛИБО письменно
объявлены пределом; `verify_M-45.sh` зелёный; `verify_design_claims --merge-preview`
зелёный». Проверено по каждому пункту:

| блокер | чем закрыт | мой замер |
|---|---|---|
| **Б-1** значение эпохи только в compose | записано в `docs/data-epochs.md`, раздел `E-002`, В ЯЧЕЙКЕ ФАКТА | `awk '/^## E-002/…' \| grep -cE '^\| *`EPOCH_ID` после *\|.*own-2026-09-m45-ethusdt'` → `1` |
| **Б-2** `T9` смотрел не туда | `T9` доведён до ячейки факта раздела `E-002` (`A-031`) | `T9` PASS; мутация tester'а №4 (проза с тем же литералом) роняет шаг |
| **Б-3** компаратор принимал устаревшую эпоху | `T10`+`T10c` через `scripts/lib/rollout_symbols_check.py` | мутация tester'а №5 (`own-2026-08`) роняет `T10` |

**Н-1 закрыт:** `§Tasks` строка 7 — `✅ DONE на ветке (f3b84d4)`
(`milestones/M-45-persist-l2delta.md:111`).

## Block-DoneBlock — ПРОЙДЕН (воспроизведён независимо, не принят на слово)

Прогонял в СВОЁМ worktree `/tmp/hft-reviewer-m45` на `6ea5ce2`, сверенном с `origin`
(`git ls-remote --heads origin docs/M-45-rollout-signature` → `6ea5ce238d3…`).

```
$ bash scripts/verify_M-45.sh 2>&1 | grep -cE '^PASS'   → 24
$ bash scripts/verify_M-45.sh 2>&1 | grep -cE '^FAIL'   → 0
  VERDICT: PASS ; VERIFY_EXIT=0

$ cargo test --workspace 2>&1 | grep -E '^test result' | awk '{p+=$4;f+=$6} END {…}'
  passed=948 failed=0 (блоков: 221) ; EXIT=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
  VERDICT: PASS (0 нарушений) ; EXIT=0
```

Пять барьеров в форме CI (`EVENT_NAME=pull_request PR_BASE_SHA=2e63a37`):
`check_gate_meta` exit=0 (вердиктов 14, до-нормативных приземлений 0) ·
`check_protected_artifacts` exit=0 · `check_docs_freeze` exit=0 · `check_artifact_ids` exit=0 ·
`check_review_fa` exit=0 (`SKIP` — диапазон вне `crates/**`).

Числа сошлись с отчётом tester'а до единицы: 24/0, 948/0/221, exit=0 везде.

## Перепроверка хватки гейта — ДВЕ СВОИ мутации, обе НОВЫЕ

Пять мутаций tester'а воспроизводить дословно смысла нет — они уже предъявлены. Я взял
два угла, которых в его наборе НЕ было, оба на классе стражей присутствия (`A-031`).

**Мутация A — асимметрия сторон.** Tester сносил оракул СПОТА (`red_l2delta_capture.rs`).
Я снёс ФЬЮЧЕРСНЫЙ (`crates/venue-binance-futures/tests/red_l2delta_futures.rs`) — то есть
проверил, что страж двусторонний, а не запиннен на одну площадку (`testing.md`
§дегенерированный вход п.1 «асимметрия»):

```
FAIL  T6 venue-binance-futures: sacred-оракул сырого захвата
      crates/venue-binance-futures/tests/red_l2delta_futures.rs ОТСУТСТВУЕТ —
      удаление оракула не является успехом (testing.md св. 4)
VERDICT: FAIL (1 нарушений) ; EXIT=1
```

**Мутация B — неподписанное расширение границы C.** Самый дорогой сценарий этого
milestone'а: состав расширен ТРЕТЬИМ символом при валидной эпохе — то есть ровно то, чего
подпись `П-026` не покрывает. Tester этот угол не брал (его №5 бил по эпохе, не по составу
живого compose):

```
$ sed -i 's/L2DELTA_CAPTURE_SYMBOLS: BTCUSDT,ETHUSDT/…,SOLUSDT/' docker-compose.yml
FAIL  T10 задача 7 НЕ исполнена — L2DELTA_CAPTURE_SYMBOLS='BTCUSDT,ETHUSDT,SOLUSDT'
      не равен подписанному ['BTCUSDT', 'ETHUSDT'] — ЛИШНИЕ
      (неподписанное расширение границы C): SOLUSDT
VERDICT: FAIL (1 нарушений) ; EXIT=1
```

Компаратор называет и предмет, и норму, которую защищает. Обе мутации откачены, оба
worktree чисты (`git status --porcelain` пуст).

---

## Н-1. `verify_M-45.sh` зеленее CI: нет `cargo test --all`, нет `--all-features`

`gates.md` §3 «Паритет с CI»: verify обязан гонять базовый CI-job ЦЕЛИКОМ. Замер расхождения:

```
$ grep -nE 'run: cargo' .github/workflows/ci.yml
20:  run: cargo fmt --all -- --check
22:  run: cargo clippy --all-targets --all-features -- -D warnings
24:  run: cargo test --all
$ grep -cE 'cargo test --(all|workspace)' scripts/verify_M-45.sh
0
```

`verify_M-45.sh` гоняет `cargo build --workspace`, `clippy --workspace --all-targets`
(**без `--all-features`**) и точечные `cargo test -p <crate> --test <target>` — полного
`cargo test --all` нет. Спека виновата вместе со скриптом: `§6 Acceptance` требует ровно
build+clippy, то есть недоспецифицирует относительно `gates.md` §3.

**Не блокер, и основание названо, а не подразумевается:** merge идёт через PR, а
`All checks passed` включает джоб с `cargo test --all` и `--all-features` — то есть предмет
паритета СТОРОЖИТСЯ, просто другим механизмом и на шаг позже. Плюс дифф не трогает
`crates/**` вовсе, значит охват фич и воркспейс-тестов этой правкой не меняется. Я прогнал
`cargo test --workspace` сам (948/0) — зелено фактически, а не по предположению.
**Зона правки — architect** (`scripts/verify_*.sh` sacred): reviewer описывает дефект,
фикс проектирует architect (`gates.md` §4, граница reviewer↔architect). Завожу карточкой.

## Н-2. Компаратор границы C по-прежнему не сторожит ни один барьер (перенос `R-167` Н-2)

`scripts/lib/rollout_symbols_check.py` решает, исполнена ли подпись founder'а, но не входит
ни в строку architect'а `scope-guard.md`, ни в зону `check_docs_freeze.sh`, ни в перечень
харнесса `gates.md` §9. Спека это НАЗЫВАЕТ пределом явно (`§5 Allowed paths`) и выносит
развилку founder'у — то есть пробел предъявлен, а не замолчан, что и требуется. Merge не
блокирую: правка НОРМЫ — зона заперта (`gates.md` §11, нужен `FOUNDER-APPROVED`), architect
её сам не делает. Решение за founder'ом.

## Н-3. Защита литерала истекает в момент merge'а (перенос `R-167` Н-3, подтверждён)

```
$ grep -c 'verify_M-45' .github/workflows/ci.yml
0
```

После merge'а `BTCUSDT,ETHUSDT` в compose не удерживается НИЧЕМ машинным: `T10`/`T10c` живут
только в milestone-скрипте, который CI не зовёт. Мутация B выше доказывает, что хватка
РЕАЛЬНА — и ровно поэтому её исчезновение после merge'а есть потеря, а не формальность.
Класс `gates.md` §4 «built-not-wired», severity **MAJOR** (предмет охраны — граница C).
Карточка заводится мной при close-out.

## Н-4. Дифф выходит за `§5 Allowed paths` четырьмя путями (перенос `R-167` Н-4)

`docs/PENDING-SIGNATURE.md`, `docs/plans/*` (2 файла), `milestones/BACKLOG.md`,
`research/{critiques,reviews,arbitration}/*`. Последнее — прямое требование
`branch-hygiene.md` §3 (артефакт гейта коммитится на ветку предмета); первые три — зона
architect'а по `scope-guard.md`, но `§5` их не называет. Семейство `TD-080` (MINOR, «НЕ
блокер merge'а»). Не блокирую по тому же основанию; фиксирую, чтобы расхождение не считалось
несуществующим.

## Н-5. Круг критика покрывает диапазон целиком — закрыт

`R-167` Н-5 отмечал, что `C-203` судил не весь диапазон. На судимой ревизии остатка нет:
`C-204` (`4eca7ad`), `A-031` (`84b956a`), `C-205` (`dd3c4bf` REJECT → `6ea5ce2` CLEARED)
покрыли всё, что легло после. Последний СОДЕРЖАТЕЛЬНЫЙ коммит `8c3d4ec` осуждён критиком
явно: `C-205:243` «mutation proves the old comment-only escape fails … this is **CLEARED**»,
`C-205:178` «CLEARED — C-205 закрыт; передать M-45 тестеру». `check_gate_meta` зелёный.

---

## Итог

**APPROVED.** Подпись `П-026` исполнена ТОЧНО и покрывает обе площадки; состав записан
литералом, неподписанное расширение отвергается компаратором с называнием нормы (мутация B);
эпохальная половина раскатки — та, за которую `R-167` отклонил круг 1 — получила механизм,
и он держит (Б-1/Б-2/Б-3 закрыты замером). Страж присутствия двусторонний (мутация A).
`crates/**` не тронут: Block-C, RISK-BLOCK и MD-only carve-out разрешаются без risk-critic.

Четыре замечания (Н-1…Н-4) merge не держат: Н-1 покрыт зелёным CI на PR и прогнан мной
руками; Н-2 объявлен пределом и вынесен founder'у; Н-3 — TD-запись при close-out; Н-4 —
семейство `TD-080`.

**Остаток на close-out (мой):** карточки `TD` по Н-1 и Н-3 в `TECH-DEBT.md`, обновление
`PROJECT-STATE.md`, `gc_worktrees.sh`, деплой-гейт `gates.md` §8 с sanity ПО КАЖДОЙ
ПЛОЩАДКЕ отдельно (`§Tasks` строка 7 требует именно этого) — спот и фьючерсы проверяются
раздельно, потому что подпись покрывает обе, а один литерал их не различает.
