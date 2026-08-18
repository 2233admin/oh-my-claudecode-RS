## 1. Contract Foundation

- [x] 1.1 Add a public randomized unknown-profile CLI conformance test that fails before implementation
- [x] 1.2 Define versioned profile, provenance, validation, probe, setup, and doctor data contracts
- [x] 1.3 Define Runtime, Provider, Model Descriptor, Protocol, permission mapping, and dependency schema types
- [x] 1.4 Add profile-schema fixtures and same-major breaking-change tests

## 2. Profile Lifecycle

- [x] 2.1 Implement deterministic explicit/project/user/organization/built-in profile resolution with digest provenance
- [x] 2.2 Implement semantic profile validation with typed path-aware errors and secret-value rejection
- [x] 2.3 Implement read-only MCP stdio probing with bounded timeout and evidence capture
- [x] 2.4 Implement idempotent generic setup with print-only mode, atomic writes, sibling preservation, conflict rejection, force backup, JSON/YAML/TOML output
- [x] 2.5 Implement generic doctor using resolved configuration, dependency evidence, permission evidence, and a real MCP handshake
- [x] 2.6 Expose thin `profile init|list|validate`, `probe`, `setup --profile`, and `doctor --profile` CLI commands with stable JSON envelopes
- [x] 2.7 Make the randomized unknown-profile conformance test pass without repository-known identifiers or recompilation

## 3. Built-in Migration

- [x] 3.1 Add built-in Generic MCP and Hermes profiles and migrate Hermes setup/doctor to the common lifecycle
- [x] 3.2 Add built-in Claude and Codex profiles while retaining native hook adapters and compatibility command forms
- [ ] 3.3 Remove duplicated setup/doctor dispatch paths after compatibility and parity tests pass
- [ ] 3.4 Run every available built-in profile through the common conformance harness

## 4. Capability and Permission Negotiation

- [x] 4.1 Implement declared/cataloged/probed evidence provenance and safe-intersection capability resolution
- [x] 4.2 Implement stable OMC permissions and Runtime-native mapping with fail-closed unknown handling
- [x] 4.3 Implement effective permission intersection and lower-trust override tightening rules
- [x] 4.4 Add unprobeable-model declared evidence and conservative first-use verification state
- [ ] 4.5 Add security tests for unknown permits, escalation attempts, secrets, malformed endpoints, and side-effect denial

## 5. Protocol Adapters

- [x] 5.1 Extract the existing MCP stdio transport behind the Protocol Adapter seam without behavior drift
- [x] 5.2 Implement MCP HTTP/SSE probing and lifecycle parity
- [ ] 5.3 Add explicitly authorized bounded subprocess Adapter sessions with capacity, TTL, health, failure eviction, close, and reuse diagnostics
- [ ] 5.4 Add protocol conformance tests proving equivalent capability discovery across stdio and HTTP/SSE

## 6. Trusted Catalogs and Dependencies

- [ ] 6.1 Move bundled profile/provider/model/dependency metadata into versioned offline catalog files
- [ ] 6.2 Implement catalog validation, compatible OMC/profile ranges, provenance, and schema breaking-change gates
- [ ] 6.3 Implement explicit trusted-source configuration plus signature and digest verification
- [ ] 6.4 Implement staged atomic activation, active-pointer recovery, and rollback
- [ ] 6.5 Implement `catalog status|refresh|rollback` and `dependencies status|refresh` with stable JSON output
- [ ] 6.6 Add offline, corrupt download, digest mismatch, incompatible range, executable-directive, activation interruption, and rollback tests

## 7. Discoverability, Performance, and Operations

- [ ] 7.1 Extend unified `omc status` with active profile, provider/model/protocol evidence, effective permissions, dependencies, catalog version, and provenance
- [ ] 7.2 Add cold profile resolution/validation and warm probe/discovery benchmarks with CI budgets
- [ ] 7.3 Add profile/catalog doctor diagnostics and actionable repair guidance without automatic executable installation
- [ ] 7.4 Document custom profile authoring, private catalogs, permission mapping, migration, security, update, and rollback
- [ ] 7.5 Update schema manifests, release notes, host-consumer fixtures, and compatibility documentation

## 8. Release Closure

- [ ] 8.1 Run formatting, clippy, workspace tests, schema gates, security scans, and three-platform release builds
- [ ] 8.2 Publish `v0.3.0-rc.1` with Linux/macOS/Windows Universal Runtime bundles and SHA-256 checksums
- [ ] 8.3 Download RC assets and validate randomized unknown profile, Hermes, Claude/Codex compatibility, permissions, catalog update/rollback, and performance
- [ ] 8.4 Fix RC findings, rerun all gates, and confirm no unresolved High findings or behavior drift
- [ ] 8.5 Publish `v0.3.0`, download final assets, verify digests and real consumer behavior, and archive the OpenSpec change
