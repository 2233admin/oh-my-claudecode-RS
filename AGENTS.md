# Agent Instructions

## OMC Native First

Use OMC native commands and adapters as the default control plane for this repository.

- Use `omc-team` for team orchestration, tracker updates, sessions, usage, memory, and handoff.
- Use GitHub/Linear through OMC tracker adapters when possible.
- Treat x-cmd as an optional toolbox only when a task explicitly benefits from it.
- Do not require x-cmd or x-cmd skills for normal project work.
- Do not use x-cmd as a hidden tracker, scheduler, memory layer, or source of team truth.

## Engineering Discipline

- Think before coding: state assumptions when they affect risk or implementation.
- Keep changes surgical and tied to the task.
- Preserve unrelated user edits.
- Verify with focused tests or checks before handoff.
- When creating GitHub issues or PRs, read and preserve repository templates and CONTRIBUTING guidance.
