<!-- GATE-META
milestone: TD-250
audited_repo: a3ka/hft-platform
audited_base: ad20a7e7b844bf3882d846ebb66d3b049753c74b
audited_head: 17a25548c031e891b43aed03ebff2b27aacccdd6
verdict: REJECT
-->

# C-290 — TD-250: `w1` «разница вместо отношения» — адверсарий харнесс-трека

**Предмет.** Ветка `harness/td250-w1-difference`, вершина на момент аудита
`17a25548c031e891b43aed03ebff2b27aacccdd6` (`git rev-parse origin/harness/td250-w1-difference`),
merge-base с `origin/main` = `ad20a7e7…` (= `origin/main`). Один коммит, один файл:
`crates/gateway/tests/red_m89_warm_resume_seek.rs` — мир `w1`, проверка отношения `rchar`
(≤ 2.0×) заменена проверкой разницы `big − small ≤ MAX_PREFIX_DEPENDENT_BYTES = 512 КиБ`.

**Роль.** Адверсарий (`docs/workflow/harness-track.md` §3), свежий контекст, не автор правки.
Дерево — `/tmp/hft-adv-td250` (detached на `17a2554`), `CARGO_TARGET_DIR` внутри него.
Все мутанты временные, откачены (`git status --porcelain` в Done Block).

**Живой инвариант тронутого модуля** (M-66): предмет — тест `crates/gateway/tests/**`; оракул
держит `I-1` спеки `docs/archive/M-89-s0-volume-guard-observability.md` §4 (работа первого
`pump` ∝ хвосту, не префиксу) и опирается на `JR-I-2` (`docs/fa/journal.md`: `seq` без дыр —
гард точности `seek_back_from_tail`, `segments.rs:3724`). Из `docs/fa/viz-backend.md` —
`VB-I-2` (эталон `assert_identical_to_reference`).

## Вердикт: REJECT

Правка убирает ОДНУ из четырёх комбинаций флака и оставляет вторую. Обоснование в константе
— «добавка ПОСТОЯННА, с длиной префикса не растёт» — ложно: на большом префиксе та же добавка
равна **≈594 КБ**, не 185 КБ, и это число уже стояло в карточке `TD-250` (CI: 766 623 Б) до
того, как автор писал константу. Разница «большой в откате − малый на быстром пути» =
**594 665 Б > 524 288** → оракул красный на честной реализации. Детали — Б-1. Остальное
(Б-2 — источник добавки, Б-3 — честность названного предела, Н-1…Н-3) — ниже.

## Б-1 (блокер). Добавка не постоянна: на большом файле ≈594 КБ, и новая мера флакует там же, где старая

**Что обещано** (doc-комментарий `MAX_PREFIX_DEPENDENT_BYTES`, `red_m89_warm_resume_seek.rs:84-103`):
«≈164 КБ или ≈349 КБ (лишние ≈185 КБ) … на МАЛОМ и на БОЛЬШОМ префиксе одинаково … Добавка
ПОСТОЯННА — с длиной префикса не растёт».

**Что есть.** Добавка — это откат быстрого пути `seek_back_from_tail` на
`cheap_tail_last_seq` (64 КиБ, `segments.rs:2713`) + бисекцию `locate_after_seq`
(`segments.rs:2727`, пробы по 64 КиБ, число проб ∝ `log2(файл)`). На файле 120 КБ бисекция
стоит ≈120 КБ (окно сужается до размера файла), на файле 10 МиБ — ≈530 КБ. Замер:

| режим | small (120 КБ) | big (10 МиБ) | добавка |
|---|---|---|---|
| быстрый путь | 163 657 | 164 158 | — |
| откат → бисекция | 349 082…349 114 | **758 322** (мутант A, сырой вывод ниже) / **766 623** (CI, `TD-250`) | **185 425** / **594 164** |

