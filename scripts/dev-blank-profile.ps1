<#
.SYNOPSIS
  Launch a Sift dev session on a BLANK profile -- separate app identifier, so
  separate appdata folder and a brand-new empty SQLite database. The real
  profile is never read, moved or written.

.DESCRIPTION
  Answers the "alternative a evaluer" of issue #40: the seven category-(B)
  observations of #15 need a virgin profile, and the written protocol moved the
  real sift.db (plus -wal / -shm) out of the way to get one. Moving 170+ MB of
  live user data is exactly what the repo guard rail forbids doing casually.

  app_data_dir() is derived from the Tauri bundle identifier, so overriding the
  identifier at launch time relocates the whole profile:

      identifier com.sift.app             -> %APPDATA%\com.sift.app        (real, untouched)
      identifier com.sift.dev.blankprofile -> %APPDATA%\com.sift.dev.blankprofile (sandbox)

  The override travels through `tauri dev --config <file>`, which merges a JSON
  fragment over src-tauri/tauri.conf.json. It is NEVER written into
  tauri.conf.json itself: that file ships to users, and a sandbox identifier (or
  a CDP port) baked into it would be distributed.

  Three keys are overridden, and only three:
    identifier             -- separate single-instance mutex + separate appdata/cache
    build.devUrl           -- dedicated Vite port, so a normal `npm run tauri dev`
    build.beforeDevCommand    on 5173 can keep running side by side

  None of them is a path relative to the config file, so the generated fragment
  is safe to keep in the temp folder (externalBin / frontendDist keep resolving
  against src-tauri as usual).

.PARAMETER Action
  run    (default) generate the override then launch `npm run tauri dev`
  config generate the override, print it, exit -- launches nothing
  status report on the sandbox profile and on the real profile, read-only
  clean  remove the sandbox profile folders (dry run unless -Force)

.PARAMETER Identifier
  Sandbox bundle identifier. Must differ from the production identifier read
  from src-tauri/tauri.conf.json; the script refuses to run otherwise.

.PARAMETER Port
  Dedicated Vite port. Default 5273, away from 5173 (normal dev) and from 5219
  (worktree coexistence recipe).

.PARAMETER CdpPort
  Optional. Exposes a CDP endpoint on the WebView2 window for this session only,
  via WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS. Useful for observation B4 (what
  app.js paints before the live wiring installs), which is not replayable once
  the window has settled. 0 = do not instrument.

.PARAMETER Force
  run:   launch even though the sandbox profile is not empty.
  clean: actually delete, instead of listing what would be deleted.

.EXAMPLE
  npm run dev:blank
  npm run dev:blank -- -Action status
  npm run dev:blank -- -Action clean -Force
  npm run dev:blank -- -CdpPort 9433
  .\scripts\dev-blank-profile.ps1 -Action config
#>
param(
    [ValidateSet("run", "config", "status", "clean")]
    [string]$Action = "run",

    [string]$Identifier = "com.sift.dev.blankprofile",

    [int]$Port = 5273,

    [int]$CdpPort = 0,

    [switch]$Force
)

$ErrorActionPreference = "Stop"

# Windows PowerShell 5.1 is the interpreter behind `npm run dev:blank`
# (powershell.exe, not pwsh). No ternary, no '&&', no '??' below, and JSON is
# written through UTF8Encoding($false) because Set-Content -Encoding UTF8 would
# prepend a BOM that a strict JSON parser can choke on.

$RepoRoot = Split-Path -Parent $PSScriptRoot
$MainConf = Join-Path $RepoRoot "src-tauri\tauri.conf.json"
$ConfDir = Join-Path $env:TEMP "sift-blank-profile"
$ConfFile = Join-Path $ConfDir "blank-profile.tauri.conf.json"

function Write-Section($text) {
    Write-Host ""
    Write-Host $text -ForegroundColor Cyan
}

function Get-ProductionIdentifier {
    if (-not (Test-Path $MainConf)) {
        Write-Host "REFUSED: $MainConf not found -- run this from the Sift repo." -ForegroundColor Red
        exit 1
    }
    $conf = Get-Content $MainConf -Raw | ConvertFrom-Json
    if ([string]::IsNullOrWhiteSpace($conf.identifier)) {
        Write-Host "REFUSED: no 'identifier' in $MainConf." -ForegroundColor Red
        exit 1
    }
    return $conf.identifier
}

$ProdIdentifier = Get-ProductionIdentifier

