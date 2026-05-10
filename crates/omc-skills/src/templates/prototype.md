---
name: prototype
description: "Build a throwaway prototype to flush out a design before committing to it. Routes between two branches — a runnable terminal app for state/business-logic questions, or several radically different UI variations toggleable from one route. Use when the user wants to prototype, sanity-check a data model or state machine, mock up a UI, explore design options, or says prototype this, let me play with it, try a few designs."
level: "2"
---

# Prototype

A prototype is **throwaway code that answers a question**. The question decides the shape.

## Pick a branch

- **"Does this logic / state model feel right?"** — Build a tiny interactive terminal app (see Logic branch below).
- **"What should this look like?"** — Generate several radically different UI variations on a single route (see UI branch below).

If the question is genuinely ambiguous and the user isn't reachable, default to whichever branch better matches the surrounding code (a backend module -> logic; a page or component -> UI).

## Rules that apply to both branches

1. **Throwaway from day one, and clearly marked as such.** Locate the prototype code close to where it will actually be used, but name it so a casual reader can see it's a prototype.
2. **One command to run.** Whatever the project's existing task runner supports.
3. **No persistence by default.** State lives in memory.
4. **Skip the polish.** No tests, no error handling beyond what makes it runnable, no abstractions.
5. **Surface the state.** After every action or variant switch, print or render the full relevant state.
6. **Delete or absorb when done.** Don't leave it rotting in the repo.

## When done

The *answer* is the only thing worth keeping. Capture it in a commit message, ADR, issue, or a `NOTES.md` next to the prototype.

---

# Logic Branch

A tiny interactive terminal app that lets the user drive a state model by hand.

## When this is the right shape

- "I'm not sure if this state machine handles the edge case where X then Y."
- "Does this data model actually let me represent the case where..."
- Anything where the user wants to **press buttons and watch state change**.

## Process

### 1. State the question
Write down what state model and what question you're prototyping. One paragraph.

### 2. Pick the language
Use whatever the host project uses. Match the project's existing conventions.

### 3. Isolate the logic in a portable module
Put the actual logic behind a small, pure interface that could be lifted out and dropped into the real codebase later. The TUI around it is throwaway; the logic module shouldn't be.

Pick the right shape for the question:
- **A pure reducer** — `(state, action) => state`. Good for discrete events.
- **A state machine** — explicit states and transitions. Good when legal actions are part of the question.
- **A small set of pure functions** over a plain data type. Good for transformations.
- **A class or module with a clear method surface** for ongoing internal state.

Keep it pure: no I/O, no terminal code. The TUI imports it and calls into it; nothing flows the other direction.

### 4. Build the smallest TUI that exposes the state
Clear the screen on every tick and re-render the whole frame. Each frame has:
1. **Current state**, pretty-printed and diff-friendly.
2. **Keyboard shortcuts**, listed at the bottom.

Behaviour:
1. Initialise state — render the first frame on start.
2. Read one keystroke at a time, dispatch to a handler that mutates state.
3. Re-render the full frame after every action — don't append, replace.
4. Loop until quit.

### 5. Make it runnable in one command

### 6. Hand it over
Give the user the run command.

### 7. Capture the answer
When the prototype has done its job, capture the answer in a `NOTES.md` before deleting the prototype.

### Anti-patterns
- Don't add tests.
- Don't wire it to the real database.
- Don't generalise.
- Don't blur the logic and the TUI together.

---

# UI Branch

Generate **several radically different UI variations** on a single route, switchable from a floating bottom bar.

## When this is the right shape

- "What should this page look like?"
- "I want to see a few options for this dashboard before committing."
- Any time the user would otherwise spend a day picking between three vague mockups in their head.

## Two sub-shapes

**Sub-shape A — adjustment to an existing page (preferred).** Variants are rendered on the same route, gated by a `?variant=` URL search param. The existing data fetching, params, and auth all stay — only the rendering swaps.

**Sub-shape B — a new page (last resort).** Only when the thing being prototyped genuinely has no existing page to live inside.

## Process

### 1. State the question and pick N
Default to **3 variants**. More than 5 stops being radically different and starts being noise.

### 2. Generate radically different variants
Variants must be **structurally different** — different layout, different information hierarchy, different primary affordance. Three slightly-tweaked card grids isn't a UI prototype, it's wallpaper.

### 3. Wire them together
Create a single switcher component on the route:
```tsx
const variant = searchParams.get('variant') ?? 'A';
return (
  <>
    {variant === 'A' && <VariantA {...data} />}
    {variant === 'B' && <VariantB {...data} />}
    {variant === 'C' && <VariantC {...data} />}
    <PrototypeSwitcher variants={['A','B','C']} current={variant} />
  </>
);
```

### 4. Build the floating switcher
A small fixed-position bar at the bottom-centre with:
- Left/right arrows to cycle variants
- Variant label showing current key and name
- Keyboard: left/right arrow keys also cycle
- Hidden in production builds

### 5. Hand it over
Surface the URL (and the `?variant=` keys).

### 6. Capture the answer and clean up
Once a variant has won, note which one and why. Delete the losing variants and switcher; fold the winner into the page.

### Anti-patterns
- Variants that differ only in colour or copy.
- Sharing too much code between variants.
- Wiring variants to real mutations.
- Promoting the prototype directly to production.
