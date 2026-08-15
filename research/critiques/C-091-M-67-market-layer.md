<!-- GATE-META
milestone: M-67
audited_repo: a3ka/hft-platform
audited_base: 1e5b85b2b5f7692982e3078e10e00d472931d256
audited_head: b7012ee92aa0a24edee43ce46140381dc34578b7
verdict: REJECT
-->

# C-091 — M-67 market layer: plan-time critique

**Verdict: REJECT — NOT REVIEWED — ARCHITECT ARTIFACTS INCOMPLETE.**

Audited subject: `origin/docs/M-67-market-layer` at
`b7012ee92aa0a24edee43ce46140381dc34578b7`; stakes: **high**. This is an audit of
the committed artifact set and the construction it specifies, not an implementation review.

## Gate precondition

The committed range contains exactly one file: `milestones/M-67-market-layer.md`.
It does not contain the required T-contract / trait-signature decision, any failing
RED tests, or `scripts/verify_M-67.sh`. `docs/04-workflow.md` §2 requires all of
them to be committed before the critic gate. In particular, task 1 postpones the
central `DepthAggregate` / `MarketTicker` boundary decision until after this gate;
that reverses the required order.

## Blocking findings

### F-1 — the band gate reads historical FA text as current authority

`milestones/M-67-market-layer.md` §6.1 says that 1.5–30% is live-proven and only
30–60% is blocked. The current, normative text immediately above the cited historical
paragraph says the opposite: `docs/fa/viz-backend.md:92-95` marks **1.5–60%** as
`verification pending`, locks all bands deeper than 1.3% out of production output,
and names M-58 as the automatic release condition. Lines 97–104 are explicitly kept
as history, not the active status.

M-67 nevertheless defines seven bands in §4.3, asks task 6 to compute all seven,
and gates only the two farthest bands. This would collect and emit data that the active
FA lock forbids. It is a blocker, not a provenance note.

### F-2 — L1 permanence and a 48-hour L2 window have no compatible storage construction

M-67 promises permanent L1 alongside L2 raw deltas pruned at 48 hours, but does not
specify a distinct journal / segment family / durable layer identity. The existing
retention implementation selects whole segments only: `RetentionPolicy` exposes age,
minimum segment count, and checkpoint coverage (`crates/journal/src/segments.rs:3375-3393`),
and `retention_plan` classifies candidates by segment age and index
(`:3468-3557`). It cannot delete L2 events while retaining L1 events interleaved in
the same segment.

The milestone neither permits nor specifies the missing storage boundary and routing;
task 9 merely requests automatic retention. Therefore its 48-hour capacity claim and
its perpetual-L1 promise cannot both be implemented from this contract.

### F-3 — TOTAL cannot be reconstructed for an arbitrary historical instant

After 48 hours, raw L2 is gone. L3 retains only per-minute `min/max/avg/last`.
Those four extrema do not identify the order-book depth at, for example, 12:34:17,
nor does M-67 define a minute-resolution API, the meaning/weighting of `avg`, a
watermark/finalisation rule, or an honest `history_truncated` response. Thus §4.5's
claim that TOTAL/TOTAL1/TOTAL2/TOTAL3/OTHERS is a replay projection at an arbitrary
past time is false as written.

There is a second unclosed input: raw deltas need a replayable seed/resync state. The
milestone describes recording all deltas, but supplies no durable raw snapshot/anchor
contract for each symbol and no test for a replay beginning at the 48-hour boundary.
The current all-symbol snapshot is described by M-67 §1 as 2-bps bucketed, so it is
not automatically an exact seed for raw-level deltas.

Finally, task 8's dated group registry has no declared durable location, effective-time
lookup rule, content/version identity in the projection, or historical oracle. A mutable
"top-10" registry makes OTHERS drift on replay. These are required inputs to the
claimed projection, not implementation details.

### F-4 — `live == replay` is stated, but the planned oracle does not prove journal-first

