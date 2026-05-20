<#
.SYNOPSIS
  Erzeugt Junctions ~\.claude\skills\ccm-* -> CCM-Repo\skills\ccm-*

.DESCRIPTION
  Idempotent. Behandelt drei Fälle pro Skill:
    1. Junction zeigt schon korrekt -> skip
    2. Junction zeigt woanders hin -> warn + replace (mit -Force)
    3. Reales Verzeichnis (kein Link) -> warn + skip (manuelle Klärung)
  Verwendet Junctions statt Symlinks: keine Admin-Rechte nötig, funktioniert
  nur für Verzeichnisse (Skills sind Verzeichnisse).
#>
param(
    [string]$CcmRepo = "C:\Users\jz\source\ClaudeCodeManager",
    [string]$TargetRoot = (Join-Path $env:USERPROFILE ".claude\skills"),
    [switch]$Force
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path "$CcmRepo\skills")) {
    throw "CCM skills directory not found at $CcmRepo\skills"
}

if (-not (Test-Path $TargetRoot)) {
    New-Item -ItemType Directory -Force -Path $TargetRoot | Out-Null
}

$skills = Get-ChildItem "$CcmRepo\skills" -Directory | Where-Object { $_.Name -like "ccm-*" }
if ($skills.Count -lt 1) {
    throw "No ccm-* skills found under $CcmRepo\skills"
}

$created = @()
$skipped = @()
$replaced = @()
$blocked = @()

foreach ($skill in $skills) {
    $target = Join-Path $TargetRoot $skill.Name
    if (Test-Path $target) {
        $item = Get-Item $target -Force
        # ReparsePoint = symlink ODER junction
        $isReparse = ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0
        if ($isReparse) {
            $currentTarget = $item.Target
            if ($currentTarget -eq $skill.FullName) {
                $skipped += $skill.Name
                continue
            } else {
                if ($Force) {
                    Remove-Item $target -Force -Recurse
                    New-Item -ItemType Junction -Path $target -Target $skill.FullName | Out-Null
                    $replaced += "$($skill.Name) (was: $currentTarget)"
                } else {
                    $blocked += "$($skill.Name) -> $currentTarget (use -Force to replace)"
                }
            }
        } else {
            $blocked += "$($skill.Name) (real directory exists, no link)"
        }
    } else {
        New-Item -ItemType Junction -Path $target -Target $skill.FullName | Out-Null
        $created += $skill.Name
    }
}

Write-Host ""
Write-Host "=== CCM Skill-Junction Setup Report ===" -ForegroundColor Cyan
Write-Host "Target: $TargetRoot"
Write-Host "Source: $CcmRepo\skills"
Write-Host ""
Write-Host "Created: $($created.Count)" -ForegroundColor Green
$created | ForEach-Object { Write-Host "  + $_" }
if ($skipped.Count -gt 0) {
    Write-Host "Skipped (already linked): $($skipped.Count)" -ForegroundColor Gray
    $skipped | ForEach-Object { Write-Host "  = $_" }
}
if ($replaced.Count -gt 0) {
    Write-Host "Replaced: $($replaced.Count)" -ForegroundColor Yellow
    $replaced | ForEach-Object { Write-Host "  ~ $_" }
}
if ($blocked.Count -gt 0) {
    Write-Host "Blocked: $($blocked.Count)" -ForegroundColor Red
    $blocked | ForEach-Object { Write-Host "  ! $_" }
}
Write-Host ""
if ($blocked.Count -gt 0) {
    exit 1
}
exit 0