Число 766 623 из карточки `TD-250` — это и есть «большой в откате»: 766 623 − 164 158 =
602 465 ≈ 9 проб × 64 КиБ + 64 КиБ. Автор процитировал карточку в константе и не сверил
её число со своей моделью «164/349».

**Четыре комбинации мод и исход обеих мер:**

| small | big | старое отношение | новая разница |
|---|---|---|---|
| быстрый 164 К | быстрый 164 К | 1.00 → PASS | 501 Б → PASS |
| откат 349 К | быстрый 164 К | 0.47 → PASS | 0 (saturating) → PASS |
| откат 349 К | откат 758 К | **2.17 → FAIL** (CI 2.20) | 409 208 → PASS |
| быстрый 164 К | откат 758 К | **4.62 → FAIL** | **594 665 → FAIL** |

Правка закрыла третью строку (ровно тот прогон CI, что завёл `TD-250`) и оставила четвёртую.
Локально малый уходит в откат в 6 из 12 прогонов, большой — в 0 из 12; в CI большой ушёл в
откат (766 623). На хосте, где большой промахивается, новая мера красна примерно в половине
прогонов — в тех, где малый попал на быстрый путь.

**Предъявление (мутант A2 — большой промахивается принудительно, окно малого сдвинуто на 4 КиБ
назад, чтобы гарантировать попадание):**

```
$ cargo test -p gateway --test red_m89_warm_resume_seek -- --exact w1_first_pump_after_warm_resume_costs_tail_not_prefix --nocapture 2>&1 | grep -E 'ADV-|test result|panicked|w1 / I-1'
ADV-EST file_len=120986 header_end=64 first_seq=0 last_seq=2499 after=1999 avg=48 approx_pos=96986
ADV-MUT A2: small window shifted back to 92890
ADV-PATH fast pos=97024
ADV-EST file_len=10060150 header_end=64 first_seq=0 last_seq=200499 after=199999 avg=50 approx_pos=10035150
ADV-MUT A2: forced miss on big
ADV-PATH fallback-after-fast-miss
ADV-PATH bisect pos=10035686
ADV-W1 small=163655 big=758657 small_fb=0 big_fb=0 small_file=120994 big_file=10060158 small_tail=23962 big_tail=24464
thread 'w1_first_pump_after_warm_resume_costs_tail_not_prefix' (1169878) panicked at crates/gateway/tests/red_m89_warm_resume_seek.rs:404:5:
w1 / I-1: первый pump прочитал 758657 Б при префиксе 200000 против 163655 Б при 2000 — на 595002 Б больше при 100-кратной разнице длины (допуск 524288 Б). Работа первого тика растёт с длиной активного сегмента — замер 2026-09-27: ×109 при ×200.
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 11 filtered out; finished in 100.86s
exit=101

```

**Почему это не «искусственный мутант», а воспроизведение природной моды.** Мутант A2 не
меняет объём чтения ни одного из путей — он лишь ВЫБИРАЕТ, какой из двух честных путей
(оба уже в `main`) сработает на каждом файле. Ту же пару мод CI выбрал сам (`TD-250`:
766 623 / 348 764); какая пара выпадет — решает рулетка Б-2.

**Мутант A (промах только на большом, малый — как выпадет; в этом прогоне выпал откат — ровно пара CI):**

```
ADV-EST file_len=119962 header_end=64 first_seq=0 last_seq=2499 after=1999 avg=47 approx_pos=96462
ADV-PATH fallback-after-fast-miss
ADV-PATH bisect pos=95998
ADV-EST file_len=10017781 header_end=64 first_seq=0 last_seq=200499 after=199999 avg=49 approx_pos=9993281
ADV-MUT A: forced miss on big
ADV-PATH fallback-after-fast-miss
ADV-PATH bisect pos=9993317
ADV-W1 small=349114 big=758322 small_fb=0 big_fb=0 small_file=119970 big_file=10017789 small_tail=23964 big_tail=24464
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 138.17s
exit=0
```

