# Repository-scale review development and acceptance

## Objective and baseline

Gewu must produce a useful walkthrough of its entire frozen repository, then
have an independent reviewer evaluate that walkthrough. A valid spec alone is
not success. Ordinary use still waits for the human between expansion batches.
Automated evaluation must use a separate session and must not leave synthetic
progress or approval in the user's review.

The Bloqade website experiment at `3735c1c3c7075b904fef6a7bb1408d4ef8d50985`
covered exactly 294 tracked paths. The first author request was 3,072,314 bytes
and failed with `Prompt is too long` before inference. Its immutable manifest
is retained at `bloqade-website/.gewu/scale-test/session/.gewu/review-input.json`.
The external Autotune driver embeds the complete manifest at investigation and
planning; expansion scopes by change ID but still copies entire hunks.
Excluding lockfiles would reduce scope without solving the general problem.

## Architecture

1. **Immutable corpus, bounded access.** Keep the complete manifest and its
   source fingerprint as the authority. Add paged inventory and quoted-source
   access in `gewu-review`, exposed by the CLI. Every serialized response has a
   hard byte limit, includes the global source identity, and advances an
   explicit cursor. UTF-8, huge individual lines, metadata and removals must be
   lossless. Transport splits are not narrative or work-item boundaries.
2. **Incremental investigation.** An author inspects bounded source packets and
   relevant consumers. Persist findings with exact source spans, evidence,
   unresolved questions, dependency relationships and proposed semantic work
   boundaries. Validate each checkpoint before accepting it. A coverage ledger
   accounts for every span; delivery of bytes is not evidence of comprehension.
   Generated artifacts may have a justified summary disposition, with inspected
   representative evidence and an explicit limitation. Handwritten behavior
   cannot disappear behind generic summaries.
3. **Bounded synthesis and planning.** Plan local semantic units from their
   findings, then order them using compact subsystem/dependency summaries.
   Reconcile cross-file claims against all relevant findings before writing the
   opening overview. Page or hierarchically reduce summaries if they exceed the
   budget; never concatenate all prior output as a hidden fallback. Validate
   global coverage and dependencies after merging local plans.
4. **Span-local expansion.** Deliver assigned spans, relevant findings and
   bounded prerequisite context. Do not repeat whole hunks, the full worklist,
   all findings or prior tool exchanges. Keep source offsets global. Use the
   existing measurement/repair gate and human feedback loop.
5. **Bounded execution and recovery.** The author transport checks the complete
   request before inference, including instructions, schemas and tool results.
   Record bytes, available model usage, retries and elapsed time per stage.
   Byte limits are deterministic transport limits, not exact token counts.
   Set per-call, per-stage and per-run limits; stop on exhausted budgets rather
   than retrying unchanged requests. Checkpoint reuse requires source, protocol,
   configuration, author identity, packet policy, prerequisite checkpoint
   hashes and relevant feedback matches. Revalidate cached outputs before
   reuse. Resume accepted work; preserve failed attempts and diagnostics.

The reusable source/coverage behavior belongs in Gewu's domain crate. CLI
presentation stays in `gewu-cli`. Generation/evaluation orchestration should be
versioned with Gewu so this capability does not depend on an unrelated checkout.
Existing small-review checkpoints remain supported during migration.

## Development gates (in order)

| Gate | Required evidence before advancing |
| --- | --- |
| A: deterministic transport | Tests reconstruct all source, including Unicode and a line larger than the page budget; every JSON response fits its bound; invalid/stale cursors fail; offline replay covers the failed 294-path manifest without any model call. |
| B: workflow integrity | End-to-end fixture tests cover investigation batching, synthesis, span planning, expansion, repair and resume. Inject omissions, stale identities, fabricated evidence, duplicate submissions and exhausted budgets; each must fail closed. Existing human feedback and replan tests still pass. |
| C: rubric calibration | Independent reviewer detects seeded false claims, missing cross-file behavior, generic summaries and unjustified handwritten-code summarization. Valid concise examples pass. Save rubric version and evidence; no score-only approvals. |
| D: small live canary | Generate one small, frozen, representative multi-file review. Retain author usage and requests; independently review all chunks under the rubric below. Repair only failed stages. All hard gates and each dimension must pass. |
| E: medium rehearsal | Use a bounded subset spanning source, tests, configuration and generated artifacts; validate cross-subsystem synthesis, recovery and measured usage. Set the full-run budget from these observations. Reviewer must approve readiness with concrete evidence. |
| F: whole Gewu self-review | Only after A–E: freeze all tracked files (including this enhancement), record exact commit/tree and tool hash, generate once under the agreed budget, validate complete span coverage, independently assess the walkthrough and exercise its actual terminal UI. Fix failures and regenerate only affected stages. |

