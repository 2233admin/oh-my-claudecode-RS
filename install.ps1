# omc-hud installer — Windows (PowerShell)
# Usage: irm https://raw.githubusercontent.com/2233admin/oh-my-claudecode-RS/master/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo    = "2233admin/oh-my-claudecode-RS"
$Bin     = "omc-hud.exe"
$InstDir = "$env:USERPROFILE\.local\bin"
$Settings = "$env:USERPROFILE\.claude\settings.json"

# Get latest release
Write-Host "Fetching latest release..."
$Release = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest"
$Tag     = $Release.tag_name

if (-not $Tag) {
    Write-Error "Could not fetch latest release. Visit: https://github.com/$Repo/releases"
    exit 1
}

Write-Host "Installing omc-hud $Tag..."

$Url = "https://github.com/$Repo/releases/download/$Tag/omc-hud-windows-x86_64.exe"

# Download
New-Item -ItemType Directory -Force $InstDir | Out-Null
$Dest = Join-Path $InstDir $Bin
Invoke-WebRequest $Url -OutFile $Dest -UseBasicParsing
Write-Host "Installed to $Dest"

# Add to PATH for this session (persistent PATH update below)
$UserPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($UserPath -notlike "*$InstDir*") {
    [Environment]::SetEnvironmentVariable("PATH", "$UserPath;$InstDir", "User")
    $env:PATH += ";$InstDir"
    Write-Host "Added $InstDir to PATH"
}

# Wire into Claude Code settings.json
if (-not (Test-Path $Settings)) {
    New-Item -ItemType Directory -Force (Split-Path $Settings) | Out-Null
    @{statusLine = @{type = "command"; command = $Dest}} | ConvertTo-Json -Depth 5 | Set-Content $Settings
    Write-Host "Created $Settings with statusLine config"
} else {
    $Cfg = Get-Content $Settings -Raw | ConvertFrom-Json
    if ($Cfg.PSObject.Properties["statusLine"]) {
        Write-Host "Note: statusLine already set in $Settings"
        Write-Host "  Update manually: `"command`": `"$Dest`""
    } else {
        $Cfg | Add-Member -MemberType NoteProperty -Name "statusLine" -Value @{type = "command"; command = $Dest}
        $Cfg | ConvertTo-Json -Depth 5 | Set-Content $Settings
        Write-Host "Updated $Settings with statusLine config"
    }
}

Write-Host ""
Write-Host "Done! Restart Claude Code to see the HUD."
