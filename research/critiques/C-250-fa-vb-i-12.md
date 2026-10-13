<!-- GATE-META
milestone: M-88
audited_repo: a3ka/hft-platform
audited_base: de33bf421388fe4bcbba1639de15b22e087ed90e
audited_head: 446026901c8399ca94adf8782603d40959214e9d
verdict: NOTE
-->

# C-250 — VB-I-12: контракт обновления книго-зависимых серий

## Verdict: NOTE

Неблокирующих расхождений текста с поставленным кодом не найдено. Ветка меняет
только `docs/fa/viz-backend.md`; код `crates/gateway/**` совпадает с
`origin/main` на проверяемом диапазоне. `VB-I-12` корректно материализует в FA
уже поставленную форму v11, а не переоткрывает архитектурное решение M-88.

## Проверка четырёх вопросов

1. **Текст соответствует коду.** В `origin/main:crates/gateway/src/lib.rs`
   `SeriesBundle` содержит `heatmap_observed_time_s` и `cob_observed`,
   `GATEWAY_SCHEMA_VERSION == 11`, а `Snapshot::apply` имеет `#[must_use]` и
   возвращает `ApplyOutcome`. До первой мутации метод возвращает
   `OutOfOrder` при `frame.from != self.cursor`, затем `Incompatible` при
   чужой schema version. `merge_heatmap` сохраняет только existing-ячейки
   не из объявленных колонок и затем вставляет incoming, поэтому объявленная
   пустая колонка действительно очищается. `heatmap_buckets_observed` и
   `heatmap_buckets` удерживаются одним предикатом `t >= lo_time_s`.
   `cob_observed` инициализируется как `false`, после L2 наблюдения только
   устанавливается в `true` и не сбрасывается эвикцией; это соответствует
   прямо названной неоконной семантике.

2. **Соседние инварианты согласованы.** `VB-I-12` восстанавливает требуемый
   `VB-I-2` parity live/replay, а не меняет его. Поля добавлены аддитивно с
   version bump, как требует `VB-I-4`. Оконность списка наблюдённых
   heatmap-бакетов продолжает `VB-I-10`; `cob_observed` — скалярная метка
   точечного среза на `at`, не бакет-ключевое состояние. Текст явно запрещает
   выводить из него свежесть (`Freshness` отдельна), поэтому следующий автор
   не должен «исправить» отсутствие эвикции. `VB-I-11` (provenance истории)
   не дублируется и не меняется.

3. **Предел назван и обеспечен.** В инварианте указано, что потребитель обязан
   гейтить `schema_version == 11`; иначе defensive default старого клиента
   означает «не наблюдали» и даёт тихое замирание. Это не обещание без
   механизма: `Snapshot::apply` отвергает несовместимый кадр до merge, а
   `red_m88_update_contract` и `red_m88_golden_vectors` проверяют эту границу.
   Претензия класса TD-138 отсутствует: текст о неоконности `cob_observed`
   совпадает с фактическими присваиваниями и эвикцией.

4. **Нумерация и форма.** `VB-I-12` единственен (`rg -c` вернул 1), находится
   в таблице семейства `VB-I` непосредственно перед разделом `MD-I`, а формат
   строки совпадает с `VB-I-1..11`. В `origin/main` находятся 11 каталогов
   эталонных сценариев в `crates/gateway/tests/fixtures/m88/`; их producer и
   expectation проверены `red_m88_golden_vectors`.

## Граница аудита

Этот круг не судит заново выбор M-88 или его продуктовые последствия: предмет —
точность новой FA-строки относительно уже принятой реализации. Нового
контрактного дефекта по существу в этом сравнении не обнаружено.

## Done Block

```text
$ git fetch origin && git rev-parse origin/docs/fa-vb-i-12 && git rev-parse origin/main
446026901c8399ca94adf8782603d40959214e9d
de33bf421388fe4bcbba1639de15b22e087ed90e
exit=0

$ git diff --name-status origin/main...HEAD && git diff --quiet origin/main...HEAD -- crates/gateway; echo "exit=$?"
M       docs/fa/viz-backend.md
exit=0

$ git show origin/main:crates/gateway/src/lib.rs | sed -n '112,118p;540,561p;2817,2838p;3044,3073p;3085,3105p;3183,3193p'
pub const GATEWAY_SCHEMA_VERSION: u32 = 11;
pub heatmap_observed_time_s: Vec<i64>,
pub cob_observed: bool,
#[must_use]
pub fn apply(&mut self, frame: &Frame) -> ApplyOutcome {
if frame.from != self.cursor {
    return ApplyOutcome::OutOfOrder;
}
if frame.schema_version != GATEWAY_SCHEMA_VERSION {
    return ApplyOutcome::Incompatible;
}
series.heatmap_buckets_observed.retain(|&t| t >= lo_time_s);
series.heatmap_observed_time_s.retain(|&t| t >= lo_time_s);
exit=0

$ git ls-tree -d --name-only origin/main:crates/gateway/tests/fixtures/m88 | wc -l
11
exit=0

$ cargo test -p gateway --test red_m88_update_contract --test red_m88_golden_vectors --test red_m88_observed_window --test red_gateway_schema_version
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit=0

$ bash scripts/verify_M-88.sh
PASS  task1: поля контракта объявлены в SeriesBundle
PASS  task3-6: red_m88_update_contract — test result: ok. 14 passed; 0 failed
PASS  task1+7: red_m88_contract_form — test result: ok. 9 passed; 0 failed
PASS  R4: батарея мутантов ЗЕЛЕНА — все изолированные мутации красят набор
PASS  A-036 п.1: эталонные векторы предъявлены и сходятся — test result: ok. 2 passed; 0 failed
PASS  task10: red_m88_observed_window — test result: ok. 4 passed; 0 failed
PASS  task10: бюджет ответа не съеден списком наблюдений — test result: ok. 1 passed; 0 failed
PASS  CI-паритет: cargo fmt --all -- --check
PASS  CI-паритет: cargo clippy --all-targets --all-features -- -D warnings
PASS  CI-паритет: cargo test --all
VERDICT: PASS
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [2-ПОКРЫТИЕ] §22: VB-I — заявлено=11, в оракулах=8 — подтверждено замером (loose=8)
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/next_artifact_id.sh C
C-250
exit=0
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-09-23T15:27Z
- Milestone: M-88-liquidity-removal-contract
- Статус: DONE
- HEAD: 4460269 — docs(fa): VB-I-12 — контракт обновления книго-зависимых серий (форма v11) [architect]

## §B — Что я сделал
- Проверил `VB-I-12` против кода `origin/main`, а не против текста M-88.
- Проверил согласованность с `VB-I-2`, `VB-I-4`, `VB-I-10`, `VB-I-11`, границы формы и нумерацию.

## §C — Артефакты / результаты
- `research/critiques/C-250-fa-vb-i-12.md`
- Done Block: `verify_M-88.sh` exit=0; merge-preview exit=0; профильные M-88 RED-наборы exit=0.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  Ветка docs/fa-vb-i-12: critic C-250 вынес NOTE без блокирующих находок над SHA
  446026901c8399ca94adf8782603d40959214e9d. Проверь, что C-250 закоммичен и
  запушен, затем передай PR #217 reviewer для unconditional PR-time гейта. Предмет
  не меняет код M-88 и не требует нового dev-шага.
  ```
- Push-статус: ⏸ commits ready; next agent in chain will push.
- Кэш: ⏸ кэш оставлен — verdict commit and push are the remaining steps of this gate.

## §E — Риски / открытые вопросы
- N/A — этот круг не переоткрывал техническое решение M-88.

=== END HANDOFF ===