# --- Guard rail: the sandbox identifier may never be the production one -------
# Everything below (including a recursive delete) is keyed on $Identifier. If it
# were allowed to equal the production identifier, `clean` would erase the real
# 170+ MB library. Derived from tauri.conf.json rather than hardcoded, so it
# keeps holding if the production identifier ever changes.
if ($Identifier -eq $ProdIdentifier) {
    Write-Host "REFUSED: -Identifier '$Identifier' is the PRODUCTION identifier." -ForegroundColor Red
    Write-Host "         That is the real profile. Pick a distinct sandbox identifier." -ForegroundColor Red
    exit 1
}
if ($Identifier -notmatch '^[A-Za-z0-9][A-Za-z0-9.-]*$') {
    Write-Host "REFUSED: -Identifier '$Identifier' is not a valid bundle identifier." -ForegroundColor Red
    Write-Host "         Allowed: letters, digits, dot, hyphen. No underscore, no path separator." -ForegroundColor Red
    exit 1
}
if ($Identifier -notlike "*.*") {
    Write-Host "REFUSED: -Identifier '$Identifier' must be reverse-DNS (contain a dot)." -ForegroundColor Red
    exit 1
}

$SandboxData = Join-Path $env:APPDATA $Identifier
$SandboxCache = Join-Path $env:LOCALAPPDATA $Identifier
$RealData = Join-Path $env:APPDATA $ProdIdentifier

function Get-DirState($path) {
    if (-not (Test-Path $path)) {
        return [pscustomobject]@{ Exists = $false; Count = 0; Bytes = 0 }
    }
    $items = @(Get-ChildItem $path -Force -Recurse -File -ErrorAction SilentlyContinue)
    $bytes = 0
    foreach ($i in $items) { $bytes = $bytes + $i.Length }
    return [pscustomobject]@{ Exists = $true; Count = $items.Count; Bytes = $bytes }
}

function Format-Size($bytes) {
    if ($bytes -ge 1073741824) { return "{0:N2} GB" -f ($bytes / 1073741824) }
    if ($bytes -ge 1048576) { return "{0:N1} MB" -f ($bytes / 1048576) }
    if ($bytes -ge 1024) { return "{0:N0} KB" -f ($bytes / 1024) }
    return "$bytes B"
}

function Show-DirState($label, $path) {
    $s = Get-DirState $path
    if ($s.Exists) {
        Write-Host ("  {0,-10} {1}" -f $label, $path)
        Write-Host ("  {0,-10} {1} file(s), {2}" -f "", $s.Count, (Format-Size $s.Bytes))
    }
    else {
        Write-Host ("  {0,-10} {1}" -f $label, $path)
        Write-Host ("  {0,-10} (does not exist yet -- blank)" -f "") -ForegroundColor Green
    }
}

function New-OverrideConfig {
    # Only these three keys. Anything more risks diverging the sandbox session
    # from the build the observations are supposed to describe.
    $cfg = [ordered]@{
        identifier = $Identifier
        build      = [ordered]@{
            devUrl           = "http://localhost:$Port"
            beforeDevCommand = "npm run dev -- --port $Port --strictPort"
        }
    }
    $json = $cfg | ConvertTo-Json -Depth 10
    if (-not (Test-Path $ConfDir)) {
        New-Item -ItemType Directory -Path $ConfDir -Force | Out-Null
    }
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($ConfFile, $json, $utf8NoBom)
    return $json
}

function Show-Profile {
    Write-Section "Blank profile (sandbox)"
    Write-Host ("  identifier " + $Identifier)
    Show-DirState "appdata" $SandboxData
    Show-DirState "cache" $SandboxCache

    Write-Section "Real profile -- NEVER touched by this script"
    Write-Host ("  identifier " + $ProdIdentifier)
    Show-DirState "appdata" $RealData
}

function Show-Cleanup {
    Write-Section "Cleanup when the observations are done"
    Write-Host "  npm run dev:blank -- -Action clean          (dry run, lists what would go)"
    Write-Host "  npm run dev:blank -- -Action clean -Force   (actually deletes)"
    Write-Host ""
    Write-Host "  Or by hand -- these two folders, and nothing else:"
    Write-Host ("    " + $SandboxData)
    Write-Host ("    " + $SandboxCache)
}

