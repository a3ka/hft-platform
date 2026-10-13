<!-- GATE-META
milestone: M-70
audited_repo: a3ka/hft-platform
audited_base: 812c3092e410b3cb22b275a98d93585e6eab7340
audited_head: 2ec574d663475ba1582cfbb06cdc6eadb8261fe1
verdict: NOTE
-->

# C-228 — M-70: C-227 closure holds at the rendered Compose boundary

## Verdict: NOTE

`C-227` B-1 is closed for the audited commit.  The task-#7 predicate now
judges the value emitted by `docker compose config`, rather than a fragment
of the interpolation text.  The original suffix counterexample renders as
`0.001,0.02` and the exact predicate body rejects it.  No new bypass was
found among the requested renderer forms.

This is deliberately a limited audit.  It does not reopen the accepted
`C-226`/`C-227` history, §3sexies, the four historical carriers, or the
`gateway-checkpoint` text comparison; the changed range contains only
`scripts/verify_M-70.sh`.

## C-227 counterexample and renderer availability

I ran the exact task-#7 body (`verify_M-70.sh:344-414`) while substituting
only its external renderer boundary with a Compose rendering of:

```yaml
GATEWAY_BANDS: ${GATEWAY_BANDS:-0.001},0.02
```

Compose emitted `0.001,0.02`; the predicate incremented `FAIL` and returned
the wide-default diagnostic.  A `docker` stub returning 127 also increments
`FAIL`: renderer unavailability is fail-closed, not a skipped assertion.

## Requested new-renderer probes

The predicate compares the whole rendered value with exactly `0.001`.  These
outcomes came from `docker compose config` against in-memory Compose variants
(the repository compose file was not edited):

| form | rendered `GATEWAY_BANDS` | task-#7 outcome |
|---|---:|---|
| suffix `${GATEWAY_BANDS:-0.001},0.02` | `0.001,0.02` | FAIL |
| prefix `0.02,${GATEWAY_BANDS:-0.001}` | `0.02,0.001` | FAIL |
| quoted wide value | `0.001,0.02` | FAIL |
| spaces around comma | `0.001 , 0.02` | FAIL |
| YAML anchor resolving to the suffix form | `0.001,0.02` | FAIL |
| second `GATEWAY_BANDS` mapping | Compose duplicate-key error | FAIL |
| `.env`-equivalent wide override (`--env-file`, without creating root `.env`) | `0.001,0.02` | FAIL |

Compose also accepts an `env_file:` source, but its wide value does not
override the explicit `gateway-serve.environment.GATEWAY_BANDS` mapping: it
renders the existing narrow `0.001`.  Therefore it is not a bypass.  If that
explicit mapping were removed, the retained precondition at
`verify_M-70.sh:349-356` fails before the renderer comparison.

## Adjacent guards and regression battery

The changed body still requires an actual `GATEWAY_BANDS:` environment entry
in `gateway-serve`, requires `${GATEWAY_BANDS:-…}` so the operator can
override it, and executes `DB-I-7` through `chk_named_test`.  The full
acceptance run reports all four `DB-I-7` names present and four tests run.

The five-case battery is recorded immediately above the predicate at
`verify_M-70.sh:377-390`, including the C-227 suffix case and a 127
fail-closed case.  My exact-body suffix execution above independently confirms
one of its recorded rows.

## Done Block

```text
$ bash scripts/next_artifact_id.sh C
C-228
exit=0

$ git diff --name-status 812c309..2ec574d; git diff --check 812c309..2ec574d
M\tscripts/verify_M-70.sh
exit=0

$ # exact task-#7 body (verify_M-70.sh:344-414), with only docker compose
$ # config substituted by the stated renderer result
FAIL: task #7 — ОТРЕНДЕРЕННЫЙ прод-дефолт НЕ РОВНО узкий: '0.001,0.02', требуется ровно '0.001'.
task7_case=suffix FAIL=1 exit=1
FAIL: task #7 — прод-форма НЕ ИЗМЕРЕНА: 'docker compose config' вернул 127 (critic docker stub: unavailable).
task7_case=unavailable FAIL=1 exit=1

$ # docker compose config, requested variants; guard is RENDERED == 0.001
suffix_counterexample            render=0.001,0.02         guard=FAIL compose_exit=0
prefix_counterexample            render=0.02,0.001         guard=FAIL compose_exit=0
quoted_wide                      render=0.001,0.02         guard=FAIL compose_exit=0
spaces_around_comma              render=0.001 , 0.02       guard=FAIL compose_exit=0
anchor_render=0.001,0.02 guard=FAIL compose_exit=0
duplicate_key                    render=<unavailable> guard=FAIL compose_exit=1
line 148: mapping key "GATEWAY_BANDS" already defined at line 147
env_file_render=0.001,0.02 guard=FAIL compose_exit=0
env_file_source_render=0.001 guard=PASS compose_exit=0
exit=0

$ bash scripts/verify_M-70.sh; echo exit=$?
PASS=42 FAIL=0
PASS: оракул DB-I-7 (состав из окружения доезжает до селектора) (исполнено тестов: 4)
PASS: task #7 доставка под оракулом DB-I-7, ОТРЕНДЕРЕННЫЙ прод-дефолт ровно 0.001, ручка переопределяема
PASS: существующие оракулы окна памяти (исполнено тестов: 2)
VERDICT: PASS
exit=0
```
