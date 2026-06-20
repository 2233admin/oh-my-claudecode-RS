# Custom Skills Guide

Skills are reusable instruction templates that extend Claude Code's capabilities.
Each skill is a directory containing a `README.md` (or `SKILL.md`) that describes
the skill's purpose and instructions.

## Adding a Custom Skill

1. Create a new directory under `~/.claude/skills/`:
   ```
   mkdir -p ~/.claude/skills/my-skill
   ```

2. Add a `README.md` with the skill instructions:
   ```
   echo "# My Skill\nDescription and instructions..." > ~/.claude/skills/my-skill/README.md
   ```

3. The skill is now available for use in Claude Code sessions.

## Skill Directory Structure

```
~/.claude/skills/
  my-skill/
    README.md       # Skill instructions (required)
    templates/      # Optional: template files
    examples/       # Optional: example inputs/outputs
```

## Registering External Skills

External skills can be registered via the OMC CLI:

```
omc skill register /path/to/external/skill
```

This creates a symlink to the external directory. If symlinks are not
available (e.g., on Windows without developer mode), the skill is
copied instead.

## Best Practices

- Keep skills focused on a single responsibility.
- Write clear, actionable instructions in the README.
- Include examples of expected input/output when helpful.
- Use subdirectories for templates, prompts, and reference files.