## Б-2. Источник добавки — `seek_back_from_tail`: окно поиска одностороннее, оценка округлена вниз

`crates/journal/src/segments.rs`:

- `:3658` `avg_frame_size = (file_len − header_end) / total_events` — целочисленный **floor**;
- `:3661-3665` `approx_pos = file_len − events_back × avg` — оценка ОТ КОНЦА;
- `:3672` окно `8 КиБ` читается **от `approx_pos` вперёд**, назад — 0 байт;
- `:3724` первый валидный кадр с `seq > target` ⇒ `Ok(None)` ⇒ откат.

Следствие: быстрый путь попадает, только если `events_back × floor(avg)` ≤ фактической длине
хвоста, т.е. если хвостовые кадры в среднем НЕ длиннее `floor(avg)`. Запас — доли байта на
кадр. Замер по 12 прогонам (`ADV-EST`/`ADV-PATH`, временный `eprintln!`):

```
# 12 прогонов честной реализации; строка ADV-W1 — rchar_delta малого/большого; margin = found − approx_pos
run  1 fast  approx=96154 found=96190 margin=+36
run  1 fast  approx=9993858 found=9993894 margin=+36
ADV-W1 small=163657 big=164158 small_fb=0 big_fb=0 small_file=120162 big_file=10018366 small_tail=23964 big_tail=24464
run  2 fast  approx=96667 found=96703 margin=+36
run  2 fast  approx=9993919 found=9993956 margin=+37
ADV-W1 small=163657 big=164157 small_fb=0 big_fb=0 small_file=120675 big_file=10018427 small_tail=23964 big_tail=24463
run  3 fast  approx=96155 found=96192 margin=+37
run  3 fast  approx=9993282 found=9993318 margin=+36
ADV-W1 small=163656 big=164158 small_fb=0 big_fb=0 small_file=120163 big_file=10017790 small_tail=23963 big_tail=24464
run  4 fast  approx=96284 found=96320 margin=+36
run  4 fast  approx=9993473 found=9993509 margin=+36
ADV-W1 small=163657 big=164158 small_fb=0 big_fb=0 small_file=120292 big_file=10017981 small_tail=23964 big_tail=24464
run  5 fast  approx=96092 found=96128 margin=+36
run  5 fast  approx=9993217 found=9993253 margin=+36
ADV-W1 small=163657 big=164158 small_fb=0 big_fb=0 small_file=120100 big_file=10017725 small_tail=23964 big_tail=24464
run  6 MISS  approx=96524 found=96062 margin=-462
run  6 fast  approx=9993409 found=9993445 margin=+36
ADV-W1 small=349082 big=164158 small_fb=0 big_fb=0 small_file=120032 big_file=10017917 small_tail=23962 big_tail=24464
run  7 MISS  approx=96445 found=95982 margin=-463
run  7 fast  approx=9993408 found=9993444 margin=+36
ADV-W1 small=349064 big=164158 small_fb=0 big_fb=0 small_file=119953 big_file=10017916 small_tail=23963 big_tail=24464
run  8 MISS  approx=96462 found=95999 margin=-463
run  8 fast  approx=10011139 found=10011430 margin=+291
ADV-W1 small=349112 big=164403 small_fb=0 big_fb=0 small_file=119970 big_file=10036147 small_tail=23963 big_tail=24709
run  9 fast  approx=97051 found=97088 margin=+37
run  9 fast  approx=10082639 found=10083175 margin=+536
ADV-W1 small=163656 big=164158 small_fb=0 big_fb=0 small_file=121059 big_file=10107647 small_tail=23963 big_tail=24464
run 10 MISS  approx=96460 found=95998 margin=-462
run 10 fast  approx=10051661 found=10052197 margin=+536
ADV-W1 small=349109 big=164161 small_fb=0 big_fb=0 small_file=119968 big_file=10076669 small_tail=23962 big_tail=24464
run 11 MISS  approx=96460 found=95998 margin=-462
run 11 fast  approx=10027981 found=10028517 margin=+536
ADV-W1 small=349106 big=164158 small_fb=0 big_fb=0 small_file=119968 big_file=10052989 small_tail=23962 big_tail=24464
run 12 MISS  approx=96462 found=96000 margin=-462
run 12 fast  approx=9993217 found=9993253 margin=+36
ADV-W1 small=349107 big=164158 small_fb=0 big_fb=0 small_file=119970 big_file=10017725 small_tail=23962 big_tail=24464

```

