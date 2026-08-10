# Gewu harness: waiting for the reviewer

The review now holds {{chunks}} chunk(s); {{remaining}} work item(s) remain
unexpanded. Nothing more will be generated until the human has read what they
have — their feedback may change the code and make the rest of the plan wrong.

Hand the review to them:

```bash
gewu review open
```

Session: `{{review_id}}`

When they have finished this batch, run `gewu harness next`. Gewu will expand
the next item, wait, or re-plan, depending on what the reviewer did and whether
the code moved underneath.
