# CCM-M0 Skills-Only Bootstrap — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** DBlicious in 1-3 Stunden so weit bringen, dass die drei Wrapper-Skills `ccm-brainstorm`, `ccm-plan`, `ccm-execute` plus `ccm-doctor` direkt auf dem aktuellen Projekt laufen — ohne CCM-Daemon, ohne Plugin-Lifecycle, ohne CCM-Iter-2-Code. Erster CCM-Lifecycle-Run mit einem echten Queue-Item.

**Architecture:** Junction-basierte Skill-Verlinkung von `C:\Users\jz\source\ClaudeCodeManager\skills\ccm-*` nach `~\.claude\skills\ccm-*` (Junctions funktionieren auf Windows ohne Admin-Rechte, da Skills Verzeichnisse sind). DBlicious bekommt minimales `.claude/ccm.toml` im Solo-Modus, `docs/queue/`-Struktur, ein Setup-Script unter `scripts/`, eine `.gitignore`-Erweiterung und ein erstes Test-Queue-Item. Kein Daemon-Pfad — `ccm-ask` fällt in-Session auf `AskUserQuestion` zurück.

**Tech Stack:** PowerShell 5.1 (Windows), keine neuen Rust-Crates, keine externen Tools. Voraussetzungen:
- `C:\Users\jz\source\ClaudeCodeManager` existiert und enthält `skills/ccm-*/SKILL.md` (verifiziert 2026-05-20)
- `~\.claude\skills\` existiert und ist beschreibbar (verifiziert via `Test-Path`)
- DBlicious ist auf Branch `feat/phase-0.6-source-architecture` (aktueller Stand) oder Branch wird hier nicht gewechselt — der Bootstrap committet ohne Branch-Wechsel

**Out of Scope (per Brainstorm vom 2026-05-20):** Plugin-Install in `settings.json`, Bootstrap-Hook (`.ps1`/`.sh`), Daemon-Setup, Telegram-Channel, Iter-2-Approval-Roundtrip, Triage→Queue-Migration (Triage ist leer). Alle diese Punkte landen in nachfolgenden Plänen für M1/M2/M3.

---

## File Structure

**Create (in DBlicious-Repo):**
- `scripts/setup-ccm-symlinks.ps1` — idempotentes PowerShell-Setup-Script (Junctions anlegen, kein Admin nötig)
- `scripts/verify-ccm-symlinks.ps1` — Read-only Verifikation (Test-Hook für Task 3)
- `.claude/ccm.toml` — minimales Solo-Mode-Manifest
- `docs/queue/.gitkeep`
- `docs/queue/done/.gitkeep`
- `docs/queue/archived/.gitkeep`
- `docs/queue/Q0001-ccm-plugin-discovery-verifizieren.md` — erstes Test-Item, Bezug zu Spec-Abschnitt D2

**Modify (in DBlicious-Repo):**
- `.gitignore` — Marker `# CCM runtime` + Einträge `.ccm/`, `.claude/*-state.json`

**Untouched but referenced:**
- `C:\Users\jz\source\ClaudeCodeManager\skills\ccm-*\SKILL.md` (23 Stück) — Quelle der Junctions
- `~\.claude\skills\ccm-*` (23 Junctions) — User-Level, außerhalb des Repos

---

## Task 1: Pre-Flight + Discovery

**Files:** keine

**Ziel:** Vor dem Schreiben des Setup-Scripts den aktuellen Zustand verifizieren, damit das Script weiß, was idempotent zu behandeln ist.

- [ ] **Step 1.1: CCM-Repo-Pfad und Skill-Anzahl verifizieren**

Run:
```powershell
$ccm = "C:\Users\jz\source\ClaudeCodeManager"
if (-not (Test-Path "$ccm\skills")) { throw "CCM skills dir missing" }
Get-ChildItem "$ccm\skills" -Directory | Where-Object { $_.Name -like "ccm-*" } | Measure-Object | Select-Object -ExpandProperty Count
```
Expected: Zahl ≥ 23 (Stand 2026-05-20). Falls < 20 → STOP, CCM-Repo ist nicht der erwartete Stand.

- [ ] **Step 1.2: User-Skills-Dir + Lock-Check**

