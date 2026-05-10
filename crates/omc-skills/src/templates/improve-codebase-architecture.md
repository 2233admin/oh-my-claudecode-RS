---
name: improve-codebase-architecture
description: "Find deepening opportunities in a codebase, informed by the domain language in CONTEXT.md and the decisions in docs/adr/. Use when the user wants to improve architecture, find refactoring opportunities, consolidate tightly-coupled modules, or make a codebase more testable and AI-navigable."
level: "advanced"
---

# Improve Codebase Architecture

Surface architectural friction and propose **deepening opportunities** — refactors that turn shallow modules into deep ones. The aim is testability and AI-navigability.

## Glossary

Use these terms exactly in every suggestion. Consistent language is the point.

- **Module** — anything with an interface and an implementation (function, class, package, slice).
- **Interface** — everything a caller must know to use the module: types, invariants, error modes, ordering, config. Not just the type signature.
- **Implementation** — the code inside.
- **Depth** — leverage at the interface: a lot of behaviour behind a small interface. **Deep** = high leverage. **Shallow** = interface nearly as complex as the implementation.
- **Seam** — where an interface lives; a place behaviour can be altered without editing in place. (Use this, not "boundary.")
- **Adapter** — a concrete thing satisfying an interface at a seam.
- **Leverage** — what callers get from depth.
- **Locality** — what maintainers get from depth: change, bugs, knowledge concentrated in one place.

Key principles:
- **Deletion test**: imagine deleting the module. If complexity vanishes, it was a pass-through. If complexity reappears across N callers, it was earning its keep.
- **The interface is the test surface.**
- **One adapter = hypothetical seam. Two adapters = real seam.**

## Process

### 1. Explore

Read the project's domain glossary and any ADRs in the area you're touching first.

Then explore the codebase. Don't follow rigid heuristics — explore organically and note where you experience friction:

- Where does understanding one concept require bouncing between many small modules?
- Where are modules **shallow** — interface nearly as complex as the implementation?
- Where have pure functions been extracted just for testability, but the real bugs hide in how they're called (no **locality**)?
- Where do tightly-coupled modules leak across their seams?
- Which parts of the codebase are untested, or hard to test through their current interface?

Apply the **deletion test** to anything you suspect is shallow.

### 2. Present candidates

Present a numbered list of deepening opportunities. For each candidate:

- **Files** — which files/modules are involved
- **Problem** — why the current architecture is causing friction
- **Solution** — plain English description of what would change
- **Benefits** — explained in terms of locality and leverage, and also in how tests would improve

Do NOT propose interfaces yet. Ask the user: "Which of these would you like to explore?"

### 3. Grilling loop

Once the user picks a candidate, drop into a grilling conversation. Walk the design tree with them — constraints, dependencies, the shape of the deepened module, what sits behind the seam, what tests survive.

Side effects happen inline as decisions crystallize:
- Naming a deepened module after a concept not in CONTEXT.md? Add it to CONTEXT.md.
- Sharpening a fuzzy term? Update CONTEXT.md right there.
- User rejects the candidate with a load-bearing reason? Offer an ADR if the reason would be needed by a future explorer.
- Want to explore alternative interfaces? Use the Interface Design process below.

---

# Reference: Language (Architecture Vocabulary)

Shared vocabulary for every suggestion this skill makes.

**Module**: Anything with an interface and an implementation. Scale-agnostic — applies equally to a function, class, package, or tier-spanning slice.

**Interface**: Everything a caller must know to use the module correctly. Includes the type signature, but also invariants, ordering constraints, error modes, required configuration, and performance characteristics.

**Implementation**: What's inside a module — its body of code.

**Depth**: Leverage at the interface — the amount of behaviour a caller can exercise per unit of interface they have to learn. A module is **deep** when a large amount of behaviour sits behind a small interface. A module is **shallow** when the interface is nearly as complex as the implementation.

**Seam**: A place where you can alter behaviour without editing in place (from Michael Feathers). The location at which a module's interface lives.

**Adapter**: A concrete thing that satisfies an interface at a seam. Describes role, not substance.

**Leverage**: What callers get from depth. More capability per unit of interface they have to learn.

**Locality**: What maintainers get from depth. Change, bugs, knowledge, and verification concentrate at one place rather than spreading across callers.

### Principles

- Depth is a property of the interface, not the implementation.
- The deletion test: imagine deleting the module. If complexity vanishes, it wasn't hiding anything.
- The interface is the test surface.
- One adapter means a hypothetical seam. Two adapters means a real one.

---

# Reference: Deepening (Dependency Categories)

How to deepen a cluster of shallow modules safely, given its dependencies.

## Dependency categories

### 1. In-process
Pure computation, in-memory state, no I/O. Always deepenable — merge the modules and test through the new interface directly.

### 2. Local-substitutable
Dependencies that have local test stand-ins (PGLite for Postgres, in-memory filesystem). Deepenable if the stand-in exists.

### 3. Remote but owned (Ports & Adapters)
Your own services across a network boundary. Define a **port** (interface) at the seam. The deep module owns the logic; the transport is injected as an **adapter**.

### 4. True external (Mock)
Third-party services you don't control. The deepened module takes the external dependency as an injected port; tests provide a mock adapter.

## Seam discipline

- One adapter = hypothetical seam. Two adapters = real seam. Don't introduce a port unless at least two adapters are justified.
- Internal seams vs external seams: a deep module can have internal seams (private to its implementation) as well as the external seam at its interface.

## Testing strategy: replace, don't layer

- Old unit tests on shallow modules become waste once tests at the deepened module's interface exist — delete them.
- Write new tests at the deepened module's interface.
- Tests assert on observable outcomes through the interface, not internal state.

---

# Reference: Interface Design (Parallel Sub-Agent Pattern)

When the user wants to explore alternative interfaces for a chosen deepening candidate, use this parallel sub-agent pattern. Based on "Design It Twice" (Ousterhout).

## Process

### 1. Frame the problem space
Write a user-facing explanation of the constraints any new interface would need to satisfy, the dependencies it would rely on, and a rough illustrative code sketch to ground the constraints.

### 2. Spawn sub-agents
Spawn 3+ sub-agents in parallel, each producing a **radically different** interface:
- Agent 1: "Minimize the interface — aim for 1-3 entry points max. Maximize leverage per entry point."
- Agent 2: "Maximize flexibility — support many use cases and extension."
- Agent 3: "Optimise for the most common caller — make the default case trivial."

Each sub-agent outputs:
1. Interface (types, methods, params — plus invariants, ordering, error modes)
2. Usage example showing how callers use it
3. What the implementation hides behind the seam
4. Dependency strategy and adapters
5. Trade-offs — where leverage is high, where it's thin

### 3. Present and compare
Present designs sequentially, then compare by **depth** (leverage at the interface), **locality** (where change concentrates), and **seam placement**. Give a recommendation.