Large-scale **offline** packet tests are allowed at any stage. Large-scale
**model generation** is forbidden until the prior gates have recorded passing
evidence. A failure is a result, not permission to shrink the final scope.

## Quality rubric, version 1

Superseded for new evaluations by version 2 in `self-review.md`, which adds
mechanical coverage and reference gates (`gewu review audit`).

Score each dimension 0–3: 0 = absent/incorrect; 1 = material problems;
2 = usable with small deficiencies; 3 = clear, supported and actionable.
Each score needs chunk IDs, source references and a concrete example. Unknown
or unobserved behavior is unassessed, never an implicit pass.

| Dimension | Evidence required for 3 |
| --- | --- |
| Accuracy | Consequential claims agree with quoted code and consumers, including conditions, exceptions and failure paths; configured checks are not described as executed checks. |
| Completeness | Every span is accounted for; every subsystem and consequential behavior is explained. Summary dispositions are justified and limitations visible. |
| Learning order | Overview defines the necessary vocabulary; chunks introduce prerequisites before consumers and make cross-file relationships explicit. |
| Review usefulness | Each chunk explains behavior and a concrete decision or question the human can assess; prose adds information beyond restating names. |
| Evidence navigation | References resolve to the frozen source and relevant spans; the reviewer can inspect summarized material and follow important relationships. |
| Reading and UI | Actual terminal interaction at 80×24 and 120×40 demonstrates usable navigation, wrapping, context, feedback and resume. Height estimates alone do not establish this. |
| Efficiency | Recorded request sizes stay bounded, total usage stays within the predeclared budget, and repetition/retries do not dominate useful explanation. |

Acceptance requires every dimension at least 2 and accuracy, completeness and
evidence navigation at 3. No unresolved material falsehood, omitted behavior,
coverage gap, malformed reference, hidden truncation, stale artifact or leaked
synthetic reviewer decision is allowed, regardless of average score. All
overview claims and high-risk state/validation/feedback behavior are checked;
the final reviewer inventories every chunk and states any sampling limits.
Author self-checks are useful but do not count as independent acceptance.

## Evidence ledger

- Baseline: reproducible initial-context failure; independent report retained
  beside the Bloqade experiment. No walkthrough quality was established.
- Gate A implementation: `gewu harness context` provides bounded inventory and
  source pages. `scripts/check_context.py` replays a complete manifest through
  the actual CLI with no model calls. It compares ordered IDs, hashes, source
  identity, original coordinates and reconstructed body text, and checks every
  response size. Test invocation:

  ```bash
  mise exec -- cargo build --bin gewu
  python3 scripts/check_context.py --binary target/debug/gewu \
    --input /path/to/review-input.json \
    --output .gewu/scale-evidence/transport.json
  mise run check
  ```

  The first offline replay covered all 294 Bloqade changes in 405 calls, with a
  largest response of 16,376 bytes under a 16,384-byte cap. Independent review
  identified repeated whole-span allocation; the reader now borrows and lazily
  indexes body lines once, then copies only the requested window. A 2,100,001
  byte Unicode line round-tripped in 133 pages in about 142 ms in a debug test.
  This timing is a local observation, not a portable performance threshold.
  CLI invocations still parse the manifest separately; an in-process driver
  should reuse the reader. **Gate A passed:** the revised CLI replay again
  reconstructed all 294 changes in 405 calls (20.125 s while other checks were
  active), with the same 16,376-byte maximum; `mise run check` passed. The
  independent reviewer confirmed the allocation finding resolved, checked
  boundary behavior and independently ran both body-index unit tests and all
  six context integration tests successfully. The binary hash is recorded in
  `.gewu/scale-evidence/transport.json`; checks are in the adjacent `check.log`.
