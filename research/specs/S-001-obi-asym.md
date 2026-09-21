# S-001-obi-asym — SignalSpec (OBI, параметрическое семейство)

STATUS: candidate (не зарегистрирован в реестре; регистрация — Граница B после валидации).
Hypothesis-предок: `research/hypotheses/H-20260710-obi-asym.md` (пре-регистрация).
Модуль: `crates/signals/src/obi.rs`. Автор формализации: architect (Fable), 2026-07-10 (M-04).

## Формула

`imbalance = depth_bid / (depth_bid + depth_ask)` → направленный score
`value = 2·(imbalance − 0.5) ×1e8 ∈ [−1e8, +1e8]` (M-04 D1). Эмиссия ТОЛЬКО при
`|value| ≥ theta_e8`, иначе `None` («нет мнения»). Время — только из `Event`.

## Params-схема (crates/signals/src/obi.rs::ObiParams)

```json
{
  "mode": "top_n",            // "top_n" {n_levels} | "bands" {d_bid_pct, d_ask_pct}
  "n_levels": 5,
  "theta_e8": 20000000,       // порог на |score| (0.2)
  "horizon_ms": 1000,         // метаданные: горизонт выхода для harness (D2)
  "venue": "Binance",
  "symbol": "BTCUSDT"
}
```

## Гриды (M-04 Трек A + Трек B)

- Трек A (top_n): `n_levels ∈ {1, 5, 10, 20}` × `theta ∈ {0.1, 0.2, 0.3, 0.4}` ×
  `horizon_ms ∈ {500, 1000, 2000, 5000}` — вычислим на всех записанных данных.
- Трек B (bands): `d_bid_pct, d_ask_pct ∈ {0.005, 0.01, 0.02, 0.03, 0.05, 0.08}`
  (включая асимметричные пары; частный случай founder'а 3%/8%) × те же theta/horizon —
  только на full-book сегментах Binance (пишутся с 2026-07-10; TD-004).
- Издержки: артефакты `research/fees/*.json`; стресс ×1.5-cost / ×2-latency —
  отдельные прогоны (RC-I-10).
- Walk-forward (D6): train 4h / test 1h / step 1h.

## Критерии фальсификации

Пре-регистрированы в H-карточке (раздел «Пре-регистрированные критерии фальсификации») —
дублирование запрещено, источник один.

## История

| Дата | Изменение | Автор |
|---|---|---|
| 2026-07-10 | v1 (M-04 task 7) | architect |
