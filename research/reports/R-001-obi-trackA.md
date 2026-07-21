# H-20260710-obi-asym — S-001-obi-asym

- report_schema_version: 2
- journal_sha256: da8695a190a177875589b7f1fbdd9a2008ef662190bba02a16ded7bd5cb795d1
- code_hash: f97a6e66ead318b53db4545b3133fad62216205c380787fc53cdaf10d238d8aa
- ledger_n (счётчик семейства): 276
- net_pnl_e8: -4457526829335335
- sharpe: -2.554496
- se_sharpe: 0.272493
- data_span_days: 0.353343
- verdict: Kill("oos_sharpe=-2.554496 <= 0.5 (пре-рег критерий Net Sharpe)")
- gap_ref: research/data-quality/gaps-own-2026-07.json
- ledger_cutoff: 5141fd9
- deflated_sharpe: 0.000000
- max_drawdown_e8: 4457522955243432
- fill_rate: 1.990176
- turnover_e8: 1015888876146265013
- capacity_notional_e8: 50794443807313248 (v1-participation)

## Decay (horizon_ms, sharpe)
- 500ms: -6.302939
- 1000ms: -5.246083
- 2000ms: -2.554496
- 5000ms: -4.486221

## Stress
- CostX15: sharpe=-2.575146 net_pnl_e8=-4965471267407640
- LatencyX2: sharpe=-6.748303 net_pnl_e8=-1805553897452487909

## Walk-forward Sharpes
- -10.410922
- -1.884132
- -4.654862
- -1.609901
- -2.539412
- -13.038902
- -4.380739
- -11.039556
- -1.334530
- -1.497054
- -2.047692
- -1.612459
- -12.239855
- -11.302815
- -14.074721
- -13.776528
- -14.324765
- -12.981279
- -15.545292
- -7.105624
- -4.431581