- Gate B, incremental investigation implemented: `harness research` now emits
  bounded tasks, accepts source-exact evidence notes, validates replayed
  journals, pages cached findings and assembles an overview whose claims cite
  accepted findings. Every source page needs accepted notes. Exact attribution
  and structural accounting are distinct from semantic quality. Raw checkpoint
  files and their exported findings must fit the configured limits before
  acceptance; the complete author-provider request remains a separate gate.
  Independent review identified and prompted fixes for missing-ledger bypass,
  stranded journals after source edits, impossible-to-page exported findings,
  and repeated whole-ledger work. Offline CLI scenarios exercise actual Git
  source drift through investigation, planning, expansion, archival and fresh
  research, as well as missing-ledger rejection and raw size limits. Domain
  tests cover restart, duplicate/stale/tampered notes, exact excerpts, omitted
  batches, grounded-overview references, output feasibility and 200-finding
  pagination. Logs: `.gewu/scale-evidence/research-check.log`.
  Re-review confirmed the four fixes; a further continuation-message mismatch
  was corrected and pinned by the source-edit CLI test. Required checks pass,
  including seven new research-domain tests and three offline CLI scenarios.
  No author model generation has been run. Current source-change recovery
  starts a fresh investigation; selective reuse of still-valid findings remains
  part of the recovery/efficiency work before large generation.
- Gate B, author orchestration foundations: `scripts/gewu_driver/` now wraps
  complete supplied requests in durable byte/call limits and exact caches,
  retains provider usage, refuses ambiguous retries, and reconciles persisted
  accounting. Subprocess tests prove capped retained output and cleanup of the
  owned process group even after leader exit. A frozen session rejects changed
  manifests/binaries before native or author calls. Hierarchical synthesis
  preserves original finding references and refuses impossible or unproductive
  calls; local plans and their bounded repairs use native scope validation.
  A two-file fixture exercises native investigation → grounded overview → local
  planning with five scripted calls; planning restart adds zero calls.
  Independent review found five defects in accounting, preflight, freshness and
  process handling; regression tests now cover all five and focused re-review
  found no remaining blockers in those fixes. `mise run check` passes, including
  23 offline Python tests. Evidence: `.gewu/scale-evidence/driver-check.log`.
  See `author-driver.md` for guarantees and limitations. No live author has run.
- Gate B, measured walkthrough orchestration: the offline two-file scenario now
  proceeds through global ordering, expansion, native measurement, repair and
  completion. Each real driver batch stops at `Await`; only the temporary test
  fixture simulates reader events. An interrupted measured draft resumes through
  cached calls and native remeasurement. Unmeasured submissions and out-of-scope
  source lookups fail. Independent review found wide sibling groups, whole-file
  note selection and missing repair instructions; the implementation now groups
  ordering requests, exports structured evidence spans for selection and retains
  the complete guide. A 30-file flat-directory regression fits 4 KiB requests.
  Explicit interrupted-call reconciliation accepts an inspected retained reply
  hash or retires a call, preserving all charges and never retrying implicitly.
  No live author has run. Evidence: `walkthrough-check.log` and
  `walkthrough-python.log` under `.gewu/scale-evidence/`.
  Focused re-review confirmed all three fixes and no blocking defect in the
  inspected ordering, expansion and reconciliation paths; the reviewer ran
  16 focused tests independently. Required checks pass with 30 offline Python
  scenarios. This is workflow evidence, not semantic quality acceptance.
- Gate B **passed**: source-change recovery archives the prior manifest and
  validates unchanged local reuse candidates; authors must recheck them and
  regenerate global synthesis. Replanning preserves expanded spans and validates
  only new work against their complement. Independent review found a surviving
  work regression; recovery-through-completion now covers it. Deleted unexpanded
  changes also trigger replanning, and retained-only plans still wait for human
  review. Required checks pass with 35 offline Python scenarios. Final focused
  review independently passed 2 retained tests, 7 walkthrough tests and 17 native
  progress tests with no remaining blocker. Evidence:
  `.gewu/scale-evidence/reuse-check-final.log`.
