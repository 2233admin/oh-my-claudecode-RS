---
name: team
description: N coordinated agents on shared task list using Claude Code native teams
argument-hint: "[N:agent-type] [ralph] <task description>"
level: 4
---

# Team Skill

Spawn N coordinated agents working on a shared task list using Claude Code's native team tools. Replaces legacy swarm mode with built-in team management, inter-agent messaging, and task dependencies.

## Usage

```
/team N:agent-type "task description"
/team "task description"
/team ralph "task description"
```

### Parameters

- **N** - Number of teammate agents (1-20). Defaults to auto-sizing based on task decomposition.
- **agent-type** - OMC agent type for team-exec stage (e.g., executor, debugger, designer). Defaults to stage-aware routing.
- **task** - High-level task to decompose and distribute among teammates
- **ralph** - Optional modifier. When present, wraps team pipeline in Ralph's persistence loop.

## Architecture

```
User: "/team 3:executor fix all TypeScript errors"
              |
              v
      [TEAM ORCHESTRATOR (Lead)]
              |
              +-- TeamCreate("fix-ts-errors")
              |       -> lead becomes team-lead@fix-ts-errors
              |
              +-- Analyze & decompose task into subtasks
              |
              +-- TaskCreate x N (one per subtask)
              |
              +-- TaskUpdate x N (pre-assign owners)
              |
              +-- Task(team_name, name) x N -> spawn teammates
              |
              +-- Monitor loop
              |       <- SendMessage from teammates (auto-delivered)
              |       -> TaskList polling for progress
              |
              +-- Completion
                      -> SendMessage(shutdown_request) to each teammate
                      <- SendMessage(shutdown_response, approve: true)
                      -> TeamDelete("fix-ts-errors")
```

## Staged Pipeline

Team execution follows a staged pipeline:

`team-plan -> team-prd -> team-exec -> team-verify -> team-fix (loop)`

### Stage Agent Routing

| Stage | Primary Agents | Optional Agents |
|-------|----------------|----------------|
| **team-plan** | `explore` (haiku), `planner` (opus) | `analyst`, `architect` |
| **team-prd** | `analyst` (opus) | `critic` (opus) |
| **team-exec** | `executor` (sonnet) | `debugger`, `designer`, `writer`, `test-engineer` |
| **team-verify** | `verifier` (sonnet) | `security-reviewer`, `code-reviewer` |
| **team-fix** | `executor` (sonnet) | `debugger` |

### Routing Rules

1. **Lead picks agents per stage.** The user's `N:agent-type` parameter only overrides the team-exec stage worker type.
2. **Specialist agents complement executor agents.** Route analysis/review to architect/critic agents and UI work to designer agents.
3. **Cost mode affects model tier.** In downgrade: `opus` to `sonnet`, `sonnet` to `haiku` where quality permits.
4. **Risk level escalates review.** Security-sensitive or >20 file changes must include security-reviewer + code-reviewer.

## Execution Flow

### Phase 1: Parse Input

- Extract **N** (agent count), validate 1-20
- Extract **agent-type**, validate it maps to a known OMC subagent
- Extract **task** description

### Phase 2: Analyze & Decompose

Use `explore` or `architect` agent to analyze the codebase and break the task into N subtasks:

- Each subtask should be file-scoped or module-scoped to avoid conflicts
- Subtasks must be independent or have clear dependency ordering
- Each subtask needs a concise `subject` and detailed `description`
- Identify dependencies between subtasks

### Phase 3: Create Team

Call `TeamCreate` with a slug derived from the task:

```json
{
  "team_name": "fix-ts-errors",
  "description": "Fix all TypeScript errors across the project"
}
```

### Phase 4: Create Tasks

Call `TaskCreate` for each subtask. Set dependencies with `TaskUpdate` using `addBlockedBy`.

```json
{
  "subject": "Fix type errors in src/auth/",
  "description": "Fix all TypeScript errors in src/auth/login.ts...",
  "activeForm": "Fixing auth type errors"
}
```

For tasks with dependencies:

```json
{
  "taskId": "3",
  "addBlockedBy": ["1"]
}
```

### Phase 5: Spawn Teammates

Spawn N teammates using `Task` with `team_name` and `name` parameters:

```json
{
  "subagent_type": "oh-my-claudecode:executor",
  "team_name": "fix-ts-errors",
  "name": "worker-1",
  "prompt": "<worker-preamble + assigned tasks>"
}
```

### Phase 6: Monitor

Monitor progress through:

1. **Inbound messages** -- Teammates send `SendMessage` to `team-lead` when they complete or need help.
2. **TaskList polling** -- Periodically call `TaskList` to check overall progress.

### Phase 7: Completion

When all tasks are completed or failed:

1. **Verify results** -- Check all subtasks are marked `completed` via `TaskList`
2. **Shutdown teammates** -- Send `shutdown_request` to each active teammate
3. **Await responses** -- Each teammate responds with `shutdown_response(approve: true)`
4. **Delete team** -- Call `TeamDelete` to clean up
5. **Report summary** -- Present results to the user

## Worker Protocol

Workers follow this protocol:

1. **CLAIM**: Call `TaskList`, pick first pending task assigned to worker, call `TaskUpdate` to set `in_progress`
2. **WORK**: Execute the task using direct tools (Read, Write, Edit, Bash)
3. **COMPLETE**: Mark task completed via `TaskUpdate`
4. **REPORT**: Notify lead via `SendMessage`
5. **NEXT**: Check for more assigned tasks, repeat or stand by
6. **SHUTDOWN**: Respond to `shutdown_request` with `shutdown_response`

## Team + Ralph Composition

When the user invokes `/team ralph`, the team pipeline wraps in Ralph's persistence loop:

1. Ralph outer loop starts (iteration 1)
2. Team pipeline runs: `team-plan -> team-prd -> team-exec -> team-verify`
3. If `team-verify` passes: Ralph runs architect verification
4. If architect approves: both modes complete
5. If `team-verify` fails: team enters `team-fix`, then loops back

## Configuration

Optional settings in `.claude/omc.jsonc` or `~/.config/claude-omc/config.jsonc`:

```jsonc
{
  "team": {
    "ops": {
      "maxAgents": 20,
      "defaultAgentType": "claude",
      "monitorIntervalMs": 30000,
      "shutdownTimeoutMs": 15000
    }
  }
}
```

## Gotchas

1. **Internal tasks pollute TaskList** -- Filter `metadata._internal: true` when counting real tasks.
2. **No atomic claiming** -- Lead should pre-assign owners via `TaskUpdate` before spawning teammates.
3. **Task IDs are strings** -- Always pass string values to `taskId` fields.
4. **TeamDelete requires empty team** -- All teammates must be shut down before calling `TeamDelete`.
5. **Messages are auto-delivered** -- Teammate messages arrive as new conversation turns.
6. **CLI workers are one-shot** -- Tmux CLI workers (Codex/Gemini) run autonomously, cannot use TaskList/TaskUpdate/SendMessage.
