<!-- GATE-META
milestone: M-99
audited_repo: a3ka/hft-platform
audited_base: 2ec2eaec211ba3540284a2a71934d59d0225bc9a
audited_head: a70f92fb2be1fbd0c175c845bbe7be6e509805bb
verdict: NOTE
-->

# C-316 — M-99 restore-drill automation: mergeability after C-314 — NOTE

## Verdict

**NOTE — mergeable.** This is a separate mergeability/document-integrity
audit after `C-314`, not a sixth plan-time repair round for the M-99 artifact
set.  The branch tip is `a70f92fb2be1fbd0c175c845bbe7be6e509805bb`; the
handoff reference `a70f92fb` is its ancestor and remains the audited tip after
a fresh fetch.

`22bc314e` resolves the sole remerge conflict in `docs/DESIGN.md` §22 without
losing either measurement: `VB-I` is **12 / 10** with the M-100
`VB-I-12` oracle, and `OPS-I` is **11 / 10** with M-99's
`OPS-I-3` oracle.  `git show --remerge-diff` names no resolution content beyond
that file.  The subsequent `a70f92fb` is a mechanical, architect-owned
milestone wording change: it makes §4 p.3 and §9 refer to the `image:` of
the `recorder` service, as §4 p.9 already required.  It does not change the
reader, image, or acceptance semantics.

The merge-preview against the freshly fetched `origin/main` at
`815d735a4c3c8b2c4f7ebfc91a30f3abdc41af09` passes with zero violations.
The live FA invariant for this subject is **OPS-I-3**: a cold copy must be
periodically restored and read, rather than merely created.

## NOTE-1 — endpoint diff is not a first-parent artifact proof

The exact requested command

```text
git diff 1643ef6d..HEAD --first-parent -- scripts/ crates/*/tests/
```

prints **38 paths**, all inherited from the merged `origin/main` M-100 tree.
For `git diff A..B`, `--first-parent` does not turn the endpoint tree
comparison into a first-parent commit traversal; it therefore cannot prove
that M-99 altered those paths.  This is advisory rather than a merge blocker:

- the only first-parent commits after `1643ef6d` are `22bc314e` and
  `a70f92fb`;
- `git show --remerge-diff 22bc314e` has one resolution file,
  `docs/DESIGN.md`;
- both `git diff 1643ef6d 22bc314e^1 -- scripts/ crates/*/tests/` and
  `git diff 22bc314e a70f92fb -- scripts/ crates/*/tests/` are empty.

Thus no M-99 artifact was changed by the merge resolution or the C-314
mechanical follow-up.  The literal endpoint command and the claim that it is
empty should not be reused as equivalent evidence in the next handoff.

## Checked scope

1. **Conflict resolution.** `22bc314e` remerge diff contains one conflict in
   the adjacent §22 rows only.  The result retains both required claims and
   does not introduce an out-of-parent change.
2. **Mechanical M-99 prose.** `a70f92fb` changes only
   `milestones/M-99-restore-drill-automation.md` (3 additions, 2 deletions).
   The only remaining `hft-recorder` occurrences are historical/container-name
   explanations at lines 73 and 195; neither is the normative reader-image
   instruction.
3. **Merged-tree claims.** The fresh `verify_design_claims.sh --merge-preview
   origin/main` reports all §22 counts, including `VB-I 12 / 10` and
   `OPS-I 11 / 10`, then `VERDICT: PASS (0 нарушений)`.
4. **RED boundary.** `red_restore_drill.sh` remains intentionally RED in
   exactly seven declared implementation points: `T`, `TF`, `RT`, `RW`,
   `CR`, `CL`, `IM`; its mutation judges `T-M`, `RW-M`, and `CL-M` pass.
5. **`gates.md` §9.**
   - **(a) claims:** checked on the current merge preview, not the stale
     branch-only tree; PASS.
   - **(b) authority:** the resolution is an architect-owned `docs/DESIGN.md`
     change and the follow-up is an architect-owned milestone change.  No
     contract change or unauthorized implementation edit was introduced;
     M-100 code present in the final tree comes from `origin/main`, not from
     the M-99 resolution.
   - **(c) linkage:** §22 now explicitly links M-99's `OPS-I-3` count to
     `red_m99_restore_drill_watch`, while M-99 §4 p.9/§9 agree that the reader
     image is the `recorder` service's `image:` value.

## Done Block