- Gate C **passed calibration**: the independent reviewer assessed all five
  held-out semantic examples without reading the answer key. It accepted the
  concise accurate example and rejected false validation, omitted cross-file
  behavior, generic praise and unsupported generated-code summarization. Every
  score includes chunk/source evidence. UI and efficiency remain explicitly
  unassessed. Packet, source hashes, independent report and result are retained
  under `.gewu/scale-evidence/calibration/`. This validates the rubric on small
  seeded examples, not live walkthrough quality.
- Gates D–F remain pending. A small bounded live canary may now run; medium and
  whole-repository generation remain forbidden until their prerequisite evidence
  passes. The external legacy driver still embeds the full input and must not
  be used for these runs.
- Gate D first attempt: frozen `storage.py` and `accounting.py` (2,943 source
  bytes), isolated evaluation copy, at most 24 calls and 1 MiB total byte
  reservations. Claude CLI was configured with a $0.25 per-call cap; that is
  a configured provider limit, not independently verified billing behavior.
  The first request stopped before inference because the CLI was not logged
  in. Provider evidence reports zero input/output tokens and zero cost. The
  failed attempt remains charged in its author ledger; no automatic retry ran.
  Evidence: `.gewu/scale-evidence/canary-v1/result.json`, its provider records
  and `canary-v1.log`. No walkthrough quality has been established. Codex CLI
  reports a signed-in ChatGPT account; provider selection is being resolved
  before a further small attempt.
- Gate D completed canary **failed quality acceptance**. The signed-in Codex
  adapter generated 10 chunks covering both frozen files. All attempts stayed
  within the original 24-call / 1 MiB ceiling: 19 calls, 561,995 reserved bytes,
  maximum complete request 20,739 bytes. Provider traces report 262,061 input
  tokens (44,800 cached) and 11,989 output tokens, including two rejected
  responses. Research and planning were retained across expansion repairs;
  remaining call/byte/stage limits were reduced by earlier usage.
  The independent reviewer inspected every overview bullet and every chunk.
  Four chunks falsely describe supplied helper definitions or callers as
  unavailable. Accuracy scored 1 and efficiency scored 1; compliance with byte
  ceilings did not count as efficiency acceptance. Completeness and evidence
  navigation scored 3, learning order and usefulness 2. Evidence:
  `.gewu/scale-evidence/canary-codex-v2/{generation-report,quality-review}.json`.
  Next work must provide bounded cross-file prerequisite evidence, repair the
  affected expansions and reduce provider/prompt repetition before another
  small trial. **No medium or whole-repository generation is permitted.**
  Actual terminal captures at both required sizes are retained in sibling
  `canary-ui-80x24-v3` and `canary-ui-120x40-v3` directories: populated overview,
  all chunks, context, raw source, outline, comment entry and resume, each using
  a separate initially empty event log. Earlier blank startup captures remain
  archived as superseded evidence, not usability proof.
- Gate D targeted semantic repair passed independent review: one bounded call
  corrected eight fields in the four flagged chunks without changing coverage,
  quotes, order or the other six chunks. This added 8,972 input and 461 output
  tokens. The repaired artifact remains efficiency-failed; semantic repair alone
  does not advance the gate. Evidence: `canary-codex-v2/quality-review-repaired.json`.
- Gate D compact-contract rehearsal still **failed efficiency**. Reusing the
  accepted research on the exact same frozen corpus, new planning and expansion
  used 10 calls, 85,492 input and 2,455 output tokens. Including reused research,
  comparable usage was 132,262 input and 7,082 output tokens across 13 calls.
  All six chunks passed independent semantic review and both terminal sizes
  were exercised. However, five search/read/submission calls consumed 44,238
  input tokens, over half of new input. The reviewer assigned efficiency 1.
  Evidence: `.gewu/scale-evidence/canary-efficient-v1/quality-review.json` and
  `generation-report.json`; terminal captures are in `efficient-ui-*`.