Запас быстрого пути — **+36 Б** (= `500 × (48 − 47.93)`) либо промах на **−462 Б**
(= `500 × (47 − 47.93)`): одно и то же, в какую сторону округлился `avg`. Что двигает `avg`
через целое — длина varint'а `ts_mono_ns` (`crates/journal/src/lib.rs:206-213`: наносекунды от
`epoch` открытия журнала, у префикса и у хвоста эпохи РАЗНЫЕ — хвост пишется после
`open_with`), то есть **скорость записи фикстуры**, то есть хост. Отсюда и «мода закреплена за
фикстурой» у автора, и «пауза перед pump'ом ничего не меняет» — мода решена в момент ЗАПИСИ.

Почему большой локально не промахивается, а в CI промахнулся: 200 тыс. событий здесь пишутся
> 0.27 с, и большинство кадров префикса несут 5-байтовый `ts_mono_ns` против 3–4 байт у
хвоста — `avg` стабильно выше хвоста на ~1 байт, запас +36…+536 Б. На хосте, где та же запись
укладывается в 0.27 с (4-байтовые varint'ы), `floor(avg)` падает до 48 и большой промахивается.
**Мода большого — свойство хоста CI**, не фикстуры (свойство 2 «Целостности гейта»).

Это дефект РЕАЛИЗАЦИИ, не оракула: быстрый путь с односторонним окном и floor-оценкой
промахивается при ошибке оценки в +1 байт и платит за промах 10–14 проб бисекции (на проде при
1 ГиБ сегменте ≈ 1 МиБ). Исправление — `crates/journal/src/**`, engine-dev, полный цикл
(`harness-track.md` §4: `crates/**` трек не покрывает). Оракул обязан это либо ЛОВИТЬ, либо
ЯВНО назвать допуском — правка делает ни то, ни другое: порог 512 КиБ рассчитан на добавку 185 КБ,
а добавка 594 КБ его пробивает.

## Б-3. Названный предел «< 5 % префикса не ловится» — честен числом, но порог куплен не той ценой

Мутант C: честный быстрый путь + чтение 4 % префикса с `header_end` (`(file_len − header_end)/25`).

```
$ # мутант C: в seek_back_from_tail дополнительно читается (file_len − header_end)/25 байт с header_end
ADV-MUT C: read 4836 B of prefix (4%)
ADV-MUT C: read 403878 B of prefix (4%)
ADV-W1 small=168492 big=568036 small_fb=0 big_fb=0 small_file=120995 big_file=10097022 small_tail=23963 big_tail=24464
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 170.28s
exit=0

```

Старое отношение ловило от ~1.6 % (`(164 К + f·9.9 М)/164 К ≤ 2`), новая разница — от 5.3 %.
Чувствительность упала втрое, и это названо честно. Но предел назван исходя из добавки 185 КБ;
с реальной добавкой 594 КБ (Б-1) порог, который не краснел бы на честной реализации, — не
меньше ≈640 КиБ, то есть «не ловится < 6.5 %». Любой порог здесь — компромисс между флаком
Б-2 и слепотой к частичному префиксу; обе стороны компромисса — следствие дефекта Б-2, и
дешевле закрыть его, чем двигать порог.

## Н-1. Анти-плацебо в обе стороны — подтверждено

- Честная реализация: 12/12 зелёных (`meas1`, сырые строки в Б-2), включая 6 прогонов с
  малым в откате — разница при этом `0` (saturating_sub), что и задумано.
- Мутант D «читать с `header_end`» при ОТКЛЮЧЁННОЙ абсолютной границе:

```
$ # мутант D: Ok(Some(th)) → (None, 0) в диспетчере segments.rs:2712 — hint отброшен, чтение с header_end;
$ # в тесте вызов assert_first_pump_bounded("w1/big") заменён на eprintln! (абсолютная граница отключена)
ADV-MUT D: hint dropped → header_end
ADV-MUT D: hint dropped → header_end
ADV-W1 small=259739 big=10157432 small_fb=0 big_fb=0 small_file=120161 big_file=10017854 small_tail=23963 big_tail=24464
ADV-NOABS: абсолютная граница отключена
thread 'w1_first_pump_after_warm_resume_costs_tail_not_prefix' (1302253) panicked at crates/gateway/tests/red_m89_warm_resume_seek.rs:404:5:
w1 / I-1: первый pump прочитал 10157432 Б при префиксе 200000 против 259739 Б при 2000 — на 9897693 Б больше при 100-кратной разнице длины (допуск 524288 Б). Работа первого тика растёт с длиной активного сегмента — замер 2026-09-27: ×109 при ×200.
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 11 filtered out; finished in 89.13s
exit=101

```

## Н-2. Что ловит оракул, а что — нет (граница названа)

- Чтение всего префикса: ловят и абсолютная граница, и разница (мутант D).
- Чтение доли префикса ≥ 5.3 %: ловит разница; < 5.3 % — нет (мутант C, названо автором).
- Откат быстрого пути на бисекцию (10 проб × 64 КиБ): абсолютная граница пропускает (в 2 МиБ
  укладывается), разница — зависит от того, какая мода выпала на малом (Б-1). Это не «предел»
  оракула, а его флак, и он остался.

## Н-3. Что делать (не проект фикса — границы для architect'а)

1. Не мержить правку как закрытие `TD-250`: она сужает флак, не снимает. Карточка остаётся
   открытой либо переписывается с настоящей причиной (Б-2).
2. Причина — в `crates/journal/src/segments.rs:3658-3724` (прод-путь, не харнесс) → полный
   цикл с RED на САМ быстрый путь: оракул, который краснеет, когда `seek_back_from_tail`
   возвращает `None` на валидном `after+1` при оценке, промахнувшейся на < 8 КиБ в ЛЮБУЮ
   сторону. После такого фикса честная мода одна, и старое отношение 2.0× перестаёт
   флаковать само собой.
3. Если оракул правится ДО фикса реализации — порог обязан быть выведен из обеих мод, и
   doc-комментарий обязан называть добавку большого (≈594 КБ) и её рост `log2`, а не
   «постоянна». Вариант с тремя точками длины не нужен: разница уже отделяет константу;
   не отделяет она добавку, растущую логарифмически, — и никакая мера на двух точках не
   отличит «log от размера» от «5 % от размера» при соотношении 100×.

## Done Block

```
$ git -C /tmp/hft-adv-td250 rev-parse HEAD
17a25548c031e891b43aed03ebff2b27aacccdd6

$ git -C /tmp/hft-adv-td250 status --porcelain        # после отката всех мутантов, до добавления вердикта
(пусто)

$ # 12 прогонов честной реализации (meas1.log): все «test result: ok»
$ grep -c 'test result: ok' meas1.log
12
$ grep -c 'ADV-PATH bisect' meas1.log                 # откатов быстрого пути: 6, все на малом
6

$ # мутанты (exit — код cargo test)
A  (промах только big; small выпал в откат)       → ok,     exit=0   extra=409 208
A2 (промах big; small принудительно быстрый)      → FAILED, exit=101 extra=595 002  ← блокер Б-1
C  (4 % префикса с header_end)                    → ok,     exit=0   extra=399 544  ← названный предел
D  (header_end, абсолютная граница отключена)     → FAILED, exit=101 extra=9 897 693

```