Run:
```powershell
$userSkills = Join-Path $env:USERPROFILE ".claude\skills"
if (-not (Test-Path $userSkills)) { New-Item -ItemType Directory -Force -Path $userSkills | Out-Null }
Get-ChildItem $userSkills -Force -ErrorAction SilentlyContinue | Where-Object { $_.Name -like "ccm-*" } | Select-Object Name, Attributes, LinkType
```
Expected: leere Ausgabe (keine pre-existing CCM-Symlinks/Junctions). Falls vorhanden: Note der Liste — Step 2.2 wird sie idempotent ersetzen.

- [ ] **Step 1.3: DBlicious-Git-Branch festhalten**

Run:
```powershell
git -C C:\Users\jz\source\DBlicious branch --show-current
```
Expected: ein Branch-Name. Notieren — wir bleiben auf diesem Branch, kein Checkout.

---

## Task 2: Setup-Script schreiben

**Files:**
- Create: `C:\Users\jz\source\DBlicious\scripts\setup-ccm-symlinks.ps1`

- [ ] **Step 2.1: Verzeichnis anlegen**

Run:
```powershell
$scripts = "C:\Users\jz\source\DBlicious\scripts"
if (-not (Test-Path $scripts)) { New-Item -ItemType Directory -Force -Path $scripts | Out-Null }
```

- [ ] **Step 2.2: Setup-Script schreiben**

File: `C:\Users\jz\source\DBlicious\scripts\setup-ccm-symlinks.ps1`

```powershell
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
```

- [ ] **Step 2.3: Verify-Script schreiben**

File: `C:\Users\jz\source\DBlicious\scripts\verify-ccm-symlinks.ps1`

```powershell
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
```

- [ ] **Step 2.4: Scripts syntaktisch prüfen**

Run:
```powershell
powershell -NoProfile -Command "& { . C:\Users\jz\source\DBlicious\scripts\setup-ccm-symlinks.ps1 -CcmRepo 'nonexistent' } 2>&1; echo EXIT=$LASTEXITCODE"
```
Expected: Fehlermeldung "CCM skills directory not found at nonexistent\skills", `EXIT=1`. Bestätigt, dass Pfad-Validierung greift.

---

## Task 3: Setup-Script ausführen

**Files:** keine (Effekt: 23 Junctions unter `~\.claude\skills\ccm-*`)

- [ ] **Step 3.1: Setup ausführen**

Run:
```powershell
& C:\Users\jz\source\DBlicious\scripts\setup-ccm-symlinks.ps1
```
Expected: Report mit `Created: 23` (oder ähnlich, abhängig vom CCM-Stand), keine Blocked-Einträge. Exit 0.

- [ ] **Step 3.2: Verifikation ausführen**

Run:
```powershell
& C:\Users\jz\source\DBlicious\scripts\verify-ccm-symlinks.ps1
```
Expected: `OK: 23 / 23` (Zahlen passend zur CCM-Skill-Anzahl), Exit 0.

- [ ] **Step 3.3: Eine konkrete Junction stichprobenartig prüfen**

Run:
```powershell
Get-Item (Join-Path $env:USERPROFILE ".claude\skills\ccm-doctor") | Select-Object Name, LinkType, Target
Get-Content (Join-Path $env:USERPROFILE ".claude\skills\ccm-doctor\SKILL.md") -TotalCount 5
```
Expected:
- `LinkType` = `Junction`, `Target` = `C:\Users\jz\source\ClaudeCodeManager\skills\ccm-doctor`
- Erste 5 Zeilen zeigen Frontmatter `---`, `name: ccm-doctor`, `description: ...`, `preferred_model: ...`, `---`

- [ ] **Step 3.4: Idempotenz-Test (zweiter Lauf)**

Run:
```powershell
& C:\Users\jz\source\DBlicious\scripts\setup-ccm-symlinks.ps1
```
Expected: Report mit `Created: 0`, `Skipped (already linked): 23`. Exit 0.

---

## Task 4: `.claude/ccm.toml` schreiben

**Files:**
- Create: `C:\Users\jz\source\DBlicious\.claude\ccm.toml`

