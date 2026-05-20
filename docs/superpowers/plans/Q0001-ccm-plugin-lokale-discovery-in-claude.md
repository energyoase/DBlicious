# Q0001 — CCM-Plugin lokale Discovery verifizieren — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Empirisch herausfinden, mit welcher Mechanik das CCM-Plugin (`C:\Users\jz\source\ClaudeCodeManager\plugin`) lokal in Claude Code aktiviert werden kann (H2 Junction → H1 Source-Path-Config → H3 Plan-B). Das Ergebnis wird als Setup-Rezept oder Negativ-Befund in §8 der Spec verewigt.

**Architecture:** Investigations-Plan, kein Feature. Pro Phase wird ein Szenario aufgesetzt, Claude Code neu gestartet, eine kurze Probe-Sequenz beobachtet, das Ergebnis in §8 der Spec eingetragen — danach Entscheidungspunkt: nächste Hypothese oder Abschluss. Reihenfolge nach Cost-to-test: billigster Pfad zuerst (H2 Junction), dann H1, dann H3.

**Tech Stack:** Windows 11 + PowerShell 5.1, Claude Code mit eigenem Session-Restart als „Build-Tool", `git` für Commits. Kein neuer Code, keine Tests in klassischem Sinn — Verifikation = sichtbare Effekte (Plugin-Listing, Hook-Console-Output, Schema-Fehler).

**Out of Scope:** PowerShell-Port des Bootstrap-Hooks (Sprint B), Marketplace-Push (Post-M3), Änderungen an `~/.claude/skills/ccm-*`-Junctions (M0-Status bleibt), Änderungen an CCM-Plugin-Code selbst.

**Zeitbudget:** 60–90 Min. Bei Überschreitung → Queue-Item zurück auf `status: new` mit Notiz, was hängt.

**Branch:** `feat/phase-0.6-source-architecture` (NICHT wechseln).

---

## File Structure

