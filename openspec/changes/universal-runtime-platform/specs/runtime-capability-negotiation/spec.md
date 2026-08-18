## ADDED Requirements

### Requirement: Evidence-based capabilities
The system SHALL distinguish declared, cataloged, and probed evidence and SHALL compute effective capabilities from their safe intersection.

#### Scenario: Probe contradicts declaration
- **WHEN** a profile declares tool calling but the live probe demonstrates it is unavailable
- **THEN** effective tool calling is unavailable and the evidence conflict is reported

### Requirement: Unprobeable model evidence
The system SHALL allow explicit capabilities for an unprobeable model, mark them as declared, and require conservative policy or successful first-use verification for dangerous behavior.

#### Scenario: Private model has no discovery endpoint
- **WHEN** a private model declares reasoning and tool calling but cannot be probed
- **THEN** status preserves declared provenance and does not report the capabilities as probed facts

### Requirement: Fail-closed permission mapping
The system SHALL map Runtime-native permits to stable OMC permissions and SHALL deny unknown or unmapped permissions.

#### Scenario: Unknown native permit
- **WHEN** a profile requests a native permit with no valid OMC mapping
- **THEN** validation or negotiation rejects it without granting authority

### Requirement: Permission intersection
The system SHALL calculate effective permissions as the intersection of OMC policy and Runtime policy.

#### Scenario: Runtime denies process spawn
- **WHEN** OMC policy allows process-spawn but Runtime policy denies it
- **THEN** effective process-spawn is denied

### Requirement: Dependency evidence
The system SHALL report required, optional, alternative, caller-supplied, and platform-specific dependencies with detected and required versions.

#### Scenario: Alternative Python command
- **WHEN** a capability accepts python3 or python and only one compatible command is available
- **THEN** the dependency is ready and reports the selected alternative and version evidence

### Requirement: Bounded process Adapter lifecycle
The system SHALL bound subprocess Adapter capacity and idle lifetime, health-check reuse, evict failures, support explicit close, and expose reuse diagnostics.

#### Scenario: Reused Adapter fails
- **WHEN** a pooled Adapter fails a request or transport check
- **THEN** it is removed and the next request opens a fresh bounded session

### Requirement: Supported initial protocols
The first formal release SHALL support MCP stdio and MCP HTTP/SSE through Protocol Adapters.

#### Scenario: Equivalent discovery across transports
- **WHEN** equivalent stdio and HTTP/SSE runtimes expose the same MCP tools
- **THEN** OMC reports equivalent effective tool capabilities with transport-specific provenance
