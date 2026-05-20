<#
.SYNOPSIS
  Verifiziert, dass alle ccm-* Junctions korrekt auf das CCM-Repo zeigen
  UND dass SKILL.md über den Link lesbar ist.
.NOTES
  Exit 0 bei vollständigem Setup, Exit 1 bei jedem Fehlschlag.
#>
param(
    [string]$CcmRepo = "C:\Users\jz\source\ClaudeCodeManager",
    [string]$TargetRoot = (Join-Path $env:USERPROFILE ".claude\skills")
)

$ErrorActionPreference = "Stop"

$expected = Get-ChildItem "$CcmRepo\skills" -Directory | Where-Object { $_.Name -like "ccm-*" } | Select-Object -ExpandProperty Name
if ($expected.Count -lt 1) {
    Write-Error "No ccm-* skills found under $CcmRepo\skills"
    exit 1
}

$ok = @()
$missing = @()
$wrong = @()
$unreadable = @()

foreach ($name in $expected) {
    $linkPath = Join-Path $TargetRoot $name
    if (-not (Test-Path $linkPath)) {
        $missing += $name
        continue
    }
    $item = Get-Item $linkPath -Force
    $isReparse = ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0
    if (-not $isReparse) {
        $wrong += "$name (not a link)"
        continue
    }
    $expectedTarget = Join-Path "$CcmRepo\skills" $name
    if ($item.Target -ne $expectedTarget) {
        $wrong += "$name (target: $($item.Target))"
        continue
    }
    $skillFile = Join-Path $linkPath "SKILL.md"
    if (-not (Test-Path $skillFile)) {
        $unreadable += "$name (SKILL.md missing via link)"
        continue
    }
    $ok += $name
}

Write-Host ""
Write-Host "=== CCM Skill-Junction Verification ===" -ForegroundColor Cyan
Write-Host "OK: $($ok.Count) / $($expected.Count)" -ForegroundColor Green
if ($missing.Count -gt 0) {
    Write-Host "Missing: $($missing.Count)" -ForegroundColor Red
    $missing | ForEach-Object { Write-Host "  - $_" }
}
if ($wrong.Count -gt 0) {
    Write-Host "Wrong target / not a link: $($wrong.Count)" -ForegroundColor Red
    $wrong | ForEach-Object { Write-Host "  ! $_" }
}
if ($unreadable.Count -gt 0) {
    Write-Host "Unreadable: $($unreadable.Count)" -ForegroundColor Red
    $unreadable | ForEach-Object { Write-Host "  ? $_" }
}

if ($missing.Count -eq 0 -and $wrong.Count -eq 0 -and $unreadable.Count -eq 0) {
    exit 0
} else {
    exit 1
}
