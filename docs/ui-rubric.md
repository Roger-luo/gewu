# Desktop UI rubric

Gewu’s desktop UI is judged by independent agents against a versioned rubric.
Judges see screenshots (1440×900 and 1024×700, light and possibly dark), aria
snapshots, mechanical `checks.json`, and interaction screenshots. They do not
read application source. This note explains what v1 measures, why, and how to
revise it. The machine-readable rubric is the highest `apps/ui-harness/rubric/vN.json`
(currently v5).

The product criteria come from `docs/design.md` and `docs/desktop.md`: guided
review is orientation, intent before mechanics, prose with quoted code, a
context pane that helps without crowding, clear feedback kinds, and a summary
that supports a real decision. General UI criteria come from established
usability, accessibility, and visual-design sources listed below.

## Scale and evidence

Scores are 0–3 with fixed verbal anchors (broken → clear), in the same spirit
as Nielsen’s severity work and the terminal-UI judge: coarse bands, not false
precision. A dimension score without cited artifact paths does not count.
Mechanical gates run first; they can fail the run or cap the aggregate before
opinion matters.

## Dimensions

### orientation (weight 3)

Reviewers must always know the stage, the chunk, and N of M. Nielsen’s
visibility of system status and Shneiderman’s reduced memory load both demand
that place-in-flow stay on screen. In Gewu this is non-negotiable chrome, not
decoration.

### overview_intent (weight 3)

The overview answers “what does this PR do?” before any hunk. That is
design.md’s first principle and matches “match the real world / recognition
over recall”: intent and scope up front, then a clear start action. A map of
the change is a drawn diagram in the desktop app (a flowchart or another
Mermaid diagram), not an ASCII picture, and its labels are at least as large
as body text. Shrinking the picture so it fits the first screen makes the
labels unreadable; scrolling a readable diagram is fine. The terminal may
show the source.
Code-review research on file order shows early framing shapes what reviewers
actually inspect. The showcase overview uses a before/after card, so judges
who only saw that fixture never met an ASCII diagram.

### prose_with_code (weight 3)

Each point sits with the few lines that support it. Shneiderman’s short-term
memory rule and Nielsen’s recognition heuristic both fail when explanation and
evidence live on different pages. Judges look for that pairing in screenshots,
not for literary quality of the prose.

### code_legibility (weight 3)

Diffs need path, line numbers, add/remove contrast, and syntax color. A lock
file that is TOML and a `.gitignore` with no extension are still source; one
color for the whole quote is a defect. WCAG contrast and cognitive-load work
on reviewability make illegible diffs a first-class defect for a review tool.
The judged fixtures used to contain only Rust, Python, and Markdown, which
already highlighted, so a monochrome `Ion.lock` never reached a judge.

### context_pane (weight 2)

Supporting context appears only when useful, stays titled, and must not steal
the reading width—aligned with Apple’s sidebar/layout guidance and Gestalt
proximity (related support near the chunk, not a second primary column). Score
clearly even when no chunk has context.

### feedback_flow (weight 3)

Comment vs change request, and code vs explanation target, must be
unambiguous; resolve/delete act on a selected item. That is design.md’s
feedback model plus Nielsen/Shneiderman error prevention and clear exits.

### review_flow (weight 3)

A guided review is a sequence, so finishing a step should cost one action:
marking it reviewed opens the next unreviewed step, and the last one opens the
summary. Shneiderman’s “design dialogs to yield closure” and Nielsen’s
efficiency of use both apply. Screenshots alone cannot show this, which is why
v2 judges never flagged its absence; the simple-diff journey now asserts the
advance, and judges score the journey evidence.

### summary_decision (weight 3)

Closure needs unresolved lists, jumps, and Approve / Comment / Request changes,
with batch-scope warnings when generation is incomplete—dialogs that yield
closure and visible status for remaining work.

### visual_hierarchy (weight 2)

Scale, weight, color, alignment, and Gestalt grouping should send the eye to
title and review body before chrome. NN/g visual-design principles and Apple
layout hierarchy are the external anchors; judges cite screenshot structure.

### layout_density (weight 2)

Spend pixels on content; avoid empty voids and crushed columns. WCAG reflow and
Apple’s adaptable layout expectations show up as overflow, clip, and overlap
checks plus the two harness viewports.

### typography_legibility (weight 2)

Type roles (prose vs UI vs code), size, and contrast must hold in light and
dark. WCAG 1.4.3 / 1.4.11 and Apple Dark Mode contrast guidance are the bar;
`text.too_small` is the mechanical floor.

### platform_consistency (weight 2)

macOS-first desktop: leading navigator, main review, optional trailing context;
consistent labels; dark mode that adapts rather than inverting into illegibility.
Apple HIG layout, sidebars, and dark mode, plus Nielsen consistency.

