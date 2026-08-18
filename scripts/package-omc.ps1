[CmdletBinding()]
param(
    [string]$ReleaseDirectory,
    [string]$PackageDirectory,
    [switch]$Build
)

$ErrorActionPreference = "Stop"

$ScriptDirectory = $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($ScriptDirectory)) {
    $ScriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
}
if ([string]::IsNullOrWhiteSpace($ReleaseDirectory)) {
    $ReleaseDirectory = Join-Path $ScriptDirectory "..\target\release"
}
if ([string]::IsNullOrWhiteSpace($PackageDirectory)) {
    $PackageDirectory = Join-Path $ScriptDirectory "..\target\omc-package"
}

function Get-WorkspaceVersion {
    $metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
    $cli = $metadata.packages | Where-Object { $_.name -eq "omc-cli" } | Select-Object -First 1
    if ($null -eq $cli) {
        throw "workspace package omc-cli was not found"
    }
    return $cli.version
}

function Get-RequiredBinary([string]$name) {
    $path = Join-Path $ReleaseDirectory "$name.exe"
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing release binary: $path (run cargo build --release -p omc-cli -p omc-mcp -p omc-team)"
    }
    return (Get-Item -LiteralPath $path)
}

if ($Build) {
    cargo build --release -p omc-cli -p omc-mcp -p omc-team
    if ($LASTEXITCODE -ne 0) {
        throw "release build failed with exit code $LASTEXITCODE"
    }
}

$binaries = @(
    Get-RequiredBinary "omc"
    Get-RequiredBinary "omc-mcp"
    Get-RequiredBinary "omc-team"
)

New-Item -ItemType Directory -Path $PackageDirectory -Force | Out-Null
foreach ($binary in $binaries) {
    Copy-Item -LiteralPath $binary.FullName -Destination (Join-Path $PackageDirectory $binary.Name) -Force
}

$checksums = [ordered]@{}
foreach ($binary in $binaries) {
    $copied = Join-Path $PackageDirectory $binary.Name
    $checksums[$binary.Name] = (Get-FileHash -LiteralPath $copied -Algorithm SHA256).Hash.ToLowerInvariant()
}

$manifest = [ordered]@{
    schemaVersion = "omc.release-bundle.v1"
    product = "OMC-RS"
    version = Get-WorkspaceVersion
    entrypoint = "omc.exe"
    binaries = @("omc.exe", "omc-mcp.exe", "omc-team.exe")
    notes = @(
        "omc.exe is the unified CLI and embedded MCP entrypoint."
        "omc team delegates to the adjacent omc-team.exe runtime."
        "omc-mcp.exe is retained as a compatibility MCP entrypoint."
    )
    sha256 = $checksums
}
$manifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $PackageDirectory "omc-bundle.json") -Encoding utf8

$versionOutput = & (Join-Path $PackageDirectory "omc.exe") --version 2>&1
if ($LASTEXITCODE -ne 0) {
    throw "packaged omc.exe failed --version: $versionOutput"
}

$capabilityOutput = & (Join-Path $PackageDirectory "omc.exe") tool capabilities --request-id package-smoke 2>&1
if ($LASTEXITCODE -ne 0) {
    throw "packaged omc.exe failed capability smoke: $capabilityOutput"
}
$capabilityResponse = $capabilityOutput | ConvertFrom-Json
if ($capabilityResponse.ok -ne $true -or $null -eq $capabilityResponse.data.capabilities) {
    throw "packaged capability smoke returned an invalid omc.tool.v1 response"
}

[ordered]@{
    packageDirectory = (Resolve-Path -LiteralPath $PackageDirectory).Path
    version = $manifest.version
    binaries = $manifest.binaries
    capabilityCount = @($capabilityResponse.data.capabilities).Count
} | ConvertTo-Json -Depth 4