- [ ] **Step 4.1: Datei schreiben**

File: `C:\Users\jz\source\DBlicious\.claude\ccm.toml`

```toml
# Minimales CCM-Manifest fuer DBlicious (Solo-Mode, Sprint A-light / M0).
# Bootstrap manuell am 2026-05-20; spaeter durch /ccm-import-project --force
# ersetzbar (siehe docs/superpowers/specs/2026-05-20-ccm-approvals-ready-design.md
# Abschnitt "Quick-Win: Sprint A-light").

[ccm.project]
name                = "DBlicious"
roles               = ["dev"]
mode                = "solo"
default_branch_main = "main"
default_branch_dev  = "dev"

[ccm.meta]
schema_version = 1
created_by     = "manual-bootstrap@2026-05-20"
created_at     = "2026-05-20T00:00:00Z"

[ccm.questions]
fallback_mode = "block"

# Keine [ccm.channels.*]-Sektionen — Solo-Mode, Daemon nicht in Verwendung.
# Keine [ccm.models]-Overrides — Defaults aus ccm-model-mapping.md greifen.
# Keine [ccm.review]-Sektion — Review-Pipeline kommt erst in M1+.
```

- [ ] **Step 4.2: Strukturellen Check der ccm.toml fahren**

Run:
```powershell
$content = Get-Content C:\Users\jz\source\DBlicious\.claude\ccm.toml -Raw
if ($content -match "(?m)^\[ccm\.project\]" -and
    $content -match '(?m)^name\s*=\s*"DBlicious"' -and
    $content -match '(?m)^mode\s*=\s*"solo"' -and
    $content -match "(?m)^\[ccm\.meta\]" -and
    $content -match "(?m)^schema_version\s*=\s*1") {
    Write-Host "ccm.toml structural-check OK" -ForegroundColor Green
} else {
    Write-Error "ccm.toml structural-check FAILED"
    exit 1
}
```
Expected: `ccm.toml structural-check OK`.

Hinweis: kein voller TOML-Parser noetig — `ccm-doctor` (in Task 9) verifiziert spaeter die echte Parse-Korrektheit. Hier reicht ein struktureller Sanity-Check.

---

## Task 5: Queue-Verzeichnisstruktur anlegen

**Files:**
- Create: `C:\Users\jz\source\DBlicious\docs\queue\.gitkeep`
- Create: `C:\Users\jz\source\DBlicious\docs\queue\done\.gitkeep`
- Create: `C:\Users\jz\source\DBlicious\docs\queue\archived\.gitkeep`

- [ ] **Step 5.1: Verzeichnisse anlegen**

Run:
```powershell
$queue = "C:\Users\jz\source\DBlicious\docs\queue"
New-Item -ItemType Directory -Force -Path $queue | Out-Null
New-Item -ItemType Directory -Force -Path "$queue\done" | Out-Null
New-Item -ItemType Directory -Force -Path "$queue\archived" | Out-Null
New-Item -ItemType File -Force -Path "$queue\.gitkeep" | Out-Null
New-Item -ItemType File -Force -Path "$queue\done\.gitkeep" | Out-Null
New-Item -ItemType File -Force -Path "$queue\archived\.gitkeep" | Out-Null
Get-ChildItem $queue -Recurse | Select-Object FullName
```
Expected: drei `.gitkeep`-Dateien plus zwei Unterverzeichnisse.

---

## Task 6: `.gitignore`-Patch

**Files:**
- Modify: `C:\Users\jz\source\DBlicious\.gitignore`

- [ ] **Step 6.1: Marker-Check + Append**

Run:
```powershell
$gi = "C:\Users\jz\source\DBlicious\.gitignore"
$content = if (Test-Path $gi) { Get-Content $gi -Raw } else { "" }
if ($content -notmatch "# CCM runtime") {
    $append = @"


# CCM runtime
.ccm/
.claude/*-state.json
"@
    Add-Content -Path $gi -Value $append -Encoding utf8
    Write-Host "Appended CCM runtime markers to .gitignore" -ForegroundColor Green
} else {
    Write-Host "CCM runtime marker already present in .gitignore" -ForegroundColor Gray
}
```
Expected (Erstlauf): "Appended CCM runtime markers". Bei zweitem Lauf: "already present".

