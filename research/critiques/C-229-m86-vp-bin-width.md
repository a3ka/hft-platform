<!-- GATE-META
milestone: M-86
audited_repo: a3ka/hft-platform
audited_base: 0f55ea7577386c08c22b4cedd428b354308db52e
audited_head: 99671a363abf128c8ac75fa7f3f32eafca4798db
verdict: REJECT
-->

# C-229 — M-86: fixed VP bin width — REJECT

## Предмет и охват

Проверен закоммиченный набор `0f55ea7..99671a3` на `feat/M-86-vp-bin-width`, а не только
текст спеки: milestone, четыре RED-файла, `verify_M-86.sh`, изменения `docs/ROADMAP.md` и
их поведение на merge-preview с `origin/main` (`10cd40f`). T1 не меняется: в диапазоне нет
`crates/contracts/**`, поэтому contract-RFC не нужен. Новая публичная T2-поверхность
`DEFAULT_VP_BIN_WIDTH_E8` / getter / setter объявлена в milestone; отдельных trait-сигнатур
предмет не вводит.

Живой инвариант затронутого read-path: **VB-I-2** (`docs/fa/viz-backend.md:200`) —
`live == replay`. `gateway-serve` отдельного FA не имеет; для этой связки действует тот же
инвариант viz-backend.

## Вердикт: REJECT

Dev не диспетчеризовать, пока не закрыты находки ниже. Они не требуют менять T1 или решать
Value Area / сторож отсутствия доставки в M-86.

### R1 — форма не определяет отрицательную цену, а RED не ловит выбранную семантику

`milestones/M-86-vp-bin-width.md:62-65` требует ключ `(price_e8 / W) * W`, «деление вниз» и
якорь ноль. `MdPayload::Trade.price` остаётся `i64` (`crates/contracts/src/lib.rs:239-244`),
а нынешний accumulator принимает `price: i64` без отказа
(`crates/gateway/src/lib.rs:652-660`). В Rust `/` для отрицательного делит к нулю: для
`price=-1`, `W=25_000_000` получается ключ `0`, а математическое округление вниз даёт
`-25_000_000`. Ноль тоже не классифицирован как допустимая цена либо вход, который обязан
быть отвергнут.

Все V2/V3/V4/V6/V8 используют только положительную цену около 77 000 USD
(`crates/gateway/tests/red_vp_bin_width.rs:120-400`). Поэтому исполнитель сам выберет между
`/`, `div_euclid` и отдельным reject — три разных контракта, из которых два не следуют из
спеки.

**Условие снятия:** форма явно задаёт исход для `price_e8 < 0` и `price_e8 == 0`; RED-набор
пиннит этот исход, включая `-1` (граница корзины), и краснеет на противоположной семантике.

### R2 — V1 и V6 не защищают точность W на прод-масштабе

V1 строит корректную крупную фикстуру: 204 161 различная цена
(`red_vp_bin_width_size.rs:45-49,87-114`), но после неё требует лишь `bins >= 1_000`
(`:152-158`). При объявленной геометрии сетка `W=0.25` даёт 20 417 корзин, тогда как
незаконная ветка «для большого профиля использовать `10*W`» даёт 2 042: кадр легко меньше
2 MB и проходит V1. V6 проверяет только четыре уже выровненные цены
(`red_vp_bin_width.rs:248-277`); такая ветка на малой форме вернёт обычный `W` и тоже
пройдёт. V2--V4/V8 также малы.

Это конкретный broken stub, проходящий пару vantage: `if hist.len() > N { width = 10 * W }`
иначе `width = W`. Он сужает продукт сверх подписанной формы, не теряет объём и не обязан
сломать существующие оракулы.

**Условие снятия:** добавить независимый large-scale oracle, который на прод-геометрии
сверяет ключи/число корзин с каноническим именно `W`-разбиением (и имеет setup-guard), а не
только размер и нижнюю границу числа корзин.

### R3 — обязательный V7 отсутствует; V8 не исполняет live-path

