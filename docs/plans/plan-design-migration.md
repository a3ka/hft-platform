# ПЛАН — миграция мастер-документа (DESIGN v1 → v2 «AlphaQuant») + ревизия контрактного слоя

**Автор:** planner (oh-my-claudecode), 2026-07-31.
**Статус:** PROPOSED — план работ, не исполнение. Ничего не закоммичено, файлы проекта не тронуты.
**Startup-протокол выполнен:** `CLAUDE.md` → `docs/04-workflow.md` → `.claude/rules/gates.md` →
`.claude/rules/scope-guard.md` → `docs/05-contract-layer.md`.

> ⚠ **Первое, что должен знать исполнитель.** Рабочий чекаут `/home/nous/hft-platform` стоит на
> ветке `docs/06-volume-truth`, где `docs/` НЕПОЛНЫЙ (нет `07`, `08`; правила `.claude/rules/*`
> устарели — в частности там НЕТ doc-гейта §9). **Все факты и все правки этого плана — от
> `origin/main` (HEAD `f930ece`).** Читать через `git show origin/main:<path>` либо из
> собственного worktree (`branch-hygiene.md`), НЕ переключая общий чекаут.

---

## §0. Что этот план решает

Три связанных блока, заданных founder'ом:

| Блок | Суть | Ключевой гейт |
|---|---|---|
| **1** | `docs/DESIGN-v2.md` заменяет `docs/DESIGN.md`; подчинённые доки приводятся в согласие | doc-гейт `gates.md` §9 класс A (critic + reviewer + founder ★) |
| **2** | Ревизия `crates/contracts` + `docs/05-contract-layer.md`: датафлоу обязан ЖЁСТКО зависеть от контрактов, и это должно быть ПРОВЕРЯЕМО | `gates.md` §1.1 (contract-RFC + critic) + §5 RISK-BLOCK (risk-critic **обязателен** на `crates/contracts/**`) |
| **3** | Порядок, роли, параллелизм, критерии готовности | scope-guard таблица владения |

---

## §1. Проверенные факты — база плана

Всё ниже проверено командой; непроверенное помечено `[verify-at-impl]`.

### 1.1. Состояние веток и документов

| Факт | Пруф |
|---|---|
| `origin/main` HEAD = `f930ece` | `git log --oneline -1 origin/main` |
| `docs/DESIGN-v2.md` (787 строк) и `docs/09-roadmap-v2.md` (277 строк) живут ТОЛЬКО на `origin/docs/design-v2`, 3 коммита поверх main (`f9a03d4`, `16206f6`, `db2f9ab`) | `git ls-tree -r origin/docs/design-v2 -- docs/`; `git log origin/main..origin/docs/design-v2` |
| `docs/06` в `main` — **старая редакция**: §2-опровержение объёмов (замер 2026-07-14) и §5.1 (снапшоты vs диффы) есть ТОЛЬКО на `origin/docs/06-volume-truth`, не смержены | `git diff origin/main..HEAD -- docs/06-data-layer-and-storage.md` |
| ⇒ DESIGN-v2 §7 ссылается на «docs/06 §2 замер» и «закрывает §5.1» — **текста, на который он ссылается, в `main` НЕТ** | сопоставление двух предыдущих строк |
| Milestone'ы в `main` — до `M-48`; `M-49-journal-tail-integrity` в полёте на `origin/feat/M-49`. Свободные номера — **с M-50** `[verify-at-impl на момент спеки]` | `git ls-tree origin/main -- milestones/`; `git ls-tree origin/feat/M-49` |
| Вердикты критиков — до `C-039`. Следующие свободные — **с C-040** `[verify-at-impl]` | `git ls-tree -r origin/main -- research/critiques/` |
| Крейтов в `main` — 17: `alpha, book, contracts, derive, gateway, gateway-serve, journal, ops, portfolio, recorder, research-cli, signals, sim, strategy, venue-binance, venue-binance-futures, venue-hyperliquid`. Крейтов `risk`/`killswitch`/`oms` **нет** | `git ls-tree origin/main -- crates/` |

### 1.2. Governance, который применяется к этой работе (`.claude/rules/gates.md` на `origin/main`)

**Это НЕ та версия правил, что лежит в рабочем чекауте.** На `main` есть **§9 DOC-ГЕЙТ**
(добавлен 2026-07-14, цикл C-006), которого в чекауте нет:

- **Класс A (гейт ОБЯЗАТЕЛЕН):** `docs/DESIGN.md` (§-структура, инварианты, **роадмап/фазы**),
  `docs/0N-*.md`, `docs/fa/*.md`, `docs/rfc/*`, `milestones/*.md` (Objective/Allowed/§Tasks/
  Acceptance), `milestones/BACKLOG.md`, `.claude/rules/*`, `.claude/agents/*`.
- **Процедура класса A:** architect коммитит `STATUS: PROPOSED` на ветку `docs/<тема>` (НЕ в main)
  → **critic** (вердикт в `research/critiques/C-NNN.md`, коммитится на ту же ветку) → **reviewer**
  (scope, cross-references, `PROPOSED → ACTIVE`, merge и push делает reviewer) → **founder ★**
  обязателен, если правка меняет ФАЗЫ/ПРИОРИТЕТЫ/границы A/B/C/лимиты.
- **risk-critic дополнительно обязателен**, если документ трогает `docs/fa/risk.md`,
  `fa/killswitch.md`, `fa/oms.md`, **RK-I-\*/INTG-I-\***, анти-оверфит гейт §6.
  ⇒ **правка `docs/03-integration-contract.md` §6 (INTG-I-1..7) ТРЕБУЕТ risk-critic.**
- §1 на `main` явно убрал формулировку «docs-правка → без critic»: «Документы — НЕ
  „низкорисковая docs-правка“».
- **`gates.md` §1.1 + §5:** любое касание `crates/contracts/**` = contract-RFC + critic
  **И** risk-critic (RISK-BLOCK; MD-only carve-out на contracts НЕ распространяется).
- **Барьер `protected-artifacts`** (CI, `scripts/check_protected_artifacts.sh`): нельзя молча
  потерять `research/critiques/*`, `milestones/*`, `docs/rfc/*`. Удаление — только коммитом с
  `ALLOW-ARTIFACT-DELETE:` в теле.
- **`branch-hygiene.md`:** worktree на роль (`git worktree add /tmp/hft-<role>-<тема> <branch>`);
  общий чекаут не переключать.

### 1.3. Контрактный слой — фактическое состояние (главная фактура Блока 2)

**Заявлено 7 T1-форм** (`docs/05-contract-layer.md` §2, `docs/fa/contracts.md` §5).
**Фактически в `crates/contracts/src/lib.rs`:**

| T1 по документу | Где реально живёт тип | Governance |
|---|---|---|
| `Event` / `EventKind` | ✅ `crates/contracts/src/lib.rs:137,148` | contract-RFC, JSON Schema, фикстуры |
| `SegmentHeader`, `DataSource`, `LegacySegmentDecl`, `LegacyManifest` (добавлены CT-RFC-02) | ✅ `crates/contracts/src/lib.rs:55–133` | contract-RFC |
| `SignalRegistry` entry | ❌ `crates/signals/src/registry.rs:19` (`RegistryEntry`) | вне RFC |
| `ValidationReport` | ❌ `crates/research-cli/src/types.rs:107` | вне RFC |
| `TrialsLedger` entry | ❌ `crates/research-cli/src/types.rs:34` (`TrialRecord`) | вне RFC |
| `SignalSpec` | ❌ **типа не существует** (есть `SignalSpecRef` в `crates/signals/src/lib.rs:106`; сама карточка — markdown `research/specs/S-001-*.md`) | прозой |
| `Ctl(ParamChange)` | ❌ **не существует** (комментарий-заглушка `crates/contracts/src/lib.rs:153`) | прозой |
| `Decision` (`D-NNN`) | ❌ **не существует**; каталога `research/decisions/` в репо нет | прозой |

**Формы, пересекающие границу наружу (в браузер / в TS), объявлены ВНЕ контрактов и НАМЕРЕННО
выведены из-под governance:**
`crates/gateway/src/lib.rs` — `Selector`, `Cursor`, `OhlcvRow`, `VolumeProfileRow`, `HeatmapCell`,
`CobLevel`, `BubbleCell`, `DepthRow`, `SeriesBundle`, `Snapshot`, `Frame`. Управляются константой
`GATEWAY_SCHEMA_VERSION: u32 = 8` с комментарием, дословно: **«T-designate (не T1, не
`crates/contracts`)»**. Без JSON Schema, без фикстур старых версий, без contract-RFC.

**Это не «пока не дошли руки» — это уже привело к нужному классу дефекта.** Из истории версий
(комментарии `crates/gateway/src/lib.rs:37–65`):

- **v5 → v6 (M-36):** «Форма `Vec<(i64,i64)>` НЕИЗМЕННА, но **СЕМАНТИКА пересмотрена**» — VWAP
  перестал быть session-anchored и стал journal-cumulative. Консюмер, читающий v5, получает
  **валидный по форме и неверный по смыслу** ответ. Ни один schema-валидатор такого не ловит —
  ловит только версионированный контракт с миграционной заметкой.
- **v6 → v7 (M-38a):** форма `cvd_session_base` изменена `i64 → Vec<(session_id, base)>` —
  **non-additive**, что по `docs/05` §4 = **major bump + миграция**, т.е. полноценный
  contract-RFC. Прошло как правка константы.
- **v7 → v8 (M-48):** добавлены `history_start_seq`/`history_truncated`; в комментарии честно
  записано, что v7-консюмер прочитает усечённую историю как полную.