- [ ] **Step 6.2: Idempotenz-Test**

Run:
```powershell
& powershell -NoProfile -Command "& {
    \$content = Get-Content C:\Users\jz\source\DBlicious\.gitignore -Raw
    if ((\$content -split [regex]::Escape('# CCM runtime')).Count -ne 2) {
        Write-Error 'CCM marker present multiple times or missing'
        exit 1
    } else {
        Write-Host 'gitignore marker count = 1, OK' -ForegroundColor Green
    }
}"
```
Expected: `gitignore marker count = 1, OK`.

---

## Task 7: Erstes Test-Queue-Item Q0001 anlegen

**Files:**
- Create: `C:\Users\jz\source\DBlicious\docs\queue\Q0001-ccm-plugin-discovery-verifizieren.md`

- [ ] **Step 7.1: Q0001-Datei schreiben**

File: `C:\Users\jz\source\DBlicious\docs\queue\Q0001-ccm-plugin-discovery-verifizieren.md`

```markdown
---
id: Q0001
created: 2026-05-20T00:00:00Z
status: new
priority: high
title: "CCM-Plugin lokale Discovery in Claude Code verifizieren"
spec: docs/superpowers/specs/2026-05-20-ccm-approvals-ready-design.md
plan: null
pending_question_id: null
resume_step: null
parent: null
artifacts: []
source: manual
fingerprint: null
requirements: null
assigned_worker: null
type: feature
review:
  status: null
  reviewer: null
  notes_path: null
  requested_at: null
  decided_at: null
security_review:
  required: false
  status: null
  notes_path: null
diagnosis_path: null
design_path: null
linked_issue: null
linked_pr: null
verification_ops: []
---

## Description

Vorarbeit fuer Milestone M1 der CCM-Approvals-Ready-Spec
([2026-05-20-ccm-approvals-ready-design.md](../superpowers/specs/2026-05-20-ccm-approvals-ready-design.md), Abschnitt D2).

**Frage:** Welche Mechanik in Claude-Code's `.claude/settings.json` laesst es
zu, ein Plugin **lokal** (kein Marketplace) zu installieren? Drei Hypothesen
laut Spec:

1. Eine Source-Definition unter `settings.json.plugins.sources.<name>.path`
2. Symlink `~/.claude/plugins/ccm/` -> CCM-Repo, dann Eintrag `"ccm@local": true`
3. Keine lokale Discovery moeglich — Marketplace-Pflicht oder Skills-only-Pfad

**Akzeptanzkriterien:**
- Eines der drei Szenarien ist praktisch verifiziert (durch Setup + Restart der Claude-Code-Session + Beobachtung).
- Bei Erfolg: Dokumentation der genauen Setup-Schritte in der Spec.
- Bei Misserfolg (Hypothese 3): Spec-Update mit Plan-B-Bestaetigung — Skills-Symlinks bleiben dauerhaft, kein Plugin.

**Bezug zur CCM-Pipeline:** Dieses Item ist der Lakmus-Test, ob der Q-Lifecycle in DBlicious ueberhaupt funktioniert. Wenn `/ccm-brainstorm Q0001` aus einer frischen Claude-Code-Session heraus diesen Body lesen, eine Spec erzeugen und den Status auf `brainstormed` setzen kann, ist M0 erfolgreich abgeschlossen.

## Log
- 2026-05-20T00:00:00Z — manual: created (M0-Bootstrap)
```

- [ ] **Step 7.2: Frontmatter-Konformitaet pruefen**

Run:
```powershell
$item = "C:\Users\jz\source\DBlicious\docs\queue\Q0001-ccm-plugin-discovery-verifizieren.md"
$content = Get-Content $item -Raw
$frontmatter_match = $content -match "(?s)^---\r?\n(.+?)\r?\n---"
if (-not $frontmatter_match) {
    Write-Error "no frontmatter"
    exit 1
}
$fm = $Matches[1]
$required = @("id: Q0001", "status: new", "priority: high", 'title: "CCM-Plugin lokale Discovery')
foreach ($r in $required) {
    if ($fm -notmatch [regex]::Escape($r)) {
        Write-Error "missing field: $r"
        exit 1
    }
}
Write-Host "Q0001 frontmatter check OK" -ForegroundColor Green
```
Expected: `Q0001 frontmatter check OK`.

