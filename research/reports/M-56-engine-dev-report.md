# M-56 engine-dev — снапшот без клонирования состояния (`TD-097`)

**Статус:** DONE — задачи 1 и 2 реализованы, GREEN, готово к reviewer.
**Ветка:** `feat/M-56-snapshot-noclone` (worktree `/tmp/hft-dev-m56`, detached HEAD на
`origin/feat/M-56-snapshot-noclone`, стартовый коммит `3b01077`).

## 0. Что сделано

Задача 1: `Reducer::finish_ref(&self) -> SeriesBundle` — построение `SeriesBundle` из ссылок
на `self`, без `.into_iter()`/потребления состояния. Вспомогательные функции переведены на
приём по ссылке (сигнатуры, не логика):
- `build_heatmap_and_cob(&Selector, &BTreeMap<i64, HeatmapBucketState>)` (было
  `BTreeMap<...>` по значению);
- `build_volume_bubbles(&BTreeMap<(i64, i64), (i64, i64)>)`;
- `VolumeProfileAcc::into_rows(self)` → переименован в `vp_rows(&self)` (не `into_rows_ref`,
  чтобы не нарушать clippy `wrong_self_convention` — `into_*` обязан брать `self` по значению,
  а этот метод больше self не потребляет);
- `compute_vp_row(session_id: i64, hist: &BTreeMap<i64, i128>)` (было по значению).

Задача 2: `LiveReducer::snapshot(&self)` теперь зовёт `self.full.finish_ref_with_at()` вместо
`self.full.clone().finish_with_at()` — клон исчез с пути построения снапшота.

## 1. `finish_ref` vs `finish` — ОДНА формула, не две копии

Важный момент, на который явно указывает задача: `Reducer::finish(self)` **не дублирует**
логику `finish_ref`, а выражена ЧЕРЕЗ неё:

```rust
fn finish(self) -> SeriesBundle {
    self.finish_ref()
}

fn finish_ref(&self) -> SeriesBundle {
    // вся формула (OHLCV/CVD/depth/VWAP/VP/heatmap-COB/bubbles) живёт ЗДЕСЬ
    ...
}
```

То есть тело расчёта существует РОВНО в одном месте (`finish_ref`), а `finish(self)` — тонкая
обёртка, которая просто дропает `self` после вызова (ownership `self` телу расчёта был не
нужен — он был нужен только старому `.into_iter()`-стилю, не самому алгоритму). Это устраняет
риск, названный в задаче постановщиком: «дублируется логика или переиспользуется — это важно,
две копии формул разойдутся молча» — здесь копий НЕТ, разойтись нечему.

Аналогично `finish_with_at(self)` и новый `finish_ref_with_at(&self)` — оба тонкие обёртки
вокруг соответствующего `finish`/`finish_ref` + `self.at_ms`.

## 2. Мутационный контроль (обязателен по milestone §3)

Временно вернул `self.full.clone().finish_with_at()` в `LiveReducer::snapshot()`:

```
$ cargo test -p gateway --test red_snapshot_noclone o1_
test o1_snapshot_allocation_does_not_grow_with_state ... FAILED
ЗАМЕР O-1: книга ×8 → аллокации 151089 → 854705 (×5.66), выход 42847 → 42847 (×1.00)
```

O-1 покраснел, как и требуется — оракул реагирует на клон, не проходит его молча. Мутацию
откатил, прогнал снова:

```
$ cargo test -p gateway --test red_snapshot_noclone o1_
test o1_snapshot_allocation_does_not_grow_with_state ... ok
```

Оракул валиден.

## 3. Замер (мой собственный прогон, не переиспользую замер founder'а из инвокации)

```
ЗАМЕР O-1: книга ×8 → аллокации 151089 → 854705 (×5.66), выход 42847 → 42847 (×1.00)
```
(это цифры из мутационного прогона — на текущем, немутированном коде O-1..O-3 GREEN, см.
Done Block ниже; конкретные числа немутированного прогона — в `verify_M-56.sh` выводе, PASS
без детализации байт в консоли verify).

## Done Block

```
$ git status --porcelain (после отката мутации, до коммита)
 M crates/gateway/src/lib.rs

$ git log -1 --oneline
3b01077 test(M-56): RED — снапшот копирует состояние, +404 ms константы на проде (TD-097)

$ cargo test -p gateway --test red_snapshot_noclone o1_ 2>&1 | tail -6
test o1_snapshot_allocation_does_not_grow_with_state ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.27s

$ bash scripts/verify_M-56.sh; echo "exit=$?"
PASS  T0 crates/gateway/tests/red_snapshot_noclone.rs
PASS  T1 build --workspace
PASS  T2 clippy --workspace --all-targets -D warnings
PASS  T2b fmt --check
PASS  T3 O-1..O-3 GREEN
PASS  T4 в теле snapshot() нет клонирования редьюсера
PASS  T4 finish_ref(&self) существует — построение по ссылке
PASS  T5 M-53/M-54 GREEN (цена тика и единый проход целы)
PASS  T5 gateway-serve GREEN (сверка WS↔реплей цела)
PASS  T6 crates/contracts/** не тронут
VERDICT: PASS
exit=0

$ cargo test -p gateway --test red_connect_cost_single --test red_frames_seek_bound --test red_push_seek_bounded 2>&1 | grep -E "^test result"
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.98s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.86s

$ cargo test -p gateway-serve 2>&1 | grep -E "^test result"
(14 блоков, все ok, 0 failed)
```

## Что дальше

reviewer: scope (`crates/gateway/src/lib.rs` + carve-out статус-колонка milestone'а + этот
отчёт — соответствует Allowed paths), Done Block, затем §6 milestone'а — прогон против прода
по протоколу R-029 §C (три точки backlog'а, CPU<5%, предсказание vs факт) — это НЕ моя зона
(read-only на прод у engine-dev нет мандата в этом задании).
