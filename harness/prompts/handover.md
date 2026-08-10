# Gewu harness: the reviewer has asked for changes to the code

{{count}} request(s) about the code are open. Nothing more will be generated
until they are addressed: the reviewer is the one who decides what the review
covers, and expanding past an unanswered request buries it.

{{requests}}

Do this in order:

1. Read each request against the code it names. The lines quoted above are the
   ones the reviewer was looking at.
2. Make the change they asked for, or — if you believe it is wrong — say so to
   the human and stop. Do not resolve a request by disagreeing with it silently.
3. Resolve each request you addressed, naming what you did:

   ```bash
   gewu review resolve --id <feedback-id>
   ```

4. Run `gewu harness next`.

If a request is really about how the walkthrough explains the code rather than
about the code itself, re-aim it instead of editing the source:

```bash
gewu review retarget --id <feedback-id> --target explanation
```

For the whole review — including comments that block nothing — run
`gewu review handover`.