---

## Task 8: Commit

**Files:** keine neuen — Git-Operation auf den in Task 2-7 erzeugten Dateien.

- [ ] **Step 8.1: Status pruefen**

Run:
```powershell
git -C C:\Users\jz\source\DBlicious status --short
```
Expected: M auf `.gitignore`, ?? auf `.claude/ccm.toml`, `scripts/setup-ccm-symlinks.ps1`, `scripts/verify-ccm-symlinks.ps1`, `docs/queue/.gitkeep`, `docs/queue/done/.gitkeep`, `docs/queue/archived/.gitkeep`, `docs/queue/Q0001-*.md`.

- [ ] **Step 8.2: Gezielt staging (keine `-A`)**

Run:
```powershell
git -C C:\Users\jz\source\DBlicious add `
    .gitignore `
    .claude/ccm.toml `
    scripts/setup-ccm-symlinks.ps1 `
    scripts/verify-ccm-symlinks.ps1 `
    docs/queue/.gitkeep `
    docs/queue/done/.gitkeep `
    docs/queue/archived/.gitkeep `
    docs/queue/Q0001-ccm-plugin-discovery-verifizieren.md
git -C C:\Users\jz\source\DBlicious status --short
```
Expected: alle obigen Files als `A` (added), keine weiteren `??`.

- [ ] **Step 8.3: Commit**

Run:
```powershell
git -C C:\Users\jz\source\DBlicious commit -m @'
chore(ccm): m0 bootstrap — skills-only adoption (sprint a-light)

- scripts/setup-ccm-symlinks.ps1 + verify-ccm-symlinks.ps1 (Junctions
  ~/.claude/skills/ccm-* -> ClaudeCodeManager/skills/ccm-*, idempotent,
  kein Admin noetig)
- .claude/ccm.toml minimal (Solo-Mode, schema_version=1, kein Channel)
- docs/queue/{,done,archived}/.gitkeep (CCM-Lifecycle-Verzeichnisse)
- .gitignore um .ccm/ + .claude/*-state.json erweitert (# CCM runtime)
- Q0001 Test-Item: CCM-Plugin lokale Discovery verifizieren (Vorarbeit M1)

Folgt der Spec docs/superpowers/specs/2026-05-20-ccm-approvals-ready-design.md
Out of Scope (M1+): Plugin-Install, Bootstrap-Hook, Daemon, Telegram,
Iter-2-Approvals.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
'@
git -C C:\Users\jz\source\DBlicious status --short
```
Expected: Commit erfolgreich (HEAD-SHA wird ausgegeben), `status` zeigt working tree clean (keine M/?? auf den gerade commiteten Files).

---

## Task 9: Smoke-Test-Anleitung (manual, durch User)

**Files:** keine

**Hinweis:** Junctions sind ab dem Moment ihrer Anlage system-weit sichtbar — Claude Code muss aber den Skill-Cache neu laden. Das passiert in einer **frisch gestarteten** Claude-Code-Session. Die laufende Session kennt die neuen Skills nicht.

- [ ] **Step 9.1: User startet neue Claude-Code-Session in DBlicious**

Aktion: Aktuelle Session beenden, neue starten:
```powershell
cd C:\Users\jz\source\DBlicious
claude
```

- [ ] **Step 9.2: User invokiert `/ccm-doctor`**

In der neuen Session: `/ccm-doctor`

Expected:
- Skill wird gefunden (kein "Unknown skill")
- Report ueber `.claude/ccm.toml` parse-status (OK, schema_version=1, solo)
- INFO-Level fuer fehlende `[ccm.channels.*]` (Solo-Mode-Branch greift, keine ERROR)
- Eventuelle INFO-Hinweise fuer Skill-Symlinks (sollte alle gruen sein, dank Task 3)
- Forward-Compat-Hinweise erlaubt

Bei ERRORS: in den Plan zurueck, betroffenen Task wiederholen.

