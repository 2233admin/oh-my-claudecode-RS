---
name: tdd
description: "Test-driven development with red-green-refactor loop. Use when user wants to build features or fix bugs using TDD, mentions red-green-refactor, wants integration tests, or asks for test-first development."
level: "3"
---

# Test-Driven Development

## Philosophy

**Core principle**: Tests should verify behavior through public interfaces, not implementation details. Code can change entirely; tests shouldn't.

**Good tests** are integration-style: they exercise real code paths through public APIs. They describe _what_ the system does, not _how_ it does it. A good test reads like a specification — "user can checkout with valid cart" tells you exactly what capability exists. These tests survive refactors because they don't care about internal structure.

**Bad tests** are coupled to implementation. They mock internal collaborators, test private methods, or verify through external means (like querying a database directly instead of using the interface). The warning sign: your test breaks when you refactor, but behavior hasn't changed.

## Anti-Pattern: Horizontal Slices

**DO NOT write all tests first, then all implementation.** This is "horizontal slicing" — treating RED as "write all tests" and GREEN as "write all code."

This produces **crap tests**:

- Tests written in bulk test _imagined_ behavior, not _actual_ behavior
- You end up testing the _shape_ of things rather than user-facing behavior
- Tests become insensitive to real changes — they pass when behavior breaks, fail when behavior is fine

**Correct approach**: Vertical slices via tracer bullets. One test, one implementation, repeat. Each test responds to what you learned from the previous cycle.

## Workflow

### 1. Planning

Before writing any code:

- Confirm with user what interface changes are needed
- Confirm with user which behaviors to test (prioritize)
- Identify opportunities for deep modules (small interface, deep implementation)
- Design interfaces for testability
- List the behaviors to test (not implementation steps)
- Get user approval on the plan

Ask: "What should the public interface look like? Which behaviors are most important to test?"

**You can't test everything.** Focus testing effort on critical paths and complex logic, not every possible edge case.

### 2. Tracer Bullet

Write ONE test that confirms ONE thing about the system:

```
RED:   Write test for first behavior -> test fails
GREEN: Write minimal code to pass -> test passes
```

This is your tracer bullet — proves the path works end-to-end.

### 3. Incremental Loop

For each remaining behavior:

```
RED:   Write next test -> fails
GREEN: Minimal code to pass -> passes
```

Rules:
- One test at a time
- Only enough code to pass current test
- Don't anticipate future tests
- Keep tests focused on observable behavior

### 4. Refactor

After all tests pass, look for refactor candidates:

- Extract duplication
- Deepen modules (move complexity behind simple interfaces)
- Apply SOLID principles where natural
- Consider what new code reveals about existing code
- Run tests after each refactor step

**Never refactor while RED.** Get to GREEN first.

## Checklist Per Cycle

```
[ ] Test describes behavior, not implementation
[ ] Test uses public interface only
[ ] Test would survive internal refactor
[ ] Code is minimal for this test
[ ] No speculative features added
```

---

# Reference: Good and Bad Tests

## Good Tests

**Integration-style**: Test through real interfaces, not mocks of internal parts.

Characteristics:
- Tests behavior users/callers care about
- Uses public API only
- Survives internal refactors
- Describes WHAT, not HOW
- One logical assertion per test

## Bad Tests

**Implementation-detail tests**: Coupled to internal structure.

Red flags:
- Mocking internal collaborators
- Testing private methods
- Asserting on call counts/order
- Test breaks when refactoring without behavior change
- Test name describes HOW not WHAT
- Verifying through external means instead of interface

---

# Reference: When to Mock

Mock at **system boundaries** only:
- External APIs (payment, email, etc.)
- Databases (sometimes — prefer test DB)
- Time/randomness
- File system (sometimes)

Don't mock:
- Your own classes/modules
- Internal collaborators
- Anything you control

## Designing for Mockability

1. **Accept dependencies, don't create them.** Pass external dependencies in rather than creating them internally.
2. **Return results, don't produce side effects.** Prefer functions that return values over those that mutate external state.
3. **Small surface area.** Fewer methods = fewer tests needed. Fewer params = simpler test setup.

---

# Reference: Deep Modules

From "A Philosophy of Software Design":

**Deep module** = small interface + lots of implementation.

**Shallow module** = large interface + little implementation (avoid).

When designing interfaces, ask:
- Can I reduce the number of methods?
- Can I simplify the parameters?
- Can I hide more complexity inside?

---

# Reference: Interface Design for Testability

1. **Accept dependencies, don't create them.**
2. **Return results, don't produce side effects.**
3. **Small surface area** — fewer methods = fewer tests needed, fewer params = simpler test setup.

---

# Refactor Candidates

After TDD cycle, look for:
- **Duplication** — Extract function/class
- **Long methods** — Break into private helpers (keep tests on public interface)
- **Shallow modules** — Combine or deepen
- **Feature envy** — Move logic to where data lives
- **Primitive obsession** — Introduce value objects
- **Existing code** the new code reveals as problematic
