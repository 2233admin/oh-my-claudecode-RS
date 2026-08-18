## Context

OMC-RS already exposes 16 host-neutral capabilities and 32 MCP tools, but the integration surface is split: `HostKind` is a closed Claude/Codex enum, Hermes setup is a CLI special case, doctor cannot diagnose Hermes, dependency discovery is compiled into Rust, and permissions have no universal negotiation contract. Runtime, Provider, Model, and Tool Protocol are currently easy to conflate even though their combinations are unbounded.

The stakeholders are end users with public or private Agents, Agent-runtime authors, operators with custom permits, provider/model operators, and OMC-RS maintainers. The design must preserve current users, work offline, fail closed, remain low latency, and avoid turning OMC into another scheduler or an implicit remote-code loader.

## Goals / Non-Goals

**Goals:**

- Allow a repository-unknown Runtime, Provider, Model, and permission vocabulary to integrate without Rust changes or recompilation.
- Provide one deep profile module whose public interface resolves, validates, probes, installs, and diagnoses profiles.
- Support MCP stdio and MCP HTTP/SSE in the first formal release while leaving a narrow Adapter seam for future protocols.
- Make capability and permission decisions from declared, cataloged, and probed evidence with explicit provenance.
- Make profile/provider/model/dependency catalogs independently updateable, integrity checked, compatible, atomic, reversible, and optional offline.
- Migrate built-in integrations without maintaining parallel setup/doctor paths.
- Release through `v0.3.0-rc.1` conformance and then `v0.3.0` after real asset consumption.

**Non-Goals:**

- Hosting model inference or making OMC the default model gateway.
- Replacing an Agent runtime's loop, scheduler, native permission authority, or credential store.
- Loading in-process dynamic libraries from profiles or catalogs.
- Automatically executing downloaded installers or metadata.
- Claiming capabilities that an external runtime, model, provider, or protocol cannot supply.

## Decisions

### One deep profile lifecycle module

The external seam is a single lifecycle interface: resolve, validate, probe, setup, and doctor a `ProfileRef`. CLI commands and tests cross this seam. Parsing formats, precedence, transport selection, dependency checks, permission mapping, atomic writes, and provenance stay behind it.

Alternative rejected: adding more `HostKind` variants and CLI branches. This multiplies setup, doctor, and compatibility logic and cannot represent unknown integrations.

### Four independent integration dimensions

A profile composes Runtime, Provider, Model Descriptor, and Protocol Adapter. Their identifiers are open strings scoped by schema and provenance, not Rust enums. Native hook semantics may retain small compile-time adapters, but ordinary tool consumers never enter the native-host enum.

Alternative rejected: a single host/provider/model enum. Its Cartesian growth makes every new combination a release event.

### Data-first extensions with isolated process adapters

Profiles are declarative. When a protocol cannot be expressed as data, a profile may reference an explicitly authorized subprocess Adapter. It uses the shared bounded session pattern: capacity, TTL, health check, failure eviction, explicit close, and observable reuse. In-process dynamic libraries are forbidden.

### Protocol scope

The first formal release supports MCP stdio and MCP HTTP/SSE. OpenAI-compatible is used for Provider/Model probing, not treated as an OMC tool transport. A future model gateway can implement a separate Adapter without changing the profile lifecycle interface.

### Evidence and capability negotiation

Profile declarations are expectations, signed/bundled catalog entries are baselines, and runtime probes are current evidence. Effective capabilities are the safe intersection. When a model cannot be probed, declared capability evidence remains tagged `declared`; dangerous behavior requires conservative policy or successful first-use verification.

### Permission intersection

OMC defines stable permissions: read, workspace-write, process-spawn, network, debug-control, credential-access, and dangerous. A profile maps native permits into this vocabulary. Effective permission is the intersection of OMC policy and Runtime policy; either side can deny. Unknown or unmapped permissions fail closed. Lower-trust profile precedence may only tighten permissions.

### Resolution and provenance

Profile precedence is explicit path > project > user > organization catalog > built-in. Every resolved view includes source, schema version, catalog version where applicable, and digest. Duplicate ambiguity at one precedence level is an error.

### Schema evolution

All profile, probe, doctor, permission, and catalog envelopes are versioned. Within a major version, declared extension fields may be preserved and ignored; unknown core fields are errors. Major-version changes require explicit migration. Compatibility gates reject removed fields, new required fields, type changes, narrowed enums, tightened bounds, and permission expansion without a major version.

### Trusted catalogs

OMC ships an offline baseline. Catalog refresh is explicit or policy controlled, verifies signature and digest, validates schema and compatible OMC range, stages content, atomically activates it, and retains rollback state. Private catalogs require explicit trust configuration. Catalog data never executes code.

### Built-in migration

Claude, Codex, Hermes, and Generic MCP become built-in profiles. Existing setup command forms resolve to those profiles. Native hook adapters remain only where hook/event semantics truly vary. Setup, probe, doctor, dependency reporting, and MCP registration use the universal lifecycle; duplicate paths are removed once conformance passes.

### Release proof

The release gate generates randomized identifiers absent from the repository and drives a compiled OMC binary through validate → probe → setup → doctor → MCP initialize/tools-list/tool-call → permission decision. It also exercises built-in profiles, offline catalogs, update/rollback, schema break detection, security rejection, latency budgets, three-platform packages, and post-release asset download.

## Risks / Trade-offs

- [Profiles become an unsafe execution mechanism] → Keep profiles declarative; isolate process Adapters behind explicit authorization and fail-closed permissions.
- [Declared model capability is inaccurate] → Preserve evidence provenance, prefer probes, and require conservative or first-use verification for dangerous behavior.
- [Catalog compromise changes behavior] → Require signature, digest, schema/range validation, staged atomic activation, provenance, and rollback.
- [Generic abstraction weakens native integrations] → Retain narrow native hook adapters while unifying only the lifecycle behavior common to every integration.
- [Two systems survive migration] → Add parity/conformance gates and delete old setup/doctor branches as each built-in profile migrates.
- [Universal doctor becomes slow] → Separate cached static validation from bounded live probes and publish cold/warm budgets.
- [Scope grows into a model gateway] → Keep inference forwarding outside the initial module; preserve only an Adapter seam.

## Migration Plan

1. Add profile schemas, typed envelopes, resolution/provenance, and randomized unknown-profile contract tests.
2. Implement MCP stdio lifecycle vertically, including validation, probe, setup, doctor, permission negotiation, and real consumer proof.
3. Migrate Hermes first to remove the existing setup/doctor inconsistency, then Claude/Codex, retaining compatibility aliases and native hooks.
4. Add MCP HTTP/SSE and bounded subprocess Adapter lifecycle.
5. Move dependency/provider/model/profile metadata to bundled catalogs; add signed refresh, activation, rollback, and compatibility gates.
6. Add status/CLI information architecture, documentation, security tests, and performance budgets.
7. Publish `v0.3.0-rc.1`, consume its assets from unknown and built-in fixtures, then publish `v0.3.0`.

Rollback keeps the previous binary, bundled catalog, active catalog pointer, and adjacent configuration backups. Existing compatibility aliases remain valid throughout v0.3.

## Open Questions

None. The Wayfinder decisions through the first two frontiers are incorporated here; newly discovered implementation conflicts must update this design before code guesses around them.
