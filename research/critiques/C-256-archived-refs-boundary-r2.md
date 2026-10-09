<!-- GATE-META
milestone: R-203
audited_repo: a3ka/hft-platform
audited_base: 29b0a93a17df5d499e607be05d9a51afbea4eb8e
audited_head: 7c282c3498fd9f8b4e4e0babf7c14df26ae90ad9
verdict: NOTE
-->

# C-256 — archived-refs boundary, round 2

## Verdict

**NOTE.** The two blocking C-255 findings are closed.  The selector and ID
parse are one `BASH_REMATCH` expression; every selected current archive name
therefore has an ID.  The probe rejects both removal of scenario 4's commit
and every tested substitution of a different fail-closed cause.  The real-tree
PR-form barrier passes, as do all six adversarial mutations.

There is no merge blocker in this round.

## C-255 closure and findings

### C-255 B-1 closed — selection cannot precede ID parsing

- **Where:** `scripts/check_archived_refs.sh:91-94`.
- The two only selection arms capture `M-[0-9]+[a-z]?` and immediately append
  `BASH_REMATCH[1]` as the old-path ID.  On the actual top-level archive, the
  independent enumerator selected 62 names (31 specs, 31 gates) and found
  `selected_without_id=0`.
- Broadening either grammar to admit a non-numeric ID (`M-[^-]*` for specs,
  `M-[^.-]*` for gates) makes scenario 14 fail: it observes two artifacts,
  rather than the required one.  The old, unreachable standalone `id` guard
  was correctly removed instead of being advertised as protection.

### C-255 B-2 closed — setup and named-cause checks are observable

- **Where:** `scripts/tests/red_archived_refs.sh:47-67`, scenario 4 at
  `scripts/tests/red_archived_refs.sh:103-112`.
- Deleting only scenario 4's `commit_all "$R3"` produces the explicit
  `SETUP не состоялся: crates/gateway/src/lib.rs не под git` failure and the
  probe exits 1.
- For scenarios 1, 3, 4, 5, 6, 7, 9, the gate positive control, 11, and 12,
  I injected a distinct failure (normally a missing baseline; for scenarios 5
  and 12, a different missing archive cause).  Each complete probe exits 1;
  `expect_fail` reports the cause mismatch where used, while scenario 11's
  explicit count-plus-exact-dangling check rejects the wrong early failure.

### N-1 — identifier grammar is an explicit current contract boundary, not a live defect

- **Where:** `scripts/check_archived_refs.sh:88-94`.
- IDs remain decimal with an optional *lowercase* letter.  No top-level
  `docs/archive/` name of an uppercase or other unsupported ID form exists;
  `M-10-obi-killscreen-retired-2026-07` is a directory, not an artifact.
  Comparing the exact selected set at `ac94882` and this head gives the same
  62 names.  A future grammar expansion (for example `M-60A`) needs a matching
  RED case, but this change neither silently widens nor narrows the present
  contract.

## Artifact, scope, and ordering

- The full base-to-head range contains the prior audit artifact
  `research/critiques/C-255-archived-refs-boundary.md`; excluding that already
  committed verdict, the implementation diff touches exactly the three allowed
  subject files.  No production, contract, milestone, or documentation file is
  changed by this revision.
- `LIVE` is byte-identical to base and the exclusion list is unchanged.  The
  baseline decreased from 29 to 21 records, never grew.
- The remedy's RED probe commit `e07f4ea` precedes its barrier commit
  `7c282c3`, satisfying the requested RED-first ordering.  The six mutation
  results below additionally prove the harness against a broken mechanism.
- The non-recursive archive selector has identical membership at `ac94882` and
  `7c282c3`: 62 names on each side, empty symmetric difference.

## Done Block

