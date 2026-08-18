# Universal Runtime Profiles

OMC 0.3 treats an Agent runtime, provider, model, and tool protocol as four
independent, open identifiers. A valid data-only profile can therefore connect
a runtime that did not exist when OMC was compiled.

## Author a profile

Generate a template and edit its identifiers and runtime command:

```bash
omc profile init --id my-agent --output .omc/profiles/my-agent.json
omc profile validate --profile .omc/profiles/my-agent.json --json
omc probe --profile .omc/profiles/my-agent.json --json
omc setup --profile .omc/profiles/my-agent.json --print-only --json
omc setup --profile .omc/profiles/my-agent.json --json
omc doctor --profile .omc/profiles/my-agent.json --json
```

Set `OMC_PROFILE` to the profile path to include its resolved Runtime,
Provider, Model, Protocol, permissions, dependencies, catalog version, digest,
and provenance in `omc status --json`.

Resolution precedence is explicit path, project `.omc/profiles`, user profile
directory, organization catalog, then built-in. Unknown core fields and secret
values are rejected. Vendor additions belong under namespaced `extensions`
keys such as `example.com/region`.

## Permissions and security

Profiles map runtime-native permits to OMC's stable vocabulary: `read`,
`workspace-write`, `process-spawn`, `network`, `debug-control`,
`credential-access`, and `dangerous`. Effective authority is the intersection
of OMC and runtime policy. Unknown mappings fail closed; a lower-trust profile
may tighten but never expand authority.

Profiles and catalogs are metadata, not installers. OMC never executes catalog
install directives or loads in-process dynamic libraries. A process Adapter is
allowed only when explicitly configured and remains capacity/TTL bounded.
Credentials must stay in the runtime credential store or environment and must
not be embedded in profiles, endpoints, or catalogs.

## Private catalogs, updates, and rollback

OMC ships `catalogs/bundled-v1.json` for offline use. Inspect the active source:

```bash
omc catalog status
omc dependencies status
```

Refreshing a private catalog requires the source path, the independently
trusted canonical path, and an operator-supplied trust key. OMC verifies the
keyed signature, digest, schema, OMC/profile compatibility ranges, and
same-major compatibility before atomic activation:

```bash
omc catalog refresh --source ./catalog.signed.json \
  --trusted-source ./catalog.signed.json --trust-key "$OMC_CATALOG_TRUST_KEY"
omc catalog rollback
```

Use `OMC_CATALOG_HOME` to isolate catalog state. Activation retains the
previous catalog; rollback atomically swaps it back. A failed download,
verification, compatibility check, or activation leaves the current catalog
usable.

## Migration from host-specific setup

The compatibility commands remain valid:

```bash
omc setup --host claude
omc setup --host codex
omc setup --host hermes
```

They resolve the built-in `claude`, `codex`, and `hermes` profiles through the
same setup/doctor lifecycle as custom profiles. Native hooks stay small host
adapters; MCP setup, probing, diagnostics, dependencies, and provenance are
universal. Prefer an explicit profile for new or private Agents.

## Contract and performance gates

Profile, validation, probe, setup, doctor, catalog, dependency, and status JSON
envelopes carry versioned schema identifiers. Same-major releases cannot remove
fields, add required fields, narrow enums/bounds, change types, or expand
permissions. Major changes require an explicit migration.

Release consumers can run:

```bash
python tests/host-consumer/consumer.py --omc target/release/omc
python tests/host-consumer/profile_benchmark.py --omc target/release/omc
```

The benchmark gates cold resolution/validation P95 at 250 ms and warm
in-process discovery P95 at 50 ms by default; both budgets are configurable.
