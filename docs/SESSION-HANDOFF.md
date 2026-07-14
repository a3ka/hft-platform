# SESSION-HANDOFF — как продолжить в новом контекстном окне

> Читать ПЕРВЫМ при старте новой сессии. Последнее обновление: 2026-07-11.
> Порядок чтения новой сессии: **этот файл → `CLAUDE.md` → `PROJECT-STATE.md` →
> `TECH-DEBT.md` → `docs/DESIGN.md` → релевантный `docs/fa/*` → `research/hypotheses/*`.**
> Почти весь контекст уже в репо — новая сессия самодостаточна.

## 0. Как резюмировать (founder → новая сессия)
Открой новую Claude-сессию, рабочий каталог `/home/nous/hft-platform`, скажи:
«Прочитай docs/SESSION-HANDOFF.md, CLAUDE.md, PROJECT-STATE.md, TECH-DEBT.md и продолжи с
раздела "Следующая задача". Ты — architect (Fable), работаем по .claude/ и docs/DESIGN.md.»

## 1. Что это за проект
Систематическая крипто-mid-freq торговая платформа (ДНК топ-фирм): journal-first, детерминизм,
fail-closed риск, LLM только на дизайн-тайме. Полная архитектура — `docs/DESIGN.md` (+ 00–06,
fa/*). Процесс постройки (EINHARD-модель) — `CLAUDE.md` + `.claude/{rules,agents}`.
Founder = a3ka. Ярус: crypto mid-freq, **Hyperliquid + Binance**. Стартовый живой капитал $500–2k.

## 2. Доступы / инфраструктура (ВСЁ РАБОТАЕТ)
- **Репо:** `github.com/a3ka/hft-platform` (private, ветка `main`). `gh` авторизован как a3ka.
- **VPS:** Hetzner Cloud cpx32, `167.233.192.131`, Ubuntu 26.04, Docker+Rust. Recorder крутится
  24/7 в контейнере `hft-recorder` (persistent Docker volume `hft-platform_journal-data`).
- **SSH на VPS:** `ssh -i /home/nous/.ssh/hft_deploy -o IdentitiesOnly=yes root@167.233.192.131`
  (мой ключ, приватная часть в песочнице; VPS deploy-key `github_deploy` — read-only clone репо).
- **CI/CD:** push в `main` → GitHub Actions (`ci.yml` fmt+clippy+test; `deploy.yml` build-on-VPS:
  SSH → git pull → `docker compose up --build` → healthcheck → rollback). Работает.
- **Локальная разработка:** `/home/nous/hft-platform`. Сборка: `cargo build --workspace`;
  гейт: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --workspace`.
- **pi-агенты (дешёвые dev-роли, MiniMax):** команды `pi-engine-dev` / `pi-venue-dev` /
  `pi-research-dev` / `pi-signal-engineer` / `pi-hft-tester` (в `~/.local/bin`;
  обвязка `.claude/wrappers/pi-dev.sh` + README там же). Лаунчер сам делает fresh
  worktree + идентичность роли + инжектит dispatch-mandate (sacred/TDD/Done Block/не-пушь);
  §D-промт архитектора вставляется как есть (`-p "<промт>"` или в TUI). Незапушенная
  работа dev'а остаётся в его worktree — путь печатается при выходе; дальше
  `HFT_WORKTREE=<путь> pi-hft-tester`, merge в main — reviewer.
- **hft-core-rs (референс):** клон `/tmp/hft-core-rs-explore` (ephemeral); разведка даталеера в
  `/tmp/hft_dataplane_recon.md` (может исчезнуть после ребута — при нужде реклонить
  `github.com/a3ka/hft-core-rs-`).

## 3. Что реализовано (детали — PROJECT-STATE.md)
- **M-04 Research core СМЁРЖЕН (2026-07-11, reviewer APPROVED ×2):** движок бэктеста
  ГОТОВ. Крейты: `sim` (честный fill-model: пессимистичная очередь tail-no-cancel-credit,
  латентность ТОЛЬКО из измеренных артефактов, fees fail-closed, BacktestExchange,
  divergence P4-gate, SplitMix64), `signals` (trait Signal, Граница A; OBI TopN+Bands;
  SignalBank c изоляцией паник; registry c code_hash-сверкой), `research-cli`
  (grid/walk-forward/trials-ledger append-only+hash-chain/deflated Sharpe BLdP/
  детерминированные отчёты/CLI grid|validate|report + бинарь latency_probe), `book` +=
  top_n_depth/levels/size_at. Артефакты честности: research/latency/*.json (δ_md из
  журнала; δ_submit/cancel = RTT×2 ПРОКСИ до P1) + research/fees/*.json (с provenance).
  Milestone: milestones/M-04-research-core.md (решения D1-D11); критик-вердикты
  research/critiques/C-001/C-002. Задачи 1-7 ✅; **задача 8 (прогон OBI) ОТКРЫТА**.
- **Пилот прогнан end-to-end на боевом журнале** (663718 событий): конвейер работает;
  вскрыт и починен дефект harness v1 (несбалансированные qty входа/выхода → фиктивный
  PnL; после фикса PnL/цикл ≈ издержки). trials-ledger: первые 4+ записи семейства obi.
- **Процессный слой:** CLAUDE.md + .claude/rules(5) + .claude/agents(9) + PROJECT-STATE + TECH-DEBT.
- **Даталеер РАБОТАЕТ в проде:** crates `contracts` (T1 Event/MdEvent, fixed-point i64×1e8),
  `journal` (append-only, seq персистится, read_all), `venue-binance` (**full-book diff-sync**:
  @trade + @depth@100ms + REST snapshot sync, эмит bucketed книги ±60% раз в 1с), `venue-hyperliquid`
  (WS trades+l2Book, тонкий — 20 уровней), `recorder` (supervisor→journal), `book` (L2 + microprice +
  `depth_within`/`notional_within` + `Books`).
- **Проверено:** Binance BTC книга достаёт ~48% глубины; полосы 1.5–60% дифференцируются; сигнал
  founder'а **DIFF 3B-8A вычислим**. HL остаётся 0.03% (фид тонкий).
- **Диагностика-примеры:** `cargo run --example dump -p journal -- <dir>` (разбивка по площадкам);
  `cargo run --example bands -p book -- <dir>` (полосы BID/ASK $ + DIFF, сверка с платформой).
  Проверить боевые данные: scp сегмент с VPS
  `root@167.233.192.131:/var/lib/docker/volumes/hft-platform_journal-data/_data/segment-00000000.jrnl`
  в локальный `<dir>/`, затем `bands`.

## 4. Founder-сигнал (OBI) — статус
Карточка `research/hypotheses/H-20260710-obi-asym.md`. Сигнал = кумулятивный объём лимиток
BID/ASK в ценовых полосах 1.5/3/5/8/15/30/60% + DIFF (напр. 3B−8A). Референс — платформа
"Trading Platform Pro", индикатор BID/ASK (аргументы: Exchange SPOT/FUTURES; Coin type
Coin/TOTAL/…). Полосы вычислимы ТОЛЬКО через full-book (сделано для Binance).

## 5. ОТКРЫТЫЕ ВОПРОСЫ (важно)
1. **🔴 Магнитудная загадка (НЕ решена).** Founder уточнил: на скрине был **Coin=BTC** (не TOTAL).
   Платформа: BTC ASK-3 = **52 005 M ($52 млрд)**. Наш расчёт по live-книге Binance: BTC ASK-3 ≈
   **$20M**. Разница ~2500× при одной монете необъяснима «полнотой книги» ($52 млрд лимиток в 3%
   для BTC-спота невозможно физически). Гипотезы для расследования: единицы («M» не USD? объём в
   монетах×цена иначе?); платформа берёт FUTURES (fstream, глубже); аккумулирует по времени; иной
   источник полной книги; баг платформы. **Расследовать в новом окне** (не критично для бэктеста —
   динамика DIFF важнее абсолюта, но надо понять для валидации).
2. **SPOT vs FUTURES** — founder ещё не ответил, что ему нужно (спот сделан; фьючерсы =
   `wss://fstream.binance.com`, отдельный адаптер/режим).
3. HL глубину >20 уровней получить не удалось (проверить nSigFigs / иной эндпоинт) — TD-005.
4. **`ts_exch_ms=0` у Binance L2Snapshot в журнале** (находка задачи 5 M-04): парсер
   venue-binance не заполняет биржевой ts у снапшотов → они исключены из δ_md-эмпирики
   (~63k событий/символ). Маленький фикс в venue-binance (+ risk-блок: venue-* → reviewer
   обязателен). Не блокирует бэктест.
5. δ_submit/δ_cancel в latency-артефактах — RTT-прокси ×2 (реальный order-path не измерен
   до P3/testnet); reviewer RN-2: обязательный фокус risk-critic на отчёте R-001.

## 5.5. ГОТОВНОСТЬ (снимок 2026-07-14) — детали в `milestones/BACKLOG.md`

| Готово | Не готово (по приоритету) |
|---|---|
| journal (ротация/эпохи/стрим/ретеншен), contracts T1 v2 (provenance), venue-* MD, recorder 24/7, book, signals+OBI, alpha/portfolio/strategy, sim, research-cli | **ops: бэкапа/метрик/алертов/recon НЕТ** (15 GB данных в ОДНОЙ копии) · **отчётов R-* — ноль** · **risk/killswitch/oms/runner — нет** · подпись Ed25519 (граница C) — нет · HL-глубина ≤20 ур. (TD-005) · полный DET-I-1 (TD-007) |

Фаза: P0 ✅ · P1 ✅ · **P2 ≈80%** (research-движок готов, отчёта нет) · **P2.5 (data safety net) — новая, следующая** · P3+ 0%.

## 6. СЛЕДУЮЩАЯ ЗАДАЧА

> **Обновлено 2026-07-13: M-07 «Strategy brain» ЗАКРЫТ** (reviewer APPROVED, merge `5141fd9`,
> CI+Deploy success, §8 eyes-on GREEN — recorder инертен; `verify_M-07.sh` 21/21 exit=0).
> Появились крейты `alpha` (ансамбль сигналов → `Forecast`), `portfolio` (sizing → `TargetPosition`,
> **pre-risk sanity, НЕ риск-гейт**), `strategy` (`DirectionalStrategy` → `OrderIntent`, владелец
> формы) + `sim::StrategyBacktest`. Ad-hoc harness грида удалён: бэктест теперь гоняет ТОТ ЖЕ код
> решений, что и будущий live (DESIGN §1, равенство 2).
>
> **Развилка для founder'а — две готовые к запуску ветки:**
> 1. **M-08 (risk + killswitch + oms)** — fail-closed `RiskApproved<Order>` МЕЖДУ `strategy` и
>    `oms`; RISK-BLOCK (`gates.md` §5): critic + **risk-critic** обязательны, `RK-I-1..10` +
>    `INTG-I-*` RED-suite. Это путь к testnet-торговле (P3).
> 2. **M-04 задача 8 — формальный прогон OBI → `research/reports/R-001`** (TD-009): теперь ОБЯЗАН
>    идти на strategy-пайплайне M-07 и соблюдать **TD-015** (эпохи trials-ledger несопоставимы:
>    в метрики/deflated-Sharpe — только записи кода `>= 5141fd9`; пре-M-07 записи мерили удалённый
>    harness). Правило закреплено в `.claude/rules/gates.md` §6.3/§6.4 + амендмент в
>    `milestones/M-04-research-core.md`. Гейт: risk-critic + подпись founder ★.
>
> **Процессные находки reviewer'а (вне M-07, ждут решения founder'а):**
> - **Deploy НЕ гейтится на CI** (`.github/workflows`): «Deploy to VPS» зеленеет, пока CI ещё идёт →
>   красный CI не остановит прод. Кандидат на `needs: ci` — мелкая правка, высокий эффект.
> - **Дрейф памяти recorder:** контейнер с ~5ч аптайма показывал MEM 48 MiB против 5–9 MiB
>   исторически. Одна точка, предшествует M-07, лик НЕ доказан — но это класс тихой деградации,
>   которую healthcheck маскирует (урок TD-011). Нужен наблюдательный оракул/замер, не молчание.

### (архив) Предыдущая формулировка
> Обновлено 2026-07-13: **M-05 (journal integrity) и M-06 (data expansion: futures depth/OI/
> liquidations/funding + funding-breadth) ЗАКРЫТЫ** (reviewer-approved, §8 live-green). TD-011
> (journal OOM) и TD-014 (futures liveness) CLOSED. Ниже — след. задача, теперь РАЗБЛОКИРОВАННАЯ.

**M-04 задача 8 — формальный прогон OBI** (единственная открытая; всё для неё готово):
1. Дать VPS накопить **3-7 дней** полной книги (пишется с 2026-07-10; на 2026-07-11 было
   ~164MB/663k событий ≈ часы — этого мало, только шум).
2. scp сегмент → `tmp/journal-vps/` (см. §3 диагностику), затем через research-cli:
   грид по `research/specs/S-001-obi-asym.md` (Трек A top_n + Трек B bands на Binance),
   time-split через `split::SplitState` (test — ОДИН раз, за ValGateToken), walk-forward,
   стресс ×1.5-cost/×2-latency, отчёт `research/reports/R-001*` (metrics.json детерминирован).
   Пилотная обвязка-пример: tmp/pilot/grid-S-001-pilot-trackA.json (ledger для формального
   прогона — ГЛОБАЛЬНЫЙ research/trials-ledger.jsonl).
3. risk-critic (сильная модель) вердикт по R-001 (фокус: RTT-прокси латентности, RN-2) →
   founder ★ принять/убить по пре-рег. критериям H-карточки.
Параллельно можно: фикс ts_exch_ms (§5 п.4); расследование магнитудной загадки (§5 п.1).
Экономия: код — субагенты (sonnet), Fable — архитектура/sacred/вердикты.

## 7. Дисциплина (напоминание)
Гейт перед «готово»: fmt+clippy(-D warnings)+test зелёные + Done Block. Атомарные коммиты.
**После КАЖДОГО push в main — post-merge деплой-гейт (`.claude/rules/gates.md` §8):
дождаться CI+Deploy success И проверить VPS по ssh (контейнер healthy, heartbeat свежий);
milestone не закрывается поверх красного/непроверенного прода.**
**Перед push — push-scope проверка (`gates.md` §8): в `git log origin/main..HEAD` только
СВОИ коммиты (чекаут общий — чужие незапушенные RED-коммиты уезжают с твоим push;
инцидент 2026-07-11). RED-оракулы до реализации — локально или на feat-ветке
(`pi-<role> --branch feat/M-NN`), main всегда зелёный.**
sacred: contracts T1, journal (DET-I-1), risk/killswitch, */tests — architect-only. Коммитить только
при зелёном clippy (был инцидент — закоммитил с clippy-ошибкой, CI покраснел; всегда гейтить commit
на clippy=0). Секреты в чат не вставлять. Push только при зелёных гейтах.