**Modify (in DBlicious-Repo):**
- `docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md` — §8 wird mit Setup-Rezept oder Negativ-Befund gefüllt; §4 ggf. mit Beobachtungs-Notizen pro H ergänzt.
- `docs/queue/Q0001-ccm-plugin-discovery-verifizieren.md` — Log-Einträge pro Phase, finale `status: done` + `artifacts`-Liste.
- Bei H3-Pfad: `docs/superpowers/specs/2026-05-20-ccm-approvals-ready-design.md` (Abschnitt D2 + Risiken-Tabelle + M1.2-Akzeptanz).
- Bei H3-Pfad: `CLAUDE.md` (Hinweis-Absatz „CCM ist Skills-only in DBlicious").

**Temporär (nicht commit, nur während Experiment):**
- `C:\Users\jz\source\DBlicious\.claude\settings.json.bak-q0001` — Backup vor jedem Edit.
- `~/.claude/plugins/ccm` — Junction (bei H2), wird je nach Ausgang behalten oder entfernt.
- `C:\Users\jz\source\DBlicious\.claude\settings.json` — wird während H1/H2 editiert; bei Fehlschlag aus Backup wiederhergestellt; bei Erfolg final committet.

**Untouched but referenced:**
- `C:\Users\jz\source\ClaudeCodeManager\plugin\plugin.json`
- `C:\Users\jz\source\ClaudeCodeManager\plugin\hooks\ccm-bootstrap-skill-check.sh`
- `~/.claude/skills/ccm-*` (M0-Junctions bleiben)

---

## Task 0: Pre-Flight + Phase-0-Recherche

**Files:** keine (nur Lesen + Notiz in Spec §4)

**Ziel:** Aktueller Zustand verifizieren, Doku-Lage klären, ggf. Hypothesen-Wahrscheinlichkeiten verschieben — bevor irgendwas editiert wird.

- [ ] **Step 0.1: Branch + Working-Tree-Check**

Run:
```powershell
git -C "C:\Users\jz\source\DBlicious" branch --show-current
git -C "C:\Users\jz\source\DBlicious" status --short
```
Expected: Branch `feat/phase-0.6-source-architecture`; keine ungetrackten Änderungen an `.claude/settings.json` oder den unten gelisteten Spec-Dateien (andere Plan-Dateien dürfen unstaged sein — die gehören anderen Tracks).

Falls Branch falsch ist → STOP, manuell wechseln, dann von vorn.

- [ ] **Step 0.2: CCM-Plugin-Existenz verifizieren**

Run:
```powershell
$ccmPlugin = "C:\Users\jz\source\ClaudeCodeManager\plugin"
Test-Path "$ccmPlugin\plugin.json"
Test-Path "$ccmPlugin\hooks\ccm-bootstrap-skill-check.sh"
Get-Content "$ccmPlugin\plugin.json" | Out-String
```
Expected: Beide Pfade `True`; `plugin.json` zeigt `name`, evtl. `dependencies`, evtl. `hooks`-Block mit `SessionStart`.

Falls einer fehlt → STOP, CCM-Repo nicht im erwarteten Stand; Item zurück auf `new`.

- [ ] **Step 0.3: Aktuelle settings.json sichern + auslesen**

Run:
```powershell
$settings = "C:\Users\jz\source\DBlicious\.claude\settings.json"
Test-Path $settings
Copy-Item $settings "$settings.bak-q0001" -Force
Get-Content $settings | Out-String
```
Expected: Datei existiert; Backup ist angelegt; Inhalt zeigt aktuellen `enabledPlugins`-Block (Format `"name@source": true`) und ggf. existierende `plugins.sources`-Definitionen.

Notieren: welche Sources sind heute eingetragen (`claude-plugins-official` etc.) — das ist die Vorlage für H1.

- [ ] **Step 0.4: User-Plugins-Verzeichnis prüfen**

Run:
```powershell
$userPlugins = Join-Path $env:USERPROFILE ".claude\plugins"
Test-Path $userPlugins
if (Test-Path $userPlugins) { Get-ChildItem $userPlugins -Force | Select-Object Name, LinkType, Target }
```
Expected: Verzeichnis existiert. Notiz machen, ob bereits Inhalte da sind (offizielle Plugins-Cache?); falls ja → H2-Junction muss kollisionsfrei sein (`ccm`-Subordner darf noch nicht existieren).

- [ ] **Step 0.5: context7 zu Claude-Code-Plugin-Schema befragen (Hard-Cap 15 Min)**

Run (zwei Tool-Calls):
```
mcp__plugin_context7_context7__resolve-library-id
  libraryName: "Claude Code"
  query: "settings.json plugins sources path local plugin enabledPlugins schema"
```
Wenn ein Treffer kommt (z.B. `/anthropics/claude-code` oder docs.claude.com):
```
mcp__plugin_context7_context7__query-docs
  libraryId: <treffer>
  query: "settings.json plugins.sources local path enabledPlugins name@source format"
```
Expected: Entweder offizielle Doku zu `plugins.sources.<name>.path` (→ H1-Wahrscheinlichkeit steigt) oder kein Hinweis (→ H2 bleibt Top-Kandidat, weil Junction-Hack).

Falls context7 nichts liefert oder Tool > 5 Min hängt → SKIP, weiter zu Task 1 (H2 ist sowieso der nächste Schritt).

- [ ] **Step 0.6: Phase-0-Notiz in Spec §4 anhängen**

Edit `C:\Users\jz\source\DBlicious\docs\superpowers\specs\Q0001-ccm-plugin-lokale-discovery-in-claude-design.md` — direkt vor `### H1 — Source-Path-Definition...` einen Block einfügen:

```markdown
### Phase-0-Befund (ausgefüllt während Execute)

- Backup angelegt: `.claude/settings.json.bak-q0001` (Step 0.3).
- Vorhandene Sources in settings.json: <Liste aus Step 0.3>
- `~/.claude/plugins/` Inhalt: <Liste aus Step 0.4>
- context7-Recherche: <kurzes Resümee oder "kein verwertbarer Treffer">
- Hypothesen-Reihung nach Phase 0: <H2-zuerst, ggf. Begründung>
```

**Decision-Point:**
- Wenn Phase 0 klar zeigt, dass H1 offiziell dokumentiert ist (z.B. ein Doku-Snippet mit `plugins.sources.<name>.path`) → Reihenfolge umkehren, Task 2 vor Task 1 ausführen.
- Sonst (Default): weiter mit Task 1 (H2 Junction-Test).

- [ ] **Step 0.7: Phase-0-Commit**

```powershell
git -C "C:\Users\jz\source\DBlicious" add docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md
git -C "C:\Users\jz\source\DBlicious" commit -m "Q0001: Phase-0-Befund (Pre-Flight + Doku-Recherche)"
```

---

## Task 1: H2 — Symlink-Test (`~/.claude/plugins/ccm` → CCM-Plugin)

**Files:**
- Modify (temporär): `C:\Users\jz\source\DBlicious\.claude\settings.json` (Eintrag in `enabledPlugins`)
- Modify (final, falls Erfolg): `docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md` (§4 Beobachtung + §8 Setup-Rezept)
- Modify (final): `docs/queue/Q0001-ccm-plugin-discovery-verifizieren.md` (Log-Eintrag)

**Ziel:** Junction unter `~/.claude/plugins/ccm` anlegen, `enabledPlugins`-Eintrag setzen, Claude-Code-Restart, beobachten, ob Plugin lädt und der `SessionStart`-Hook sichtbar feuert.

- [ ] **Step 1.1: Junction anlegen**

Run:
```powershell
$target = "C:\Users\jz\source\ClaudeCodeManager\plugin"
$link   = Join-Path $env:USERPROFILE ".claude\plugins\ccm"
if (Test-Path $link) {
  Write-Host "Junction-Pfad existiert bereits — Inhalt prüfen:"
  Get-Item $link | Select-Object Name, LinkType, Target
  throw "Pfad $link existiert; manuell prüfen, ggf. entfernen, dann Step neu ausführen."
}
New-Item -ItemType Junction -Path $link -Value $target | Out-Null
Get-Item $link | Select-Object Name, LinkType, Target
```
Expected: `LinkType=Junction`, `Target` zeigt auf `C:\Users\jz\source\ClaudeCodeManager\plugin`. Keine Admin-Prompt.

- [ ] **Step 1.2: settings.json-Eintrag setzen**

Edit `C:\Users\jz\source\DBlicious\.claude\settings.json` — in den `enabledPlugins`-Block den Eintrag `"ccm@local": true` ergänzen (Komma-Disziplin beachten, JSON gültig halten).

Beispiel-Diff:
```diff
   "enabledPlugins": {
     "claude-plugins-official": true,
+    "ccm@local": true
   }
```

Run zur Validierung:
```powershell
$settings = "C:\Users\jz\source\DBlicious\.claude\settings.json"
Get-Content $settings -Raw | ConvertFrom-Json | Out-Null
Write-Host "JSON OK"
```
Expected: `JSON OK`. Falls Parse-Error → sofort aus Backup wiederherstellen (`Copy-Item "$settings.bak-q0001" $settings -Force`) und Edit neu machen.

- [ ] **Step 1.3: Claude-Code-Restart-Anweisung (manuell)**

> Manueller Schritt für den Executor: **Diese Claude-Code-Session schließen** (alle Fenster) und im DBlicious-Workspace neu öffnen. Erst danach Step 1.4 ausführen.

Optional vorher zum schnellen Wiederfinden in der neuen Session den Plan-Pfad in die Zwischenablage:
```powershell
Set-Clipboard "docs/superpowers/plans/Q0001-ccm-plugin-lokale-discovery-in-claude.md"
```

- [ ] **Step 1.4: Probe-Sequenz nach Restart**

In der frisch gestarteten Session folgende Beobachtungen sammeln (jede einzeln in §4 der Spec notieren):

| Verifikation | Wie messen | Erwartung bei H2-Erfolg |
|---|---|---|
| V2.1 — Schema-Validation | Startup-Output / Status-Bar | Keine Schema-Validation-Errors zu `enabledPlugins` oder `ccm@local` |
| V2.2 — Plugin gelistet | `/help` oder `/plugin`-Listing falls vorhanden | Eintrag `ccm` (oder `ccm@local`) erscheint |
| V2.3 — Hook-Feuer | Console-Output beim Session-Start | Sichtbarer Output zum `ccm-bootstrap-skill-check.sh` — auch ein Bash-„command not found"-Error reicht, weil das beweist, dass das Plugin geladen wurde |

Run zum Auslesen der Settings (zur Doppelkontrolle, dass Claude-Code den Eintrag nicht entfernt hat):
```powershell
Get-Content "C:\Users\jz\source\DBlicious\.claude\settings.json" -Raw
```
Expected: `ccm@local`-Eintrag ist noch da (Claude rewritet die Datei nicht ungefragt).

- [ ] **Step 1.5: Optional — Variante ohne Source-Suffix**

Nur wenn 1.4 alle drei Verifikationen `false` zeigt, **bevor** zu Task 2 gewechselt wird:

Edit `settings.json`: `"ccm@local": true` → `"ccm": true`. JSON-Lint (siehe 1.2), Session-Restart (siehe 1.3), Probe-Sequenz (siehe 1.4) wiederholen.

- [ ] **Step 1.6: Entscheidungspunkt H2**

In `docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md` unter `### H2 — Symlink ins ~/.claude/plugins/-Verzeichnis` einen Beobachtungs-Block anhängen:

```markdown
**Beobachtung (Execute):**
- V2.1: <true/false + Zitat aus Console>
- V2.2: <true/false + wo gelistet>
- V2.3: <true/false + Hook-Output verbatim>
- Variante ohne Suffix getestet: <ja/nein, Ergebnis>
- Entscheidung: <H2 erfolgreich | H2 verworfen → weiter zu Task 2>
```

**Decision-Point:**
- V2.1 + V2.2 + V2.3 alle `true` → **H2 bestätigt**, weiter zu Task 4 (Setup-Rezept dokumentieren, H2 als primärer Weg).
- Sonst → Task 1 abbauen (Step 1.7) und weiter zu Task 2 (H1).

- [ ] **Step 1.7: Rollback (nur bei H2-Fehlschlag)**

Run:
```powershell
$link = Join-Path $env:USERPROFILE ".claude\plugins\ccm"
if (Test-Path $link) { Remove-Item $link -Force }   # Junction entfernen, NICHT -Recurse
$settings = "C:\Users\jz\source\DBlicious\.claude\settings.json"
Copy-Item "$settings.bak-q0001" $settings -Force
Get-Content $settings -Raw | ConvertFrom-Json | Out-Null
Write-Host "settings.json aus Backup wiederhergestellt — JSON OK"
```
Expected: Junction weg, settings.json identisch zu Pre-Phase-1-Stand.

- [ ] **Step 1.8: Phase-1-Commit (immer — egal Erfolg oder Fehlschlag)**

```powershell
git -C "C:\Users\jz\source\DBlicious" add docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md
# Bei H2-Erfolg gehört auch der settings.json-Eintrag dazu:
# git -C "C:\Users\jz\source\DBlicious" add .claude/settings.json
git -C "C:\Users\jz\source\DBlicious" commit -m "Q0001: Phase 1 (H2 Junction-Test) Beobachtung"
```

---

## Task 2: H1 — Source-Path-Config in `settings.json`

**Voraussetzung:** Task 1 fehlgeschlagen, Step 1.7 (Rollback) ausgeführt — settings.json ist auf Pre-Phase-1-Stand.

**Files:**
- Modify (temporär oder final): `C:\Users\jz\source\DBlicious\.claude\settings.json` (neuer `plugins.sources.local-ccm`-Block + `enabledPlugins`-Eintrag)
- Modify: `docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md` (§4 Beobachtung, §8 bei Erfolg)
- Modify: `docs/queue/Q0001-ccm-plugin-discovery-verifizieren.md` (Log)

- [ ] **Step 2.1: Backup auffrischen**

Run:
```powershell
$settings = "C:\Users\jz\source\DBlicious\.claude\settings.json"
Copy-Item $settings "$settings.bak-q0001" -Force
Write-Host "Backup aufgefrischt (Pre-Phase-2-Stand)"
```

- [ ] **Step 2.2: `plugins.sources.local-ccm`-Block ergänzen**

Edit `C:\Users\jz\source\DBlicious\.claude\settings.json` — auf Top-Level einen neuen Block `plugins` mit `sources` einfügen (falls noch nicht vorhanden) und `enabledPlugins` um `"ccm@local-ccm": true` ergänzen.

Beispiel-Diff (Pfad mit doppeltem Backslash JSON-escapen):
```diff
 {
+  "plugins": {
+    "sources": {
+      "local-ccm": {
+        "path": "C:\\Users\\jz\\source\\ClaudeCodeManager\\plugin"
+      }
+    }
+  },
   "enabledPlugins": {
     "claude-plugins-official": true,
+    "ccm@local-ccm": true
   }
 }
```

Run zur Validierung:
```powershell
$settings = "C:\Users\jz\source\DBlicious\.claude\settings.json"
$json = Get-Content $settings -Raw | ConvertFrom-Json
$json.plugins.sources.'local-ccm'.path
$json.enabledPlugins.'ccm@local-ccm'
```
Expected: Erste Zeile gibt den Pfad aus, zweite Zeile gibt `True` aus. Falls einer der beiden leer: Edit fehlgeschlagen, korrigieren bevor weiter.

- [ ] **Step 2.3: Claude-Code-Restart (manuell — siehe Step 1.3)**

- [ ] **Step 2.4: Probe-Sequenz**

| Verifikation | Wie messen | Erwartung |
|---|---|---|
| V1.1 — Schema | Startup-Output | Keine Schema-Validation-Errors zu `plugins.sources` oder `local-ccm` |
| V1.2 — Plugin gelistet | `/help` / `/plugin`-Listing | Eintrag `ccm@local-ccm` erscheint |
| V1.3 — (optional) Plugin-Pfad ist die Discovery-Quelle | Skill-Befehl `/ccm-doctor` aufrufen | Antwortet, ohne dass eine M0-Junction unter `~/.claude/skills/ccm-*` zwingend benötigt wäre. **Nicht testen, wenn das M0-Setup gefährdet wäre** — nur als Bonus-Bestätigung |
| V1.4 — Hook-Feuer | Console-Output | Sichtbarer Output / Error vom `ccm-bootstrap-skill-check.sh` |

- [ ] **Step 2.5: Entscheidungspunkt H1**

In `docs/superpowers/specs/Q0001-...-design.md` unter `### H1 — Source-Path-Definition in settings.json` einen Beobachtungs-Block anhängen (Format wie Step 1.6).

**Decision-Point:**
- V1.1 + V1.2 + (V1.3 ODER V1.4) `true` → **H1 bestätigt**, weiter zu Task 4 (Setup-Rezept, H1 als primärer Weg).
- Sonst → Task 2 abbauen (Step 2.6), weiter zu Task 3 (H3).

- [ ] **Step 2.6: Rollback (nur bei H1-Fehlschlag)**

Run:
```powershell
$settings = "C:\Users\jz\source\DBlicious\.claude\settings.json"
Copy-Item "$settings.bak-q0001" $settings -Force
Get-Content $settings -Raw | ConvertFrom-Json | Out-Null
Write-Host "settings.json aus Backup wiederhergestellt — JSON OK"
```

- [ ] **Step 2.7: Phase-2-Commit**

```powershell
git -C "C:\Users\jz\source\DBlicious" add docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md
# Bei H1-Erfolg auch:
# git -C "C:\Users\jz\source\DBlicious" add .claude/settings.json
git -C "C:\Users\jz\source\DBlicious" commit -m "Q0001: Phase 2 (H1 Source-Path-Test) Beobachtung"
```

---

## Task 3: H3 — Plan-B aktivieren (Skills-only permanent)

**Voraussetzung:** Task 1 UND Task 2 sind fehlgeschlagen (beide Rollbacks ausgeführt).

**Files:**
- Modify: `docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md` (§7 wird Befund-Träger, §8 wird Negativ-Befund)
- Modify: `docs/superpowers/specs/2026-05-20-ccm-approvals-ready-design.md` (D2: „Plan-B aktiv", Risiken-Tabelle, M1.2-Akzeptanzkriterium)
- Modify: `CLAUDE.md` (Hinweis-Absatz)
- Modify: `docs/queue/Q0001-ccm-plugin-discovery-verifizieren.md` (Log + `status: done`)

- [ ] **Step 3.1: Negativ-Befund in Spec §8 schreiben**

Edit `docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md` — §8 (`## 8. Setup-Rezept / Negativ-Befund`) komplett ersetzen durch:

```markdown
## 8. Setup-Rezept / Negativ-Befund

**Status:** Plan-B aktiv (H3) — lokale Plugin-Discovery in Claude Code (Stand 2026-05-20) nicht möglich.

### Getestete Varianten

| # | Hypothese | Setup-Kurzform | Fehlersymptom |
|---|---|---|---|
| 1 | H2 — Junction `~/.claude/plugins/ccm` + `"ccm@local": true` | <Kurzform aus Task 1> | <Fehler verbatim aus Step 1.4> |
| 2 | H2-Variante — `"ccm": true` ohne Source-Suffix | <Kurzform aus Step 1.5, falls getestet> | <Fehler verbatim> |
| 3 | H1 — `plugins.sources.local-ccm.path` + `"ccm@local-ccm": true` | <Kurzform aus Task 2> | <Fehler verbatim aus Step 2.4> |

### Konsequenz

- CCM bleibt in DBlicious **Skills-only** (M0-Junctions unter `~/.claude/skills/ccm-*`).
- `SessionStart`-Hooks (`ccm-bootstrap-skill-check.sh`) feuern nicht — Bootstrap-Check muss vom User manuell ausgelöst werden (`/ccm-doctor`).
- Marketplace-Push bleibt Post-M3-Option, ist aber kein Blocker für M1.

### Quellen / Belege

- context7-Recherche aus Phase 0: <Zusammenfassung aus Step 0.5>
- Vollständige Fehler-Verbatims: oben pro Variante
```

- [ ] **Step 3.2: Parent-Spec D2 aktualisieren**

Edit `docs/superpowers/specs/2026-05-20-ccm-approvals-ready-design.md`:

1. Abschnitt D2: Überschrift / Status-Zeile auf `**Status: Plan-B aktiv — Skills-only permanent (Q0001 §8)**` setzen.
2. Risiken-Tabelle: Zeile `Claude-Code-Plugin-Discovery nimmt keinen lokalen Pfad` von `Wahrscheinlichkeit: mittel` → `eingetreten`; Mitigation-Spalte: „Skills-only-Pfad ist primärer Weg".
3. Akzeptanzkriterium M1.2 (`ccm-bootstrap-skill-check.ps1` im SessionStart-Hook): umformulieren auf „manuell durch User via `/ccm-doctor` auszulösen" oder ganz streichen, je nach Restlich-Strukturierung.

Run zur Sichtkontrolle:
```powershell
Select-String -Path "C:\Users\jz\source\DBlicious\docs\superpowers\specs\2026-05-20-ccm-approvals-ready-design.md" -Pattern "Plan-B|M1.2|eingetreten" | Select-Object LineNumber, Line
```
Expected: Drei Treffer (D2-Status, M1.2-Umformulierung, Risiken-Zeile).

- [ ] **Step 3.3: CLAUDE.md-Hinweis**

Edit `C:\Users\jz\source\DBlicious\CLAUDE.md` — am Ende des Dokuments (vor dem letzten Abschnitt oder als neuer Absatz unter „Conventions worth knowing") einen Block einfügen:

```markdown
- **CCM ist Skills-only in DBlicious.** Plugin-Hooks (z.B. `SessionStart`-Bootstrap) greifen nicht, weil Claude Code keine lokale Plugin-Discovery erlaubt (Stand 2026-05-20). Skills werden via Junctions unter `~/.claude/skills/ccm-*` eingebunden; Bootstrap-Checks laufen manuell über `/ccm-doctor`. Begründung und getestete Varianten: `docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md` §8.
```

- [ ] **Step 3.4: Queue-Item abschließen**

Edit `docs/queue/Q0001-ccm-plugin-discovery-verifizieren.md`:

1. Front-Matter: `status: brainstormed` → `status: done`.
2. Front-Matter: `artifacts: []` → `artifacts: [docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md]`.
3. Log am Ende anhängen:
   ```
   - 2026-05-20T<HH:MM>:00Z — ccm-execute: Phasen 1-3 durchgeführt, H3 bestätigt (Plan-B aktiv). Setup-Rezept-Slot ist Negativ-Befund (Spec §8). Parent-Spec D2 + CLAUDE.md aktualisiert.
   ```

- [ ] **Step 3.5: Phase-3-Commit**

```powershell
git -C "C:\Users\jz\source\DBlicious" add `
  docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md `
  docs/superpowers/specs/2026-05-20-ccm-approvals-ready-design.md `
  CLAUDE.md `
  docs/queue/Q0001-ccm-plugin-discovery-verifizieren.md
git -C "C:\Users\jz\source\DBlicious" commit -m "Q0001: Phase 3 — H3 bestätigt (Plan-B: Skills-only permanent)"
```

**Decision-Point:** Nach diesem Commit ist das Investigation-Item abgeschlossen — weiter zu Task 5 (Abschluss-Verifikation), Task 4 wird übersprungen.

---

## Task 4: Setup-Rezept dokumentieren (nur bei H1- oder H2-Erfolg)

**Voraussetzung:** Genau eines der Tasks 1/2 hat `true`-Decision erreicht; Plugin lädt und wurde nicht zurückgerollt.

**Files:**
- Modify: `docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md` (§8 wird Setup-Rezept)
- Modify: `docs/superpowers/specs/2026-05-20-ccm-approvals-ready-design.md` (D2: Verweis auf §8)
- Modify: `docs/queue/Q0001-ccm-plugin-discovery-verifizieren.md` (Log + `status: done`)

- [ ] **Step 4.1: Setup-Rezept in Spec §8 schreiben**

Edit `docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md` — §8 komplett ersetzen durch (Vorlage; konkrete Variante = gewinnende Hypothese):

```markdown
## 8. Setup-Rezept / Negativ-Befund

**Status:** Verifiziert — <H1 | H2> ist der primäre Weg.

### Vorbedingungen

- `C:\Users\jz\source\ClaudeCodeManager\plugin\plugin.json` existiert.
- `~/.claude/plugins/` existiert (wird sonst angelegt).
- DBlicious-Repo unter `C:\Users\jz\source\DBlicious` — `.claude/settings.json` ist gültiges JSON.

### Setup (PowerShell)

```powershell
# 1. Backup
$settings = "C:\Users\jz\source\DBlicious\.claude\settings.json"
if (-not (Test-Path $settings)) { throw "settings.json fehlt" }
Copy-Item $settings "$settings.bak-q0001" -Force

# 2a. NUR bei H2:
$link = Join-Path $env:USERPROFILE ".claude\plugins\ccm"
if (-not (Test-Path (Split-Path $link))) {
  New-Item -ItemType Directory -Force -Path (Split-Path $link) | Out-Null
}
if (-not (Test-Path $link)) {
  New-Item -ItemType Junction -Path $link `
    -Value "C:\Users\jz\source\ClaudeCodeManager\plugin" | Out-Null
}

# 2b. NUR bei H1:
# (settings.json-Edit manuell oder via Skript — siehe Diff unten)

# 3. settings.json-Eintrag setzen (manuell editieren; JSON validieren):
Get-Content $settings -Raw | ConvertFrom-Json | Out-Null
Write-Host "Setup fertig — Claude-Code-Session neu starten"
```

Eintrag in `enabledPlugins`:
- Bei H2: `"ccm@local": true`
- Bei H1: `"ccm@local-ccm": true` plus `plugins.sources.local-ccm.path = "C:\\Users\\jz\\source\\ClaudeCodeManager\\plugin"`

### Verifikation nach Restart

```powershell
# JSON ist gültig
Get-Content "C:\Users\jz\source\DBlicious\.claude\settings.json" -Raw | ConvertFrom-Json | Out-Null
```
- In Claude Code: `/help` listet `ccm` / `ccm@local-ccm`.
- `SessionStart`-Hook-Output erscheint beim Start (auch ein Bash-„command not found"-Error reicht — wird in Sprint B durch `.ps1` ersetzt).

### Rollback

```powershell
Copy-Item "C:\Users\jz\source\DBlicious\.claude\settings.json.bak-q0001" `
          "C:\Users\jz\source\DBlicious\.claude\settings.json" -Force
# Bei H2 zusätzlich:
$link = Join-Path $env:USERPROFILE ".claude\plugins\ccm"
if (Test-Path $link) { Remove-Item $link -Force }
```

### Bekannte Einschränkungen

- Bootstrap-Hook ist Bash-only (`ccm-bootstrap-skill-check.sh`); unter Windows feuert er sichtbar, bricht aber ab. PowerShell-Port → Sprint B.
- Falls Marketplace-Push (Post-M3) kommt, kollidiert die lokale Source nicht — sie wird über `enabledPlugins` deaktiviert oder umbenannt.
```

Verbatim-Felder (Plugin-Listing-Zeile, Hook-Output) aus Task-1- bzw. Task-2-Beobachtung einfügen.

- [ ] **Step 4.2: Parent-Spec D2 verlinken**

Edit `docs/superpowers/specs/2026-05-20-ccm-approvals-ready-design.md` — Abschnitt D2 um eine Zeile ergänzen:

```markdown
**Verifizierter Pfad:** siehe Q0001-Spec §8 (`docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md`).
```

- [ ] **Step 4.3: Queue-Item abschließen**

Edit `docs/queue/Q0001-ccm-plugin-discovery-verifizieren.md`:

1. `status: brainstormed` → `status: done`.
2. `artifacts: []` → `artifacts: [docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md]`.
3. Log am Ende anhängen:
   ```
   - 2026-05-20T<HH:MM>:00Z — ccm-execute: <H1 | H2> verifiziert; Setup-Rezept in Spec §8; Parent-Spec D2 verlinkt.
   ```

- [ ] **Step 4.4: Task-4-Commit**

```powershell
git -C "C:\Users\jz\source\DBlicious" add `
  docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md `
  docs/superpowers/specs/2026-05-20-ccm-approvals-ready-design.md `
  docs/queue/Q0001-ccm-plugin-discovery-verifizieren.md `
  .claude/settings.json
# Bei H2 zusätzlich keine weitere Datei — die Junction ist außerhalb des Repos und wird nicht committet.
git -C "C:\Users\jz\source\DBlicious" commit -m "Q0001: Setup-Rezept für CCM-Plugin-Discovery (<H1|H2>) dokumentiert"
```

---

## Task 5: Abschluss-Verifikation + Backup-Cleanup

**Files:** keine Code-Änderung — nur Putzen + Final-Check.

- [ ] **Step 5.1: Acceptance-Criteria der Spec prüfen**

Run:
```powershell
Select-String -Path "C:\Users\jz\source\DBlicious\docs\superpowers\specs\Q0001-ccm-plugin-lokale-discovery-in-claude-design.md" -Pattern "^- \[x\]|^- \[ \]" | Select-Object LineNumber, Line
```
Erwartet: Alle Acceptance-Criteria-Checkboxen in §10 sind manuell auf `[x]` gesetzt (durch Edit der Spec); offene `[ ]` sind nur erlaubt, wenn der entsprechende Pfad nicht zutrifft (z.B. H3-Pfad bei H2-Erfolg).

- [ ] **Step 5.2: Backup-File aufräumen**

Run:
```powershell
$bak = "C:\Users\jz\source\DBlicious\.claude\settings.json.bak-q0001"
if (Test-Path $bak) { Remove-Item $bak -Force; Write-Host "Backup gelöscht" }
```

- [ ] **Step 5.3: Queue-Item-File ggf. nach `done/` schieben**

Run (nur wenn DBlicious dem CCM-Konvention folgt, dass erledigte Items in `docs/queue/done/` landen):
```powershell
$src = "C:\Users\jz\source\DBlicious\docs\queue\Q0001-ccm-plugin-discovery-verifizieren.md"
$dst = "C:\Users\jz\source\DBlicious\docs\queue\done\Q0001-ccm-plugin-discovery-verifizieren.md"
Test-Path "C:\Users\jz\source\DBlicious\docs\queue\done"
# Falls done/ existiert und der Move-Konvention entspricht:
# git -C "C:\Users\jz\source\DBlicious" mv $src $dst
```
Expected: Bei Existenz von `done/` ggf. `git mv`; sonst SKIP (Konvention nicht etabliert).

- [ ] **Step 5.4: Final-Commit (falls 5.2/5.3 etwas verändert haben)**

```powershell
git -C "C:\Users\jz\source\DBlicious" status --short
# Falls Output nicht leer:
git -C "C:\Users\jz\source\DBlicious" add docs/queue/
git -C "C:\Users\jz\source\DBlicious" commit -m "Q0001: Abschluss-Cleanup"
```

- [ ] **Step 5.5: Final-Report an Caller**

In §11 der Spec einen abschließenden Hinweis hinzufügen (oder als letzten Log-Eintrag im Queue-Item):

```markdown
**Investigation abgeschlossen am 2026-05-20.** Ergebnis: <H1 | H2 | H3>. Setup-Rezept / Negativ-Befund: §8.
```

---

## Self-Review (Hinweise für den Executor)

- Vor jedem `settings.json`-Edit: Backup. Nach jedem Edit: `ConvertFrom-Json` als JSON-Lint.
- Decision-Points sind **explizit** — beim Übergang zwischen Tasks kurz innehalten und prüfen, ob die Beobachtung wirklich die Bedingungen erfüllt (nicht halbwegs / „sieht aus wie").
- Junction-Removal: `Remove-Item -Force` (NICHT `-Recurse`, sonst wird das Target-Verzeichnis im CCM-Repo gelöscht).
- Wenn nach 90 Min keine Entscheidung steht: aktuelles Backup zurückspielen, Junction entfernen falls noch aktiv, Queue-Item auf `status: new` + Log-Eintrag, was hängt — dann STOP und an User zurückmelden.
- Nie an `~/.claude/skills/ccm-*`-Junctions anfassen; M0 bleibt unangetastet.
