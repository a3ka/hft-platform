<!-- FACTS: audited_head=939d1b91df081731cb114dab956a8af60f93cf2c collected=2026-09-27 -->
# M-89 — замер: первый `pump` после `resume` из прогретого слепка читает активный сегмент с начала

architect-клон (Fable, свежий контекст), 2026-09-27 ~12:10–12:25Z. Только чтение прода, ничего не закоммичено.
Ревизия кода эксперимента: `origin/main` = `939d1b9` (Merge PR #230 M-87 close-out); локальный `main` чекаута = `9f6d55a`.
Замер: `git worktree add --detach /tmp/hft-arch-m89-measure origin/main` → `HEAD is now at 939d1b9`.

## ВЕРДИКТ

**Гипотеза ПОДТВЕРЖДЕНА.** После `LiveReducer::resume` из валидного слепка `tail_hint = None`
(`crates/gateway/src/lib.rs:4910` на `9f6d55a`; на `939d1b9` та же строка сдвинута ниже — см. блок
`resume` ~`:5060-5110`), и первый `pump` открывает активный сегмент с `header_end`, то есть с
НАЧАЛА: `events_scanned` первого pump = `N_сегмента + хвост` РОВНО (2 500 / 200 500 / 400 500 при
хвосте 500), `rchar` ≈ размер всего файла сегмента (20 620 325 Б при файле 20 554 613 Б).
Работа первого pump линейна по длине активного сегмента при неизменном хвосте:
**×2.007 по rchar при ×2 длине** (200k→400k), **×109 по rchar / ×160 по events_scanned при ×200
длине** (2k→400k). Второй и третий pump — дешёвые и константные: `events_scanned=3`,
`rchar_delta ≈ 65.9 КБ` независимо от N (2k / 200k / 400k).

Проекция на прод (12:00Z слепок, 12:11Z замер): курсор слепка `712 075 015`, `first_seq`
активного сегмента 839 = `711 770 132` ⇒ **304 883 события / ≈ 412 МБ** (при ≈1 351 Б/событие
по сегменту 838) лежат ПЕРЕД курсором внутри активного сегмента и читаются первым pump каждой
новой подписки; хвост ПОСЛЕ слепка при каденсе прогрева 15 мин — порядка 30–90 тыс. событий.
Перед ротацией (1 ГиБ) префикс достигает ≈ 800 тыс. событий / 1 ГБ.

## 1. Прод-форма (read-only ssh, 2026-09-27 12:11Z)

```
$ ssh … root@167.233.192.131 'ls -la $V | tail -8; ls $V | grep -c jrnl; cat $V/journal.meta; ls -la ckpt-том; ls /etc/cron.d; …'
-rw-r--r-- 1 root root  106929672 Sep 27 03:50 segment-00000832.jrnl.zst
-rw-r--r-- 1 root root   96987993 Sep 27 03:50 segment-00000833.jrnl.zst
-rw-r--r-- 1 root root 1073740384 Sep 27 00:11 segment-00000834.jrnl
-rw-r--r-- 1 root root 1073741537 Sep 27 03:19 segment-00000835.jrnl
-rw-r--r-- 1 root root 1073719072 Sep 27 06:06 segment-00000836.jrnl
-rw-r--r-- 1 root root 1073741730 Sep 27 08:36 segment-00000837.jrnl
-rw-r--r-- 1 root root 1073717859 Sep 27 10:54 segment-00000838.jrnl
-rw-r--r-- 1 root root  585773911 Sep 27 12:09 segment-00000839.jrnl     ← АКТИВНЫЙ, сырой
447                                                                       ← файлов *jrnl* в томе
journal.meta: 5133 722a 0000 0000  → next_seq = 0x2a723351 = 712 061 777 (мета ОТСТАЁТ от covered_through_seq — по докам «возможно отставшая», TD-011)
journal.legacy.json journal.meta journal.replay-digest.json recorder.heartbeat   ← служебные файлы; sidecar'ов курсора НЕТ
df: /dev/sda1 150G 110G 35G 77%
```

Форма: закрытые 834–838 — СЫРЫЕ по 1 ГиБ (ещё не компактированы; компакция cron 03:50), старше —
`.zst` ~100 МБ; ротация по `max_segment_bytes ≈ 1 073 741 xxx` (1 ГиБ). Активный 839: 585.8 МБ в
12:09, 597.1 МБ в 12:11 (`stat`: `597140074 2026-09-27 12:11:16`) — рост ≈ 5.7 МБ/мин.

Заголовки (`head -c 160 | xxd`, декодированы `postcard::from_bytes::<SegmentHeader>` в бинаре
эксперимента, вручную сошлось до байта):

```
segment-00000834.jrnl: first_seq=707261091 created_wall_ms=1790453049606 epoch=own-2026-09-m45-ethusdt schema=4 | first frame len=3977
segment-00000838.jrnl: first_seq=710975367 created_wall_ms=1790498208436 epoch=own-2026-09-m45-ethusdt schema=4 | first frame len=109
segment-00000839.jrnl: first_seq=711770132 created_wall_ms=1790506483632 epoch=own-2026-09-m45-ethusdt schema=4 | first frame len=26648
```

Число событий (метод: разность `first_seq` соседних заголовков; активный — от
`covered_through_seq`/оценка по байтам, журнал НЕ читался):
- сегмент 838: `711 770 132 − 710 975 367 = 794 765` событий на 1 073 717 859 Б ⇒ **≈ 1 351 Б/событие**;
  834–838 (5 сегментов): `4 509 041` событий ⇒ ≈ 902 тыс./сегмент, ≈ 1 190 Б/событие.
- активный 839 на момент слепка 12:00: `712 075 015 − 711 770 132 = 304 883` события ПЕРЕД курсором;
  на 12:11 по байтам `597 140 074 / 1 351 ≈ 442 тыс.` (по темпу 838: 5.76 тыс./мин × 77 мин ≈ 443 тыс. — сходится).

Слепки (`hft-platform_gateway-ckpt`, монтируется в cron как `/ckpt`):
```
-rw-r--r-- 1 root root 3298205 Sep 10 21:31 HIDDEN.bak
-rw-r--r-- 1 root root 2516926 Aug 30 12:00 ckpt-2a00318f774d9689.bin
-rw-r--r-- 1 root root 3636143 Sep 23 12:45 ckpt-8f69809dd707e8c9.bin
-rw-r--r-- 1 root root 2050975 Sep 27 12:00 ckpt-b0f1ed89ec2ec142.bin   ← живой (fingerprint текущего Selector)
-rw-r--r-- 1 root root       9 Sep 27 12:00 covered_through_seq          → "712075015"
-rw-r--r-- 1 root root       9 Sep 20 19:43 covered_through_seq.warm7
-rw-r--r-- 1 root root       0 Sep 27 12:00 zz.lock
```
Заголовок живого слепка (scp копии 2 МБ, разобран `postcard::from_bytes::<checkpoint::CkptHeader>`; копия удалена):
```
size=2050975 magic="HFTCKP01" ckpt_v=3 gw_v=11
header_len=30449 cursor=Cursor { upto_seq: Some(712075015) } history_start_seq=332493724 history_truncated=true lineage_len=447
  lineage tail: first_seq=711770132 … / 710975367 … / 710163297 …
```
Заголовок слепка несёт ТОЛЬКО `cursor` (seq) — ни `pos`, ни `tail_hint` (`CkptHeader` поля:
`crates/gateway/src/lib.rs:4041-4065` на `9f6d55a`: magic, ckpt_schema_version,
gateway_schema_version, selector_fingerprint, epoch_filter_fingerprint, journal_lineage, cursor,
history_start_seq, history_truncated).

Расписание прогрева (`/etc/cron.d/hft-journal-retention`):
```
*/15 * * * * root flock -n /var/lock/hft-gateway-checkpoint.lock /root/hft-platform/deploy/bin/gateway-checkpoint-cron.sh
```
`deploy/bin/gateway-checkpoint-cron.sh`: `docker compose run --rm gateway-checkpoint`, `CHECKPOINT_WINDOW_MS=60000`,
`CHECKPOINT_CURSOR=LATEST`, out `/ckpt/covered_through_seq`. Лог (`/var/log/hft/gateway-checkpoint.log`):
```
gateway-checkpoint: ok … achieved_cursor=Cursor { upto_seq: Some(712006674) } covered=712006674
gateway-checkpoint: ok … achieved_cursor=Cursor { upto_seq: Some(712075015) } covered=712075015
```
(два последних прогона: +68 341 событий за 15 мин ⇒ хвост после слепка ≤ ~70–90 тыс. событий / ~100 МБ.)

## 2. Локальный эксперимент

Временный пример `crates/gateway/examples/m89_measure.rs` в worktree (удалён, не коммитился).
Форма: один активный сегмент (`max_segment_bytes = 1<<30`, как прод), трейды BTCUSDT; Selector как у
прод-cron (`tf=1000, bands=[0.001], window_ms=Some(60000)`); слепок `checkpoint::advance_to(…, Cursor::LATEST)`
у конца тела; затем хвост 500 событий; затем `resume` → `pump#1` → (+3 событий) `pump#2` → (+3) `pump#3`.
Мера: `ReadStats` и `rchar` из `/proc/self/io` до/после каждого шага. Сборка: `cargo build -p gateway --example m89_measure --release` → exit 0.

```
$ ./target/release/examples/m89_measure 500 2000 200000 400000
pid=2210808 rchar0=7670

===== N=2000: n_big=2000 tail=500 =====
advance_to: achieved=Cursor { upto_seq: Some(1999) } in 19.0137ms
ckpt dir bytes = 46282
segments: [("segment-00000000.jrnl", 123053)]
RESUME  : rchar_delta=46500    elapsed=355µs   | events_scanned=0      events_decoded=0   segments_opened=0 segment_meta_ops=0 payload_bytes_read=46282
PUMP#1  : rchar_delta=188753   elapsed=1.75ms  frames=2 cursor=Some(2499)   | events_scanned=2500   events_decoded=500 segments_opened=1 segment_meta_ops=7
PUMP#2  : rchar_delta=65893    elapsed=194µs   frames=1 cursor=Some(2502)   | events_scanned=3      events_decoded=3   segments_opened=1 segment_meta_ops=3
PUMP#3  : rchar_delta=65893    elapsed=109µs   frames=1 cursor=Some(2505)   | events_scanned=3      events_decoded=3   segments_opened=1 segment_meta_ops=3

===== N=200000: n_big=200000 tail=500 =====
advance_to: achieved=Cursor { upto_seq: Some(199999) } in 14.65s
ckpt dir bytes = 1053649
segments: [("segment-00000000.jrnl", 10207543)]
RESUME  : rchar_delta=1053876  elapsed=3.75ms  | events_scanned=0      events_decoded=0   segments_opened=0 segment_meta_ops=0 payload_bytes_read=1053649
PUMP#1  : rchar_delta=10273252 elapsed=73.5ms  frames=2 cursor=Some(200499) | events_scanned=200500 events_decoded=500 segments_opened=1 segment_meta_ops=7
PUMP#2  : rchar_delta=65909    elapsed=1.24ms  frames=1 cursor=Some(200502) | events_scanned=3      events_decoded=3   segments_opened=1 segment_meta_ops=3
PUMP#3  : rchar_delta=65909    elapsed=970µs   frames=1 cursor=Some(200505) | events_scanned=3      events_decoded=3   segments_opened=1 segment_meta_ops=3

===== N=400000: n_big=400000 tail=500 =====
advance_to: achieved=Cursor { upto_seq: Some(399999) } in 32.74s
ckpt dir bytes = 1053649
segments: [("segment-00000000.jrnl", 20554613)]
RESUME  : rchar_delta=1053879  elapsed=3.81ms  | events_scanned=0      events_decoded=0   segments_opened=0 segment_meta_ops=0 payload_bytes_read=1053649
PUMP#1  : rchar_delta=20620325 elapsed=105.6ms frames=2 cursor=Some(400499) | events_scanned=400500 events_decoded=500 segments_opened=1 segment_meta_ops=7
PUMP#2  : rchar_delta=65913    elapsed=1.79ms  frames=1 cursor=Some(400502) | events_scanned=3      events_decoded=3   segments_opened=1 segment_meta_ops=3
PUMP#3  : rchar_delta=65913    elapsed=1.01ms  frames=1 cursor=Some(400505) | events_scanned=3      events_decoded=3   segments_opened=1 segment_meta_ops=3
```
(полный сырой вывод — `scratchpad/run-exp1.txt`, прод-декод — `scratchpad/run-prod.txt`.)

### Разбор

| N активного | хвост | pump#1 events_scanned | pump#1 rchar | файл сегмента | pump#2 rchar |
|---|---|---|---|---|---|
| 2 000 | 500 | 2 500 | 188 753 | 123 053 | 65 893 |
| 200 000 | 500 | 200 500 | 10 273 252 | 10 207 543 | 65 909 |
| 400 000 | 500 | 400 500 | 20 620 325 | 20 554 613 | 65 913 |

- `events_scanned(pump#1) = N + tail` ТОЧНО, `events_decoded = tail` — парсер прочёл весь сегмент, редьюсеру отдан только хвост.
- `rchar(pump#1) = размер файла + ≈ 65.7 КБ` (64 КиБ `BufReader` заголовка + probe + read_dir) — чтение ФАЙЛА ЦЕЛИКОМ.
- Множитель по длине сегмента: 200k→400k: rchar ×2.007, scanned ×1.998; 2k→400k: rchar ×109, scanned ×160 (при ×200 длине; хвост и константа 65 КБ размывают малую точку).
- Множитель «работа / нужная работа» при N=400k: прочитано 20.6 МБ ради хвоста ≈ 25 КБ (500 событий × ~51 Б) ⇒ ≈ ×800; по событиям 400 500 / 500 = ×801.
- Установившийся тик (pump#2, #3): `events_scanned=3`, `rchar ≈ 65.9 КБ` — **константа, от N не зависит** (2k/200k/400k: 65 893 / 65 909 / 65 913). Механизм `tail_hint` работает со второго тика. (65.9 КБ — это 64 КиБ-буфер `PositionedBufReader::open` при чтении заголовка, `segments.rs:1494`, не сам хвост; отдельный, малый предмет.)
- `resume` сам журнал не читает (`events_scanned=0`, rchar = размер слепка) — верно докам (`lib.rs:4917-4921`).

Существующий оракул `crates/gateway/tests/red_tick_read_cost.rs` (`f036`) этот дефект НЕ ловит по построению:
он делает `resume` БЕЗ слепка, догоняет хвост несколькими pump'ами (первый из них и есть дорогой) и меряет
только СЛЕДУЮЩИЙ тик (`tick_read_bytes`, `:191-236`) — то есть ровно установившийся тик, который и здесь дёшев.

Проекция на прод при 1 351 Б/событие: первый pump новой подписки читает `(cursor − first_seq_active) × 1351`:
сразу после прогрева в 12:00 — 304 883 × 1 351 ≈ **412 МБ**; за 15 мин до ротации — ≈ 1 ГиБ; сразу после
ротации — почти 0. Хвост после слепка (≤ 15 мин) — 30–90 тыс. событий, что при 65 КБ/тик и `PUSH_MAX_EVENTS=256`
и так дренируется курсором; лишняя работа первого pump — от ×4 до ×30 относительно самого хвоста и до ×10⁴
относительно одного тика.

## 3. Есть ли уже механизм сдвига внутри сегмента по курсору (seq→offset)?

**Нет seq→offset индекса, sidecar'а или `pos` в слепке.** Есть только in-memory `TailHint` СЕССИИ:
- `crates/journal/src/segments.rs:132-136` — `pub struct TailHint { seg_idx, last_seq, pos }`; `:110-131` — история: файловый sidecar `journal.tail-offset` удалён (M-57 круг 2, `R-035` F-035-1/2: `:ro`-том, драка сессий).
- `segments.rs:1656-1716` — `resolve_active_start_offset`: без hint (`None`) → `header_end` (`:1676-1679`); hint применяется только при `seg_idx` равном, `pos ≥ header_end`, `after ≥ last_seq`, `pos ≤ len`, и `probe_frame_boundary` (`:1717-1731`) подтверждает границу кадра по CRC.
- `segments.rs:1984-2058` — `stream_from_at_with_catalog`: сегменты, чей `next_first − 1 ≤ after`, отбрасываются целиком (`:2028-2033`); сегмент, СОДЕРЖАЩИЙ курсор, остаётся и читается от начала (внутрисегментный фильтр `ev.seq <= after → continue`, `:1787-1791`).
- `crates/gateway/src/lib.rs:4841` (`9f6d55a`) — `LiveReducer.tail_hint: Option<TailHint>`; `:4910` и `:4972` — оба пути `resume` ставят `tail_hint: None` с комментарием «hint обязан быть заполнен ПЕРВЫМ ЖЕ pump'ом… прямо здесь его вычислить нельзя»; `:5213` — `self.tail_hint = stream.tail_hint().or(self.tail_hint)`.
- `crates/gateway/src/lib.rs:4041-4065` — `CkptHeader` несёт `cursor: Cursor` (seq), поля `pos`/hint нет; `read_checkpoint` (`:4502-4515`) / `read_checkpoint_header` (`:4552-4574`) читают только заголовок и state.
- `crates/gateway/src/lib.rs` (`939d1b9`) `:3881-3895` — `payload_bytes_after_cursor` (M-87 задача 23) считает ТОЛЬКО сегменты с `first_seq > cursor` — активный сегмент, содержащий курсор, в «честной верхней границе прочитанного» не учтён вовсе; это объясняет `counter 11 157 < rchar 57 711` в `R-202` и след «хвост 466 Б, прочитано 57 711 Б» (в той фикстуре сегмент 8 КиБ, так что превышение складывается из полного чтения активного + заголовков 24 сегментов + tail-scan, а не из одного большого сегмента).
- Примитивы, из которых можно собрать позиционирование по seq без индекса, в коде ЕСТЬ: `scan_tail_for_last_seq` (`crates/journal/src/lib.rs:363-…`, чанк `TAIL_SCAN_CHUNK = 4 MiB`, `lib.rs:53`) — чтение с произвольного байтового смещения с байт-ресинком по валидному `[len][payload][crc]`; `probe_frame_boundary` (`segments.rs:1717`); `PositionedBufReader::open(path, start_offset)` (`segments.rs:1490-1502`); `RawFrameVerifier::open_at` (`segments.rs:2812-2826`). Ни один из них не используется для сдвига ПЕРВОГО pump'а.

## Уборка

```
$ ls /tmp | wc -l            → 19177 (до)  · 19178 (после worktree add)
$ du -sh /tmp/hft-arch-m89-measure/target → 172M ; rm -rf …/target
$ rm -f …/crates/gateway/examples/m89_measure.rs ; git status --porcelain → пусто
$ git worktree remove /tmp/hft-arch-m89-measure → WORKTREE_REMOVED ; ls -d /tmp/hft-arch-m89-measure → No such file
$ rm -f scratchpad/prod-ckpt.bin (копия прод-слепка удалена)
$ ls /tmp | wc -l            → 19178 (после): +1 против старта — НЕ мой: новейшие записи `ls -td /tmp/* | head -3` = hft-critic-ci-aggregate, hft-arch-ci, hft-arch-m89 (чужие деревья соседних сессий); моего каталога нет.
$ git -C /home/nous/hft-platform status --porcelain → только `?? .omc/` (было на старте)
```
Оставлены в scratchpad: `m89-measure.md` (этот отчёт), `run-exp1.txt`, `run-prod.txt`.
