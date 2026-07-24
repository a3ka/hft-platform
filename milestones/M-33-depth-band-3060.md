# M-33 — верификация живости полос 30–60% (предусловие включения внешних TPP-полос)

STATUS: **PROPOSED** (2026-07-24, architect). Follow-up #1 из вердикта M-32 (`depth-verdict.md` §5).
Расширяет lifetime-анализатор до полосы `[3000,6000)` bps (30–60%) и переснимает её живость на gap-free
segment 78 — ПРЕДУСЛОВИЕ включения полос 30–60% в TPP-контракт (founder APPROVED 1.5–60% с provenance,
но живость доказана лишь для 1.5–30%). Критик НЕ триггерится (`crates/research-cli/{src,tests}`,
`research/data-quality/` — не contracts/risk/ks/oms/venue, не новый крейт) → reviewer-бэкстоп.

## Мотивация (из вердикта M-32)

Founder одобрил TPP-полосы **1.5–60%** на diff-книге с `depth_band_provenance` (граница C, 2026-07-24).
Но эмпирическая живость Q2 (cancel_fraction/order-flow) была доказана ТОЛЬКО для **1.5–30%** — схема
`BANDS_BPS` анализатора кончалась на `[1500,3000)` bps. Полоса **30–60% ([3000,6000) bps) НЕ измерялась** и
сидит на/за структурным потолком reach книги (p50 54–58%, cap ±60% `MAX_REL_DIST`), где книга разрежена.
Пока не измерена — 30–60% несёт provenance + caveat «beyond-measured-reach» и НЕ выдаётся как живо-верифицированная.

**Этот milestone закрывает пробел:** добавить полосу `[3000,6000)`, переснять `cancel_fraction`/frozen/born на
segment 78 и вынести вердикт: 30–60% ЖИВАЯ (как 1.5–30%) / РАЗРЕЖЕНА-но-живая / ЗАМЁРЗШАЯ-у-потолка (слабая).

## Contract impact (T1) — НЕТ

Расширение `BANDS_BPS` (`pub const` в `crates/research-cli/src/depth_lifetime.rs`) — не T1, не export-схема.
Существующие DV-I-1..8 не ломаются (band_of/агрегация итерируют `BANDS_BPS` обобщённо). CT-RFC не нужен.

## §Tasks (RED-first)

| # | Статус | Задача | Кто | Acceptance |
|---|---|---|---|---|
| 1 | ✅ | **RED DV-I-9** (`crates/research-cli/tests/red_depth_band_3060.rs`) — уровень на 45% (4500 bps) обязан атрибутироваться в ОТДЕЛЬНУЮ полосу `lo_bps=3000` (`[3000,6000)`), не клампиться в `[1500,3000)`; born/cancel учитываются в новой полосе. Sacred. | architect | compile-RED; FAIL против текущего impl (клампит 4500→1500 ⇒ `band(_,3000)=None`), GREEN после расширения `BANDS_BPS` (reachability обе стороны) |
| 2 | ✅ | **impl:** расширить `BANDS_BPS` → добавить `(3000, 6000)` (30–60%); поправить комментарий clamp (`>=6000` → последняя полоса); переснять `depth_lifetime` example на gap-free **segment 78** → дописать в `research/data-quality/depth-lifetime-results.md` per-band 30–60% (born/cancelled/frozen/censored/cancel_fraction, bid+ask) | research-dev | DV-I-9 GREEN; DV-I-1..8 остаются GREEN; прогон на segment 78; числа 30–60% в memo |
| 3 | ✅ | **Вердикт-апдейт:** дописать `research/data-quality/depth-verdict.md` §5 — вердикт по 30–60% (живая/разрежена/замёрзшая по cancel_fraction) → 30–60% включается как live-verified ИЛИ остаётся provenance+caveat. Если cancel_fraction 30–60% ≪ 1.5–30% (замёрзшая у потолка) → флаг founder'у. | architect | вердикт называет 30–60% результат; при аномалии — §D handoff founder |

## §Инвариант (RED-оракул; sacred, architect-only)

| ID | Инвариант | Оракул |
|---|---|---|
| **DV-I-9** | Полоса **30–60% различима**: уровень на 45% от mid (4500 bps) атрибутируется в `lo_bps=3000` (`[3000,6000)`), НЕ клампится в `[1500,3000)`; born/cancelled учитываются в новой полосе. Полоса `[1500,3000)` (20% level) остаётся отдельной. | `red_depth_band_3060.rs::dv_i_9_band_3060_distinct` (+ `::cancel_in_3060`). **Анти-плацебо:** текущий impl (клампит 4500→1500) ⇒ `band(_,3000)=None` → FAIL; после `BANDS_BPS += (3000,6000)` → GREEN |

## §Анти-плацебо чек-лист
- **Границы:** 4500 bps (внутри [3000,6000)); 2999 bps → [1500,3000); 6000 bps → клампит в последнюю (≥6000 за схемой).
- **Различимость:** 20% и 45% уровни в РАЗНЫХ полосах (не сливаются).
- **Живость новой полосы:** явный `size=0` на 45%-уровне → `band(_,3000).cancelled≥1` (не теряется).

## Allowed / Forbidden paths
- **architect (sacred):** `milestones/M-33-depth-band-3060.md`, `crates/research-cli/tests/red_depth_band_3060.rs`, `scripts/verify_M-33.sh`, вердикт-апдейт `research/data-quality/depth-verdict.md`.
- **research-dev (impl):** `crates/research-cli/src/depth_lifetime.rs` (`BANDS_BPS` + comment), `research/data-quality/depth-lifetime-results.md` (числа 30–60%).
- **Forbidden:** contracts (T1), risk/ks/oms/venue/book, export-схема, sacred DV-I-1..9 (кроме architect).

## Acceptance (`scripts/verify_M-33.sh`)
CI-точно (RN-17/TD-035): `cargo fmt --all -- --check` + `cargo clippy -p research-cli --all-targets --all-features -- -D warnings`.
- DV-I-9 GREEN (`--test red_depth_band_3060`);
- DV-I-1..8 РЕГРЕСС-GREEN (`--test red_depth_lifetime --test red_orderflow_faith --test red_depth_scale --release`);
- memo grep: `depth-lifetime-results.md` содержит полосу `30`–`60` / `3000` (числа сняты);
- финал `VERDICT: PASS`/`FAIL`, exit соответствует.

## Данные
gap-free **segment 78** (тот же авторитетный эталон, что M-32; `gaps=0`). VPS: `root@167.233.192.131`,
ключ `/home/nous/.ssh/hft_deploy`, журнал `/var/lib/docker/volumes/hft-platform_journal-data/_data/`.

## Handoff
Task 1 (RED) — architect ПЕРЕД impl. Task 2 — research-dev. Task 3 (вердикт) — architect; founder-флаг ТОЛЬКО
если 30–60% аномальна (замёрзшая) — иначе решение founder'а (1.5–60% с provenance) уже покрывает.