Параллельно существует ВТОРАЯ, независимая нумерация того же downstream-контракта:
`research/exports/format.md` — `export_schema_version: 1`, **Owner: `research-dev`**, консюмер —
`code2alpha` (**репозиторий вне нашего дерева**). Две версионные шкалы на одного потребителя.

⇒ **Это и есть будущий SaaS-wire-формат: сегодня его читает один фронт founder'а, завтра —
десятки тысяч клиентов и Agent Runtime.**

**Прочее, проверенное:**

| Факт | Пруф |
|---|---|
| `SCHEMA_VERSION = 4`; схемы в репо: `crates/contracts/schema/{event,segment-header,legacy-manifest}.schema.json`; фикстуры `valid/` 8 шт., `invalid/` 7 шт. | `crates/contracts/src/lib.rs:26`; `git ls-tree -r origin/main` |
| **`scripts/verify_contracts.sh` НЕ СУЩЕСТВУЕТ**, хотя обещан в `docs/05` §5 и `docs/fa/contracts.md` §8/§T. CT-I-4 фактически держит `crates/contracts/tests/red_schema.rs::ct_i_4_committed_schema_matches_rust_types` | `git ls-tree origin/main -- scripts/` |
| **CT-RFC-05 существует в коде (`MarginInventory`, SCHEMA_VERSION→4, тест `ct_rfc05.rs`), но документа `docs/rfc/CT-RFC-05-*.md` НЕТ.** Это нарушение `docs/05` §4 (RFC = атомарный набор из 7 пунктов, включая rationale-документ) | `git ls-tree -r origin/main \| grep rfc` |
| Дублирующийся исторический путь RFC: `docs/contract-rfc/CT-RFC-01` и `docs/rfc/CT-RFC-01` | то же |
| `research/registry/signals.json` (граница B, объявлен SACRED в scope-guard) — **файла нет** | `git ls-tree -r origin/main -- research/` |
| `research/trials-ledger.jsonl` — расширение разошлось с доками (`.json` в `gates.md` §6, `docs/03` §6) | то же |
| CT-I-5 («Python-тулинг валидирует против той же схемы») — **фикция**, Python-кода в репо нет (подтверждено `docs/08` R9) | `docs/08` R9 |
| **Канарейка CT-I-1 существует и работает** — `crates/contracts/tests/ct_rfc01.rs:147 ct_i_1_single_definition_canary`: рекурсивно обходит `crates/`, ищет текстовый needle `enum <T> {`, требует ровно 1 попадание и путь `contracts/src/lib.rs`. **Это готовый шаблон для CT-I-7/CT-I-10** (§4.3) — механизм изобретать не нужно, нужно обобщить | `crates/contracts/tests/ct_rfc01.rs:145–173` |
| **Но канарейка покрывает только `Venue` и `MdPayload`** — `EventKind`, который `docs/05` §4 прямо называет примером grep-канарейки, **не проверяется**. Микро-случай нашего же класса «заявлено ≠ проверено» | там же (в теле теста только два needle) |
| Покрытие инвариантов оракулами (замер architect'а 2026-07-29, DESIGN-v2 §12): INTG-I 7/0, BK-I 8/0, VN-I 9/0, CT-I 6/5, JR-I 7/2 | `docs/DESIGN-v2.md` §12 |
| **Перепроверено 2026-07-31 — таблица §12 уже частично УСТАРЕЛА:** `INTG-I` — подтверждено **0 оракулов** (единственное упоминание — комментарий `crates/strategy/tests/structural.rs:3`, не тест); `BK-I` — подтверждено **0**; **`VN-I` — БОЛЬШЕ НЕ 0**: `crates/venue-hyperliquid/tests/{red_parse_l2book,red_parse_trades,red_malformed_envelope,red_fail_closed_values}.rs` ссылаются на VN-I (приземлился M-41). ⇒ **при принятии v2 таблицу §12 нужно ПЕРЕМЕРИТЬ, а не скопировать** — иначе мастер-документ въезжает в `main` с уже неверным числом (наш же класс «решение по числу, которое никто не перемерил», `docs/06` §2) | `git grep -l "<ID>" origin/main -- '*/tests/*'` |
| Прод: 1 × Hetzner cpx32, `next_seq ≈ 122 млн` событий, 125 сегментов (115 `.zst`) | `docs/DESIGN-v2.md` §0/§11 `[verify-at-impl — перепроверить ssh на момент работ]` |

### 1.4. Точки расхождения подчинённых документов с DESIGN-v2 (стартовый список)

| Документ | Место | Что расходится |
|---|---|---|
| `docs/DESIGN.md` | §0 «НЕ-цели» | «**Не многопользовательская SaaS**» — прямо отменено founder'ом |
| `docs/DESIGN.md` | шапка, §1.5, §10 | «источник правды», роадмап P0–P5, схема «три подсистемы» — вытесняются v2 §0/§2/§14 и `09-roadmap-v2` Ф0–Ф6 |
| `docs/06` | §1 | «Мы НЕ поднимаем старый стек… построен под **market-data-as-a-service**… Никакого Redis/ClickHouse/MinIO» — обоснование мертво (продукт = сервис данных); DESIGN-v2 §4 вводит HOT/WARM/COLD **как проекции** |
| `docs/06` | §2 / §2-old | замер-опровержение живёт на `docs/06-volume-truth`, **не в main** |
| `docs/06` | §5.1 | открытый вопрос «снапшоты vs диффы» закрыт DESIGN-v2 §7 (L2Delta обязателен, инверсия ролей) — но сам §5.1 тоже не в main |
| `docs/06` | §7, §10, §11 | «одна cpx32 на весь P0–P4», «Cloud Volume/Storage Box не нужны», «1–3 пары» — противоречит DESIGN-v2 §11 (разнос узлов: событийный / fan-out ×N / app / batch) |
| `docs/07` | §5 D6 и D1 | «**Fastify отменён**» → уточнение founder'а: не отменён, отложен как выделенный API-сервер; Node НЕ релеит горячий бинарь (D1 остаётся в силе) |
| `docs/07` | шапка, §1, §4 | «фронт — founder (`code2alpha`)», «мы — бэкенд» — модель однопользовательского кокпита, а не SaaS-терминала |
| `docs/08` | R7 | severity `HIGH` → **блокер существования** при SaaS (DESIGN-v2 §6) |
| `docs/08` | таблица «Привязка к milestone'ам», ШАГ 0–5 | последовательность построена под кокпит-пивот, а не под Ф0–Ф6 |
| `docs/03` | §1, §4, §5 | границы A/B/C описаны только для «квант-деска founder'а» → расширяются на «агенты пользователей» (DESIGN-v2 §8.3) |
| `docs/03` | §6 | INTG-I-1..7 заявлены как RED, оракулов 0 (R9) — при расширении на пользователей ложная гарантия становится опаснее |
| `docs/01`, `docs/02`, `docs/fa/*` | — | требуют сплошной вычитки на однопользовательскую модель, единственный VPS, отсутствие tenant/квот; `fa/risk.md`, `fa/killswitch.md`, `fa/oms.md` описывают несуществующие крейты (должны получить статус DEFERRED-шва) |
| `docs/09-roadmap-v2.md` | весь | новый фазовый документ Ф0–Ф6; надо решить его отношение к `DESIGN.md` §10, `docs/08` «Последовательность работ», `milestones/BACKLOG.md` |

---

## §2. Governance-рамка миграции (какой гейт где) — BINDING

```
                      ветка docs/design-v2-adopt (PROPOSED)
architect ──пишет──▶ critic ──вердикт C-NNN──▶ [risk-critic, если тронуты INTG-I/RK-I]
                          │                          │
                          └──── REJECT → тот же цикл ┘   (gates.md §9 «петля самоправки»)
                                       │ PASS
                                       ▼
                        reviewer: scope + cross-ref + PROPOSED→ACTIVE
                                       │ merge + push в main
                                       ▼
                        §8 post-merge деплой-гейт (docs-only: CI success;
                        ssh-проверка нужна, только если правка тронула деплой)
                                       ▼
                              founder ★ (фазы/приоритеты/границы)
```

**Явные следствия для этой работы:**

1. **Ни один шаг не пушится в `main` автором.** Мержит и пушит reviewer (`gates.md` §9 п.3,
   `commit-discipline.md` «Auto-push» п.4).
2. **Вердикт критика коммитится на ветку milestone'а** тем же ответом, где вынесен
   (`branch-hygiene.md` п.3). Untracked-вердикт = потерянный аудит-трейл + провал
   `protected-artifacts`.
3. **Founder-подпись.** Founder уже подписал ЗАМЕНУ `DESIGN.md` на `DESIGN-v2.md`. План
   предполагает, что подпись **не покрывает автоматически**: (а) `docs/09-roadmap-v2.md`
   (фазы Ф0–Ф6 = приоритеты), (б) расширение границ A/B/C в `docs/03` (§9 п.4 называет границы
   явно), (в) переприоритизацию `docs/08`/`BACKLOG.md`. Эти три подписи запрашиваются отдельно —
   см. §7 «Открытые вопросы».
4. **Каждое касание `crates/contracts/**` = отдельный contract-RFC** с полным набором из 7
   пунктов (`docs/05` §4) + critic + **risk-critic** + reviewer Block-C.
5. **RED-first без исключений.** Для Блока 2 это означает: оракул пишется architect'ом ДО
   промоушена типа, и обязан ПАДАТЬ против текущего состояния (тип ещё в чужом крейте / канарейки
   нет). Оракул, зелёный сразу, — дефект (`testing.md` анти-плацебо).

---

## §3. БЛОК 1 — замена мастер-документа и синхронизация подчинённых

### 3.0. Решение о форме замены (рекомендация архитектурного плана)

**Рекомендуется: сохранить ИМЯ `docs/DESIGN.md` как мастер-документ, заменив его СОДЕРЖИМОЕ на
текст `DESIGN-v2.md`; v1 сохранить как `docs/archive/DESIGN-v1.md` со `STATUS: SUPERSEDED`.**

**Обоснование — замер:** `git grep -l "DESIGN\.md" origin/main` → **33 файла, 74 вхождения**:
`docs/fa/*` — 11, **`research/critiques/*` — 6**, корень (`CLAUDE.md`, `PROJECT-STATE.md`,
`TECH-DEBT.md`, …) — 4, `.claude/rules/*` — 4, `.claude/agents/*` — 2, плюс `BACKLOG.md`,
`SESSION-HANDOFF.md`, `docs/01/04/07/08`.

**Решающий аргумент — 6 ссылок в `research/critiques/`.** Вердикты критиков — **неизменяемые
аудит-артефакты**, защищённые CI-барьером `protected-artifacts`; переписывать их задним числом
нельзя (это ровно тот аудит-трейл, ради которого барьер написан). При переименовании мастера
6 вердиктов навсегда останутся ссылающимися на несуществующий файл, а `verify_docs_links.sh`
придётся с самого рождения снабдить исключением — то есть гейт родится с дырой.

Альтернатива («мастером становится файл `DESIGN-v2.md`») допустима, но тогда Б1.1 разрастается на
27 правимых файлов + вечное исключение для 6 неправимых.
**Решение — founder ★** (см. §7 п. 1).

### 3.1. Шаги Блока 1

Все шаги — одна ветка `docs/design-v2-adopt` (PROPOSED), кроме Б1.0, который идёт отдельно и
ПЕРВЫМ. Внутри ветки — атомарные коммиты по шагам (`commit-discipline.md`).

---

#### **Б1.0 — Домерж `docs/06-volume-truth` в `main` (предусловие, отдельная ветка)**

| | |
|---|---|
| **Зачем** | DESIGN-v2 §7 объявляет «§5.1 закрыт» и опирается на «замер §2», но обоих текстов в `main` НЕТ. Синхронизировать `docs/06` с DESIGN-v2, не имея в `main` того, что синхронизируем, — гарантированное противоречие. Это же — задача №4 существующего `milestones/M-42-docs-governance-sync.md`. |
| **Исполнитель** | **architect** (ребейз ветки на текущий `main`) → **critic** → **reviewer** (merge + push).<br>**Проверено:** `origin/docs/06-volume-truth` = ОДИН коммит `0307035` поверх ДАВНЕГО main (его `research/critiques/` заканчивается на `C-005`, тогда как в `main` уже `C-039`) ⇒ (а) нужен ребейз, ff-мержа не будет; (б) **вердикта критика на эту правку НЕТ**, а `docs/06` — класс A ⇒ critic обязателен. Формулировка `M-42` «critic не нужен (docs-only)» здесь НЕ применима (см. Р10). |
| **Артефакт** | `origin/main` содержит `docs/06` §2 (опровержение) + §2-old + §5.1; вердикт `research/critiques/C-0NN.md` закоммичен на ветке |
| **Готовность** | `git show origin/main:docs/06-data-layer-and-storage.md \| grep -c "ОПРОВЕРГНУТА ЗАМЕРОМ"` → `1`; CI green |
| **Параллельность** | Блокирует Б1.2. Не блокирует Б1.1. |

---

#### **Б1.1 — Перенос DESIGN-v2 + roadmap-v2 в мастер-позицию**

| | |
|---|---|
| **Исполнитель** | **architect** (scope-guard: `docs/` — зона architect'а) |
| **Что делает** | 1. Ветка `docs/design-v2-adopt` от `origin/main`, worktree `/tmp/hft-architect-designv2`.<br>2. Rebase/cherry-pick 3 коммитов `origin/docs/design-v2` (`f9a03d4`, `16206f6`, `db2f9ab`).<br>3. Содержимое `DESIGN-v2.md` → `docs/DESIGN.md`; статус `DESIGN v2 PROPOSED → ACTIVE` ставит reviewer на мерже. Старый текст → `docs/archive/DESIGN-v1.md`, `STATUS: SUPERSEDED by DESIGN.md v2 (2026-07-31)`, с явной пометкой, какие § перенесены в v2 без изменений (v2 сам называет: §1 журнал, §3 T-модель, §4 RK-I, §5 sim).<br>4. `docs/09-roadmap-v2.md` вносится как есть; в нём — явная строка о его отношении к `DESIGN.md` §10 и `docs/08`. |
| **Артефакты** | `docs/DESIGN.md` (v2), `docs/archive/DESIGN-v1.md`, `docs/09-roadmap-v2.md`; удалён `docs/DESIGN-v2.md` (имя больше не нужно) |
| **Готовность (проверяемо)** | (а) `grep -n "Не многопользовательская SaaS" docs/` → пусто;<br>(б) `grep -rn "DESIGN-v2.md" docs/ .claude/ CLAUDE.md milestones/` → пусто (кроме `docs/archive/`);<br>(в) `grep -c "\[ЕСТЬ\]\|\[ЧАСТИЧНО\]\|\[ПЛАН\]" docs/DESIGN.md` > 0 — маркировка честности сохранена;<br>(г) все ссылки в §Cross-references резолвятся (`scripts/verify_docs_links.sh`, см. Б1.7 и Р4);<br>(д) **таблица §12 ПЕРЕМЕРЕНА на момент коммита** (`git grep -l "<ID>" -- '*/tests/*'` по каждому семейству), а не скопирована из замера 2026-07-29 — как минимум `VN-I` уже неверен (§1.3) |
| **Гейт** | class A → critic (обязателен) → reviewer; **founder ★** (фазы/роадмап) |

---

#### **Б1.2 — `docs/06-data-layer-and-storage.md`: снятие «мы НЕ сервис данных» + закрытие §5.1**

| | |
|---|---|
| **Исполнитель** | **architect** |
| **Что делает** | §1: обоснование «построен под market-data-as-a-service ⇒ нам не нужно» **аннулируется явно** (не удаляется молча — это след решения; помечается как `ОТМЕНЕНО 2026-07-31: продукт — именно сервис данных`), заменяется формулировкой DESIGN-v2 §4: БД-слои допустимы как **производные проекции**, никогда как источник истины; выбор WARM (ClickHouse vs Parquet+DuckDB) и HOT (in-proc vs Redis-класс) — по нагрузочным замерам (DESIGN-v2 §14.5).<br>§5.1: `ОТКРЫТЫЙ ВОПРОС → ЗАКРЫТ решением DESIGN.md §7` (L2Delta основной поток, снапшот — редкий якорь 10–30 с; M-45 = предусловие продукта).<br>§7/§10/§11: «одна cpx32 на весь P0–P4» → целевая топология DESIGN.md §11 (событийный узел / fan-out ×N / app / batch), с сохранением текущего состояния как `[ЕСТЬ]`.<br>§9 «Открытые вопросы»: вычистить закрытые, добавить оставшиеся из DESIGN.md §14. |
| **Предусловие** | Б1.0 (иначе правим текст, которого нет) |
| **Готовность** | `grep -n "НЕ market-data-as-a-service\|market-data-as-a-service" docs/06*` → только в блоке с меткой `ОТМЕНЕНО`; `grep -n "ОТКРЫТЫЙ ВОПРОС" docs/06*` → §5.1 отсутствует в списке открытых; §7 не утверждает «одна машина на весь роадмап» |
| **Гейт** | class A → critic → reviewer |

---

#### **Б1.3 — `docs/07-cockpit-backend-roadmap.md`: D6/D1 уточнение + SaaS-контекст**

| | |
|---|---|
| **Исполнитель** | **architect** |
| **Что делает** | D6: «Fastify лишний» → **«Fastify не отменён, отложен: базово app-плоскость = Next.js full-stack; Fastify выделяется как отдельный API-сервер при росте (B2A API, тяжёлый rate-limit); НЕ релей market-данных»** (формулировка DESIGN.md §2, примечание к D6).<br>D1 остаётся в силе и усиливается: Node НЕ релеит горячий бинарь; добавить обоснование fan-out на десятки тысяч клиентов (DESIGN.md §6).<br>Шапка/§1/§4: «фронт — founder (`code2alpha`), мы — бэкенд» → перечитать в терминах продукта AlphaQuant; кокпит = Workspace-терминал (Ф4 roadmap-v2), а не персональный кокпит founder'а.<br>§8 milestone-последовательность: пометить как **историческую** и переадресовать на `docs/09-roadmap-v2.md` (единый источник порядка фаз). |
| **Готовность** | `grep -n "Fastify отменён" docs/07*` → пусто; D6 содержит слова «выделенный API-сервер»; §8 несёт явную строку «авторитетный порядок — `docs/09-roadmap-v2.md`» |
| **Гейт** | class A → critic → reviewer; **founder ★** (D6 — решение, не описание) |

---

#### **Б1.4 — `docs/08-arch-improvement-roadmap.md`: R7 → блокер + пересборка последовательности**

| | |
|---|---|
| **Исполнитель** | **architect** |
| **Что делает** | R7: severity `HIGH` → **`BLOCKER (существования при SaaS)`** со ссылкой на DESIGN.md §6 (N клиентов = N сканов; соединения без cap; `GATEWAY_WINDOW_MS` parse-error → unbounded).<br>Пересмотреть severity остальных под SaaS: R1 (CRIT, остаётся), R5 staleness (при платном продукте — «показ мёртвой ликвидности платящему клиенту»), R10в алертинг (SLA подписки).<br>«Последовательность работ» и таблица «Привязка к milestone'ам» → согласовать с Ф0–Ф6 `09-roadmap-v2` (не дублировать: `docs/08` = каталог рисков, `docs/09` = порядок фаз; в каждом — явная строка о разделении ролей).<br>Добавить R11-класс: **«контрактный дрейф» — формы наружу вне T1** (фактура §1.3 этого плана) как отдельный системный риск, входящий в Блок 2. |
| **Готовность** | `grep -n "R7" docs/08*` показывает `BLOCKER`; в `docs/08` есть строка, называющая `docs/09-roadmap-v2.md` источником порядка; риск контрактного дрейфа заведён с ID и владельцем |
| **Гейт** | class A → critic → reviewer; **founder ★** (переприоритизация) |

---

#### **Б1.5 — `docs/03-integration-contract.md`: границы A/B/C → «агенты пользователей»**

| | |
|---|---|
| **Исполнитель** | **architect** |
| **Что делает** | §1: правило направления INTG-I-1 расширяется: «LLM» → «LLM/агент (платформенный ИЛИ пользовательский)».<br>§2 (граница B): реестр = «то, что исполняется, существует только как версионированная запись со статусом и `code_hash`» — распространяется на пользовательские правила/алерты/стратегии; per-tenant скоуп.<br>§3 (граница C): для платформы — Ed25519-подпись founder'а; **для пользователя — approval в UI с аудитом** (DESIGN.md §8.3, AlphaQuant §12.2).<br>§4 (граница A): зона пользовательского агента = его tenant-данные в Postgres (annotation/note/spec), запись в event-store не существует (PL-I-2/PL-I-3).<br>§6: INTG-I-1..7 **честно помечаются `PENDING — оракул не написан`** (R9) и получают редакцию «в терминах агентов пользователей»; добавляются PL-I-2/3/4/9 как связанные. |
| **⚠ Гейт** | class A → critic → **risk-critic ОБЯЗАТЕЛЕН** (документ трогает INTG-I-*, `gates.md` §9) → reviewer; **founder ★** (границы A/B/C названы явно в §9 п.4) |
| **Готовность** | каждый INTG-I-1..7 несёт либо путь к оракулу, либо явную метку `PENDING P-<фаза>`; ни один инвариант не заявлен как действующий без файла-оракула |

---

#### **Б1.6 — Сплошная вычитка `docs/01`, `docs/02`, `docs/00`, `docs/fa/*`**

| | |
|---|---|
| **Исполнитель** | **architect** (правки), разведка — **explore** (дёшево) |
| **Что делает** | Механический скан на: «не SaaS», «один пользователь», «личный», «единственный VPS/одна машина», отсутствие tenant/квот там, где SaaS их требует; упоминания `risk`/`killswitch`/`oms` как строящихся.<br>`docs/fa/risk.md`, `fa/killswitch.md`, `fa/oms.md` → шапка `STATUS: DEFERRED — крейта не существует; документ описывает ШОВ (DESIGN.md §9), не реализацию`.<br>`docs/fa/viz-backend.md`, `fa/ai-copilot.md` → согласовать с DESIGN.md §6/§8 (проекции, Agent Runtime, роли AI).<br>`docs/fa/contracts.md` → **не трогать в Блоке 1**, он целиком переписывается в Блоке 2 (Б2.1). |
| **Готовность** | сводный отчёт explore «файл → строка → цитата → тип расхождения» приложен к вердикту критика; после правок `grep -rniE "не.{0,3}многопользовательск\|не SaaS\|единственн\w+ пользовател" docs/` → пусто |
| **Гейт** | class A → critic → reviewer (**risk-critic** дополнительно, если тронуты `fa/risk.md`/`fa/killswitch.md`/`fa/oms.md`) |

---

#### **Б1.7 — Правила, состояние, cross-references**

| | |
|---|---|
| **Исполнитель** | **architect** (`.claude/rules/*`, `CLAUDE.md`), **reviewer** (`PROJECT-STATE.md`, `TECH-DEBT.md`) |
| **Что делает** | `CLAUDE.md`: «Источник правды по архитектуре — `docs/DESIGN.md`» — остаётся верным при рекомендации §3.0; порядок чтения дополняется `docs/09-roadmap-v2.md`. Принцип «LLM НЕ в горячем торговом цикле» → редакция DESIGN.md §1.7 (LLM порождает ПРЕДЛОЖЕНИЕ). Принцип «Риск fail-closed RK-I-1..10 sacred» → пометка «PENDING фазы execution; крейтов нет».<br>`gates.md` §5 RISK-BLOCK: явная строка «спит до фазы execution (крейтов risk/killswitch/oms нет); действует на `crates/contracts/**` безусловно» — сейчас читается так, будто блокирует несуществующее.<br>`gates.md`: новая подсекция про **Node/TS-код** (пробел №4 DESIGN-v2 — кто пишет оракулы для Next.js/Fastify/Agent Runtime) — либо явная запись «решается на входе Ф3», чтобы пробел не был молчаливым.<br>**Новый `scripts/verify_docs_links.sh`** (architect, зона `scripts/verify_*`): все `docs/**.md` ссылки на файлы репо резолвятся; `VERDICT: PASS/FAIL`, `exit≠0` на FAIL. Это превращает «cross-references не сломаны» из мнения reviewer'а в гейт.<br>`PROJECT-STATE.md`/`TECH-DEBT.md`: гигиена долга по `09-roadmap-v2` §3 — 15 записей помечены CLOSED внутри `## OPEN`, TD-043 продублирован, TD-022/TD-024 противоречат живому проду, статус не заполнен у M-38b/M-47/M-48. |
| **Готовность** | `bash scripts/verify_docs_links.sh; echo exit=$?` → `VERDICT: PASS`, `exit=0`; в `TECH-DEBT.md` секция `## OPEN` не содержит записей со словом CLOSED; `grep -c "TD-043" TECH-DEBT.md` = ожидаемое число вхождений |
| **Гейт** | `.claude/rules/*` — class A → critic → reviewer (петля самоправки §9). `PROJECT-STATE`/`TECH-DEBT` — **class B**, self-push reviewer'а разрешён |

---

## §4. БЛОК 2 — ревизия контрактного слоя

> Требование founder'а дословно: «перепроверить весь контрактный слой и внести туда изменения,
> чтобы весь датафлоу и все процессы **ЖЁСТКО ЗАВИСЕЛИ ОТ КОНТРАКТОВ**».
>
> Ключевая мысль плана: «жёстко зависит» — это не «мы так написали в docs». Это **свойство,
> которое падает, если его нарушить**. У нас уже есть класс дефекта «заявлено в docs, оракулов
> ноль» (INTG-I 7/0, BK-I 8/0, VN-I 9/0, CT-I-5 — фикция). Поэтому Блок 2 построен как:
> **сначала механизм проверки (Б2.2), потом промоушены (Б2.4+)** — иначе мы просто перенесём
> декларации на этаж выше.

### 4.0. Где зависимость УЖЕ жёсткая, а где нет (рамка, чтобы не чинить работающее)

**Проверено:** все 16 крейтов workspace имеют `contracts` в `[dependencies]`
(`git show origin/main:crates/<c>/Cargo.toml | grep '^contracts'` — 16/16). Внутри Rust
зависимость держит **компилятор**: чужой формат просто не соберётся. Это работает и ломать это
не надо.

**Зависимость исчезает ровно там, где кончается Rust:**

| Граница | Кто держит форму сегодня | Чем это плохо |
|---|---|---|
| Rust → браузер / TS | ручной `export_schema_version` + `#[serde(default)]` в `crates/gateway` | компилятора на той стороне нет; расхождение обнаружит пользователь |
| Rust → файл/архив (сегменты, ledger, export) | частично контракты (`SegmentHeader`), частично проза (`research/exports/format.md`) | бессмертные данные под неформальной формой |
| Rust ↔ конфиг/env (лимиты, allow-list, окна) | строки и `unwrap_or` | `GATEWAY_WINDOW_MS` parse-error → unbounded (R7) — цена уже заплачена |
| Rust ↔ Postgres / Agent Runtime (будущее) | ничего (не существует) | если не заложить сейчас — повторим hft-core-rs |
| Rust ↔ «человеческие» артефакты (D-NNN, signals.json, SignalSpec) | markdown и намерение | границы B/C формально SACRED, физически отсутствуют |

⇒ Блок 2 целится **в границы, а не в ядро**. Ядро уже жёсткое.

### 4.1. Инвентаризация: где датафлоу СЕЙЧАС не зависит от контрактов

| # | Кандидат | Где живёт сейчас | Кто консюмер | Почему это T1 (или почему нет) | Приоритет |
|---|---|---|---|---|---|
| **К1** | **WS/экспорт-формы кокпита**: `Snapshot`, `Frame`, `SeriesBundle`, `Selector`, `Cursor`, `OhlcvRow`, `VolumeProfileRow`, `HeatmapCell`, `CobLevel`, `BubbleCell`, `DepthRow` | `crates/gateway/src/lib.rs`, `GATEWAY_SCHEMA_VERSION = 8`, помечено «T-designate, не T1»; плюс вторая шкала `export_schema_version: 1` в `research/exports/format.md` | браузер (Next.js/TS), `code2alpha` (репо вне дерева), Agent Runtime tools, десятки тысяч клиентов | **Кросс-языковая граница №1 SaaS.** Уже дала non-additive смену формы (v6→v7) и **смену СЕМАНТИКИ при неизменной форме** (v5→v6 VWAP) без RFC и без миграционной заметки — ровно болезнь hft-core-rs (несовместимые wire-форматы), только в зачаточной стадии | **P0** |
| **К2** | `source_kind` / `license_class` на источнике данных | не существует; есть `DataSource{OwnCapture\|Vendor\|Synthetic}` + `provenance: String` (свободный текст) в `SegmentHeader` | Source Adapter, read-path отдачи наружу, research | DESIGN.md §7.1: замена источника обязана быть тривиальной; §12 LIC-I-1: **отсутствие метки = `internal-only` (запрет)**. Свободнотекстовый `provenance` машинно не проверяем | **P0** (метаданные; сам гейт LIC — Ф6) |
| **К3** | **Watermark проекции** `(last_applied_seq, reducer_code_version)` | не существует | gateway-serve, HOT/WARM-проекции, клиентский ответ (staleness-метка) | DESIGN.md §4: «каждая проекция несёт watermark»; PL-I-7 «staleness в типе». Без типа — это соглашение в голове | **P0** (Ф2) |
| **К4** | **AI Event** (единая модель: eventId, observation, interpretation, importance, horizon, supporting/conflicting evidence, confirmation, **invalidation**, data quality, annotations; уверенность Low/Med/High) | не существует | Agent Runtime (TS) пишет, UI (TS) читает, audit-store (Postgres), позже — B2A | Кросс-языковой; DESIGN.md §8.1 требует версионируемого контракта; «никаких необоснованных процентов» — свойство ТИПА, не соглашения | **P1** (Ф5) |
| **К5** | Пользовательские сущности: `Workspace`/`View`/`Widget`-layout, `decision-journal` запись, `Agent` (goal/tools/policy/memory), `policy`, `approval`, `audit_log` | не существует (Postgres — [ПЛАН]) | Next.js, Agent Runtime, gateway-serve (claims), позже Rust-редьюсеры replay-закладок | Живут в Postgres, но формы пересекают Rust↔TS (JWT-claims, replay-закладки, tool-контракты). Промоушен — по правилу TD-008 «кросс-языковой консюмер ⇒ в `crates/contracts`» | **P1** (Ф3) |
| **К6** | **JWT-claims тарифа**: `{user_id, tier, allowed_channels, quotas, exp}` | `crates/gateway-serve/src/lib.rs:18` — `struct Claims { sub: String, exp: usize }`, **два поля**, объявлен локально. В коде явно записано: «мы НЕ доверяем claim-метаданным Next.js для авторизации, только самой подписи» | Auth.js/Next.js (TS) выпускает, Rust применяет | PL-I-5: «отсутствие/невалидность лимита = отказ, не unbounded». При SaaS claims **становятся** авторизацией (tier/квоты), т.е. текущее проектное решение придётся сознательно менять — форма на границе двух языков с security-последствиями ⇒ risk-critic обязателен | **P0** (Ф2/Ф3) |
| **К6b** | **WS-конверт протокола**: `ServeMsg{Snapshot, Frame, Error}` | `crates/gateway-serve/src/lib.rs:63` (модуль `wire`), JSON-only | браузер, будущие клиенты/агенты | Это ВЕРХНИЙ уровень wire-протокола (то, что клиент парсит первым). Живёт вне контрактов; бинарные фреймы (heatmap) по DESIGN.md §2 добавят второй кодек — конверт обязан быть контрактом до, а не после | **P0** (вместе с К1) |
| **К7** | `ValidationReport`, `TrialRecord` (trials-ledger) | `crates/research-cli/src/types.rs:107,34` — **объявлены T1 в docs, но не в `contracts`** | research-cli пишет, critic/founder/анти-оверфит-гейт читают | Уже заявлены T1 (docs/05 §2). Расхождение docs↔код — это и есть «мягкая» зависимость | **P1** |
| **К8** | `SignalRegistry` entry | `crates/signals/src/registry.rs:19`; файла `research/registry/signals.json` **нет** | движок читает, деск пишет через подпись | Граница B. Объявлена SACRED в scope-guard, физически отсутствует | **P2** (спит до квант-фазы, но статус в доках обязан быть честным) |
| **К9** | `SignalSpec`, `Ctl(ParamChange)`, `Decision` | **типов нет**; `SignalSpec` — markdown; `ParamChange` — комментарий-заглушка `contracts/src/lib.rs:153`; `research/decisions/` — каталога нет | границы A/B/C | Вводятся contract-RFC при старте своих фаз (DESIGN.md §9 п.2). Сейчас — честная метка PENDING, НЕ промоушен | **P3 / DEFERRED** |
| **К10** | Форматы «прозой и по соглашению»: `research/exports/format.md` (export v1), `recorder.heartbeat` (JSON `ts_wall_ms/next_seq/segment_index/free_bytes/writable`, форма задана только в комментарии `docker-compose.yml`), `storage_status`, Prometheus-метрики, `deploy/alerts/ops.rules.yml` | markdown + код + yaml | recorder, gateway, ops, деплой, оператор, healthcheck | Не всё обязано быть T1. Критерий отбора — §4.2. Heartbeat — пограничный: его читает healthcheck контейнера и оператор, форма нигде не типизирована | **разбирается по критерию** |
| **К11** | **Topic / Subscription descriptor** — `venue × symbol × серия × разрешение` | не существует как тип; сегодня это ENV одного процесса: `GATEWAY_VENUE`, `GATEWAY_SYMBOL`, `GATEWAY_TIMEFRAME_MS`, `GATEWAY_BANDS`, `GATEWAY_WINDOW_MS` (`docker-compose.yml`, дефолты `:-`) | клиент (TS) подписывается, gateway-serve валидирует, тарифный enforce режет по нему | DESIGN.md §6 строит fan-out **per topic**, а §5 режет тарифы по «N символов / M соединений / разрешение (Free = 1 с, не 100 мс)». Сейчас «что отдаётся» решает env ОДНОГО процесса, клиент об этом не знает и проверить не может. При SaaS подписка обязана быть типизированным запросом, валидируемым против claims (PL-I-5) | **P0** (Ф2) |
| **К12** | **Coverage / Instrument descriptor** — какой инструмент есть, с какой fidelity, с какой глубиной истории, с каким `license_class` | размазано: `L2DELTA_CAPTURE_SYMBOLS = &["BTCUSDT"]` (`crates/venue-binance-futures/src/lib.rs:460`, хардкод-константа), `env_csv("BINANCE_SYMBOLS", ["BTCUSDT","ETHUSDT"])` (`crates/recorder/src/main.rs:374,381,388,427`) | UI (что показать), тарифный слой, Agent Runtime tools, research | Продуктово-видимый факт: BTC имеет суб-секундную книгу, ETH — нет (M-45 это чинит, но частично и всегда будет граница). Продавать «real-time order flow: N символов», не имея машинного описания покрытия, — обещание без основания. Связывается с К2 (`license_class`) | **P1** (Ф1/Ф2) |

### 4.2. Критерий «что становится T1» (обязателен к фиксации в `docs/05` §2)

Сегодня в `docs/05` критерия НЕТ — есть список из 7 форм. Без критерия любой спор о промоушене
решается вкусом. Предлагаемый критерий (**решение architect + critic; founder ★ не требуется**):

> Форма — **T1**, если выполняется ХОТЯ БЫ ОДНО:
> 1. **Кросс-языковая** (продюсер и консюмер на разных языках: Rust↔TS, Rust↔Python).
> 2. **Бессмертная** (записана в журнал/архив — читается новым кодом навсегда, CT-I-3).
> 3. **Отдаётся наружу платящему клиенту** (WS/REST/API-поверхность).
> 4. **Несёт fail-closed решение** (лимит, лицензионный класс, staleness, approval) — то, чьё
>    отсутствие обязано означать отказ, а не дефолт.
>
> Иначе — T2 (владеет крейт) или T3 (внутреннее). Пограничный случай ⇒ T1 (fail-closed:
> лишний RFC дешевле разошедшегося формата).

По этому критерию К1 (3), К2 (2+4), К3 (4), К4 (1+3), К6 (1+4) — T1 безусловно.

### 4.3. Как сделать «жёсткость зависимости» ПРОВЕРЯЕМОЙ (ядро блока)

Механизмы-кандидаты; каждый — оракул или CI-канарейка, обязан **падать против сегодняшнего
состояния** (иначе это плацебо):

| ID | Что проверяет | Форма проверки | Против чего обязан упасть СЕЙЧАС |
|---|---|---|---|
| **CT-I-7** | **Ни один тип, пересекающий внешнюю границу, не объявлен вне `crates/contracts`** | Канарейка-скрипт: перечень «экспортных» модулей (gateway/gateway-serve/публичные API) сканируется на `#[derive(Serialize)]` у `pub`-типов; whitelist ведётся явно и коротко | Падает: `Snapshot`/`Frame`/`SeriesBundle`/… (К1) |
| **CT-I-8** | **Версия wire-формы не может быть поднята «руками»**: `export_schema_version` bump обязан сопровождаться RFC-документом + перегенерированной схемой + фикстурой | Тест сверяет константу версии против набора файлов `docs/rfc/CT-RFC-NN` и `schema/*.json` | Падает: v6→v7→v8 подняты без RFC |
| **CT-I-9** | **Каждый RFC-номер, упомянутый в коде, имеет документ** (и наоборот) | grep-канарейка `CT-RFC-\d+` по `crates/**` ↔ `docs/rfc/*.md` | Падает: **CT-RFC-05 в коде, документа нет** (см. §1.3) |
| **CT-I-10** | **Каждая T1-форма из `docs/05` §2 определена ровно в `crates/contracts`** (обобщение существующей канарейки с `Venue`/`MdPayload` на ВЕСЬ список T1, включая `EventKind`, который сейчас не проверяется) | **Механизм уже есть** — `ct_i_1_single_definition_canary` (`crates/contracts/tests/ct_rfc01.rs:147`, обход `crates/` + needle `enum <T> {` + assert ровно 1 попадание и путь). Обобщить: список T1 берётся из машиночитаемой таблицы `docs/05` §2, а не хардкодится | Падает: `ValidationReport`, `TrialRecord`, `RegistryEntry` — в чужих крейтах; `SignalSpec`/`ParamChange`/`Decision` — нигде; `EventKind` вообще не под канарейкой |
| **CT-I-11** | **Отсутствие метки = отказ** (fail-closed на метаданных источника): событие/сегмент без `license_class`/`source_kind` не читается read-path'ом отдачи наружу | Оракул с деградированной фикстурой (сегмент без метки) → `Err`, не дефолт | Падает: полей нет вовсе |
| **CT-I-12** | **Watermark обязателен в ответе клиенту**: любой user-facing ответ несёт `(last_applied_seq, reducer_version)`; ответ без него не собирается (тип, не поле-опция) | Тип-барьер: конструктор ответа принимает `Watermark` (приватный конструктор в проекции) | Падает: типа нет |
| **CT-I-13** | **Кросс-языковой паритет реален**: TS-консюмер валидируется против той же JSON Schema теми же фикстурами (замена мёртвого CT-I-5 «Python») | CI-job: генерация TS-типов/валидатора из `schema/*.json` + прогон общих фикстур `valid/`+`invalid/` | CT-I-5 сейчас **фикция** — надо либо реализовать в TS-редакции, либо честно снять |
| **CT-I-14** | **«Объявлено ⟹ проверено»** (мета-канарейка против нашего же класса дефекта): каждый ID инварианта, заявленный в `docs/`, имеет либо файл-оракул, либо явную метку `PENDING` | Скрипт сверки таблиц инвариантов в `docs/**` против `git grep` по `*/tests/` | Падает: INTG-I 7/0, BK-I 8/0, VN-I 9/0 |
| **CT-I-15** | **Смена СМЫСЛА при неизменной форме не проходит молча** (самый тонкий класс — случай v5→v6 VWAP, §1.3) | Golden-фикстура на версию: замороженный вход (кусок журнала) → замороженный выход (байты бандла) на КАЖДУЮ живую версию. Меняется смысл ⇒ golden краснеет ⇒ автор обязан либо откатить, либо поднять версию через RFC с миграционной заметкой | Падает при любой будущей тихой смене семантики; сегодня golden-фикстур на версии нет |

**`scripts/verify_contracts.sh` — создать.** Он обещан в `docs/05` §5 и `docs/fa/contracts.md`
§8/§T, но не существует. Он и есть агрегатор CT-I-1..14 с `VERDICT: PASS/FAIL` и `exit≠0` —
единая точка, которую можно поставить в CI (`ci.yml`, рядом с `protected-artifacts`).

### 4.4. Порядок изменений (журнал не сломать: 122 млн событий, `SCHEMA_VERSION = 4`)

Правило, которое диктует порядок: **аддитивно и только в конец** (postcard-дискриминанты),
`schema_version` bump'ится при каждом новом ЭМИТИРУЕМОМ варианте (комментарий
`contracts/src/lib.rs:13–26`), старые сегменты обязаны читаться байт-в-байт (CT-I-3),
`decide_open_segment` изолирует эпохи по `schema_version` — значит каждый bump = новый сегмент,
и это ожидаемое, не аварийное поведение.

| Шаг | Что | RFC? | Ломает журнал? |
|---|---|---|---|
| **Б2.1** | Переписать `docs/05-contract-layer.md` + `docs/fa/contracts.md`: критерий T1 (§4.2), список T1 v2, CT-I-7..14, честный статус CT-I-5, канонический путь `docs/rfc/` | нет (docs) | нет |
| **Б2.2** | `scripts/verify_contracts.sh` + оракулы CT-I-9, CT-I-10, CT-I-14 (**самые дешёвые и самые доказательные — «зеркало»: они краснеют на сегодняшнем репо**) | нет | нет |
| **Б2.3** | **CT-RFC-05 ретро-документ** (`MarginInventory` уже в проде без RFC) — закрыть governance-дыру ДО новых RFC | да (ретро) | нет |
| **Б2.4** | **CT-RFC-06 — wire-контракт кокпита (К1+К6b):** перенос `Snapshot`/`Frame`/`SeriesBundle`/`HeatmapCell`/… **и конверта `ServeMsg`** в `crates/contracts`, JSON Schema, фикстуры, TS-паритет (CT-I-13), golden-фикстуры на версии (CT-I-15), CT-I-7/CT-I-8. Слить две версионные шкалы (`GATEWAY_SCHEMA_VERSION=8` и `export_schema_version=1`) в одну | да | нет (не журнальные формы; риск — совместимость с развёрнутым gateway-serve и фронтом `code2alpha`) |
| **Б2.5** | **CT-RFC-07 — `source_kind` + `license_class` (К2):** аддитивные поля `SegmentHeader`, `SCHEMA_VERSION → 5`, правило «нет метки = `internal-only`» (CT-I-11), миграционная заметка для существующих 125 сегментов | да | **риск-точка**: bump → новая эпоха сегмента; старые читаются через существующий механизм (`SCHEMA_VERSION_PRE_HEADER`, `LegacyManifest`) — обязателен роундтрип-оракул на реальном хвосте прода |
| **Б2.6** | **CT-RFC-08 — Watermark + Staleness (К3):** тип `Watermark{last_applied_seq, reducer_version}` + `BookView<Fresh\|Stale>`-класс (R5), CT-I-12 | да | нет (read-path) |
| **Б2.7** | **CT-RFC-09 — подписка и тариф (К6+К11):** `Topic{venue, symbol, series, resolution}` + JWT-claims `{user_id, tier, allowed_channels, quotas, exp}` как T1 с Rust↔TS паритетом. Подписка — типизированный запрос, валидируемый против claims; невалидные/отсутствующие лимиты → отказ старта/соединения (PL-I-5). Переводит `GATEWAY_VENUE/SYMBOL/TIMEFRAME_MS/BANDS/WINDOW_MS` из env-дефолтов одного процесса в контракт | да | нет |
| **Б2.7b** | **CT-RFC-09b — Coverage descriptor (К12):** машинное описание «инструмент × fidelity × глубина истории × `license_class`»; хардкод `L2DELTA_CAPTURE_SYMBOLS` и `env_csv(BINANCE_SYMBOLS)` становятся производными от него, не источником | да | нет |
| **Б2.8** | **CT-RFC-10 — AI Event (К4)** и **CT-RFC-11 — пользовательские сущности (К5)** | да | нет |
| **Б2.9** | `ValidationReport`/`TrialRecord`/`RegistryEntry` (К7/К8): либо промоушен в `contracts`, либо **явное понижение до T2 с правкой `docs/05` §2** — но не «заявлено T1, лежит в чужом крейте» | да (если промоушен) | нет |

**Привязка к фазам `09-roadmap-v2`:** Б2.1–Б2.4 — сейчас (не ждут фаз, чинят существующий дрейф);
Б2.5 — Ф0/Ф1 (до расширения L2Delta, пока сегментов относительно мало); Б2.6/Б2.7 — Ф2;
Б2.8 — Ф3/Ф5; Б2.9 — по готовности.

### 4.5. Чего Блок 2 НЕ делает (границы, чтобы не разрослось)

- Не вводит `risk`/`killswitch`/`oms` T1-типы (`RiskApproved`, `Order`) — DEFERRED (DESIGN.md §9).
- Не строит Postgres-схему и Prisma-миграции — это Ф3, здесь только КОНТРАКТЫ форм, пересекающих
  Rust↔TS.
- Не меняет `Event`/`EventKind` payload'ы (кроме аддитивных полей заголовка в Б2.5).
- Не трогает L2Delta-расширение (M-45) — это данные, не контракты.

---

## §5. БЛОК 3 — последовательность, роли, параллелизм

### 5.1. Роли по таблице владения (`.claude/rules/scope-guard.md`, редакция `origin/main`)

| Работа | Роль | Зона (по scope-guard) |
|---|---|---|
| Все правки `docs/**`, `milestones/**`, `.claude/rules/**` | **architect** (Fable) | `docs/`, `milestones/M-NN-*.md`, `.claude/rules/*` |
| T1-типы в `crates/contracts/**`, RED-оракулы `*/tests/**`, `scripts/verify_*.sh` | **architect** — **SACRED, architect-only** | тот же |
| Plan-time аудит документов и milestone'ов | **critic** | ТОЛЬКО `research/critiques/C-NNN.md` |
| Аудит правок, трогающих INTG-I/RK-I и `crates/contracts/**` | **risk-critic** | ТОЛЬКО `research/critiques/C-NNN.md` |
| Импл под RED в `crates/{gateway,gateway-serve,journal,book,ops,recorder,...}/src` | **engine-dev** | по таблице (на `main` в зону engine-dev входят `ops`, `gateway`, `gateway-serve`, `recorder`, `deploy/**`) |
| Импл в `crates/venue-*/src` | **venue-dev** | `venue-*` |
| Импл в `crates/research-cli/src` (если К7 промоутится) | **research-dev** | `research-cli` |
| Разведка «где ещё расходится» | **explore** (дёшево) | read-only |
| Прогон на чистом чекауте | **tester** | read-only |
| PR-гейт, merge, push, `PROJECT-STATE`/`TECH-DEBT`, §8 деплой-гейт | **reviewer** | `PROJECT-STATE.md`, `TECH-DEBT.md`, merge |
| Подписи (замена мастера, фазы, границы A/B/C, права на данные, тарифы) | **founder** | — |

**Важно:** `crates/contracts/**` и `*/tests/**` — SACRED. Ни engine-dev, ни venue-dev, ни
research-dev не пишут туда даже «свой» тип. Промоушен формы в T1 делает **architect**, импл
вокруг него — dev-роль.

### 5.2. Последовательность

```
─── СТРОГО ПОСЛЕДОВАТЕЛЬНО (фундамент) ────────────────────────────────────────
Б1.0  домерж docs/06-volume-truth              reviewer
  │
Б1.1  DESIGN v2 в мастер-позицию               architect → critic → reviewer → founder ★
  │        (без него любая правка подчинённого дока «синхронизирует с чем?»)
  ├──────────────────────────────────────────────────────────────────────────────
  │
  ├─ ПАРАЛЛЕЛЬНО (разные файлы, один цикл гейта на ветке) ──────────────────────
  │   Б1.2  docs/06        architect     (предусловие Б1.0)
  │   Б1.3  docs/07 D6/D1  architect  → founder ★
  │   Б1.4  docs/08 R7     architect  → founder ★
  │   Б1.6  docs/01/02/00/fa/*  architect (+explore на разведку)
  │
  ├─ ОТДЕЛЬНО (свой risk-critic) ──────────────────────────────────────────────
  │   Б1.5  docs/03 A/B/C  architect → critic → risk-critic → reviewer → founder ★
  │
  └─ ПОСЛЕ мержа Б1.1–Б1.6 ────────────────────────────────────────────────────
      Б1.7  .claude/rules/*, CLAUDE.md, verify_docs_links.sh, TECH-DEBT гигиена

─── БЛОК 2 (стартует после мержа Б1.1; Б2.1 зависит от Б1.5 по INTG-I) ────────
Б2.1 docs/05 + fa/contracts (критерий T1, CT-I-7..14)   architect → critic → reviewer
  │
Б2.2 verify_contracts.sh + CT-I-9/10/14 (зеркало)       architect(RED) → engine-dev(GREEN)
  │     ⟵ ДОКАЗАТЕЛЬНАЯ ТОЧКА: они обязаны краснеть на сегодняшнем main
  │
Б2.3 CT-RFC-05 ретро-документ                            architect → critic → risk-critic → reviewer
  │
  ├─ ПАРАЛЛЕЛЬНО (независимые RFC, разные формы) ────────────────────────────
  │   Б2.4 CT-RFC-06 wire-контракт кокпита    architect(RED+типы) → engine-dev
  │   Б2.6 CT-RFC-08 watermark/staleness      architect → engine-dev
  │   Б2.7 CT-RFC-09 JWT-claims               architect → engine-dev
  │
  └─ СТРОГО ПОСЛЕ Б2.2 и в одиночку (трогает журнальный заголовок) ──────────
      Б2.5 CT-RFC-07 source_kind/license_class, SCHEMA_VERSION→5
             architect(RED, вкл. роундтрип на реальном хвосте прода)
             → engine-dev → tester → risk-critic → reviewer → §8 деплой-гейт

Б2.8/Б2.9 — по фазам Ф3/Ф5 и по готовности.
```

**Что НЕЛЬЗЯ параллелить:**
- Б1.1 и всё остальное Блока 1 (иначе синхронизируем с движущейся целью).
- Б2.5 с любым другим касанием `crates/contracts` (bump `SCHEMA_VERSION` — один за раз, иначе
  эпохи сегментов перепутаются).
- Два RFC в одном PR (`docs/05` §4: RFC **атомарный**).

**Что можно и нужно параллелить:** Б1.2/Б1.3/Б1.4/Б1.6 (разные файлы); Б2.4/Б2.6/Б2.7 (разные
формы, ни одна не журнальная); разведку explore — с чем угодно.

### 5.3. Критерии готовности — сводка (проверяемые, не «согласовано»)

| Шаг | Критерий (команда → ожидание) |
|---|---|
| Б1.0 | `git show origin/main:docs/06-data-layer-and-storage.md \| grep -c "ОПРОВЕРГНУТА ЗАМЕРОМ"` → `1` |
| Б1.1 | `grep -rn "Не многопользовательская SaaS" docs/` → пусто; `grep -rn "DESIGN-v2.md" docs/ .claude/ CLAUDE.md milestones/` → пусто вне `docs/archive/` |
| Б1.2 | `grep -n "ОТКРЫТЫЙ ВОПРОС" docs/06*` не содержит §5.1; §1 несёт метку `ОТМЕНЕНО` с датой |
| Б1.3 | `grep -n "Fastify отменён" docs/07*` → пусто; D6 содержит «выделенный API-сервер» |
| Б1.4 | `grep -n "R7" docs/08*` содержит `BLOCKER` |
| Б1.5 | каждый `INTG-I-[1-7]` в `docs/03` §6 имеет либо путь к файлу-оракулу, либо метку `PENDING`; вердикт risk-critic'а закоммичен |
| Б1.6 | `grep -rniE "не.{0,3}многопользовательск\|единственн\w+ пользовател" docs/` → пусто; `fa/{risk,killswitch,oms}.md` несут `STATUS: DEFERRED` |
| Б1.7 | `bash scripts/verify_docs_links.sh; echo exit=$?` → `VERDICT: PASS`, `exit=0`; `## OPEN` в TECH-DEBT без записей CLOSED |
| Б2.1 | `docs/05` §2 содержит машиночитаемый список T1 + критерий из 4 пунктов |
| **Б2.2** | **`bash scripts/verify_contracts.sh` на текущем `main` → `VERDICT: FAIL`, `exit≠0` (доказательство, что канарейка не плацебо); после Б2.3/Б2.4 соответствующие пункты зеленеют** |
| Б2.3 | `docs/rfc/CT-RFC-05-*.md` существует; CT-I-9 зелёный |
| Б2.4 | wire-типы определены только в `crates/contracts` (CT-I-7 зелёный); TS-фикстуры проходят (CT-I-13); `export_schema_version` bump'нут через RFC (CT-I-8) |
| Б2.5 | роундтрип старых сегментов на **реальном хвосте прода** байт-в-байт; сегмент без `license_class` → `Err` (CT-I-11); §8 деплой-гейт: recorder пишет, heartbeat свежий, сегмент растёт |
| Б2.6 | ответ клиенту без watermark **не компилируется** (тип-барьер, не runtime-проверка) |
| Б2.7 | старт gateway-serve с невалидным/отсутствующим лимитом → **отказ старта**, не unbounded |

---

## §6. РИСКИ МИГРАЦИИ

| # | Риск | Где рвётся | Митигация |
|---|---|---|---|
| **Р1** | **Промежуточное состояние: v2 в `main`, подчинённые доки — старые.** Агент, читающий `DESIGN.md`(v2) + `docs/06`(v1), получает прямое противоречие «сервис данных vs НЕ market-data-as-a-service» и исполняет по тому, что прочитал первым | между Б1.1 и Б1.2/1.3/1.4 | Б1.1 **обязан** внести в `DESIGN.md` явный блок «MIGRATION IN PROGRESS: подчинённые доки 01/02/03/06/07/08/fa синхронизируются шагами Б1.2–Б1.6; при расхождении действует DESIGN.md». Убирается reviewer'ом на мерже последнего шага. Альтернатива — мержить Блок 1 одним PR — хуже: doc-гейт на 15 файлов = нечитаемый вердикт критика |
| **Р2** | **DESIGN-v2 §7 ссылается на текст, которого в `main` нет** (`docs/06` §2-замер, §5.1) | Б1.1 без Б1.0 | Б1.0 — жёсткое предусловие; порядок в §5.2 |
| **Р3** | **Подпись founder'а прочитана шире, чем дана.** Подписана замена `DESIGN.md`. Фазы Ф0–Ф6, границы A/B/C, переприоритизация `docs/08`, тарифы — отдельные решения (`gates.md` §9 п.4) | Б1.1/Б1.3/Б1.4/Б1.5 | §7 п.2 — запросить три подписи явно ДО мержа соответствующих шагов; вердикт критика обязан проверить наличие |
| **Р4** | **Ссылки ломаются молча.** Замер: **33 файла / 74 вхождения** `DESIGN.md`, из них **6 — в `research/critiques/`** (неизменяемые артефакты под `protected-artifacts`: переписать нельзя). Ни один существующий гейт не проверяет, что ссылка в `docs/**` резолвится | Б1.1/Б1.7 | `scripts/verify_docs_links.sh` — но он нужен **раньше**, чем стоит в плане. **Рекомендация: поднять его в Б1.1 первым коммитом ветки**, чтобы каждый следующий шаг мерился им. Вердикты критиков — в whitelist гейта как исторические (или сохранение имени `DESIGN.md`, §3.0, снимает вопрос целиком) |
| **Р5** | **Блок 2 повторит наш класс дефекта: «заявлено в docs, оракулов ноль».** Мы перепишем `docs/05`, объявим CT-I-7..15 и на этом остановимся — ровно как INTG-I (7 заявлено / 0 оракулов, проверено) и BK-I (8/0, проверено) | Б2.1 без Б2.2 | Б2.2 идёт ВТОРЫМ, до любого промоушена, и его критерий готовности — **`VERDICT: FAIL` на сегодняшнем main**. Плюс CT-I-14 («объявлено ⟹ проверено») делает этот класс дефекта самопроверяемым навсегда |
| **Р6** | **`SCHEMA_VERSION → 5` (Б2.5) на живом проде.** Bump открывает новый сегмент; при дефекте `decide_open_segment`/`retention_plan` (класс R2/R2b — половина истории терялась при зелёном healthcheck) можно повредить восстановление | Б2.5 | Б2.5 идёт в одиночку; оракул обязан включать **прод-масштаб** (`testing.md`: sacred I/O-путь + деградированный вход) и роундтрип реального хвоста; после мержа — §8 eyes-on на VPS, не только «Deploy success». Не запускать до закрытия M-40/M-49-класса работ по энумератору сегментов `[verify-at-impl — проверить, смержены ли]` |
| **Р7** | **Wire-контракт кокпита (Б2.4) ломает уже развёрнутый gateway-serve и фронт founder'а.** `export_schema_version` v8 живёт в проде и, возможно, в `code2alpha` | Б2.4 | Перенос типов **без изменения формы** (чистая передислокация + схема + фикстуры), bump версии — отдельным следующим RFC. Оракул: сериализация v8 до и после переноса байт-идентична |
| **Р8** | **Scope-взрыв.** «Ревизия всего контрактного слоя» естественно тянет Postgres-схему, Node-гейты, unit-economics, prompt-injection (4 пробела DESIGN-v2 §2 роадмапа) | весь Блок 2 | §4.5 — явные границы. Пробелы DESIGN-v2 заводятся как **открытые вопросы с владельцем и фазой**, а не как задачи этого плана |
| **Р9** | **Правило `gates.md` §5 RISK-BLOCK формально требует risk-critic на `crates/contracts`, а его читают как «спит, потому что risk/oms нет».** Легко пропустить обязательный гейт | весь Блок 2 | Б1.7 явно переформулирует §5: «спит на execution-крейтах (их нет); **безусловно действует на `crates/contracts/**`**» |
| **Р10** | **Существующий `M-42-docs-governance-sync` говорит «critic не нужен (docs-only)»** — прямое противоречие doc-гейту §9 класс A (он трогает `docs/05`, `docs/03`, `.claude/rules/testing.md`) | Б1.6/Б1.7 | Правка `M-42` (сама — class A) в рамках Б1.7 либо явное поглощение M-42 шагами Б1.х с закрытием milestone'а |
| **Р11** | **Общий чекаут стоит на устаревшей ветке.** Проверено: `CLAUDE.md` в чекауте совпадает с `main`, но `.claude/rules/gates.md` и `scope-guard.md` — СТАРЫЕ (в чекауте нет §9 doc-гейта; в scope-guard нет крейтов `ops`/`gateway`/`gateway-serve`/`recorder` и `deploy/**` у engine-dev). Агент, стартовавший «как обычно», прочитает правила без doc-гейта и пойдёт пушить документы в `main` сам — ровно то, ради чего §9 и вводился | любой шаг | Каждый paste-ready промпт в Handoff обязан открываться строкой «работай от `origin/main`, свой worktree, `gates.md` §9 действует; правила в общем чекауте УСТАРЕЛИ». Отдельно: `[verify-at-impl]` — стоит ли вернуть общий чекаут на `main` после Б1.0 (решает тот, кто ведёт ветку; `branch-hygiene.md` п.2) |
| **Р12** | **Мастер-документ въезжает в `main` с уже неверными числами.** DESIGN-v2 §12 (покрытие инвариантов) и §0/§11 (125 сегментов, 85 GB свободно, `next_seq` 122 млн, 17 закрытых milestone'ов) — замер **2026-07-29**. Проверено: `VN-I 9/0` уже неверно (M-41 приземлился). Это ровно наш повторяющийся класс — «решение по числу, которое никто не перемерил» (`docs/06` §2, TD-021) | Б1.1 | Перемер — часть критерия готовности Б1.1 (пункт «д»): §12 через `git grep -l "<ID>" -- '*/tests/*'`, §0/§11 — через ssh на VPS (§8 всё равно требуется). Числа, которые лень мерить, помечаются `[verify-at-impl]`, а не переносятся как факт |
| **Р13** | **Нельзя оставить «на потом»:** (а) CT-RFC-05 без документа — каждый следующий RFC наследует дырявый прецедент; (б) `license_class`/`source_kind` — данные пишутся forward-only, задним числом источник не проставить (`contracts/src/lib.rs:78–83`); (в) R7 cap соединений — при первом внешнем пользователе это отказ обслуживания, не деградация | — | (а) → Б2.3, до новых RFC; (б) → Б2.5 в Ф0/Ф1, не позже; (в) → Ф2, зафиксировано как BLOCKER в Б1.4 |

---

## §7. Открытые вопросы (требуют founder-решения ДО или ВНУТРИ соответствующего шага)

1. **Форма замены мастера (§3.0):** содержимое `DESIGN.md` = v2 (рекомендация плана) **или**
   мастером становится файл `DESIGN-v2.md` с переписыванием всех cross-reference'ов? — влияет на
   объём Б1.1/Б1.7.
2. **Объём уже данной подписи:** покрывает ли подпись на замену `DESIGN.md` также
   (а) `docs/09-roadmap-v2.md` (фазы Ф0–Ф6 = приоритеты), (б) расширение границ A/B/C в `docs/03`,
   (в) переприоритизацию `docs/08` + `milestones/BACKLOG.md`? `gates.md` §9 п.4 требует по каждой
   отдельного ★.
3. **Судьба `M-42-docs-governance-sync`:** поглощается шагами Б1.х или живёт отдельно (и тогда
   его строка «critic не нужен» правится)?
4. **К7/К8 — промоушен или понижение:** `ValidationReport`/`TrialRecord`/`RegistryEntry` едут в
   `crates/contracts` или `docs/05` §2 честно понижает их до T2? (Оба варианта закрывают
   расхождение; первый дороже, второй ослабляет заявленный контроль над анти-оверфит-гейтом.)
5. **CT-I-5:** реализуем кросс-языковой паритет в TS-редакции (CT-I-13) или снимаем инвариант как
   несостоявшийся? Сейчас он заявлен и мёртв.
6. Из DESIGN-v2 §2 роадмапа, не решаемое этим планом, но требующее владельца и фазы:
   **prompt/tool injection** (Ф5), **unit-economics до тарифной сетки** (Ф6), **схема Postgres +
   RLS** (Ф3), **Node/TS вне гейт-системы** (Ф3), **JWT HS256 → Ed25519** (Ф2/Ф3),
   **нагрузочный стенд ДО выбора топологии** (Ф2).

---

## §8. Что этот план сознательно НЕ содержит

- Текстов правок (это работа architect'а внутри шагов) — план задаёт ЧТО и КАК гейтится.
- Номеров milestone'ов и C-NNN (свободные — с M-50 и C-040 `[verify-at-impl на момент спеки]`).
- Оценок в днях: единица измерения проекта — milestone (≈5–12 атомарных коммитов), а не время.
</content>
</invoke>

---

## §9. ПРАВКИ ПО ВЕРДИКТУ CRITIC C-040 (architect, 2026-07-31) — BINDING

Critic вынес **REJECT** (`research/critiques/C-040-design-migration-plan.md`). Фактура плана
подтверждена им на 100% выборки; REJECT — за процессные расхождения. Приняты все три находки.

### Ф2 (основание REJECT) — risk-critic пропущен в операционной диаграмме
§5.2 показывала цепочку CT-RFC-работ как `architect(RED+типы) → engine-dev`. Это расходится с
§2 плана и с `gates.md` §5, где **любое касание `crates/contracts/**` требует risk-critic**
(асимметричная цена ошибки: контракт — это то, во что упирается весь датафлоу).

**ИСПРАВЛЕНО:** для ВСЕХ шагов Блока 2, трогающих `crates/contracts/**` (Б2.4–Б2.9), цепочка
читается так:

```
architect (contract-RFC + RED-оракулы + verify)
   → critic (plan-time гейт: gates.md §1, триггер «трогает contracts/**»)
   → engine-dev (реализация)
   → tester
   → risk-critic (ОБЯЗАТЕЛЕН, gates.md §5 — вердикт в research/critiques/)
   → reviewer (PR-гейт §4 + merge + §8)
```

risk-critic НЕ пропускается ни для одного CT-RFC. Исключение возможно только по
`gates.md` §5 MD-only carve-out, который к контрактам НЕ применим (carve-out про
venue-адаптеры без order-egress).

### Ф1 (эскалация) — прекондиция Б2.5 была `[verify-at-impl]`, стала ЖЁСТКОЙ
Б2.5 — единственный шаг плана, бампающий `SCHEMA_VERSION` и открывающий новую эпоху сегмента
на живом проде (122+ млн событий). Он трогает ТОТ ЖЕ код (`segments()` / `decide_open_segment` /
энумератор), где лежал CRITICAL-дефект TD-049.

**ИСПРАВЛЕНО, прекондиция:** Б2.5 НЕ стартует, пока `TD-049` не имеет статус CLOSED в
`TECH-DEBT.md`. Проверка механическая:
```bash
grep -A3 'TD-049' TECH-DEBT.md | grep -qi 'CLOSED' || echo "Б2.5 ЗАБЛОКИРОВАН"
```
Статус на момент правки: M-49 **rev2** прошёл acceptance (`verify_M-49.sh` → PASS, 9/9 оракулов,
регресс 21 блок, две мутации подтвердили load-bearing) и находится на PR-гейте reviewer'а.
После merge и закрытия TD-049 прекондиция снимается автоматически.

### Ф0 — база плана устарела
План фиксировал `origin/main = f930ece`; на момент аудита `main` ушёл на 6 коммитов вперёд
(гигиена долга, статусы, ревизия TD-049), затем ещё дальше (домерж `docs/06-volume-truth` —
замер объёмов, на который опирается решение §7 DESIGN-v2, физически отсутствовал в main).

**ИСПРАВЛЕНО:** исполнитель ЛЮБОГО шага плана обязан начинать с `git fetch origin main` и
сверять факты по актуальному `origin/main`, а не по зафиксированным в §1.1 хешам. Хеши в §1.1
считаются иллюстрацией состояния на 2026-07-31, а не источником истины.

### Статус плана после правок
Правки внесены architect'ом, вердикт C-040 адресован полностью. План готов к исполнению
Блока 1 (замена мастер-документа); Блок 2 стартует пошагово с Б2.1–Б2.4, Б2.5 — только после
закрытия TD-049.
