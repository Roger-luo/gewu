"""Compact author contract; native measurement remains the validation authority."""


def expansion_contract(max_rows):
    return f"""Write a newcomer walkthrough for the supplied item using frozen evidence.
Return JSON with schema_version:1, source copied exactly, item_id:item.id, chunks:[...].
Each chunk has id, title, summary (nonempty, <=500 characters), kind
(concept|refactor|integration|test|other), and content:[blocks]. Blocks are
{{"type":"narrative","markdown":"behavior and review question"}} or
{{"type":"changes","quotes":[{{"change_id":"original id","start":1,"end":20}}],
"presentation":"diff|added_code|rendered_markdown|summary_only"}}.
Optional chunk confidence is a specific supported uncertainty. Optional context is
{{"kind":"concept","definition":"short explanation","example":"plain code, no fence",
"relation":"why this helps read the chunk"}}. Omit optional fields without value.
At most 6 chunks per item. Together their quotes must cover every item.covers line;
never quote outside those spans. Keep each chunk within {max_rows} rendered rows.
Chrome costs 8 rows; prose/source wrap at 80 columns with source gutters and headers.
Native measurement decides fit; do not claim a draft fits before inspecting it.
Use the fewest coherent chunks that fit, with specific prose before source views.
Explain consequential behavior, conditions, failure paths and a review decision.
Avoid separate import inventories, repeated caveats, generic praise or line-by-line paraphrase.
Use added_code for new code; diff for meaningful removals; rendered_markdown only for
.md files. summary_only is only for demonstrably mechanical generated artifacts,
never handwritten code, configuration, legal text or policy. Do not hide behavior to fit.
Connect verified dependencies with path/line references. Distinguish local packet limits
from corpus absence. No invented motivation, guarantees, executed tests or approvals.
Use context for new concepts, not for hiding reviewable content. Context is measured
separately. oversized_reason is allowed only for an indivisible over-budget source line.
Return the draft for measurement; submission requires the exact valid measured hash."""