### accessibility_interaction (weight 2)

What axe does not settle alone: visible focus (2.4.7 / 2.4.11), names and
landmarks in aria, target size (2.5.8), documented shortcuts, and feedback that
actions took. Interaction screenshots and keyboard checks are primary evidence.

## Gates vs judgement

Gates use only harness counters (`axe.*`, `layout.*`, `text.too_small`,
`targets.too_small`, `console.errors`, `hooks.missing`, `keyboard.failures`).

- **fail** — unshippable: critical axe, console errors, missing hooks, keyboard
  failures. The scorecard does not pretend a pretty mean can override these.
- **cap** — serious but still scoreable: serious/moderate axe, overflow, clip,
  overlap, undersized text or targets. Caps bound the aggregate so chronic
  mechanical problems cannot hide behind generous dimension scores.

Gates are not a substitute for the accessibility_interaction dimension: a clean
axe run can still fail focus visibility or unnamed controls in screenshots.

## LLM-judge failure modes and mitigations

Vision and text judges are useful and unreliable in known ways:

- **Position bias** in pairwise comparison (Peiyi Wang et al.; Shi et al.):
  prefer first- or second-shown artifacts. Mitigation: when ranking UI versions,
  score each against the rubric independently, then compare; for explicit
  pairwise prompts, swap order or average both orders.
- **Verbose or uncalibrated critique** (GPT-4 heuristic plugins; UICrit): models
  invent issues or over-praise once a design is “mostly fine.” Mitigation:
  anchored 0–3 scales with observable criteria; evidence paths required;
  mechanical checks first.
- **JSON ≠ pixels** (Duan et al.): text-only UI descriptions miss visual
  failures. Mitigation: judges receive screenshots plus aria, never source;
  dimension `look_for` names which artifact to inspect.
- **Single-evaluator noise** (Nielsen severity practice): one rater is unstable.
  Mitigation: multiple independent judges; disagreement flags on the scorecard;
  human calibration on reference captures.

The harness never lets a judge read `apps/desktop` or crate source for the UI
under test. Friction in the UI is the signal; knowing the intended CSS is not.

## Revising the rubric

### Versioning

Publish `v2.json`, `v3.json`, … — never edit a published version in place.
Runs record `rubric_version` in the manifest and scorecard so old judgements
stay interpretable. Update this document’s changelog when shipping a new file.

### Calibration

1. Pick a few reference captures humans agree on (good overview, bad density,
   missing orientation).
2. Run the mechanical checks; fix harness false positives before blaming the
   rubric.
3. Have ≥2 judges score blindly. Where they disagree by more than one band, or
   where means diverge from human consensus, sharpen that dimension’s anchors
   and `look_for` — do not average away the ambiguity in prose.
4. Optionally rank two builds pairwise after independent scoring to confirm
   ordering matches human preference.

### When to add or remove dimensions

Add a dimension only when a recurring defect is not expressible in an existing
one and is observable from harness artifacts. Remove or merge when two
dimensions always co-vary or when judges cannot find distinct evidence. Prefer
sharper anchors over more dimensions. Product-rule changes belong in
`docs/design.md` first; the rubric follows.

## Changelog

- **v9** — A chunk that presents a new form of an existing structure has to
  show the difference. `representation_contrast` (weight 3) scores 1 when the
  title names the property, such as a compact word, and the prose only
  restates the new fields, or the previous definition is not on the page.
  Score 3 requires the prose to say what the earlier definition stored and
  which difference earns the claim, with that earlier definition in the diff
  or in the refactor context pane. A capture that replaces no structure
  scores 3 with that noted. v8 scored whether the pane crowded the code, so
  a dry chunk with no pane still passed.
- **v8** — Status labels in one list form one column. A row with an action
  button beside it and a row without one show the status at the same x
  position. `visual_hierarchy` and `multi_review_navigation` ask for it, and
  `layout.column_drift` caps the aggregate at 80. v7 only checked the center
  line of a button against its caption (`layout.misaligned`), which skips
  grid rows, so a status pushed sideways by its button passed review. v8 also
  asks that the picture beside a quote sit on the code's surface. The picture
  column used its own, darker box that stopped partway down the quote, which
  left a visible step in dark mode; the look-for is judged by screenshot in
  both schemes.
- **v7** — The ground behind a drawing has to be the whole figure, or no
  separate page at all. A white fill that covers only part of the marks, with
  the rest on the app background, scores 1 on `shown_result`, and
  `preview.backdrop` caps the aggregate at 60. v6 asked that the viewport
  include every mark, so the dots were no longer cropped, and the page fill
  stayed the old page size.
- **v6** — The picture beside a quote has to be the whole result. A panel
  that cuts off part of the drawing scores 1 on `shown_result`, and
  `preview.clipped` caps the aggregate at 60. v5 only asked whether anything
  was drawn, so a grid of dots cropped down to the corner of the page still
  counted as showing the code's result.
