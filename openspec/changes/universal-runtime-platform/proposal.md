## Why

OMC-RS has a host-neutral MCP core but still requires hard-coded host branches for setup, diagnosis, permissions, and dependencies. Unknown and future Agent runtimes, providers, models, and protocols must be able to integrate without changing or rebuilding OMC-RS, otherwise the platform will harden around today's Claude/Codex/Hermes combinations.

## What Changes

- Add a versioned, composable profile protocol for Runtime, Provider, Model, Tool Protocol, permissions, and dependencies.
- Add one public lifecycle for profile resolution, validation, read-only probing, idempotent setup, and real doctor diagnostics.
- Convert Claude, Codex, Hermes, and Generic MCP into built-in profiles while retaining native hook adapters and compatible command forms.
- Add capability negotiation that treats declarations as expectations, catalogs as baselines, probes as current evidence, and effective permissions as the intersection of OMC and runtime policy.
- Add independently updateable, integrity-checked catalogs with an offline bundled baseline, compatibility gates, atomic activation, and rollback.
- Add bounded subprocess protocol adapters for cases not expressible as data-only profiles; in-process dynamic libraries remain prohibited.
- Add cross-platform conformance, security, compatibility, performance, packaging, and release-asset gates.
- **BREAKING**: machine-readable profile, probe, doctor, permission, and catalog contracts become explicitly versioned; incompatible future major versions require migration instead of silent interpretation.

## Capabilities

### New Capabilities

- `universal-profile-lifecycle`: Resolve, validate, probe, install, diagnose, and report provenance for built-in and repository-unknown profiles.
- `runtime-capability-negotiation`: Compose Runtime, Provider, Model, Protocol, permission, and dependency evidence into a fail-closed effective capability view.
- `trusted-catalog-updates`: Update profile/provider/model/dependency metadata independently with integrity, compatibility, offline, activation, and rollback guarantees.

### Modified Capabilities

<!-- No existing OpenSpec capabilities; this repository is adopting OpenSpec with this change. -->

## Impact

- Host setup/doctor dispatch and MCP registration move behind the universal profile lifecycle.
- Existing Claude, Codex, and Hermes behavior is preserved through built-in profiles and compatibility aliases.
- Capability and dependency discovery move from hard-coded catalog logic to versioned data with a bundled fallback.
- CLI gains profile/catalog lifecycle commands and stable JSON envelopes.
- Release validation gains a randomized unknown-profile consumer, permission rejection tests, schema gates, performance budgets, and downloaded-asset verification on Linux, macOS, and Windows.
- Public documentation and release artifacts describe Generic MCP and custom profile integration as the primary extension path.
