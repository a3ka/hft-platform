# C-152 — PR #96: круг 3, исполнение A-019

<!-- GATE-META
milestone: M-60a
audited_repo: a3ka/hft-platform
audited_base: 721461cb7d41a625b28026f73d1aa5a3e62091cd
audited_head: 8aa20f68ffe09639117acb4af9bdb3f7862e37ab
verdict: NOTE
-->

## Verdict: NOTE

`8aa20f6` исполняет обязательное решение A-019 буквально.  Оба предписанных
ханка совпадают с арбитражным текстом байт-в-байт; все новые утверждения о
коде подтверждены на дереве слияния PR #96.  B-2 не переоткрывался.

## Проверка исполнения

### A-019 §3 — PASS, обе точные замены применены без изменений

`diff -u` между каждым fenced-ханком A-019 §3 и его участком в
`docs/fa/viz-backend.md` завершился с `exit=0`.  В коммите изменён ровно один
файл — `docs/fa/viz-backend.md` (+17/-9); постороннего решения о доме GW-I/GS-I,
нового инварианта или состава выдачи в нём нет.

### Новые утверждения §4 — PASS

На merge-tree `refs/pull/96/merge`:

- `DepthRow` имеет `depth_band_provenance` и вызывает
  `depth_provenance_label(band, side, reach)`; функция даёт три deep-исхода:
  confirmed, unconfirmed, `not-observed`.
- `HeatmapCell` — второй производитель: `deep.then(...)` в обоих bid/ask-циклах
  несёт неизменный `diff-reconstructed` только при глубине более 1.3 %.
- `CobLevel` содержит ровно `side`, `price_e8`, `size_e8`; provenance-поля и
  конструктора с такой меткой нет.
- Окно heatmap есть `max(selector.bands)`. `docker-compose.yml` передаёт
  `GATEWAY_BANDS` в `--bands`, а `gateway-serve` парсит его в `Selector.bands`.
  Поэтому утверждение о появлении deep heatmap-ячеек при исполнении `П-014` п.4
  — корректный вывод из кода, а не расширение самой подписи.

Тело `П-014` подтверждает границу: п.1 требует постороннюю метку **депт-серии**
и называет `row.band_pct_e8`; heatmap и COB там не названы.  Новая формулировка
правильно говорит, что heatmap-факт не является предусловием `П-014`.

### TD-161 — NOTE для reviewer-owned журнала, не блокер этой правки

Карточка существует в `origin/main`, имеет **Ф4 / MINOR**, и её живое ядро
соответствует факту: у depth-series и heatmap разные словари provenance в одной
выдаче.  Однако её историческая часть «FA обещает строку, которой в коде нет»
стала неактуальной именно после этой правки FA. Это не ложное утверждение,
добавленное `8aa20f6`, и не меняет вердикт: новая FA ссылается только на
сохранившееся словарное расхождение. Reviewer при close-out должен сузить TD-161,
сохранив Ф4/MINOR и live-часть находки.

### Регресс кругов 1–2 — PASS

- B-1 сохранён: M-24 в scope только SVP; CVP/FRVP/Anchored/Composite/HVN/LVN
  явно вне scope, и в `gateway`/`research-cli` реализации вариантов нет.
- B-3 сохранён: M-28 сопоставляет `GS-I-1` с `VB-I-9a`, `GS-I-2` с `VB-I-9b`;
  FA не объявляет ни одной строки `GS-I-*`, а `GS-I-4` лишь упомянут в VB-I-6.
- Скан всех ссылок этой FA на `П-011`/`П-013`/`П-014`/`П-017` не показал нового
  обязательства вне соответствующих тел. Для `П-014` ссылки распределены по её
  фактическим пп.1 (DepthRow), 2 (каденция) и 4 (состав bands).

## Done Block

```text
$ bash scripts/reserve_artifact_id.sh C
[stdout was empty in this harness; exit=0]

$ bash scripts/reserve_artifact_id.sh --list C | tail -4
C-144      0 дн  reserve C-144 nous 2026-08-25T10:24:21Z ...
C-148      0 дн  reserve C-148 nous 2026-08-25T11:44:00Z ...
C-149      0 дн  reserve C-149 nous 2026-08-25T11:45:03Z ...
C-152      0 дн  reserve C-152 nous 2026-08-25T12:06:01Z ...
reserve: резервов: 14
exit=0

$ git show -s --format='%H%n%P' refs/pull/96/merge
04e3b85d96ebf8da1354deecd276440fd8c4cfc7
721461cb7d41a625b28026f73d1aa5a3e62091cd 8aa20f68ffe09639117acb4af9bdb3f7862e37ab
exit=0

$ git diff --name-only 8aa20f6^ 8aa20f6
docs/fa/viz-backend.md
exit=0

$ git diff --check 8aa20f6^ 8aa20f6
exit=0

$ diff -u <(A-019 §3 hunk 1) <(viz-backend.md:95-108)
exit=0

$ diff -u <(A-019 §3 hunk 2) <(viz-backend.md:140-142)
exit=0

$ sed -n '205,250p;1096,1110p;1185,1255p;1345,1386p' crates/gateway/src/lib.rs
HeatmapCell { ... depth_band_provenance: Option<String> }
CobLevel { side, price_e8, size_e8 }
DepthRow { ... depth_band_provenance: Option<String> }
depth_band_provenance: depth_provenance_label(...)
let w = selector.bands.iter().copied().fold(0.0_f64, f64::max);
let prov_str = "diff-reconstructed".to_string();
depth_band_provenance: deep.then(|| prov_str.clone())
... confirmed / unconfirmed / not-observed ...
exit=0

$ git show origin/main:TECH-DEBT.md | rg -n -C 2 'TD-161'
TD-161 ... Severity: MINOR ... Фаза: Ф4 ...
... depth_series ... liveness ... heatmap ... diff-reconstructed ...
exit=0

$ sed -n '749,822p' docs/PENDING-SIGNATURE.md
П-014 п.1: метка посторонне для глубинных полос; п.4: GATEWAY_BANDS меняется
на канонический набор.
exit=0

$ bash scripts/verify_design_claims.sh
VERDICT: PASS (0 нарушений)
exit=0
```
