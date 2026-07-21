# M-10 — R-001: первый сквозной прогон OBI (Трек A) как KILL-SCREEN

STATUS: **PROPOSED** (2026-07-20, architect; rev2 2026-07-21 — фиксы C-019 REJECT: verify fail-closed
на R-001, report-level RED честности отчёта, эпоха ledger'а `.jsonl` механически). Doc-гейт
`gates.md` §9 Class A (новый milestone) + анти-оверфит гейт §6. Founder дал «go» на пивот к M-10
после закрытия M-09-корректности.

## Objective

У нас есть grid / walk-forward / trials-ledger / deflated Sharpe / честный `sim` — и **НИ ОДНОГО
отчёта**. Весь research-стек ЮНИТ-проверен, но **в бою не бывал** (в отличие от recorder-части, §8
многократно). R-001 — **первое сквозное испытание стека**: прогнать сигнал OBI Трек A (top-N imbalance)
на записанных данных и вынести вердикт по ПРЕ-РЕГИСТРИРОВАННЫМ критериям (`H-20260710-obi-asym`).

**⚠ ЧТО ЭТО НЕ ЕСТЬ.** Это НЕ решение «торговать/не торговать». На текущем окне данных (единицы—десятки
дней) точечный Sharpe физически недостоверен (SE годового Sharpe ≈ ±11 на 3–7 днях). Поэтому R-001 —
**KILL-SCREEN**: он может ТОЛЬКО (а) **УБИТЬ** сигнал, если сработал пре-рег критерий фальсификации
(дёшево и ценно — «сигнал мёртв» тоже результат), либо (б) вернуть **INCONCLUSIVE** («данных мало,
вердикт недостоверен»). **PASS/промоушен на этих данных АРХИТЕКТУРНО ЗАПРЕЩЁН** — без касания test-окна
и без подписи founder'а (промоушен = отдельная фаза с бОльшими данными + двойная подпись, `gates.md` §7).

## Scope: только Трек A (Трек B — fast-follow, теперь разблокирован)

- **Трек A (этот milestone):** `top_n` imbalance, `n_levels ∈ {1,5,10,20}` × `theta ∈ {0.1..0.4}` ×
  `horizon_ms ∈ {500,1000,2000,5000}`. Вычислим на ВСЕХ записанных данных (не требует глубины >0.1%).
- **Трек B (НЕ этот milestone; заметка-находка 2026-07-20):** ценовые полосы 3%/8% — H-карточка/S-001
  помечали «заблокирован TD-004», но **это устарело для Binance**: spot/futures теперь пишут полную
  книгу (REST `/depth?limit=5000` + `@depth@100ms` diff-stream → локальная книга ~60% от mid, эмит
  бакетами 0.02%). Трек B на Binance ТЕПЕРЬ вычислим; ждёт (1) прохождения Трека A через пайплайн +
  (2) уточнения founder'ом смысла «3%/8%» (H-карточка §открытый вопрос). Hyperliquid остаётся мелким
  (top-20) — Трек B на HL всё ещё заблокирован (остаток TD-004, сузился до HL). **Reviewer: обнови TD-004.**

## Contract impact (T-designate, НЕ crates/contracts) — CT-RFC НЕ нужен

`ValidationReport` (`research-cli/src/types.rs`, T1-DESIGNATE, промоушен в `contracts` отложен TD-008)
получает поля для честного kill-screen (сейчас их НЕТ — отчёт не может быть честным без них):
- `data_span_days: f64` — календарная длина тестового окна (дни);
- `se_sharpe: f64` — стандартная ошибка оценки Sharpe (на коротком окне огромна → любой точечный SR — шум);
- `verdict: Verdict { Kill(reason), Inconclusive(reason), Pass }`;
- `gap_ref: String` — ссылка на E8 gap-артефакт тестового окна (`data_quality.rs`);
- `ledger_cutoff: String` — code_hash-граница эпохи ledger'а, по которой посчитаны deflated-Sharpe/`ledger_n`
  (TD-015; `≥ 5141fd9`) — отчёт обязан НАЗВАТЬ свою эпоху машинно, а не в прозе.

Плюс **валидатор честности отчёта** `research_cli::report::{ReportHonesty, validate_report_honesty}`
(C-019 B2/B3) — УЗКИЙ, отдельно от классификатора: `gap_ref` (E8) и эпоха ledger'а (TD-015) генерируются
ВНЕ `classify_verdict`, поэтому гейтятся report-level RED'ом, а не пихаются в `KillScreenInputs`.
research-dev обязан звать его ПЕРЕД записью R-001 (пустой `gap_ref` / пустая или пре-M-07 `ledger_cutoff`
`f7f4761` / не-конечный span / отрицательная se → `Err`, отчёт не пишется).

Тип живёт в `research-cli` (не в `crates/contracts`) → это research-dev-правка формы под RED architect'а,
БЕЗ atomic contract-RFC. `report_schema_version` бампается (аддитивно).

## KILL-SCREEN инвариант (KS-I-*, sacred RED architect'а) — ЯДРО M-10

Это анти-плацебо для ОТЧЁТА (тот же класс, что 7 раундов TD-027, но для вывода research'а: отчёт НЕ
смеет заявить промоушабельный сигнал на шуме).

| ID | Инвариант |
|---|---|
| **KS-I-1** | **PASS запрещён без нижней границы CI над баром.** `verdict=Pass` допустим ТОЛЬКО если `sharpe − 2·se_sharpe > BAR` (нижняя 95%-граница Sharpe выше порога). На коротком окне `se_sharpe` огромна → нижняя граница глубоко отрицательна → PASS невозможен. Отчёт БЕЗ `data_span_days`/`se_sharpe` = НЕВАЛИДЕН. RED: отчёт с `Pass` при `sharpe−2·se ≤ BAR` → гейт ВАЛИТ (ложный промоушен) |
| **KS-I-2** | **Пре-регистрация (§4.1).** H-карточка с критериями фальсификации существует ДО test (`require_preregistration` уже есть). Отчёт без пре-рег предка → невалиден |
| **KS-I-3** | **Эпоха ledger'а (TD-015).** `ledger_n` и V[SR] семейства — ТОЛЬКО по записям кода `≥ 5141fd9`; отчёт НАЗЫВАЕТ эпоху (`code_hash` + диапазон). Смешение эпох → KILL (risk-critic пункт 0) |
| **KS-I-4** | **Критерии фальсификации — МАШИННО.** Пре-рег критерии H-карточки (Net Sharpe ≤0.5 OOS / deflated ≤0 / walk-forward нестабилен / decay < горизонта / отрицательный fill-PnL) вычисляются из отчёта; сработал любой → `verdict=Kill(reason)`. RED: отчёт, где критерий сработал, но `verdict≠Kill` → гейт ВАЛИТ |
| **KS-I-5** | **E8 честность окна.** `verdict` ссылается на gap-статистику тестового окна (`data_quality.rs`); если доля разрывов высока — это в отчёте, не спрятано. Test-окно трогается ОДИН раз (§4.2). **Механически (C-019 B2):** `validate_report_honesty` валит отчёт с пустым `gap_ref` (report-level RED, не classifier); `verify_M-10.sh` требует непустой `gap_ref` в R-001 json |
| **KS-I-3 (доп.)** | **Механический анти-микс эпох (C-019 B3):** `validate_report_honesty` валит пустой/пре-M-07 (`f7f4761`) `ledger_cutoff`; `verify_M-10.sh` (a) читает ledger по пути `research/trials-ledger.jsonl`, (b) требует непустой `ledger_cutoff` в отчёте, (c) валит отчёт, содержащий `f7f4761`. Глубокий подсчёт `ledger_n`/V[SR] по эпохе — risk-critic пункт 0 |

`BAR` для KS-I-1 — фиксированная константа в RED (не калибруется): нижняя граница Sharpe, ниже которой
сигнал не промоушабелен даже теоретически (стартово `BAR = 0.5`, согласовано с пре-рег порогом «≤0.5 →
мёртв»). Анти-плацебо: impl, штампующий `Pass` игнорируя `se`, валится; impl, зовущий всё `Inconclusive`,
валится на входе с реальным KILL-критерием (KS-I-4).

## Allowed / Forbidden paths (scope-guard)

- `*/tests/**` (KS-I-* RED, sacred), `scripts/verify_M-10.sh`, `milestones/M-10-*.md` — **architect**.
- `research/hypotheses/`, `research/specs/` — пре-рег уже есть (`H-20260710`, `S-001`); правки — signal-engineer по назначению.
- `crates/research-cli/src/**` (ValidationReport +поля, verdict-классификатор, report-генерация, прогон грида) — **research-dev**.
- `crates/signals/src/obi.rs` — сигнал УЖЕ есть (M-04); детерминизм-тест — signal-engineer при нужде.
- `research/reports/R-001*`, `research/trials-ledger.json` (append-only механизм, НЕ ручная правка) — **research-dev** (генерация).
- **Forbidden:** ручная правка `trials-ledger.json`; касание test-окна >1 раза; `crates/{risk,killswitch,oms,contracts}`; промоушен статуса сигнала (Граница B/C — только founder-подпись).

## §Tasks (RED-first: architect гейты ДО прогона)

| # | Статус | Задача | Кто | Acceptance |
|---|---|---|---|---|
| 1 | ✅ | **KS-I-* RED-оракулы** (`crates/research-cli/tests/red_killscreen.rs`): classifier-честность (KS-I-1 CI-бар, KS-I-4 критерии→Kill) + **report-level честность** (KS-I-5 gap_ref, KS-I-3 эпоха через `validate_report_honesty`), анти-плацебо в обе стороны (Pass-на-шуме валит; always-Inconclusive валит на реальном Kill; always-Ok валидатор валит err-тесты; always-Err валит достижимо-честный отчёт) | architect | RED падает против текущего (нет verdict/se/span/ReportHonesty); достижим корректным классификатором+валидатором |
| 2 | ✅ | `scripts/verify_M-10.sh` — анти-оверфит §6 как гейт: пре-рег есть, эпоха TD-015 (путь `.jsonl`, `ledger_cutoff` + анти-микс `f7f4761`), **FINAL fail-closed требует R-001 `.json`+`.md`** (флаг `--red` для RED-фазы), отчёт несёт span/se/verdict/gap_ref, kill-screen enforced | architect | FINAL exit≠0 без отчёта; ВАЛИТ ложный Pass/пустой gap_ref/пре-M-07 эпоху; exit=0 на валидном отчёте |
| 3 | ⏳ | `ValidationReport` +`data_span_days`/`se_sharpe`/`verdict`/`gap_ref`/`ledger_cutoff`; **классификатор** `report::classify_verdict(&KillScreenInputs,BAR)->Verdict` (чистый) + **валидатор честности** `report::validate_report_honesty(&ReportHonesty)->Result<(),String>` (звать ПЕРЕД записью R-001); генерация в `write_metrics_json`/`write_narrative_md` | research-dev | KS-I-* GREEN (classifier + report-honesty); `verify_M-10.sh --red` PASS |
| 4 | ⏳ | **Прогон R-001 Трек A** через `StrategyBacktest`: grid на train → топ-K → OOS+walk-forward+стресс(×1.5/×2) → `research/reports/R-001-obi-trackA.{json,md}` с ОБЯЗАТЕЛЬНЫМИ span/se/verdict/gap_ref | research-dev | отчёт валиден по verify_M-10; вердикт Kill/Inconclusive (Pass запрещён на этих данных) |
| 5 | ⏳ | **risk-critic анти-оверфит §6** (пункт 0 — эпоха ledger'а; lookahead, издержки ×1.5/×2, режимы, ёмкость, корреляция) → `research/critiques/C-0NN` (KILL/CONCERNS/PASS) | risk-critic | вердикт закоммичен |
| 6 | ⏳ | **founder ★** — принять/убить по пре-рег критериям (Граница C). НЕ промоушен (данных мало) — фиксация «мёртв / недостоверно / вернуться при N данных» | founder ★ | подпись/решение |

## Acceptance (исполняемые ворота)

0. **FINAL fail-closed (C-019 B1):** `verify_M-10.sh` (дефолт) → FAIL, пока нет `research/reports/R-001*obi*trackA*.{json,md}`. Milestone НЕ закрывается без артефакта, ради которого создан. RED-фаза — `verify_M-10.sh --red` (отчёт отложен).
1. **Kill-screen честность (KS-I-1):** отчёт с `Pass` при `sharpe−2·se ≤ BAR` → verify FAIL. Отчёт без непустых `data_span_days`/`se_sharpe`/`verdict`/`gap_ref`/`code_hash`/`ledger_cutoff` → невалиден.
2. **Эпоха (KS-I-3/TD-015, C-019 B3):** ledger по пути `research/trials-ledger.jsonl`; отчёт называет эпоху (`ledger_cutoff` непустой), НЕ содержит пре-M-07 `f7f4761`; `ledger_n`/V[SR] только по записям `≥ 5141fd9` (глубоко — risk-critic пункт 0).
3. **Критерии фальсификации (KS-I-4):** пре-рег критерий сработал → `verdict=Kill`; иначе гейт валит.
4. **E8 честность окна (KS-I-5, C-019 B2):** `validate_report_honesty` валит пустой `gap_ref`; отчёт ссылается на gap-артефакт тестового окна.
5. **Пре-рег + time-split:** H-карточка ДО test; test-окно тронуто ОДИН раз; trials-ledger append-only.

## Гейты

- **Plan-time:** critic (новый milestone §9 Class A). Анти-оверфит §6.
- **risk-critic ОБЯЗАТЕЛЕН** (`gates.md` §5/§6 — отчёт стратегии, асимметричная цена ошибки): пункт 0 эпоха + чек-лист §4.
- **founder ★** — Граница C (принять/убить; промоушен НЕ на этих данных).
- НЕ safety-код (risk/killswitch/oms не трогаются) → risk-critic здесь = адверсарий ОТЧЁТА, не safety-пути.

## Handoff (план)

critic (milestone+RED) → research-dev (ValidationReport +поля + классификатор + прогон R-001) →
risk-critic (анти-оверфит §6) → founder ★ (вердикт). Architect пишет KS-I-* RED + verify ДО прогона.
