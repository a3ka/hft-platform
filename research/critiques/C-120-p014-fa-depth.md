<!-- GATE-META
milestone: P-014
audited_repo: a3ka/hft-platform
audited_base: d61904d71604f75cd1ccbd93ad0cd295ccd2e744
audited_head: bd56b88249dd89255110c22bb3e3f501ba33a376
verdict: PASS
-->

# C-120 — P-014 FA depth: plan-time verdict

## Verdict: PASS

`bd56b88` changes only `docs/fa/viz-backend.md`; on the merge tree with
`origin/main`, it correctly materialises founder signature `П-014` without
expanding it. This is a `gates.md` §9 document-form audit, not an
implementation milestone: no T-contract, trait signature, RED suite, milestone
file, or acceptance script is introduced or required by this one-file FA
correction.

`VB-I-5` remains the live FA invariant: every series deeper than 1.3% carries
`depth_band_provenance`; the change correctly retains it as a truthfulness
requirement rather than treating it as authorisation.

## Findings

1. **Founder scope preserved.** `A-002` §2 permits two unlock paths. `П-014`
   supplies path (b), so the new FA states that only З-1 is lifted; З-2 remains.
   It neither turns `cancel_fraction` into proof of ask-side liveness nor
   permits a label that hides the bid/ask distinction. `П-014` still requires a
   per-side confidence label.
2. **No premature enablement.** The revised FA says that both named `П-014`
   preconditions are open and bars enabling bands before them: provenance is
   presently width-only and silent about side/resync; depth-series remains
   snapshot-only. It consequently does not broaden the founder decision into a
   bypass of those prerequisites.
3. **Coverage and validation are correctly separated.** I agree with the
   `VB-I-5` claim, with its stated limits: a diff book can supply up to 60% in
   its established state while an independent exchange-truth comparator ends at
   roughly 1.3% for spot BTC (and varies by symbol). Absence of that comparator
   limits validation, not the existence of the locally reconstructed source.
   The conclusion does not itself authorise output: `П-014` path (b) does, and
   the two still-open implementation prerequisites constrain that authorisation.
4. **No conflicting active FA carrier found.** Merge-tree grep finds `A-002`,
   З-1, `cancel_fraction`, and `live-verified` in `docs/fa/**` only in this
   document. The old M-32 paragraph is explicitly marked as history, not
   current status. The older `П-017` statement that FA *then* contradicted
   `П-014` is a dated antecedent for this correction, not a third active norm.

## Evidence

| Claim | Merge-tree evidence |
|---|---|
| Source cap and projection cap | `crates/venue-binance/src/lib.rs:27` is `REST_DEPTH_LIMIT = "5000"`; `:33` is `MAX_REL_DIST = 0.60`. |
| Measured source coverage / REST limits | `research/data-quality/depth-probe-binance.md:15-18,62-65`: 50.07–59.51% source reach; spot BTC ≈1.3%, spot ETH ≈4.5% REST cap. |
| M-58 result remains side-specific | `research/data-quality/depth-verdict.md:85-105`: bid 0.713–0.992 across seven bands; ask 0.419, 0.247, 0.403 in the three contradicted bands. |
| Path (a) did not lift the lock | `research/data-quality/depth-verdict.md:64` says `замок A-002 ОСТАЁТСЯ`; `A-002:214-227` defines paths (a) and (b). |
| Current code cannot satisfy the prerequisites | `crates/gateway/src/lib.rs:1035-1036` labels only band width; `:938-941` leaves `depth_series` snapshot-only. |
| Signature limit | `docs/PENDING-SIGNATURE.md:768-788` lifts З-1 through (b), retains З-2, and requires per-side marking plus honest cadence. |

## Done Block

```text
$ bash scripts/next_artifact_id.sh C
C-120
exit=0

$ git merge-tree --write-tree origin/main bd56b88
1cb58bf9aa591e4a402bc5138f49476dc1f6779d
exit=0

$ git log --oneline origin/main..bd56b88
bd56b88 docs(fa): viz-backend §4 — FA противоречила подписи П-014; охват 60% отделён от эталона [architect]
exit=0

$ git diff --check origin/main...bd56b88
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [1-ЕСТЬ] все 3 маркеров [ЕСТЬ] в таблицах статусов сопровождены существующим пруфом
PASS  [3-ССЫЛКИ] все 13 ссылок `DESIGN.md §N` указывают на существующие разделы
PASS  [4-МЁРТВЫЕ-ФАЙЛЫ] все 262 ссылок вида docs/*.md указывают на существующие файлы
PASS  [6-RFC-SHA] SHA-подобных токенов: всего=36 проверено=36 пропущено=0
PASS  [7-RFC-PATH] путей-кандидатов: всего=272 проверено=181 пропущено=91
VERDICT: PASS (0 нарушений)
exit=0

$ git show <merge-tree>:crates/venue-binance/src/lib.rs | nl -ba | sed -n '24,35p'
27  const REST_DEPTH_LIMIT: &str = "5000";
33  const MAX_REL_DIST: f64 = 0.60;

$ git show <merge-tree>:crates/gateway/src/lib.rs | nl -ba | sed -n '934,943p;1031,1039p'
938  // ... depth_series (полосы) НЕ апдейтится — депт-серия остаётся snapshot-only.
1035 depth_band_provenance: (row.band_pct_e8 > 1_300_000)
1036     .then(|| "diff-reconstructed, validated<=1.3%".to_string()),
exit=0

$ git grep -n -E 'A-002|З-1' <merge-tree> -- docs/fa
docs/fa/viz-backend.md:87: ... `A-002` ...
docs/fa/viz-backend.md:98: ... `A-002` З-1 СНЯТ ПОДПИСЬЮ FOUNDER'А ... З-2 ОСТАЁТСЯ.
docs/fa/viz-backend.md:101: ... `A-002` §2 ...
exit=0

$ git status --porcelain
exit=0  # recorded before adding this verdict
```
