# SESSION-HANDOFF — как продолжить в новом контекстном окне

> Читать ПЕРВЫМ при старте новой сессии. Последнее обновление: 2026-07-10.
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
- **hft-core-rs (референс):** клон `/tmp/hft-core-rs-explore` (ephemeral); разведка даталеера в
  `/tmp/hft_dataplane_recon.md` (может исчезнуть после ребута — при нужде реклонить
  `github.com/a3ka/hft-core-rs-`).

## 3. Что реализовано (детали — PROJECT-STATE.md)
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

## 6. СЛЕДУЮЩАЯ ЗАДАЧА (founder дал добро)
**Строить движок бэктеста: `sim` + `research-cli`** (docs/fa/sim.md, research-cli.md, 02-quant-desk.md).
Цель — проверить, предсказывает ли `DIFF 3B-8A` (и какие полосы/горизонты) движение цены. Дисциплина:
пре-регистрация (карточка есть), time-split train/val/test, trials-ledger append-only, пессимистичный
fill, критерии фальсификации (в карточке). Порядок: сначала `sim` (fill-model из journal),
затем `research-cli` (грид/walk-forward/ValidationReport), затем прогон OBI Трек A (top-N imbalance
вычислим уже сейчас) и Трек B (полосы 3%/8% — теперь вычислимы через full-book).
Экономия: делегировать код субагентам (sonnet), Fable — архитектура/sacred/сборка. Дать VPS
накопить несколько часов глубокой книги перед серьёзным бэктестом.

## 7. Дисциплина (напоминание)
Гейт перед «готово»: fmt+clippy(-D warnings)+test зелёные + Done Block. Атомарные коммиты.
sacred: contracts T1, journal (DET-I-1), risk/killswitch, */tests — architect-only. Коммитить только
при зелёном clippy (был инцидент — закоммитил с clippy-ошибкой, CI покраснел; всегда гейтить commit
на clippy=0). Секреты в чат не вставлять. Push только при зелёных гейтах.