switch ($Action) {

    "config" {
        $json = New-OverrideConfig
        Write-Section "Override merged over src-tauri/tauri.conf.json via --config"
        Write-Host ("  file: " + $ConfFile)
        Write-Host ""
        Write-Host $json
        Show-Profile
        exit 0
    }

    "status" {
        Show-Profile
        Show-Cleanup
        exit 0
    }

    "clean" {
        $targets = @($SandboxData, $SandboxCache)
        $present = @()
        foreach ($t in $targets) {
            if (Test-Path $t) { $present = $present + $t }
        }
        if ($present.Count -eq 0) {
            Write-Host "Nothing to clean: the sandbox profile does not exist." -ForegroundColor Green
            Write-Host ("  " + $SandboxData)
            Write-Host ("  " + $SandboxCache)
            exit 0
        }
        Write-Section "Sandbox folders for identifier '$Identifier'"
        foreach ($t in $present) {
            $s = Get-DirState $t
            Write-Host ("  " + $t + "  (" + $s.Count + " file(s), " + (Format-Size $s.Bytes) + ")")
        }
        if (-not $Force) {
            Write-Host ""
            Write-Host "DRY RUN -- nothing deleted. Re-run with -Force to delete." -ForegroundColor Yellow
            exit 0
        }
        foreach ($t in $present) {
            # Re-assert the guard on the literal path about to be removed, not on
            # a variable computed far above: this is the irreversible step.
            $expectedData = Join-Path $env:APPDATA $Identifier
            $expectedCache = Join-Path $env:LOCALAPPDATA $Identifier
            if ($t -ne $expectedData -and $t -ne $expectedCache) {
                Write-Host "REFUSED: '$t' is not a sandbox profile folder." -ForegroundColor Red
                exit 1
            }
            Remove-Item $t -Recurse -Force
            Write-Host ("Deleted " + $t) -ForegroundColor Green
        }
        Write-Host ""
        Write-Host ("Real profile untouched: " + $RealData) -ForegroundColor Green
        exit 0
    }

    "run" {
        $state = Get-DirState $SandboxData
        if ($state.Exists -and $state.Count -gt 0 -and -not $Force) {
            Write-Host "REFUSED: the sandbox profile is NOT blank." -ForegroundColor Red
            Write-Host ("  " + $SandboxData + " holds " + $state.Count + " file(s), " + (Format-Size $state.Bytes)) -ForegroundColor Red
            Write-Host ""
            Write-Host "  The seven observations of #40 only mean something on a virgin profile." -ForegroundColor Yellow
            Write-Host "  Wipe it first:  npm run dev:blank -- -Action clean -Force" -ForegroundColor Yellow
            Write-Host "  Or keep it:     npm run dev:blank -- -Force" -ForegroundColor Yellow
            exit 1
        }

        $json = New-OverrideConfig

        Write-Section "Sift dev -- BLANK PROFILE"
        Write-Host ("  identifier   " + $Identifier + "   (production is " + $ProdIdentifier + ", untouched)")
        Write-Host ("  appdata      " + $SandboxData)
        Write-Host ("  cache        " + $SandboxCache)
        Write-Host ("  database     " + (Join-Path $SandboxData "sift.db") + "   (created empty on first launch)")
        Write-Host ("  vite port    " + $Port)
        if ($CdpPort -gt 0) {
            Write-Host ("  cdp port     " + $CdpPort + "   (this session only, never in tauri.conf.json)")
        }
        else {
            Write-Host "  cdp port     none   (add -CdpPort 9433 to instrument, useful for B4)"
        }
        Write-Host ("  override     " + $ConfFile)

        Write-Section "Override JSON"
        Write-Host $json

        Write-Section "Real profile -- NEVER touched by this script"
        Write-Host ("  " + $RealData)
        $realState = Get-DirState $RealData
        if ($realState.Exists) {
            Write-Host ("  " + $realState.Count + " file(s), " + (Format-Size $realState.Bytes) + " -- read once for this line, never moved or written")
        }

        Show-Cleanup

        Write-Section "Notes"
        Write-Host "  - Changing the identifier recompiles the final crate (~1 min): the"
        Write-Host "    config is baked into the build. Later launches are fast."
        Write-Host '  - A normal "npm run tauri dev" on 5173 can keep running: different'
        Write-Host "    identifier means a different single-instance mutex."
        Write-Host "  - Observation B4 (what app.js paints before the live wiring) happens in"
        Write-Host "    the first second and is not replayable. Watch the window from the"
        Write-Host "    moment it appears, or launch with -CdpPort."

        if ($CdpPort -gt 0) {
            $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$CdpPort"
        }

        Write-Section "Launching -- Ctrl+C stops the session"
        Push-Location $RepoRoot
        try {
            # npm invoked directly, not through `cmd /c`: the wrapper does not
            # attach to a long-lived `tauri dev` and exits without running it.
            & npm run tauri dev -- --config $ConfFile
            $code = $LASTEXITCODE
        }
        finally {
            Pop-Location
        }

        Write-Section "Session over"
        Write-Host ("  The sandbox profile is still on disk: " + $SandboxData)
        Write-Host "  Clean it with:  npm run dev:blank -- -Action clean -Force"
        exit $code
    }
}
