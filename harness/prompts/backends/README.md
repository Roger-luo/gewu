# Backend additions

The prompts one directory up are what every agent is sent. A backend that
needs more — a model that skips a step the others take, or reads a rule too
loosely — gets an addition here, appended to the general prompt for that
backend only:

    backends/<preset>/<prompt>.md

`<preset>` is the agent preset's id (`claude`, `codex`, `cursor`, `gemini`,
`opencode`) and `<prompt>` the general prompt's file stem (`expand`,
`proofread`, …). Register each file in `crates/gewu-prompts/src/addenda.rs`;
a test fails on a file that is not registered.

Write an addition only for a failure measured on that backend, and say in it
what it corrects, so the general prompt stays the one source of the rules.
