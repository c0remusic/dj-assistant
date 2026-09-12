<#
.SYNOPSIS
  Backup/swap/restore/status helper for M8 Rekordbox master.db spikes.

.DESCRIPTION
  Consolidates a pattern hand-written for every M8 spike since Evaluation 5
  (docs/ressources-externes.md): back up the real Pioneer folder, swap in a
  test copy, then restore -- always verified by SHA256, never taken on
  faith. This script NEVER writes to master.db's content: it only copies
  whole files. Any content mutation (tags, paths, flags) stays a separate
  script (Python/pyrekordbox or the Rust engine), run against the copy
  BEFORE `swap`.

  Refuses `swap`/`restore` if Rekordbox is running (same safety invariant
  as the Rust engine). `backup` records the backup path in a local state
  file (not in the repo) so `restore` finds it automatically without
  retyping a timestamped path.

.PARAMETER Action
  backup | swap | restore | status

.PARAMETER CopyDir
  Required for `swap`: folder containing the modified master.db +
  masterPlaylists6.xml to install into the live Pioneer folder.

.PARAMETER BackupDir
  Optional. For `backup`: where to write (defaults to a timestamped folder
  on the Desktop). For `restore`: which backup to restore (defaults to the
  last one this script created, found via the state file).

.EXAMPLE
  .\rekordbox-spike-helper.ps1 -Action backup
  .\rekordbox-spike-helper.ps1 -Action swap -CopyDir "C:\Users\LEETJ\Desktop\sift-m8-spike\copy"
  .\rekordbox-spike-helper.ps1 -Action restore
  .\rekordbox-spike-helper.ps1 -Action status
#>
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("backup", "swap", "restore", "status")]
    [string]$Action,

    [string]$CopyDir,
    [string]$BackupDir
)

$ErrorActionPreference = "Stop"

$PioneerDir = Join-Path $env:APPDATA "Pioneer\rekordbox"
$LiveDb = Join-Path $PioneerDir "master.db"
$LiveXml = Join-Path $PioneerDir "masterPlaylists6.xml"
$StateFile = Join-Path $env:TEMP "sift-rekordbox-spike-last-backup.txt"

function Test-RekordboxRunning {
    return $null -ne (Get-Process -Name "rekordbox" -ErrorAction SilentlyContinue)
}

function Assert-RekordboxClosed {
    if (Test-RekordboxRunning) {
        Write-Host "REFUSED: Rekordbox is open. Close it before continuing." -ForegroundColor Red
        exit 1
    }
}

function Get-Sha256Short($path) {
    (Get-FileHash -Path $path -Algorithm SHA256).Hash.Substring(0, 16)
}

switch ($Action) {
    "backup" {
        Assert-RekordboxClosed
        if (-not $BackupDir) {
            $stamp = Get-Date -Format "yyyy-MM-dd-HHmmss"
            $BackupDir = Join-Path $HOME "Desktop\rb-backup-$stamp"
        }
        New-Item -ItemType Directory -Force -Path $BackupDir | Out-Null
        Copy-Item $LiveDb (Join-Path $BackupDir "master.db")
        Copy-Item $LiveXml (Join-Path $BackupDir "masterPlaylists6.xml")
        Set-Content -Path $StateFile -Value $BackupDir
        Write-Host "Backup -> $BackupDir" -ForegroundColor Green
        Write-Host "  master.db  sha256[:16]=$(Get-Sha256Short (Join-Path $BackupDir 'master.db'))"
        Write-Host "  xml        sha256[:16]=$(Get-Sha256Short (Join-Path $BackupDir 'masterPlaylists6.xml'))"
    }

    "swap" {
        if (-not $CopyDir) {
            Write-Host "REFUSED: -CopyDir is required for swap." -ForegroundColor Red
            exit 1
        }
        $srcDb = Join-Path $CopyDir "master.db"
        $srcXml = Join-Path $CopyDir "masterPlaylists6.xml"
        if (-not (Test-Path $srcDb) -or -not (Test-Path $srcXml)) {
            Write-Host "REFUSED: $CopyDir must contain both master.db and masterPlaylists6.xml." -ForegroundColor Red
            exit 1
        }
        if (-not (Test-Path $StateFile)) {
            Write-Host "REFUSED: no known backup (state file missing) -- run '-Action backup' first." -ForegroundColor Red
            exit 1
        }
        Assert-RekordboxClosed
        Copy-Item $srcDb $LiveDb -Force
        Copy-Item $srcXml $LiveXml -Force
        Write-Host "Swap done from $CopyDir." -ForegroundColor Green
        Write-Host "Safety backup: $(Get-Content $StateFile)"
    }

    "restore" {
        if (-not $BackupDir) {
            if (-not (Test-Path $StateFile)) {
                Write-Host "REFUSED: no known backup (state file missing) and -BackupDir not given." -ForegroundColor Red
                exit 1
            }
            $BackupDir = Get-Content $StateFile
        }
        $bkDb = Join-Path $BackupDir "master.db"
        $bkXml = Join-Path $BackupDir "masterPlaylists6.xml"
        if (-not (Test-Path $bkDb) -or -not (Test-Path $bkXml)) {
            Write-Host "REFUSED: $BackupDir does not contain both expected files." -ForegroundColor Red
            exit 1
        }
        Assert-RekordboxClosed
        Copy-Item $bkDb $LiveDb -Force
        Copy-Item $bkXml $LiveXml -Force

        $liveDbHash = Get-Sha256Short $LiveDb
        $bkDbHash = Get-Sha256Short $bkDb
        $liveXmlHash = Get-Sha256Short $LiveXml
        $bkXmlHash = Get-Sha256Short $bkXml

        if ($liveDbHash -eq $bkDbHash -and $liveXmlHash -eq $bkXmlHash) {
            Write-Host "Restore verified: live == backup (identical SHA256)." -ForegroundColor Green
        }
        else {
            Write-Host "WARNING: live hash does NOT match backup after restore -- check manually." -ForegroundColor Red
            exit 1
        }
    }

    "status" {
        Write-Host "Rekordbox: $(if (Test-RekordboxRunning) { 'RUNNING' } else { 'closed' })"
        Write-Host "Pioneer folder: $PioneerDir"
        if (Test-Path $LiveDb) {
            Write-Host "  master.db  sha256[:16]=$(Get-Sha256Short $LiveDb)  ($(Get-Item $LiveDb | Select-Object -ExpandProperty LastWriteTime))"
        }
        if (Test-Path $StateFile) {
            $bk = Get-Content $StateFile
            Write-Host "Last known backup: $bk"
            if (Test-Path (Join-Path $bk "master.db")) {
                $bkHash = Get-Sha256Short (Join-Path $bk "master.db")
                $liveHash = Get-Sha256Short $LiveDb
                $match = if ($bkHash -eq $liveHash) { "matches live" } else { "DIFFERS from live" }
                Write-Host "  master.db backup sha256[:16]=$bkHash ($match)"
            }
        }
        else {
            Write-Host "No known backup (state file missing)."
        }
    }
}
