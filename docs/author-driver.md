# Bounded author orchestration

`scripts/gewu_driver/` is the in-repository orchestration layer under development.
It currently supports investigation, grounded overview synthesis, validated
plans and measured expansion batches. It is not yet a complete whole-repository generator. The acceptance
sequence in `repository-scale-review.md` still gates live generation.

## Responsibilities

- `Session` pins the manifest, Gewu binary and driver protocol hashes. It checks
  manifest and binary freshness before native execution. `Preparation` also
  installs the freshness check before every author call or cache return.
- `BudgetedAuthor` wraps complete agent-supplied requests. It reserves request
  bytes plus response capacity before invoking a provider, persists total and
  per-stage call limits, and charges failures without assuming they were free.
  Exact successful requests may be reused; cached artifacts still face native
  domain validation when applied. Persisted accounting is reconciled against
  retained request files before reuse or inference.
- An interrupted call remains pending. It blocks new paid work; the driver does
  not silently retry an ambiguous outcome. A reply saved before a crash is
  retained but is not automatically promoted to success. `reconcile` resolves
  an exact pending call with an audit reason: accept its retained canonical
  reply using an inspected SHA-256 hash, or retire it as failed. Both preserve
  the original reservation; neither retries the call. Domain validation still
  applies when a recovered artifact is used.
- The subprocess adapter retains bounded stdout/stderr, enforces a timeout,
  terminates its owned process group even when the leader has exited, and saves
  process evidence. `ClaudeProcess` requires an explicitly pinned model and
  disables tools and persistence in an isolated directory. It has not yet been
  used for a live canary in this enhancement.
- `CodexProcess` uses the signed-in Codex account in an ephemeral, read-only
  process with user config, shell, apps, plugins, hooks and multi-agent features
  disabled. A short JSON-author instruction file replaces the general coding
  prompt; its bytes are charged and its hash is checked. It uses the final agent
  message and retains earlier commentary in JSONL evidence. Tool activity in
  started, updated or completed events rejects the output. This is transcript
  validation, not proof that every CLI tool schema has been removed. Codex
  account quota has no per-call dollar cap here. Configuration uses the
  [documented instruction override](https://learn.chatgpt.com/docs/config-file/config-reference).
- `compact` reduces large sets of findings through bounded calls while keeping
  references to original findings. It refuses impossible reference budgets,
  missing/invented references and rounds that do not shrink. Reference coverage
  is a structural property; semantic fidelity needs independent evaluation.
- `plan_scope` requests semantic work items for a bounded scope, then invokes
  Gewu's `harness validate-plan --change-id <id>` checks. Repairs replace the
  previous error context rather than accumulating history. The eventual merged
  plan must pass `harness validate-plan` without scope flags.
- `order_plans` orders compact file and directory descriptions, preserves every
  local item, and namespaces identifiers before full native plan validation.
  Wide directories use bounded subgroups and group summaries; leaf purposes
  are compacted first. Every call remains bounded and the hierarchy has a
  depth limit. Semantic dependency order still needs independent evaluation.
- `Expansion.next_batch()` delivers assigned source spans and relevant notes,
  selects findings by structured evidence-span overlap, permits bounded lookups
  within assigned spans, and measures every draft using
  native validation and row counts. Submission must name the exact valid draft
  hash supplied with the measurement. Each accepted batch stops at `Await`;
  the driver never writes reviewer events. Restart replays cached author calls
  and measures the draft again against the current native state.
- Source replanning archives the old manifest beside its research ledger.
  `Preparation.investigate(previous_archive=...)` requests reuse candidates only
  after native validation of the old ledger, identical policy, whole change and
  source page. An author must recheck each proposed local finding; unchanged
  text alone does not prove a claim. A compact confirmation avoids rewriting
  accepted notes. Changed material is investigated normally and global overview
  synthesis always runs afresh. Candidates that would exceed the request bound
  fall back to ordinary investigation. This saves output/regeneration, not the
  source inspection or a model call for each retained page.
- Replanning preserves native surviving expanded items and plans only uncovered
  intervals. New items cannot overlap retained spans; combined coverage is
  validated natively. A retained-only plan still passes through the human gate.

Byte reservations are deterministic application limits, **not a billing or
exact-token guarantee**. Provider-internal system context is not measured by
this wrapper. Available provider token usage is retained, including replies
that exceed the application limit. An oversized generated reply may already
have incurred usage before rejection; no automatic retry follows.

## Offline evidence

`mise run check` builds the native binary and runs the Python scenarios with
scripted providers. They exercise exact cache reuse, budget exhaustion and
tampering, request/response bounds, crash recovery refusal, process output and
descendant cleanup, synthesis reference conservation, scoped-plan rejection,
source/binary drift and native investigation through local planning. No external
model is called by these tests. A separate walkthrough fixture exercises global
ordering, invalid-draft repair, measured submission, restart and both human
gates through native completion. Only that temporary test repository receives
explicitly simulated reader events.

The current two-file preparation fixture makes five scripted calls: two local
investigations, one grounded overview and two local plans. Restarting planning
revalidates the exact cached plans and makes no additional provider calls.

Still required before whole-repository generation: a passing medium rehearsal.
The latest frozen small canary passed independent assessment; its evidence is
recorded in `repository-scale-review.md`. Semantic rubric
calibration has passed five held-out examples; this does not establish live
generation quality. Ordinary human review gates remain intact throughout.

The first completed two-file canary covers all source but fails independent
semantic assessment: span-local prompts led the author to describe available
cross-file evidence as unavailable. Dependency evidence access and targeted
repairs are required before scaling. All generation attempts used 19 calls,
262,061 reported input tokens (44,800 cached) and 11,989 output tokens, within
24 calls and 1 MiB of reservations. These totals include two rejected outputs;
they are not a clean-run efficiency benchmark.

`scripts/check_terminal.py` (optional `pyte==0.8.2`) captures actual native UI
interaction at a specified terminal size into a new directory and separate
event log. It visits every chunk, captures context where present, adds an
explicit UI evaluation comment and resumes. Text/ANSI captures require
inspection; completion of the script is not a usability pass.

Optional `Limits.observed_input_tokens_stop` and
`observed_output_tokens_stop` stop new inference once retained provider usage
reaches either threshold. Missing or invalid usage also stops admission.
Successful-call usage must match the hash-checked retained reply. Exact cached
responses remain usable without inference. Codex cached input is already part
of its input total; separately reported Claude cache reads and creation are
added. `observed_input_tokens_warning` emits a warning below the input stop.
These are observed-usage admission checks, not billing caps: an in-flight call
can cross a threshold. All call and byte limits remain independently enforced.
Disabled optional fields preserve existing author profiles.