```text
$ git fetch origin && git rev-parse origin/harness/archived-refs-boundary
7c282c3498fd9f8b4e4e0babf7c14df26ae90ad9
$ git merge-base --is-ancestor 7c282c3 origin/harness/archived-refs-boundary; echo exit=$?
exit=0
$ git merge-base origin/main origin/harness/archived-refs-boundary
29b0a93a17df5d499e607be05d9a51afbea4eb8e

$ bash scripts/tests/red_archived_refs.sh; echo exit=$?
ok    известный случай: висячая ссылка в живом коде ⇒ отказ
ok    после починки пути: принятие
ok    новое нарушение при непустой базовой линии ⇒ отказ (амнистия не распространяется)
ok    протухшая строка базовой линии ⇒ отказ (список обязан сокращаться)
ok    отсутствие базовой линии ⇒ отказ (fail-closed)
ok    пустой архив ⇒ отказ (барьер не судит пустоту)
ok    исключение оснастки точечное: чужой файл под scripts/** по-прежнему судится
ok    граница спеки: ссылка на живой M-99b при вынесенном M-99 ⇒ принятие
ok    позитивный контроль: ссылка на вынесенный M-99 (конец слова и дефис) ⇒ отказ
ok    граница гейта: ссылка на живой verify_M-99b.sh при вынесенном verify_M-99.sh ⇒ принятие
ok    позитивный контроль гейта: ссылка на вынесенный verify_M-99.sh ⇒ отказ
ok    суффикс гейта: verify_M-98-umbrella-*.sh считается артефактом (найдено 2), ссылка на scripts/verify_M-98.sh ⇒ отказ
ok    ложная амнистия: строка на файл, ссылающийся только на живой M-99b, ⇒ отказ (протухла)
ok    отбор = разбор: M-probe-notes.md и verify_M-probe.sh без номера артефактами не считаются (найдено 1)

VERDICT: PASS — сценариев: 14, расхождений: 0
exit=0

$ EVENT_NAME=pull_request PR_BASE_SHA=29b0a93a17df5d499e607be05d9a51afbea4eb8e bash scripts/check_archived_refs.sh; echo exit=$?
PASS  вынесенных артефактов найдено: 62
PASS  НОВЫХ висячих ссылок нет (проверено 62 артефактов)
PASS  базовая линия актуальна (унаследованных ссылок: 21)

VERDICT: PASS
exit=0

$ selection_proof (the two selector arms, each printing BASH_REMATCH[1])
selected=62 spec=31 gate=31 selected_without_id=0
exit=0

$ mutation a_search_fixed; git diff --unified=0 -- scripts/check_archived_refs.sh
-  hits=$(git grep -n -E "$(boundary_re "$old")" -- "${LIVE[@]}" 2>/dev/null | grep -v 'ARCHIVED-REF-OK' || true)
+  hits=$(git grep -n -F "$old" -- "${LIVE[@]}" 2>/dev/null | grep -v 'ARCHIVED-REF-OK' || true)
VERDICT: FAIL — сценариев: 14, расхождений: 2
exit=1
$ mutation b_suffix_filter; git diff --unified=0 -- scripts/check_archived_refs.sh
-  elif [[ "$art" =~ ^verify_(M-[0-9]+[a-z]?)(-.*)?\.sh$ ]]; then
+  elif [[ "$art" =~ ^verify_(M-[0-9]+[a-z]?)\.sh$ ]]; then
VERDICT: FAIL — сценариев: 14, расхождений: 1
exit=1
$ mutation c_blind_search; git diff --unified=0 -- scripts/check_archived_refs.sh
-  hits=$(git grep -n -E "$(boundary_re "$old")" -- "${LIVE[@]}" 2>/dev/null | grep -v 'ARCHIVED-REF-OK' || true)
+  hits=''
VERDICT: FAIL — сценариев: 14, расхождений: 6
exit=1
$ mutation d_baseline_fixed; git diff --unified=0 -- scripts/check_archived_refs.sh
-  if ! git grep -q -n -E "$(boundary_re "$bold")" -- "$bf" 2>/dev/null; then
+  if ! git grep -q -n -F "$bold" -- "$bf" 2>/dev/null; then
VERDICT: FAIL — сценариев: 14, расхождений: 1
exit=1
$ mutation e_spec_grammar; git diff --unified=0 -- scripts/check_archived_refs.sh
-  if [[ "$art" =~ ^(M-[0-9]+[a-z]?)-.*\.md$ ]]; then
+  if [[ "$art" =~ ^(M-[^-]*)-.*\.md$ ]]; then
FAIL  имя без идентификатора принято за артефакт или сломало барьер (rc=0): PASS  вынесенных артефактов найдено: 2
VERDICT: FAIL — сценариев: 14, расхождений: 1
exit=1
$ mutation f_gate_grammar; git diff --unified=0 -- scripts/check_archived_refs.sh
-  elif [[ "$art" =~ ^verify_(M-[0-9]+[a-z]?)(-.*)?\.sh$ ]]; then
+  elif [[ "$art" =~ ^verify_(M-[^.-]*)(-.*)?\.sh$ ]]; then
FAIL  имя без идентификатора принято за артефакт или сломало барьер (rc=0): PASS  вынесенных артефактов найдено: 2
VERDICT: FAIL — сценариев: 14, расхождений: 1
exit=1

$ mutation remove_case4_commit; git diff --unified=0 -- scripts/tests/red_archived_refs.sh
-commit_all "$R3"
FAIL  SETUP не состоялся: crates/gateway/src/lib.rs не под git — сценарий судил бы не то, что заявляет
VERDICT: FAIL — сценариев: 14, расхождений: 1
exit=1

$ (cd cause_s1 && bash scripts/tests/red_archived_refs.sh); echo exit=$?
FAIL  известный случай ПРОПУЩЕН — барьер не ловит то, ради чего заведён — отказ был, но НЕ по заявленной причине «висячая ссылка на вынесенный артефакт «M-99-probe-subject.md»»: FAIL  нет базовой линии scripts/lib/archived_refs_baseline.txt — барьер fail-closed
FAIL  исправленный путь всё ещё считается висячим — барьер краснеет на правде
VERDICT: FAIL — сценариев: 14, расхождений: 2
exit=1
$ (cd cause_s3 && bash scripts/tests/red_archived_refs.sh); echo exit=$?
FAIL  новое нарушение ПРОЩЕНО базовой линией — список стал амнистией для всего дерева — отказ был, но НЕ по заявленной причине «crates/journal/src/lib.rs»: FAIL  нет базовой линии scripts/lib/archived_refs_baseline.txt — барьер fail-closed
VERDICT: FAIL — сценариев: 14, расхождений: 1
exit=1
$ (cd cause_s4 && bash scripts/tests/red_archived_refs.sh); echo exit=$?
FAIL  протухшая строка ПРИНЯТА — список амнистирует то, чего уже нет, и слабеет молча — отказ был, но НЕ по заявленной причине «базовая линия протухла»: FAIL  нет базовой линии scripts/lib/archived_refs_baseline.txt — барьер fail-closed
VERDICT: FAIL — сценариев: 14, расхождений: 1
exit=1
$ (cd cause_s5 && bash scripts/tests/red_archived_refs.sh); echo exit=$?
FAIL  барьер работает БЕЗ базовой линии — молчание принято за чистоту — отказ был, но НЕ по заявленной причине «нет базовой линии»: FAIL  в docs/archive/ не найдено НИ ОДНОГО вынесенного артефакта — барьер
VERDICT: FAIL — сценариев: 14, расхождений: 1
exit=1
$ (cd cause_s6 && bash scripts/tests/red_archived_refs.sh); echo exit=$?
FAIL  пустой архив принят за чистоту — барьер зеленел бы, ничего не проверяя — отказ был, но НЕ по заявленной причине «не найдено НИ ОДНОГО вынесенного артефакта»: FAIL  нет базовой линии scripts/lib/archived_refs_baseline.txt — барьер fail-closed
VERDICT: FAIL — сценариев: 14, расхождений: 1
exit=1
$ (cd cause_s7 && bash scripts/tests/red_archived_refs.sh); echo exit=$?
FAIL  исключение оснастки ослепило весь scripts/** — барьер перестал видеть свою же зону — отказ был, но НЕ по заявленной причине «scripts/some_other_gate.sh»: FAIL  нет базовой линии scripts/lib/archived_refs_baseline.txt — барьер fail-closed
VERDICT: FAIL — сценариев: 14, расхождений: 1
exit=1
$ (cd cause_s9 && bash scripts/tests/red_archived_refs.sh); echo exit=$?
FAIL  граница УБИЛА обнаружение — ссылка на вынесенный M-99 принята — отказ был, но НЕ по заявленной причине «висячая ссылка на вынесенный артефакт «M-99-probe-subject.md»»: FAIL  нет базовой линии scripts/lib/archived_refs_baseline.txt — барьер fail-closed
VERDICT: FAIL — сценариев: 14, расхождений: 1
exit=1
$ (cd cause_gate_positive && bash scripts/tests/red_archived_refs.sh); echo exit=$?
FAIL  граница гейта УБИЛА обнаружение — ссылка на вынесенный verify_M-99.sh принята — отказ был, но НЕ по заявленной причине «висячая ссылка на вынесенный артефакт «verify_M-99.sh»»: FAIL  нет базовой линии scripts/lib/archived_refs_baseline.txt — барьер fail-closed
VERDICT: FAIL — сценариев: 14, расхождений: 1
exit=1
$ (cd cause_s11 && bash scripts/tests/red_archived_refs.sh); echo exit=$?
FAIL  суффикс гейта: барьер насчитал не 2 артефакта — суффиксный гейт не признан артефактом (или архив судился как пустой)
VERDICT: FAIL — сценариев: 14, расхождений: 1
exit=1
$ (cd cause_s12 && bash scripts/tests/red_archived_refs.sh); echo exit=$?
FAIL  ложная амнистия ПРИНЯТА — проверка протухания ищет подстрокой и видит M-99 в M-99b — отказ был, но НЕ по заявленной причине «базовая линия протухла»: FAIL  в docs/archive/ не найдено НИ ОДНОГО вынесенного артефакта — барьер
VERDICT: FAIL — сценариев: 14, расхождений: 1
exit=1

$ fixture directories before/after probe
before=26927
after=26927
fixture_dir_delta=0

$ git diff --name-status 29b0a93..7c282c3 -- . ':(exclude)research/critiques/C-255-archived-refs-boundary.md'
M scripts/check_archived_refs.sh
M scripts/lib/archived_refs_baseline.txt
M scripts/tests/red_archived_refs.sh
$ compare LIVE blocks at base and head; echo exit=$?
LIVE_unchanged=yes
exit=0
$ baseline record counts
base=29
head=21
$ git log --reverse --format='%H %s' ac94882..7c282c3
e07f4ea5fa06ee98b70ba9fdd9d8678f665a6219 fix(harness): archived-refs — проба: страж под-git и отказ по НАЗВАННОЙ причине; сценарий 14 (C-255) [architect]
7c282c3498fd9f8b4e4e0babf7c14df26ae90ad9 fix(harness): archived-refs — отбор и разбор ОДНИМ выражением; недостижимый страж снят (C-255 B-1) [architect]
$ compare non-recursive selected sets at ac94882 and 7c282c3
ac94882_count=62
head_count=62
symmetric_difference:
same_names=yes
exit=0

$ git show -s --format='%H%n%s' refs/reserved-cache/C-256
4b8c2a9eb9a698140ae431669c01d2a5af0971c6
reserve C-256 nous 2026-09-26T19:23:58Z Ubuntu-2404-noble-amd64-base 647873 35a3f337-c00d-4044-b796-cbbffc889774
```

## Handoff

NOTE → architect.  C-255 B-1/B-2 are verified closed; retain the numeric plus
optional-lowercase identifier grammar unless a separately RED-tested contract
change broadens it.
