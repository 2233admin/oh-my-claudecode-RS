---
name: ultrawork
description: Parallel execution engine for high-throughput task completion
argument-hint: "<task description with parallel work items>"
level: 4
---

# Ultrawork Skill

Ultrawork is a parallel execution engine and execution protocol for independent work. It emphasizes intent grounding, parallel context gathering, dependency-aware task graphs for non-trivial work, and concise evidence-backed execution summaries.

## Purpose

Ultrawork enables firing multiple agents simultaneously and routing each to the right model tier, reducing total execution time while controlling token costs. It is a composable component that ralph and autopilot layer on top of.

## Use When

- Multiple independent tasks can run simultaneously
- User says "ulw", "ultrawork", or wants parallel execution
- You need to delegate work to multiple agents at once
- Task benefits from concurrent execution but the user will manage completion themselves

## Do Not Use When

- Task requires guaranteed completion with verification -- use `ralph` instead
- Task requires a full autonomous pipeline -- use `autopilot` instead
- There is only one sequential task with no parallelism opportunity -- delegate directly
- User needs session persistence for resume -- use `ralph` which adds persistence on top of ultrawork

## Execution Policy

- **Fire all independent agent calls simultaneously** -- never serialize independent work
- **Always pass the `model` parameter explicitly** when delegating
- **Read `docs/shared/agent-tiers.md`** before first delegation for agent selection guidance
- **Use `run_in_background: true`** for operations over ~30 seconds (installs, builds, tests)
- **Run quick commands** (git status, file reads, simple checks) in the foreground
- **Resolve intent before implementation** -- explore first, ask only when still blocked
- **For non-trivial tasks, produce a dependency-aware plan** with parallel waves before execution
- **Keep delegated-task reports concise** -- short summary, files touched, verification status, blockers
- **Manual QA is required** for implemented behavior, not just diagnostics

## Steps

1. **Read agent reference**: Load `docs/shared/agent-tiers.md` for tier selection
2. **Ground intent first**: Confirm whether the request is implementation, investigation, evaluation, or research
3. **Gather context in parallel**:
   - direct tools for quick reads/searches
   - exploration/docs agents for broad context
4. **Classify tasks by independence**: Identify which tasks can run in parallel vs which have dependencies
5. **Create a task graph for non-trivial work**:
   - Parallel Execution Waves
   - Dependency Matrix
   - Acceptance criteria and verification steps per task
6. **Route to correct tiers**:
   - Simple lookups/definitions: LOW tier (Haiku)
   - Standard implementation: MEDIUM tier (Sonnet)
   - Complex analysis/refactoring: HIGH tier (Opus)
7. **Fire independent tasks simultaneously**: Launch all parallel-safe tasks at once
8. **Run dependent tasks sequentially**: Wait for prerequisites before launching dependent work
9. **Background long operations**: Builds, installs, and test suites use `run_in_background: true`
10. **Verify when all tasks complete** (lightweight):
    - Build/typecheck passes
    - Affected tests pass
    - Manual QA completed for implemented behavior
    - No new errors introduced

## Tool Usage

- `Task(subagent_type="oh-my-claudecode:executor", model="haiku", ...)` for simple changes
- `Task(subagent_type="oh-my-claudecode:executor", model="sonnet", ...)` for standard work
- `Task(subagent_type="oh-my-claudecode:executor", model="opus", ...)` for complex work
- `run_in_background: true` for package installs, builds, and test suites
- Foreground execution for quick status checks and file operations

## Examples

### Good: Three independent tasks fired simultaneously

```json
Task(subagent_type="oh-my-claudecode:executor", model="haiku", prompt="Add missing type export for Config interface")
Task(subagent_type="oh-my-claudecode:executor", model="sonnet", prompt="Implement the /api/users endpoint with validation")
Task(subagent_type="oh-my-claudecode:executor", model="sonnet", prompt="Add integration tests for the auth middleware")
```

Why good: Independent tasks at appropriate tiers, all fired at once.

### Good: Correct use of background execution

```json
Task(subagent_type="oh-my-claudecode:executor", model="sonnet", prompt="npm install && npm run build", run_in_background=true)
Task(subagent_type="oh-my-claudecode:executor", model="haiku", prompt="Update the README with new API endpoints")
```

Why good: Long build runs in background while short task runs in foreground.

### Bad: Sequential execution of independent work

```python
result1 = Task(executor, "Add type export")  # wait...
result2 = Task(executor, "Implement endpoint")     # wait...
result3 = Task(executor, "Add tests")              # wait...
```

Why bad: These tasks are independent. Running them sequentially wastes time.

### Bad: Wrong tier selection

```json
Task(subagent_type="oh-my-claudecode:executor", model="opus", prompt="Add a missing semicolon")
```

Why bad: Opus is expensive overkill for a trivial fix. Use executor with Haiku instead.

## Escalation and Stop Conditions

- When ultrawork is invoked directly (not via ralph), apply lightweight verification only -- build passes, tests pass, no new errors
- For full persistence and comprehensive architect verification, recommend switching to `ralph` mode
- If a task fails repeatedly across retries, report the issue rather than retrying indefinitely
- Escalate to the user when tasks have unclear dependencies or conflicting requirements

## Final Checklist

- [ ] All parallel tasks completed
- [ ] Build/typecheck passes
- [ ] Affected tests pass
- [ ] No new errors introduced

## Relationship to Other Modes

```
ralph (persistence wrapper)
 \-- includes: ultrawork (this skill)
     \-- provides: parallel execution only

autopilot (autonomous execution)
 \-- includes: ralph
     \-- includes: ultrawork (this skill)
```

Ultrawork is the parallelism layer. Ralph adds persistence and verification. Autopilot adds the full lifecycle pipeline.
