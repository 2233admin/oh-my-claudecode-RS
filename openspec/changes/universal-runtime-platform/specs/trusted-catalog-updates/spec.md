## ADDED Requirements

### Requirement: Offline bundled baseline
The system SHALL ship bundled profile, provider, model, and dependency catalogs that remain usable without network access.

#### Scenario: Refresh endpoint unavailable
- **WHEN** the network is unavailable and no external catalog is active
- **THEN** resolution and diagnosis use the bundled baseline without false failure

### Requirement: Integrity-checked refresh
The system SHALL verify catalog signature, digest, schema, and compatible OMC range before staging an update.

#### Scenario: Catalog digest mismatch
- **WHEN** downloaded catalog bytes do not match the signed digest
- **THEN** refresh rejects the update and preserves the active catalog

### Requirement: Atomic activation and rollback
The system SHALL activate a validated catalog atomically and retain sufficient state to restore the previous active catalog.

#### Scenario: Roll back active catalog
- **WHEN** an operator requests rollback after a successful activation
- **THEN** the previous catalog becomes active atomically and provenance reflects the rollback

### Requirement: Explicit private trust
The system SHALL require explicit trust configuration before consuming a private catalog source.

#### Scenario: Untrusted private source
- **WHEN** a profile references a private catalog that has not been trusted
- **THEN** resolution rejects it without downloading or activating metadata

### Requirement: Metadata cannot execute
The system SHALL treat catalog content as data and SHALL NOT execute installers, scripts, dynamic libraries, or embedded commands during refresh or validation.

#### Scenario: Catalog contains executable directive
- **WHEN** a catalog contains an executable directive outside the supported declarative schema
- **THEN** schema validation rejects the catalog

### Requirement: Catalog compatibility gate
The system SHALL reject removal, new required fields, type changes, narrowed enums, tightened bounds, or permission expansion within a compatible schema major version.

#### Scenario: Update adds required profile field
- **WHEN** a same-major catalog schema update makes an optional profile field required
- **THEN** the breaking-change gate rejects publication or activation

### Requirement: Independently versioned updates
The system SHALL allow catalogs to release independently while declaring compatible OMC and profile-schema ranges.

#### Scenario: New catalog requires newer OMC
- **WHEN** a catalog's compatible OMC range excludes the running binary
- **THEN** refresh reports an upgrade requirement and keeps the current catalog active
