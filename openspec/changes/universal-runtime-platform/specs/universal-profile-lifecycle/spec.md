## ADDED Requirements

### Requirement: Repository-unknown profile lifecycle
The system SHALL accept a valid profile whose Runtime, Provider, and Model identifiers are absent from the OMC-RS source and SHALL complete validation, read-only probing, setup, doctor, and real tool consumption without source changes or recompilation.

#### Scenario: Unknown profile completes the lifecycle
- **WHEN** a user supplies a valid profile and conforming fixture Runtime with randomized identifiers
- **THEN** the compiled OMC binary completes validate, probe, setup, doctor, MCP initialize, tools/list, and a tool call

### Requirement: Composable profile dimensions
The system SHALL model Runtime, Provider, Model Descriptor, and Tool Protocol as independent profile dimensions with open identifiers.

#### Scenario: Known provider with unknown runtime
- **WHEN** an unknown Runtime is combined with a known Provider and a conforming protocol
- **THEN** the combination is evaluated by capability rather than rejected by identifier

### Requirement: Deterministic resolution and provenance
The system SHALL resolve profiles in explicit path, project, user, organization catalog, then built-in order and SHALL report the selected source, schema version, and digest.

#### Scenario: Project profile overrides built-in
- **WHEN** a project profile and built-in profile use the same identifier
- **THEN** the project profile is selected and its provenance is reported

### Requirement: Safe setup
The system SHALL make setup idempotent, preserve unrelated configuration, write atomically, back up replacements, and reject conflicts unless replacement is explicitly authorized.

#### Scenario: Repeated setup
- **WHEN** setup runs twice with the same resolved profile
- **THEN** the second run performs no configuration mutation and reports the existing registration

### Requirement: Real doctor diagnosis
The system SHALL diagnose the installed configuration and perform a bounded live protocol handshake where supported instead of treating file presence as readiness.

#### Scenario: Configured process cannot handshake
- **WHEN** configuration exists but the Runtime exits before protocol initialization
- **THEN** doctor reports a typed not-ready result with dependency and transport evidence

### Requirement: Built-in compatibility profiles
The system SHALL expose Claude, Codex, Hermes, and Generic MCP through the same lifecycle while retaining compatible existing command forms.

#### Scenario: Hermes setup and doctor parity
- **WHEN** Hermes is selected through its built-in profile
- **THEN** setup and doctor resolve the same profile and agree on configuration readiness

### Requirement: Versioned machine contracts
The system SHALL version profile, validation, probe, setup, and doctor envelopes and SHALL reject incompatible major versions.

#### Scenario: Unsupported major profile
- **WHEN** a profile uses an unsupported schema major version
- **THEN** validation fails with a typed migration-required error before side effects
