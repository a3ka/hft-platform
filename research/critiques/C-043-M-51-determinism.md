# C-043 — critic verdict: M-51 (`DET-I-1/2/3` — determinism becomes executable)

**Milestone:** `milestones/M-51-determinism.md` · **HEAD аудирован:** `f0799d2`
**Branch:** `feat/M-51-determinism` · **Дата (UTC):** 2026-08-02

## Вердикт: **NOTE**

RED-набор — исполнимый, оракулы бьют по существу (не по компиляции), анти-плацебо
проверен ФАКТИЧЕСКИ (не только заявлен) и совпадает с независимой перепроверкой один в
один. Один advisory-пункт (scope-guard, §3 ниже) — не блокирует dev, но architect/dev
обязаны знать о нём при инвокации.

---

## Проверенная фактура (сырой вывод команд)

### 1. Compile-RED подтверждён прогоном (не только заявлен)

```
$ cargo test -p journal --test red_det_replay_digest 2>&1 | tail -5
error[E0425]: cannot find function `replay_digest` in crate `journal`
error: could not compile `journal` (test "red_det_replay_digest") due to 26 previous errors

$ cargo test -p book --test red_det_projection 2>&1 | tail -8
error[E0599]: no method named `iter_sorted` found for struct `Books` in the current scope
error: could not compile `book` (test "red_det_projection") due to 3 previous errors
```

### 2. Поведенческий RED (`sim`) подтверждён прогоном — 4/4 FAILED, как заявлено

```
$ cargo test -p sim --test red_det_fill_order 2>&1 | tail -8
test result: FAILED. 0 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out
failures: det_14_..., det_15_..., det_16_..., det_17_...
```
Причина дефекта подтверждена на исходнике: `crates/sim/src/exchange.rs:51`
(`active: HashMap<u64, SimOrder>`) и `:240` (`for (id, order) in self.active.iter_mut()`)
— ровно то, что называет milestone.

### 3. Анти-плацебо — перепроверено НЕЗАВИСИМО (не поверено на слово)

Я сам временно добавил в `crates/journal/src/segments.rs`/`lib.rs` заглушку
(`replay_digest` → `Ok(ReplayDigest::default())`, константный `[0u8;32]`) и в
`crates/book/src/lib.rs` — `iter_sorted() -> Vec::new()`, прогнал RED-наборы, откатил
(`git checkout --`, дерево чистое после — `git status --porcelain` пуст). Результат
СОВПАДАЕТ с заявленным в milestone дословно:

```
red_det_replay_digest: 6 failed (det_2,3,5,6,7,8); 2 passed (det_1, det_4 — самосравнение)
red_det_restart:       2 failed (det_10, det_11);   2 passed (det_9 — самосравнение через
                        границу процесса, тривиально совпадает на константе)
red_det_projection:    3 failed (det_18, det_19, det_21); 1 passed (det_20 — САМОСРАВНЕНИЕ,
                        три вызова одной константной пустой проекции равны себе; milestone
                        честно называет это в §Состояние RED и НЕ засчитывает det_20 в
                        анти-плацебо-покрытие)
```
Ни одного расхождения с тем, что заявляет milestone. `det_1`/`det_4`/`det_9`/`det_20`
корректно не претендуют на анти-плацебо-роль — эта роль явно и правильно возложена на
соседние тесты в том же файле, которые её выполняют.

### 4. Канарейка источников (`det_22`/`det_23`) ловит РЕАЛЬНЫЕ находки, не шум

```
$ cargo test -p journal --test red_det_sources 2>&1 | tail -30
det_22: 2 hits — sim/exchange.rs:240 (реальный дефект) + research-cli/export_io.rs:283
        (легитимный, уже сортирует ключи сразу после .keys() — верно назван "безопасным,
        но без waiver")
det_23: 2 hits — segments.rs:652 (dedup_indexed_paths, безопасно через BTreeMap, но без
        waiver) + segments.rs:2374 (enumerate_retention_segments — источник det_25)
det_24: 0 hits (честно объявлено как "нулевая база", не мнимый PASS)
det_25: FAILED — воспроизведено вручную (12 нераспознанных имён, полная перестановка,
        падает на реальном порядке ФС, а не на подстроенной фикстуре)
```
Проверил исходники по указанным строкам — совпадают дословно с тем, что цитирует
milestone (`sim/exchange.rs:51,240`, `export_io.rs:278-283` — сортировка сразу после
`.keys()`, `segments.rs:645-660` и `:2368-2382`).

### 5. Verify-скрипт — прогнан целиком на текущем RED HEAD

```
$ bash scripts/verify_M-51.sh; echo exit=$?
... (T1-T4 — 6 FAIL, все шесть оракульных таргетов — FAIL, регресс — 4/4 блока PASS:
     journal 25, prodscale op_*/ti_*/fs_9, book 6, sim 5, strategy/portfolio/alpha 10,
     gateway/research-cli 40, cargo fmt PASS, workspace-clippy FAIL по причине
     iter_sorted не скомпилировался — ожидаемо на RED)
=== итог: FAIL=13 ===
VERDICT: FAIL
exit=1
```
Гейт корректно fail-closed на RED-состоянии (не даёт ложный PASS), при этом ВЕСЬ
существующий регресс (M-49/M-50/book/sim/strategy/portfolio/alpha/gateway/research-cli)
зелёный — RED-набор M-51 не сломал ничего чужого. Setup-guard'ы (`fn_body` literal-match,
`grep -c` с явным `${N_RD:-0}` вместо `|| echo 0`, exit-код в `run_reg`) — все присутствуют
и снабжены комментарием о конкретном инциденте ложного PASS, который они закрывают.