- **v5** — A quote of something a reviewer judges by looking must show that
  result. `shown_result` (weight 3) scores 0 when a color, a diagram, or a
  drawing function is paired with a blank rendered panel or with no picture.
  `preview.blank` caps the aggregate at 60. v4 never asked, so compiling a
  Typst library into an empty page beside `route` still scored as a readable
  quote.
- **v4** — Overview diagrams must be readable, not merely drawn. A label
  shorter than body text on screen (about 14px) fails `overview_intent`, and
  `illustration.too_small` caps the score at 70. v3 missed this: it asked only
  whether boxes and arrows were drawn, and `text.too_small` reads CSS
  `font-size`, which stays 16px when the SVG is scaled down.
- **v3** — Adds review_flow (weight 3): marking a step reviewed must open the
  next unreviewed step and the last one must land on the summary; the
  simple-diff journey asserts it, so a regression blocks the run. Typography now
  covers agent Markdown rendered as syntax, and visual hierarchy covers captions
  off their controls' center line. New mechanical counters `text.raw_markdown`
  (cap 70) and `layout.misaligned` (cap 80). The showcase-feedback fixture and
  the fake ACP agent now write inline code in titles and summaries, as real
  agents do; v2 missed literal backticks because no judged fixture had any.
  Code legibility now requires syntax color, including lock files and
  extensionless files such as `.gitignore`, and overview intent requires a
  drawn diagram rather than ASCII art. New counters `code.unhighlighted`
  (cap 70) and `illustration.ascii` (cap 75). v2 missed both because its
  fixtures only quoted Rust, Python, and Markdown, and its only illustration
  was a before/after card.
- **v2** — Adds journey_completion, source_selection, agent_progress,
  multi_review_navigation, and failure_recovery (17 dimensions). Gate
  `journeys-complete` fails the run when `journey.blocked` > 0. Journeys cover
  the six workbench user stories in `docs/workbench.md`.
- **v1** — Initial desktop rubric: seven Gewu review dimensions (orientation
  through summary/decision) plus five general UI dimensions (hierarchy, density,
  typography, platform consistency, accessibility/interaction); fail/cap gates
  on the harness counter keys.

## Sources

- [Nielsen — 10 Usability Heuristics](https://www.nngroup.com/articles/ten-usability-heuristics/)
- [Nielsen — Severity Ratings for Usability Problems](https://www.nngroup.com/articles/how-to-rate-the-severity-of-usability-problems/)
- [Shneiderman — Eight Golden Rules](https://www.cs.umd.edu/users/ben/goldenrules.html)
- [WCAG 2.2](https://www.w3.org/TR/WCAG22/)
- [WCAG 1.4.3 Contrast (Minimum)](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum)
- [WCAG 1.4.11 Non-text Contrast](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast)
- [WCAG 2.4.7 Focus Visible](https://www.w3.org/WAI/WCAG22/Understanding/focus-visible)
- [What’s New in WCAG 2.2 (incl. 2.4.11, 2.5.8)](https://www.w3.org/WAI/standards-guidelines/wcag/new-in-22/)
- [WCAG 2.5.8 Target Size (Minimum)](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum)
- [WCAG 1.4.10 Reflow](https://www.w3.org/WAI/WCAG22/Understanding/reflow)
- [Apple HIG — Layout](https://developer.apple.com/design/human-interface-guidelines/layout)
- [Apple HIG — Sidebars](https://developer.apple.com/design/human-interface-guidelines/sidebars)
- [Apple HIG — Dark Mode](https://developer.apple.com/design/human-interface-guidelines/dark-mode)
- [NN/g — 5 Principles of Visual Design](https://www.nngroup.com/articles/principles-visual-design/)
- [NN/g — Gestalt Proximity](https://www.nngroup.com/articles/gestalt-proximity/)
- [Fregnan et al. — File Position in Code Review](https://arxiv.org/abs/2208.04259)
- [Wurzel Gonçalves, Fregnan, et al. — Cognitive load / guided checklists in code review](https://link.springer.com/article/10.1007/s10664-022-10123-8)
- [Duan et al. — UICrit](https://dl.acm.org/doi/10.1145/3654777.3676381)
- [Duan et al. — GPT-4 heuristic feedback on UI mockups](https://arxiv.org/abs/2403.13139)
- [Peiyi Wang et al. — LLMs are not Fair Evaluators](https://arxiv.org/abs/2305.17926)
- [Shi et al. — Position Bias in LLM-as-a-Judge](https://arxiv.org/html/2406.07791v7)
- Local: `docs/design.md`, `docs/desktop.md`, `scripts/self_review/tui_judge.md`