```text
$ git fetch origin && git rev-parse origin/feat/M-99-restore-drill-automation
a70f92fb2be1fbd0c175c845bbe7be6e509805bb
exit=0

$ git rev-parse origin/main && git merge-base origin/main HEAD
815d735a4c3c8b2c4f7ebfc91a30f3abdc41af09
2ec2eaec211ba3540284a2a71934d59d0225bc9a
exit=0

$ git show --remerge-diff --format= 22bc314e
remerge CONFLICT (content): Merge conflict in docs/DESIGN.md
-VB-I | viz-агрегаты | 12 | 9
-OPS-I | ops/алерты | 11 | 10 | ... M-99 ... red_m99_restore_drill_watch
+VB-I | viz-агрегаты | 12 | 10 | ... M-100 ... red_m100_heatmap_frame_close
+OPS-I | ops/алерты | 11 | 10 | ... M-99 ... red_m99_restore_drill_watch
exit=0

$ grep -n 'hft-recorder' milestones/M-99-restore-drill-automation.md
73:| 9 | читатель «из образа работающего `hft-recorder`» | ... `hft-recorder` — имя КОНТЕЙНЕРА ... |
195:| прежняя обёртка (`b070e04d`): шов не зовётся, образ `hft-recorder` | ... |
exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main
PASS  [2-ПОКРЫТИЕ] §22: VB-I — заявлено=12, в оракулах=10 — подтверждено замером (loose=10)
PASS  [2-ПОКРЫТИЕ] §22: OPS-I — заявлено=11, в оракулах=10 — подтверждено замером (loose=10)
PASS  [3-ССЫЛКИ] все 8 ссылок `DESIGN.md §N` указывают на существующие разделы
PASS  [4-МЁРТВЫЕ-ФАЙЛЫ] все 662 ссылок вида docs/*.md указывают на существующие файлы
VERDICT: PASS (0 нарушений)
exit=0

$ git diff --name-only 1643ef6d..HEAD --first-parent -- scripts/ crates/*/tests/ | wc -l
38
exit=0

$ git diff --name-only 1643ef6d 22bc314e^1 -- scripts/ crates/*/tests/
exit=0

$ git diff --name-only 22bc314e a70f92fb -- scripts/ crates/*/tests/
exit=0

$ bash scripts/tests/red_restore_drill.sh
FAIL  T; FAIL  TF; FAIL  RT; FAIL  RW; FAIL  CR; FAIL  CL; FAIL  IM
PASS  T-M (7 mutants); PASS  RW-M (11 command + 10 endpoint mutants); PASS  CL-M (8 mutants)
VERDICT: FAIL (7 из 33)
exit=1  # expected RED before implementation
```

=== HANDOFF: CRITIC → ARCHITECT ===

## §A — Метаданные
- Дата (UTC, ISO-8601): 2026-10-13T21:33Z
- Milestone: M-99-restore-drill-automation
- Статус: DONE
- HEAD: a70f92fb — docs(M-99): §4 п.3 и §9 — образ читателя назван по §4 п.9, не именем контейнера (C-314) [architect]

## §B — Что я сделал
- Audited the post-C-314 mergeability subject only: `22bc314e`, `a70f92fb`, and the current merge preview against `origin/main`.
- Verified the §22 conflict resolution preserves both M-100 and M-99 measurements and that `OPS-I-3` remains explicitly linked.

## §C — Артефакты / результаты
- `research/critiques/C-316-M-99-restore-drill-automation-mergeability.md`
- Done Block: merge-preview exit=0; expected M-99 RED battery exit=1 (7 of 33); endpoint diff NOTE documented above.

## §D — Следующий агент + инвокация
- **Следующий агент:** `architect`
- **Paste-ready промпт:**
  ```
  M-99 mergeability after C-314 received NOTE in C-316 at audited_head a70f92fb2be1fbd0c175c845bbe7be6e509805bb. No sixth M-99 plan repair is authorized or needed: the current merge preview against origin/main passed and DESIGN §22 retains VB-I 12/10 plus OPS-I 11/10. Record the C-316 mechanical appendix in the M-99 milestone if the workflow requires it, then continue the existing M-99 chain. Treat the documented `git diff 1643ef6d..HEAD --first-parent` endpoint result as a Git-diff caveat: its 38 paths are inherited M-100 changes from main; use the two parent-bounded diffs in C-316 to assert that M-99 itself did not change scripts or crate tests after C-314.
  ```
- Push-статус: pending commit and push of C-316 to `origin/feat/M-99-restore-drill-automation`
- Кэш: pending cleanup after push

## §E — Риски / открытые вопросы
- NOTE-1 is advisory only, but future gates must not cite the endpoint diff as evidence of a first-parent-only range.

=== END HANDOFF ===