- [ ] **Step 9.3: User invokiert `/ccm-brainstorm Q0001`**

In derselben neuen Session: `/ccm-brainstorm Q0001`

Expected (siehe `ccm-brainstorm/SKILL.md`):
- Skill liest `docs/queue/Q0001-*.md`
- Validiert Frontmatter (`status: new`)
- Dispatcht Sub-Agent fuer `superpowers:brainstorming`
- Sub-Agent fragt User-Klaerungsfragen, erzeugt eine Spec in `docs/superpowers/specs/Q0001-<slug>-design.md`
- Bei Erfolg: Q0001-Frontmatter wird auf `status: brainstormed`, `spec: <pfad>` aktualisiert
- Commit-Vorschlag `chore(queue): Q0001 brainstormed`

Falls der Skill stoppt mit "kein Daemon": das ist OK fuer M0 — Daemon-Fallback greift, in-Session `AskUserQuestion` ersetzt Push.

- [ ] **Step 9.4: Erfolgsmeldung**

Wenn beide Smoke-Tests durch sind: M0 ist abgeschlossen. Next-Step (separater Plan): M1 = CCM Iter-2 + Plugin-Discovery (Q0001 als Eingangstor).

---

## Failure modes

| Lage | Verhalten |
|------|-----------|
| `~\.claude\skills\` enthaelt bereits ccm-* Verzeichnisse (keine Junctions) | Task 3 stoppt mit "Blocked", User loescht manuell, dann erneut |
| Junction-Anlage scheitert mit Permission-Denied | Windows-User ohne Developer-Mode UND `New-Item -ItemType Junction` blockiert (sehr selten); Fallback: `mklink /J` per `cmd.exe`. Plan-B im Setup-Script ergaenzen. |
| Step 3.1 Created < 20 | CCM-Repo-Stand abweichend; Task 1 hat das erkannt — Plan stoppt. |
| `ccm-doctor`-Skill nicht findbar in Step 9.2 | Junction-Cache von Claude Code nicht refreshed; Session komplett neu starten (nicht reload), zur Not Tab-completion `/ccm-` verifizieren. |
| `/ccm-brainstorm Q0001` faengt mit Daemon-Fehler an | Erwartetes Verhalten in M0 — Skill faellt auf in-Session-Fragen zurueck. Falls Skill hart abbricht: Bug im Skill-Fallback-Pfad, in Q0001-Body als Folge-Notiz festhalten. |
| Commit-Hook in DBlicious schlaegt fehl | Untersuchen welcher Hook (Git ist hier installiert, sonst nichts erwartet); falls Format-Hook: Spec-/Plan-Files duerfen LF haben (gitattributes erlaubt). |

## Verification-Log (Vorlage fuer ccm-execute, falls dieses Item ueber CCM-Pipeline laeuft)

```json
{
  "verification": [
    {"cmd": "powershell -NoProfile -File C:\\Users\\jz\\source\\DBlicious\\scripts\\verify-ccm-symlinks.ps1", "exit_code": 0, "stdout_tail": "OK: 23 / 23"},
    {"cmd": "git -C C:\\Users\\jz\\source\\DBlicious status --short", "exit_code": 0, "stdout_tail": ""},
    {"cmd": "Test-Path C:\\Users\\jz\\source\\DBlicious\\.claude\\ccm.toml", "exit_code": 0, "stdout_tail": "True"}
  ]
}
```

## Referenzen

- Spec: `docs/superpowers/specs/2026-05-20-ccm-approvals-ready-design.md` (insb. Abschnitt "Quick-Win: Sprint A-light" und "Acceptance Criteria → M0")
- Queue-Format: `C:\Users\jz\source\ClaudeCodeManager\skills\_shared\ccm-queue-format.md`
- ccm-doctor: `C:\Users\jz\source\ClaudeCodeManager\skills\ccm-doctor\SKILL.md` — Soll-Bild-Check
- ccm-brainstorm: `C:\Users\jz\source\ClaudeCodeManager\skills\ccm-brainstorm\SKILL.md`
- CCM-Project-Fabric: `C:\Users\jz\source\ClaudeCodeManager\skills\_shared\ccm-project-fabric.md`