The existing safe gateway path is available: `LiveReducer::pump` calls
`journal::stream_from_at_with_catalog` before producing frames
(`crates/gateway/src/lib.rs:3093-3167`). M-67 does not bind task 11 to that path.
`MD-I-6` only compares a stream value with a replay value. A direct in-memory stream
and a later (or failed) journal append can produce equal bytes and pass this test.

The RED suite must make a value unavailable to the stream when its append is withheld
or fails, and prove the stream's selected source is the persisted event/sequence—not
just equal output from two reducers. Without that negative path, a value can reach
the stream without ever having been journaled, violating DESIGN §1.

### F-5 — the storage evidence and multiplier do not support procurement

The claimed `6.8x` is reproducible only by comparing the entire production journal
with **one BTC spot** wire stream: current journal rate is 17.848 KB/s and
17.848 / 2.6 = 6.864. Production contains BTC and ETH over three venues, so this is
not a like-for-like overhead factor and cannot be used to project the new stream set.

The live re-measurement differs from §1.1 as well: 47.803 decimal GB / 291 segments,
not 46.7 GB / 290. Compression itself is consistent (282 closed 1-GiB source segments
over 38,852,716,100 stored bytes = 7.793x), but the cited claim has no committed
measurement artifact with a timestamp, command, symbol set, byte definition, and
sample duration.

The table addition is arithmetically 2.264 TB/year for permanent layers (2.277 TB
including the 13-GB hot window), but the critical L1 1.9-TB term is not derived by
§1.1. Its own figures give a lower-bound 4.355 GB/day for top-50 spot depth from the
"33% outside top-50" statement; adding only the stated BTC futures sample already
reaches 1.8608 TB/year. That leaves 39.2 GB/year for the other 49 futures streams
and all top-50 trades. No such budget can be accepted, especially when the milestone
also says BTC futures are 3.3x spot. Re-measure a named, representative spot+futures
top-50 set and derive each permanent/hot term before Storage Box sizing.

## RED-oracle anti-placebo audit

No RED test is committed, so this is an assessment of the oracle descriptions only.

| Oracle | Falls against a no-op/stub as specified? | Gap |
|---|---|---|
| MD-I-1 unknown symbol rejects | **No** | A reject-everything stub passes; it needs a known allowed symbol positive case on the real ingress path. |
| MD-I-2 48-hour boundary | **Yes, conditionally** | A no-op retention leaves the old segment and fails, but it still does not protect L1 and L2 co-residence in a segment. |
| MD-I-3 intra-minute minimum | **Not demonstrated** | The description does not require positive values nor assert all `min/max/avg/last`; a zero/default aggregate can satisfy a zero-dip-only check. |
| MD-I-4 mid rather than best bid | **Yes, conditionally** | A wide-spread fixture with an exact, non-zero expected notional rejects the wrong anchor and a default result. |
| MD-I-5 TOTAL plus registry version | **Not as a complete oracle** | A positive three-coin sum can reject a zero stub, but no valid-version positive path or historical effective-time lookup is specified. |
| MD-I-6 live equals replay | **No** | Equality does not pin persistence order or the journal read path (F-4). |
| MD-I-7 suppress far bands without artifact | **No** | A permanently-disabled implementation passes the negative condition; it needs a verified-artifact positive case and a semantic validity check, not merely a non-empty file. |

Each conditional row also needs a setup guard and a recorded mutation result; neither
can be evaluated until the actual RED suite exists.

## Required resubmission conditions

1. Commit the complete plan-time set: resolved contract/T-designate signatures and
   storage identity, failing RED tests, and a real `verify_M-67.sh` before re-invoking
   critic.
2. Align the band set and its fail-closed gate with the active FA lock; do not use
   the historical M-32 paragraph as authorization.
3. State exactly what is replayable at every query time, including raw-seed, group
   version/effective time, minute semantics, retention boundary, and honest degraded
   response.