Таблица M-86 требует V7: `snapshot(C) + frames` равно replay при включённой сетке
(`milestones/M-86-vp-bin-width.md:186`). В закоммиченном RED-наборе нет ни `V7`, ни
`fn v7_*`; есть V2, V3, V4, V6/V6b/V6c и V8/V8b
(`crates/gateway/tests/red_vp_bin_width.rs:120,172,203,248,285,315,354,400`). V8 вызывает
`snapshot` и `frames_since`, но не `LiveReducer`, поэтому не предъявляет VB-I-2 на
продовой live-ветке. Шаг E verify считает только восемь функций по шаблону
`^fn v[0-9]` (`scripts/verify_M-86.sh:44-54`), то есть отсутствие названного V7 остаётся
зелёным.

**Условие снятия:** предъявить именно V7 с live-path и независимым replay-эталоном при
включённой сетке, а verify должен проверять именованный состав обязательств, не только число
функций.

### R4 — V5 не доказывает, что env-конфигурация дошла до сетки

Задача 4 требует, чтобы `serve_config_from_env` один раз вызвал setter
(`milestones/M-86-vp-bin-width.md:71-86,167`). Но V5 проверяет только `Result<ServeConfig,
String>`: валидные значения должны вернуть `Ok` (`red_vp_bin_width_startup.rs:139-150`).
Контроль C вручную вызывает `set_effective_vp_bin_width_e8` и тем самым не покрывает
env→setter (`red_vp_bin_width_governed.rs:76-122`).

Следовательно, реализация «распарсить и валидировать `GATEWAY_VP_BIN_WIDTH_E8`, но забыть
setter» проходит V5, C, V1 и V6; произвольная конфигурация оператора инертна.

**Условие снятия:** RED-проверка после вызова `serve_config_from_env` должна наблюдать
эффективную ширину либо её результат в `vp_rows()` для отличного от дефолта валидного
значения, включая правило, что отклонённый старт не меняет прежнее значение.

### R5 — verify обещает сторожей, которые зелены на конкретных нарушениях

1. Шаг I ищет только изменённую строку `fn selector_fingerprint`
   (`scripts/verify_M-86.sh:135-138`). Добавление строки hash ширины в тело существующей
   функции не меняет эту строку, так что шаг печатает PASS, хотя нарушен §4.2 milestone и
   все checkpoint'ы становятся другими. Это особенно существенно, потому что fingerprint
   реально определяет имя checkpoint (`crates/gateway/src/lib.rs:3710-3731`), а
   `read_checkpoint` принимает/отвергает state по нему (`:4107-4121,4242-4244`).
2. Шаг F считает «рабочим» любой compose-дефолт `> 1`
   (`scripts/verify_M-86.sh:69-79`), хотя зафиксированное значение — ровно
   `25_000_000` (`milestones/M-86-vp-bin-width.md:42-47,88-91`). Например, строка
   `${GATEWAY_VP_BIN_WIDTH_E8:-2}` проходит обе проверки F, но практически выключает
   огрубление и возвращает аварию.

**Условие снятия:** сторожа должны краснеть соответственно при изменении тела
`selector_fingerprint` и при compose-дефолте, отличном от подписанного литерала; предъявить
их анти-плацебо мутациями.

### R6 — коммитный scope шире `Allowed paths`

M-86 разрешает architect только tests, milestone, verify и `docs/fa/viz-backend.md`
(`milestones/M-86-vp-bin-width.md:139-145`). Однако `99671a3` меняет
`docs/ROADMAP.md` (добавляет M-86 и 9-A-bis). Сам текст roadmap предметно связан, но путь
не назван в `Allowed paths`; PR-time scope-gate обязан судить именно заявленный список.

**Условие снятия:** согласовать закоммиченный путь с `Allowed paths` milestone без
ретроспективного расширения dev-зоны.

## Не является находкой этого круга

§8 честно фиксирует, что `W=0.25` не даёт вечной гарантии доставки при экстремальном дневном
ходе. Это не ослабляет текущий PL-I-5: лимит 2 MB остаётся fail-closed; счётчик корзин и
сторож отсутствия доставки уже отделены из M-86 в roadmap. Не требую в этом предмете ни
перепроектирования Value Area, ни этих отдельных работ.

## Done Block

