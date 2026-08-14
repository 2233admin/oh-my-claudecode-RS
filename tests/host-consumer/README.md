# OMC-RS host consumer fixture

This fixture is intentionally not a Rust crate and imports no OMC-RS module.
It consumes only the public JSON envelope through both supported transports:

- CLI: `omc tool capabilities` plus a machine-readable side-effect failure;
- CLI/MCP: the read-only `team_observability` projection of existing team state;
- CLI/MCP: `code_intel_artifact_query` against a committed, current Code Intel run;
- MCP stdio: `initialize`, `tools/list`, and `tools/call`.

Run it against a release build from the repository root:

```bash
python tests/host-consumer/consumer.py --omc target/release/omc
```

On Windows:

```powershell
python tests/host-consumer/consumer.py --omc .\target\release\omc.exe
```

To include the real Code Intel check, provide its published artifact root. The
fixture then requires a committed/current `code_evidence.agent_slice` result on
both CLI and MCP:

```powershell
$env:CODE_INTEL_ARTIFACT_ROOT = "C:\path\to\code-intel\artifacts"
python tests/host-consumer/consumer.py `
  --omc .\target\release\omc.exe `
  --artifact-root $env:CODE_INTEL_ARTIFACT_ROOT `
  --repo omc-rs-src `
  --repo-path (Get-Location)
```

Hermes, Sentinel, or another host can copy the JSON parsing boundary from this
fixture without depending on OMC-RS Rust types. A successful run proves only
transport and contract consumption; it does not claim that a host's own agent
loop or private credentials are available.