- Bounded prerequisite preloading is now implemented to reduce discovery turns.
  It selects at most two deterministic lexical candidates within a 4 KiB
  envelope, labels them as incomplete hints, preserves source identity and
  never extends assigned quote coverage. The independent reviewer found no
  remaining blocker and independently passed all ten walkthrough tests.
  `mise run check` passed, including 41 Python tests; evidence is
  `.gewu/scale-evidence/prefetch-check.log`. This is implementation evidence,
  not demonstrated efficiency improvement. The next isolated trial reuses the
  accepted research and plan, with at most six new calls and 393,216 reserved
  bytes. Automatic approval review initially stopped it pending explicit user
  authorization to send source to the signed-in Codex service. The user then
  authorized the six-call trial. It completed in four calls with no failures:
  36,266 new input and 1,538 output tokens, six chunks, 114,197 reserved bytes.
  Including the six reused research/planning calls, comparable totals are
  105,082 input and 6,753 output tokens. Both terminal capture runs exited
  successfully. Independent review of all six chunks, the complete overview,
  retained calls and both terminal sizes passed Gate D: accuracy, completeness,
  usefulness and evidence navigation scored 3; learning order, UI and efficiency
  scored 2. No material semantic defect remains in this frozen small canary.
  Evidence: `canary-prefetch-v1/generation-report.json` and `prefetch-ui-*`
  under `.gewu/scale-evidence/`; the independent assessment is
  `canary-prefetch-v1/quality-review.json`. Gate D is passed only at this small
  scope. Gate E requires a representative source/test/config/generated subset
  and explicit cost ceilings before execution; Gate F remains forbidden until
  the medium rehearsal passes. The comparable 105,082 input tokens for 77 lines
  remain expensive and do not establish repository-scale economy.

- Medium preparation: a twelve-file candidate now includes the connected
  author/accounting/storage/reconciliation modules, their budget and usage
  tests, all workspace dependency manifests, mise configuration and Cargo.lock.
  The optional per-inference usage guard passed independent review and four
  focused tests; `mise run check` passed all native checks and 45 Python tests.
  Evidence: `.gewu/scale-evidence/usage-guard-check-final.log`. The candidate
  scope/budget is in `medium-plan.json`; the earlier nine-file transport
  preflight is superseded for scope purposes. Medium-specific recovery evidence,
  refreshed frozen provenance and final launch readiness remain outstanding.
  No medium generation has run.

- Medium recovery preflight passed on an isolated copy of all twelve candidate
  files using scripted responses only. A measured draft resumed with zero new
  draft calls and one required confirmation. In working-tree mode, changing
  accounting and deleting unexpanded Cargo.toml preserved six prior chunks,
  reused twelve unchanged source pages, regenerated the overview and returned
  to Await with no reviewer events. Frozen reviews intentionally ignore later
  source edits; the recovery fixture explicitly uses working-tree mode.
  Pending-reply recovery rejected an incorrect hash, accepted the retained exact
  hash, preserved reservations and served the response without invoking a
  provider. Evidence: `medium-recovery-v5/report.json`; earlier fixture setup
  failures are retained separately and made no model calls. The reproducible
  runner is `scripts/check_medium_recovery.py`. All required checks passed with
  45 Python tests (`medium-recovery-check.log`).
  A fresh twelve-file corpus is pinned in `medium-plan.json` and
  `medium-candidate-v2`. The prepared live runner has a shared 60-call,
  2,500,000-byte reservation ceiling, observed stops at 600,000 input or 40,000
  output tokens, and an input warning at 450,000. These are admission limits,
  not a guaranteed billing cap. Independent launch review and explicit scope
  authorization remain required before execution.

  Final independent inspection found no runner/recovery blocker and verified
  all 68 retained recovery reply hashes. Current twelve-file transport replay
  reconstructed all source in 14 source pages (15 total CLI calls), largest
  response 12,163 bytes under the 12,288-byte cap; see `medium-preflight-v2.json`.
  Required checks passed. The remaining launch condition is explicit user
  authorization for this expanded twelve-file, sixty-call external trial.