### 6. Контракт vs `docs/fa/journal.md`

`docs/fa/journal.md:114` (`JR-I-4`, «snapshot + tail == full replay по state_hash») и
`:124` (имя `test_snapshot_equals_full_replay`) — процитированы milestone'ом точно;
такого файла в `crates/journal/tests/` действительно нет (проверено `ls`). `det_19`
закрывает именно этот пробел (префикс+догон через границу компакции).

### 7. `contracts/` не тронут

```
$ git diff --stat origin/main...HEAD -- crates/contracts/
(пусто)
$ git diff --stat origin/main...HEAD
crates/book/tests/red_det_projection.rs       | 478
crates/journal/tests/red_det_prodscale.rs     | 241
crates/journal/tests/red_det_replay_digest.rs | 593
crates/journal/tests/red_det_restart.rs       | 232
crates/journal/tests/red_det_sources.rs       | 356
crates/sim/tests/red_det_fill_order.rs        | 323
milestones/M-51-determinism.md                | 274
scripts/verify_M-51.sh                        | 290
```
Только sacred-тесты + milestone + verify. Никакого impl-кода в дифе — RED честный.

---

## Находки

### [ADVISORY] §3 Allowed paths — `research-cli/src/export_io.rs` вне зоны engine-dev

Milestone разрешает engine-dev тронуть `crates/research-cli/src/export_io.rs` (только
waiver-коммент на det_22-хит `:283`). По `.claude/rules/scope-guard.md` таблице
владения `research-cli/src/**` — зона **research-dev**, не engine-dev (`crates/research-cli`
явно не входит в список engine-dev: «другие крейты вне своего списка» запрещены).
Правка минимальна (одна строка `// DET-OK: <причина>`, без логики) и architect уже назвал
её явно и обосновал экономией цикла — это снижает риск, но формально это исключение из
роль-таблицы `scope-guard.md`, которое ни сам scope-guard.md, ни gates.md не описывают
как санкционированный механизм (нет carve-out, аналогичного MD-only для venue-*).
Reviewer на PR-time может по букве `scope-guard.md` посчитать это нарушением зоны —
дешевле явно решить сейчас (либо founder/architect подтверждает разовое исключение в
Handoff dev'у, либо однострочный waiver выносится отдельной задачей research-dev).
Не блокирует передачу dev — но должно быть явно проговорено в Handoff architect→dev.

### [ИНФОРМАЦИОННО] `DOMAIN_CRATES` не включает `ops`/`venue-*`/`gateway-serve`

`venue-*` — осознанно исключены (Легитимное расхождение №4: граница I/O ДО журнала, не
предмет этого milestone) — согласуется с `docs/03-integration-contract.md` §4. `ops`
(watchdog/cron-состояние, не journal-derived проекция) и `gateway-serve` (грепом — ни
одного `HashMap`/`HashSet` в src, канарейка ничего не нашла бы, будь он в списке) — вне
скоупа по определению «редьюсер над потоком журнала». Не нахожу здесь дефекта, называю
для полноты аудита: если позже `ops`/`gateway-serve` станут journal-derived (например,
серию отдаёт цифры пользователю), `DOMAIN_CRATES` придётся расширить осознанно — список
уже проверяет себя (`assert!(!files.is_empty()` при переименовании крейта), но не ловит
«новый journal-derived крейт появился и не добавлен».

---

## Что НЕ проверял

- Полный `bash scripts/verify_M-51.sh` до конца при ГИПОТЕТИЧЕСКИ реализованном коде
  (dev ещё не приступал) — верификация ограничена RED-состоянием, что и есть мандат
  critic на этом этапе.
- `--release` прогон `det_12`/`det_13` под РЕАЛЬНОЙ прод-нагрузкой (27 GB/146M) — по
  контракту milestone'а это опционально и происходит на §8 eyes-on после merge, не здесь.
  Сам прогон `det_12`/`det_13` на подвыборке в составе `verify_M-51.sh` — да, прогнан
  (FAIL ожидаемо на RED HEAD, примитив отсутствует).
- Не проверял корректность SHA-256/postcard-формата контракта против альтернативных
  вариантов кодирования (например, что postcard `Event` детерминирован сам по себе —
  это существующее свойство postcard/serde, не предмет ревью M-51).
- Не оценивал производительность будущей реализации (объём вне зоны critic).

---

## Cross-ref

TD-007 · `research/measurements/td-007-determinism-coverage.md` · `docs/DESIGN.md` §0/§1/§14/§22
· `docs/fa/journal.md:114,124` · `.claude/rules/testing.md` (чек-лист деградированного
входа — все пять пунктов покрыты фикстурами, не тривиальными: асимметрия det_16/det_18,
множественность det_14/det_17/det_8, отсутствие det_6/det_16/det_18, границы det_6/det_17,
прод-масштаб det_12/det_13).