4. Supply reproducible measurement artifacts and a spot+futures capacity derivation
   whose numerator and denominator cover the same streams.
5. Make every MD-I oracle fail against a named broken implementation, including
   journal-write failure and the positive sides of MD-I-1 and MD-I-7.

## Done Block

```text
$ bash scripts/next_artifact_id.sh C
C-091
exit=0

$ git diff --name-status b7012ee^ b7012ee
A	milestones/M-67-market-layer.md
exit=0

$ git cat-file -e b7012ee:milestones/M-67-market-layer.md
exit=0

$ git grep -n -E 'DepthAggregate|MarketTicker|MD-I-[1-7]' b7012ee -- crates scripts contracts
grep_exit=1

$ git ls-tree -r --name-only b7012ee -- scripts | rg 'verify_M-67\\.sh'
verify_path_exit=1

$ ssh -i /home/nous/.ssh/hft_deploy -o IdentitiesOnly=yes root@167.233.192.131 'du -sb /var/lib/docker/volumes/hft-platform_journal-data/_data; find /var/lib/docker/volumes/hft-platform_journal-data/_data -maxdepth 1 -type f -name "segment-*" -printf "%f %s\\n" | awk '\''{n++; total+=$2; if ($1 ~ /\\.zst$/) {z_n++; z+=$2} else {r_n++; r+=$2}} END {printf "segments=%d compressed=%d compressed_bytes=%d avg_compressed_bytes=%.0f raw=%d raw_bytes=%d total_segment_bytes=%d\\n", n,z_n,z,z/z_n,r_n,r,total}'\'''
47802867962	/var/lib/docker/volumes/hft-platform_journal-data/_data
segments=291 compressed=282 compressed_bytes=38852716100 avg_compressed_bytes=137775589 raw=9 raw_bytes=8950151518 total_segment_bytes=47802867618
exit=0

$ python3 - <<'PY'  # direct WebSocket collector; nominal 15-second sample, elapsed includes connection and final receive
spot_btc_depth	elapsed_s=21.58	messages=141	bytes=41608	bytes_per_s=1928.0	KiB_per_s=1.883
spot_btc_aggtrade	elapsed_s=24.53	messages=18	bytes=3013	bytes_per_s=122.8	KiB_per_s=0.120
spot_eth_depth	elapsed_s=21.68	messages=140	bytes=24079	bytes_per_s=1110.8	KiB_per_s=1.085
spot_eth_aggtrade	elapsed_s=22.15	messages=5	bytes=833	bytes_per_s=37.6	KiB_per_s=0.037
spot_cow_depth	elapsed_s=24.58	messages=61	bytes=12552	bytes_per_s=510.6	KiB_per_s=0.499
spot_cow_aggtrade	elapsed_s=22.04	messages=2	bytes=320	bytes_per_s=14.5	KiB_per_s=0.014
futures_btc_depth	elapsed_s=24.94	messages=138	bytes=143189	bytes_per_s=5741.4	KiB_per_s=5.607
futures_btc_aggtrade	elapsed_s=24.94	messages=0	bytes=0	bytes_per_s=0.0	KiB_per_s=0.000
spot_all_miniticker	elapsed_s=22.58	messages=14	bytes=96512	bytes_per_s=4273.5	KiB_per_s=4.173
exit=0

$ awk 'BEGIN { ... M-67 storage arithmetic ... }'
journal_decimal_GB=47.803
journal_GiB=44.520
journal_daily_decimal_GB=1.542028
journal_equiv_KB_per_s=17.848
overhead_vs_claimed_BTC_spot=6.864x
L2_full_market_decimal_TB_per_year=2.3725
top50_spot_lower_GB_per_day_from_33pct_tail=4.355
BTC_futures_GB_per_day_from_8.6KBps=0.743040
top50_spot_plus_BTC_futures_TB_per_year=1.8608
table_permanent_TB_per_year=2.2640
table_plus_hot_TB=2.2770
exit=0
```