```text
$ git rev-parse HEAD; git merge-base HEAD origin/main
99671a363abf128c8ac75fa7f3f32eafca4798db
0f55ea7577386c08c22b4cedd428b354308db52e
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [1-ЕСТЬ] все 3 маркеров [ЕСТЬ] в таблицах статусов сопровождены существующим пруфом
PASS  [2-ПОКРЫТИЕ] §22: VB-I — заявлено=11, в оракулах=8 — подтверждено замером
PASS  [7-RFC-PATH] путей-кандидатов: все проверенные существуют в дереве репозитория
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/verify_M-86.sh
PASS: cargo fmt --all -- --check
FAIL: cargo clippy --all-targets --all-features -- -D warnings
FAIL: cargo test --all --quiet
FAIL: cargo test -p gateway --test red_vp_bin_width --quiet
FAIL: cargo test -p gateway --test red_vp_bin_width_size --quiet
FAIL: cargo test -p gateway --test red_vp_bin_width_governed --quiet
FAIL: cargo test -p gateway-serve --test red_vp_bin_width_startup --quiet
PASS: red_vp_bin_width.rs несёт 8 оракулов (ожидается 8)
FAIL: docker-compose.yml объявляет GATEWAY_VP_BIN_WIDTH_E8 с дефолтом
FAIL: docs/fa/viz-backend.md вообще называет VP-I-4
FAIL: точка мутации не найдена — DEFAULT_VP_BIN_WIDTH_E8 отсутствует (набор ещё RED)
VERDICT: FAIL (11)
exit=1

$ rg -n '\\bV7\\b|fn v7_' crates/gateway/tests/red_vp_bin_width*.rs crates/gateway-serve/tests/red_vp_bin_width*.rs scripts/verify_M-86.sh
exit=1

$ awk 'BEGIN { for (i=0;i<510401;i++) if (i%5<2) bins[int(i/25)]=1; for (b in bins) n++; for (i=0;i<510401;i++) if (i%5<2) coarse[int(i/250)]=1; for (b in coarse) m++; print "v1_exact_W_bins=" n; print "v1_illegal_10W_bins=" m }'
v1_exact_W_bins=20417
v1_illegal_10W_bins=2042
exit=0

$ printf '+        effective_vp_bin_width_e8().hash(&mut h);\\n' | grep -qE '^[+-].*fn selector_fingerprint'; P=$?; [ "$P" -eq 0 ] && echo VERIFY_FAIL || echo VERIFY_PASS
VERIFY_PASS
exit=0

$ bash scripts/next_artifact_id.sh C
C-229
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-09-19T13:18Z
- Milestone: M-86-vp-bin-width
- Статус: BLOCKED — REJECT
- HEAD: 99671a3 — docs(M-86): роадмап — дом предмета 9-A и отдельная строка 9-A-bis (сторож отсутствия доставки) [architect]

## §B — Что я сделал
- Проверил полный закоммиченный plan-time набор и merge-preview с `origin/main`.
- Воспроизвёл baseline RED и шесть блокирующих дефектов спецификации/оракулов/verify.

## §C — Артефакты / результаты
- `research/critiques/C-229-m86-vp-bin-width.md`
- Done Block: `verify_design_claims` exit=0; `verify_M-86.sh` exit=1 — ожидаемый RED, но набор недостаточен по R1--R6.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  Исправь только артефакты architect-zone M-86 по C-229 R1--R6: уточни форму отрицательной/нулевой цены; закрой large-scale fidelity дыру V1/V6; добавь обязательный V7 на live-path; свяжи env→setter RED-оракулом; сделай verify fail-closed на изменении fingerprint и неверном compose default; синхронизируй Allowed paths с уже закоммиченным docs/ROADMAP.md. Не меняй Value Area, T1, delivery-watcher или implementation. Закоммить и push на feat/M-86-vp-bin-width, затем передай новый head critic.
  ```
- Push-статус: см. коммит этого verdict на `origin/feat/M-86-vp-bin-width`.
- ⏸ кэш оставлен — это общий рабочий checkout, его удаление не является безопасной уборкой кэша роли.

## §E — Риски / открытые вопросы
- M-86 остаётся plan-time BLOCKED до нового RED-набора; следующая повторная причина R1--R6 требует маршрут арбитража по `gates.md` §0 при выполнении его триггеров.

=== END HANDOFF ===
