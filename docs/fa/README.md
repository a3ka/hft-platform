# FA-слой — индекс module Functional Architecture

Per-module FA документы глубины EINHARD `fa.md`. Шаблон: `_TEMPLATE.md`. Каждый FA —
STABLE/APPEND-ONLY (§-нумерация фиксирована после ACCEPTED; правки — append-only).
Инвариант-префикс на модуль пинит downstream-ссылки. Vector правды над FA: `../DESIGN.md`.

## Карта готовности (reviewer-проход 2026-07-10, architect Fable)

| Модуль | Слой | FA | Инвар. | Автор | Статус ревью |
|---|---|---|---|---|---|
| contracts | cross-cut | contracts.md | CT-I-1..6 | Fable (cornerstone) | ✅ ACCEPTED |
| journal | 0 | journal.md | JR-I + DET-I-1 | Fable (cornerstone) | ✅ ACCEPTED |
| venues | 1 | venues.md | VN-I-1..9 | subagent → gated | ✅ APPROVED |
| book | 1 | book.md | BK-I | subagent → gated | ✅ APPROVED |
| signals | 2 | signals.md | SG-I | subagent → gated | ✅ APPROVED |
| alpha | 3 | alpha.md | AL-I | subagent → gated | ✅ APPROVED |
| portfolio | 3 | portfolio.md | PF-I | subagent → gated | ✅ APPROVED |
| strategy | 4 | strategy.md | ST-I | subagent → gated | ✅ APPROVED |
| oms | 4 | oms.md | OM-I | subagent → gated | ✅ APPROVED |
| risk | 5 | risk.md | RK-I-1..10 | Fable (cornerstone) | ✅ ACCEPTED |
| killswitch | 5 | killswitch.md | KS-I-1..7 | Fable (cornerstone) | ✅ ACCEPTED |
| sim | 6 | sim.md | SM-I | subagent → gated | ✅ APPROVED |
| runner | 6 | runner.md | RN-I | subagent → gated | ✅ APPROVED |
| research-cli | 7 | research-cli.md | RC-I | subagent → gated | ✅ APPROVED |

14 module-FA. Sacred (4) авторил architect; остальные 10 — субагенты по шаблону+эталону,
проверены architect'ом как reviewer (структура шаблона · IS-NOT границы соседей · направление
зависимостей · привязка к сквозным RK-I-*/INTG-I-*/DET-I-1/CT-I-* · RED-оракулы «падают на
заглушке»).

## Сквозные инварианты (определены в DESIGN/03/05, цитируются в FA)
- **DET-I-1** (journal) — replay бит-идентичен; фундамент детерминизма.
- **RK-I-1..10** (risk) — pre-trade gate fail-closed; ордер только через `RiskApproved`.
- **KS-I-1..7** (killswitch) — независимый рубильник.
- **INTG-I-1..7** (03) — LLM влияет на рантайм только через границы A/B/C с подписью.
- **CT-I-1..6** (contracts) — T1-формы, contract-RFC дисциплина.

## Reviewer-findings (закрыты)
- research-cli: RC-I-2 не ссылался на сквозной INTG-I-6 по ID → **fixed** (cross-ref добавлен).
- venues §O: зависимость `venues→risk` по типу (`RiskApproved` в сигнатуре `place`) требует
  arch-lint DAG-подтверждения на реализации (цикла нет — risk near-leaf) → **записано в
  open-question**, не блокер FA, проверяется на P3.

## Что дальше
FA-слой полон. Следующее (README «Next»): решение по git-репо → карточка гипотезы
`H-20260710-obi-asym` → M-00 bootstrap (`contracts` крейт + process/ + verify-каркас) →
M-01 (журнал, DET-I-1). FA каждого модуля становится спекой для его milestone'ов.
