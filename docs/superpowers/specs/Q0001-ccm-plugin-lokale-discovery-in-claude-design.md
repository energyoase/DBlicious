# Q0001 — CCM-Plugin lokale Discovery in Claude Code verifizieren

**Date:** 2026-05-20
**Status:** Investigation-Spec (Pre-M1)
**Queue-Item:** [`docs/queue/Q0001-ccm-plugin-discovery-verifizieren.md`](../../queue/Q0001-ccm-plugin-discovery-verifizieren.md)
**Parent-Spec:** [`2026-05-20-ccm-approvals-ready-design.md`](./2026-05-20-ccm-approvals-ready-design.md), Abschnitt D2
**Typ:** Investigation/Experiment — keine Architektur, kein Code-Output. Liefert eine Entscheidung plus Setup-Doku.

## 1. Problem

Milestone M1 der CCM-Approvals-Ready-Adoption setzt voraus, dass das CCM-Plugin (`C:\Users\jz\source\ClaudeCodeManager\plugin\plugin.json`) lokal in DBlicious aktiv ist — ohne Marketplace-Veröffentlichung. Nur dann greifen die `SessionStart`-Hooks (z.B. `ccm-bootstrap-skill-check`), die Dep-Resolution gegen `superpowers`/`code-review`/etc. und der saubere `enabledPlugins`-Lifecycle.

DBlicious arbeitet aktuell mit **Skills-only via Junctions** (Sprint M0 abgeschlossen): `~/.claude/skills/ccm-*` zeigen per Windows-Junction auf `C:\Users\jz\source\ClaudeCodeManager\skills\ccm-*`. Das funktioniert für die Skill-Discovery, aber:

- Plugin-Hooks (`SessionStart`, künftig `PreToolUse`) feuern nicht
- `plugin.json::dependencies` wird nicht ausgewertet → keine harte Garantie, dass `superpowers` mitinstalliert ist
- `enabledPlugins`-Schalter in `settings.json` greift nicht für ein nur-symlinktes Skill-Set

Daher die drei Hypothesen aus Abschnitt D2:

1. **H1 — Source-Path-Definition:** `settings.json` akzeptiert eine eigene Source-Definition, z.B.
   ```json
   "plugins": { "sources": { "local-ccm": { "path": "C:\\Users\\jz\\source\\ClaudeCodeManager\\plugin" } } }
   ```
   plus Eintrag `"ccm@local-ccm": true` unter `enabledPlugins`.
2. **H2 — Symlink ins Plugin-Verzeichnis:** Junction `~/.claude/plugins/ccm/` → `C:\Users\jz\source\ClaudeCodeManager\plugin`, dann Eintrag `"ccm@local": true` in `enabledPlugins`.
3. **H3 — Keine lokale Discovery möglich:** Claude Code verlangt Marketplace-Resolution; lokaler Pfad wird ignoriert oder mit Schema-Fehler abgelehnt. Plan-B = Skills-only-Pfad bleibt dauerhaft.

Diese Spec definiert, wie die drei Hypothesen mit minimalem Aufwand verifiziert werden, in welcher Reihenfolge, und wie das Ergebnis dokumentiert wird.

## 2. Ziele & Non-Goals

**Ziele:**
- Eine der drei Hypothesen ist **empirisch** verifiziert (Setup + Claude-Code-Restart + reproduzierbare Beobachtung).
- Die Setup-Schritte des erfolgreichen Szenarios sind so dokumentiert, dass M1 sie 1:1 übernehmen kann.
- Falls H3 gewinnt: Plan-B ist explizit bestätigt, und die Parent-Spec wird auf `Skills-only permanent` aktualisiert.

**Non-Goals:**
- Keine Implementierung des Bootstrap-Hooks in PowerShell (das ist Sprint B, separater Track).
- Kein Marketplace-Push-Versuch — explizit Post-M3 laut Parent-Spec.
- Keine Änderung an `~/.claude/skills/ccm-*`-Junctions; die bleiben unangetastet.
- Keine Telemetrie oder strukturiertes Log-Mining — Beobachtung läuft über sichtbare Effekte (Console-Output, Tool-Listings, Skill-Verfügbarkeit).

## 3. Experiment-Plan

Reihenfolge nach **Cost-to-test** (billigstes zuerst), damit bei frühem Erfolg die teureren Schritte entfallen.

### Phase 0 — Discovery der Plugin-Mechanik (~30 Min, einmalig)

Bevor irgendetwas modifiziert wird:

1. **context7 / docs.claude.com konsultieren** zu `settings.json`-Schema, `plugins`-Block, `enabledPlugins`-Format. Frage: gibt es offiziell dokumentierte lokale Source-Mechanik?
2. **Lokale Recherche:** existierende `enabledPlugins`-Einträge in `C:\Users\jz\source\DBlicious\.claude\settings.json` analysieren (`name@source`-Format). Schema-URL `https://docs.claude.com/plugin.schema.json` aus `plugin.json` aufrufen falls erreichbar.
3. **Output:** kurzer Notizblock in der Investigation (welche der drei Hypothesen plausibel ist nach Doku-Lage). Verschiebt nur Wahrscheinlichkeiten, ersetzt keinen Test.

### Phase 1 — H2 testen (Symlink, ~15 Min)

Billigster Test, weil Junctions in DBlicious bereits etabliertes Pattern sind.

### Phase 2 — H1 testen (Source-Path-Config, ~15 Min)

Nur wenn H2 fehlschlägt. Editiert `settings.json` strukturell; Rollback durch Backup-File.

### Phase 3 — H3 dokumentieren (falls 1+2 fehlschlagen, ~10 Min)

Plan-B aktivieren, Parent-Spec aktualisieren.

**Gesamtzeit-Budget:** 60-90 Min. Wenn nach 90 Min keine Entscheidung steht: Item zurück auf `status: new` mit Notiz, was hängt.

## 4. Hypothesen — Setup, Verifikation, Erfolgskriterien

### Phase-0-Befund (ausgefüllt während Execute)

- Backup angelegt: `.claude/settings.json.bak-q0001` (Step 0.3).
- Vorhandene Sources in settings.json: Nur `claude-plugins-official` (implizit via `enabledPlugins`-Einträge wie `"superpowers@claude-plugins-official": true`). Kein `plugins.sources`-Block vorhanden. Kein `extraKnownMarketplaces`-Block vorhanden.
- `~/.claude/plugins/` Inhalt: Verzeichnis existiert, aber **leer** (keine vorhandenen Einträge). H2-Junction kann kollisionsfrei angelegt werden.
- context7-Recherche: **Relevanter Befund** — Die offizielle Doku nennt `extraKnownMarketplaces` (nicht `plugins.sources`) als Mechanik für Custom-Sources. Format: `"extraKnownMarketplaces": { "<name>": { "source": { "source": "github", "repo": "..." } } }`. Kein `"path"`-Key für lokale Verzeichnisse dokumentiert. Für H1 müsste `plugins.sources.<name>.path` ein undokumentiertes/internes Feature sein. Die Doku zeigt außerdem `claude plugin install --scope local` als CLI-Befehl — aber der installiert aus Marketplaces, nicht aus lokalen Pfaden. Ein SDK-Snippet zeigt `{ type: "local", path: "./dev-plugins/my-plugin" }` im Agent-SDK-Kontext — könnte für H1 relevant sein, aber es ist unklar ob das für `settings.json` gilt.
- Hypothesen-Reihung nach Phase 0: **H2 zuerst** (Junction-Hack ist etabliertes DBlicious-Pattern, `~/.claude/plugins/` ist leer → kollisionsfrei). H1 danach mit `extraKnownMarketplaces`-Format statt `plugins.sources` (Doku korrigiert die Plan-Annahme). H3 als Fallback.

### H1 — Source-Path-Definition in `settings.json`

**Setup:**
1. Backup: `Copy-Item C:\Users\jz\source\DBlicious\.claude\settings.json settings.json.bak-q0001`
2. Block in `settings.json` ergänzen (vor `enabledPlugins`):
   ```json
   "plugins": {
     "sources": {
       "local-ccm": {
         "path": "C:\\Users\\jz\\source\\ClaudeCodeManager\\plugin"
       }
     }
   }
   ```
3. Eintrag in `enabledPlugins` ergänzen: `"ccm@local-ccm": true`
4. Datei speichern, Claude-Code-Session komplett beenden (alle Fenster).
5. Claude Code neu starten im DBlicious-Workspace.

**Verifikation (in dieser Reihenfolge):**
- **V1.1:** Startet die Session ohne Schema-Validation-Error? (Beobachtung: Status-Bar, Startup-Output, ggf. `~/.claude/logs/`)
- **V1.2:** `/help` oder Plugin-Liste enthält `ccm@local-ccm`?
- **V1.3:** Mindestens ein CCM-Skill (z.B. `/ccm-doctor`) ist über den Plugin-Pfad erreichbar (nicht nur über die Junction). Test: temporär eine Junction entfernen und prüfen, ob der Skill noch da ist. **Optional** — nur wenn nötig zur Eindeutigkeit.
- **V1.4:** Der `SessionStart`-Hook (`./hooks/ccm-bootstrap-skill-check.sh`) feuert sichtbar — selbst wenn er unter Windows abbricht (Bash-only), zeigt der Abbruch, dass das Plugin geladen wurde. Erwartet: Fehlermeldung in der Session-Console à la `sh: command not found` oder Hook-Error-Toast.

**Erfolg = H1 bestätigt:** V1.1 + V1.2 + (V1.3 ODER V1.4) sind `true`. Setup-Schritte werden in dieser Spec unter Abschnitt 6 dokumentiert.

**Fehlschlag:** Schema-Validation-Error, Plugin nicht in Liste, oder Session startet, ignoriert den Eintrag aber sichtbar (kein Hook-Feuer). Rollback via Backup, weiter zu H2.

### H2 — Symlink ins `~/.claude/plugins/`-Verzeichnis

**Setup:**
1. Prüfen ob `~/.claude/plugins/` existiert, sonst anlegen:
   ```powershell
   New-Item -ItemType Directory -Force "$env:USERPROFILE\.claude\plugins" | Out-Null
   ```
2. Junction anlegen (Windows-Junction, **nicht** Symlink — Junctions brauchen keinen Admin):
   ```powershell
   New-Item -ItemType Junction `
     -Path "$env:USERPROFILE\.claude\plugins\ccm" `
     -Value "C:\Users\jz\source\ClaudeCodeManager\plugin"
   ```
3. Backup: `Copy-Item .claude\settings.json settings.json.bak-q0001`
4. Eintrag in `enabledPlugins` ergänzen: `"ccm@local": true` (Source-Suffix `local` ist Konvention, ggf. weglassen — beide Varianten testen falls die erste fehlschlägt).
5. Claude-Code-Session beenden, neu starten.

**Verifikation:**
- **V2.1:** Session startet ohne Schema-Validation-Error.
- **V2.2:** `/help` oder Plugin-Liste enthält `ccm` (egal mit welchem Suffix).
- **V2.3:** Der `SessionStart`-Hook feuert (sichtbarer Output, auch wenn Hook abbricht — siehe V1.4).
- **V2.4:** Falls V2.1-V2.3 alle `false`: Variante ohne `@local`-Suffix testen (`"ccm": true`). Wenn auch das fehlschlägt → H2 verworfen.

**Erfolg = H2 bestätigt:** V2.1 + V2.2 + V2.3 sind `true`.

**Fehlschlag:** Plugin-Liste leer, oder Session ignoriert den Eintrag. Rollback. Weiter zu H3.

### H3 — Keine lokale Discovery möglich

**Aktivierung:** Wenn H1 und H2 beide fehlgeschlagen sind.

**Was zu tun ist:**
1. Diese Spec unter Abschnitt 7 mit dem Negativ-Befund ergänzen (welche Fehlermeldungen kamen, welche Doku-Stellen das bestätigen).
2. Parent-Spec `2026-05-20-ccm-approvals-ready-design.md` Abschnitt D2 aktualisieren:
   - Status auf "Plan-B aktiv" setzen.
   - Akzeptanzkriterium M1.2 (`ccm-bootstrap-skill-check.ps1` im SessionStart-Hook) streichen oder umformulieren auf "manuell durch User auszulösen".
   - Risiken-Tabelle Zeile `Claude-Code-Plugin-Discovery nimmt keinen lokalen Pfad` von `Wahrscheinlichkeit: mittel` auf `eingetreten` setzen.
3. Queue-Item Q0001 schließen (`status: done`), kurzer Final-Log-Eintrag mit Verweis auf diese Spec.

**Verifikation:** Keine — H3 ist die Negation. Die Beweislast liegt in den dokumentierten Fehlern aus H1/H2.

## 5. Entscheidungsbaum

```
┌── Phase 1: H2 (Symlink-Test)
│      ├── Erfolg → goto §6 (Dokumentation H2)
│      └── Fehlschlag → Phase 2
│
├── Phase 2: H1 (Source-Path-Test)
│      ├── Erfolg → goto §6 (Dokumentation H1)
│      └── Fehlschlag → Phase 3
│
└── Phase 3: H3 (Plan-B aktivieren)
       └── goto §7 (Plan-B-Doku + Parent-Spec-Update)
```

Die Reihenfolge H2-vor-H1 ist bewusst: Junctions sind die etablierte DBlicious-Mechanik (Skills laufen so), Schema-Änderungen an `settings.json` sind invasiver. Falls beide funktionieren, wird **H2 als primärer Weg** dokumentiert (weniger settings.json-Surface), H1 als Fallback erwähnt.

## 6. Dokumentations-Anforderung bei Erfolg (H1 oder H2)

Bei Erfolg wird in **diese Spec, Abschnitt 8** ein neuer Sub-Abschnitt "Setup-Rezept" eingetragen, der enthält:

1. **Vorbedingung:** welche Dateien/Verzeichnisse vorher da sein müssen (CCM-Repo, `plugin.json`).
2. **Schritt-für-Schritt-Befehle:** copy-pasteable PowerShell, kein Pseudo-Code, mit Fehlerprüfung (`if (-not (Test-Path ...)) { ... }`).
3. **Rollback-Pfad:** wie der User das wieder deinstalliert.
4. **Verifikations-Snippet:** wie der User selbst nach dem Setup prüfen kann, ob es geklappt hat (z.B. erwartete Console-Zeile beim Session-Start).
5. **Bekannte Einschränkungen:** etwa "Hook ist Bash-only, feuert sichtbar aber bricht ab — wird in Sprint B durch `.ps1` ersetzt".

Zusätzlich:
- Parent-Spec D2 erhält einen Verweis auf diesen Abschnitt ("verifizierter Pfad: siehe Q0001-Spec §8").
- Queue-Item Q0001 wird auf `status: done` gesetzt, `artifacts: [docs/superpowers/specs/Q0001-ccm-plugin-lokale-discovery-in-claude-design.md]`.

## 7. Dokumentations-Anforderung bei Fehlschlag (H3)

1. Abschnitt 8 dieser Spec wird mit "Negativ-Befund" überschrieben: Liste der getesteten Varianten, Fehlermeldungen, Quellen (Doku-Stellen, die das bestätigen).
2. Parent-Spec D2 wird auf "Plan-B aktiv: Skills-only permanent" gesetzt; Risiken-Tabelle, Acceptance-Criteria M1.2, Implementation-Reihenfolge Tag 3-5 entsprechend angepasst.
3. Ein neues Folge-Queue-Item wird **nicht** automatisch angelegt — der User entscheidet, ob ein Marketplace-Push-Track aufgemacht wird (Post-M3 sowieso).
4. CLAUDE.md erhält einen kurzen Hinweis-Absatz: "CCM ist Skills-only in DBlicious — Plugin-Hooks greifen nicht. Begründung: siehe Q0001-Spec."

## 8. Setup-Rezept / Negativ-Befund

**Resultat: H3 (Plan-B) bestätigt — Skills-only via M0-Junctions bleibt der primäre Pfad.**

### Phase 1 — H2-Beobachtung (Junction in `~/.claude/plugins/ccm/`)

**Setup tatsächlich durchgeführt am 2026-05-20:**
- Junction `~/.claude/plugins/ccm` → `C:\Users\jz\source\ClaudeCodeManager\plugin` angelegt (`Get-Item` `LinkType=Junction` bestätigt)
- `.claude/settings.json` erweitert: `"ccm@local": true` in `enabledPlugins` (JSON-Lint OK)
- Backup `.claude/settings.json.bak-q0001` angelegt
- Claude-Code komplett beendet (alle Fenster), in DBlicious neu gestartet

**Beobachtung nach Restart:**

| Check | Erwartet | Tatsächlich | Bewertung |
|-------|----------|-------------|-----------|
| V2.1 Schema-Validation | Eintrag bleibt erhalten, kein Error | Eintrag erhalten, Session startete fehlerfrei | ✅ PASS |
| V2.2 Plugin-Listing | `ccm`/`ccm@local` in Plugin- oder Skill-Liste | Alle `ccm-*`-Skills sichtbar — **aber:** Skills kommen bereits über M0-Junctions in `~/.claude/skills/`; nicht eindeutig vom Plugin-Pfad unterscheidbar | ❓ INKONKLUSIV |
| V2.3 SessionStart-Hook | `[CCM] SessionStart-Check (Stub)`-Output und/oder neuer Eintrag in `.ccm/audit.log` (`event=session_start_stub`) | `.ccm/audit.log` mtime nach Restart unverändert. Hook hat NICHT gefeuert | ❌ FAIL |

**Hook-Funktionstest (Kontrolle):** Hook manuell aus DBlicious-PWD ausgeführt (`bash plugin/hooks/ccm-bootstrap-skill-check.sh`) — schrieb erfolgreich den erwarteten Audit-Log-Eintrag, Exit 0. Das beweist: **der Hook selbst ist funktional, Claude Code hat ihn nur bei Session-Start nicht ausgelöst** → Plugin wurde von Claude Code nicht aktiviert.

**Fazit H2:** Schema-Validation akzeptiert den Eintrag, aber das Plugin-Lifecycle wird nicht ausgelöst. H2 ist **unzureichend** für vollständige Plugin-Aktivierung.

### Root-Cause: Echte Plugin-Discovery-Mechanik

Inspektion der Claude-Code-Plugin-Registry-Dateien ergab:

```
~/.claude/plugins/
├── known_marketplaces.json     ← Marketplace-Registry
├── installed_plugins.json      ← Installierte Plugins pro Marketplace + Projekt
├── plugin-catalog-cache.json   ← Catalog mit Tokens/Components-Metadata
├── marketplaces/<marketplace>/ ← Marketplace-Metadaten (.claude-plugin/, plugins/, …)
└── cache/<marketplace>/<plugin>/<version>/   ← echte Plugin-Quelle (HIER sucht der Loader)
```

Unser H2-Junction lag im **falschen Layout** (`~/.claude/plugins/<name>/`, nicht `~/.claude/plugins/cache/<marketplace>/<plugin>/<version>/`). Der `"ccm@local"`-Eintrag in `settings.json` referenziert einen Marketplace `local`, der in `known_marketplaces.json` **nicht existiert** → Claude Code akzeptiert ihn schema-technisch, kann ihn aber nicht zu einem konkreten Plugin auflösen.

### H4-Hypothese (NICHT in Q0001 getestet — Risiko zu hoch)

Eine echte lokale Installation würde erfordern:

1. Marketplace-Entry in `known_marketplaces.json` mit `source: { source: "local", path: "<plugin-pfad>" }` (Schema geraten, undokumentiert)
2. Plugin-Entry in `installed_plugins.json` mit korrektem `installPath` (Cache-Layout-Pfad)
3. Cache-Struktur `~/.claude/plugins/cache/local/ccm/0.1.0/` als Junction (oder Kopie) auf die Plugin-Quelle
4. Marketplace-Metadaten in `~/.claude/plugins/marketplaces/local/` (`.claude-plugin/`, `plugins/`, README.md)
5. Catalog-Eintrag in `plugin-catalog-cache.json` (Tokens/Components — sonst inkonsistentes Cost-Tracking)

**Warum nicht getestet:** Surgery betrifft **User-globalen Plugin-State**, nicht Projekt-State. Ein Fehler könnte die 11 funktionierenden offiziellen Plugins beschädigen. Außerhalb des Q0001-Investigations-Scopes. Implementierung gehört in ein M1-Tooling-Item (`ccm-install-local`-Script mit Backups + Idempotenz + Rollback).

### Phase 2 — H1 nicht durchgeführt

Begründung: Phase-0-Recherche (siehe §4) ergab, dass die dokumentierte Custom-Marketplace-Mechanik `extraKnownMarketplaces` nur GitHub-Sources beschreibt. Lokale Paths nur im Agent-SDK-Kontext (`{ type: "local", path: "..." }`) erwähnt, nicht für `settings.json`. H1 würde dasselbe Underlying-Problem wie H2 treffen (Marketplace-Registry nicht populated). Test wäre redundant.

### Cleanup nach Beobachtung

- `"ccm@local": true` aus `.claude/settings.json` entfernt (dangling reference)
- Junction `~/.claude/plugins/ccm/` entfernt (dangling, im falschen Layout)
- Backup `.claude/settings.json.bak-q0001` entfernt
- M0-Junctions in `~/.claude/skills/ccm-*` BLEIBEN — produktive Mechanik

### H3 (Plan-B) — Bestätigte Schlussfolgerung

**Skills-only via `~/.claude/skills/ccm-*`-Junctions (Sprint M0) ist und bleibt der primäre Pfad für CCM in DBlicious**, bis eine der folgenden Bedingungen eintritt:

1. Claude Code lokale Plugin-Quellen **offiziell und dokumentiert** unterstützt, ODER
2. CCM auf einem öffentlich erreichbaren GitHub-Marketplace publiziert wird (Anthropic-offiziell oder eigene Org-Marketplace), ODER
3. Ein M1-Item liefert ein `ccm-install-local`-Script, das den H4-Surgical-Install sicher (Backup + Rollback + Idempotenz + Schema-Verifikation) durchführt.

### Konsequenzen für CCM-Roadmap (Parent-Spec D2)

- M1-Akzeptanzkriterium `ccm-bootstrap-skill-check.ps1 läuft im SessionStart-Hook` ist **nicht erreichbar** ohne H4-Tooling (Hooks feuern nicht ohne aktivierte Plugin-Registry-Einträge). M1-AC reformulieren auf "manuell durch User triggerbar" oder als "blockiert bis H4-Tooling".
- Sprint B (`Plugin-Distribution Windows-tauglich`) re-scopen: aus "Bootstrap-Hook portieren" wird "H4-Install-Script + PowerShell-Hook-Variante bauen".
- Risiken-Tabelle der Parent-Spec: Zeile `Claude-Code-Plugin-Discovery nimmt keinen lokalen Pfad` von `Wahrscheinlichkeit: mittel` auf `eingetreten` (Plan-B aktiv) setzen.

### Verifizierter Status für CCM in DBlicious

- ✅ **23 CCM-Skills laden via M0-Junctions** (`/ccm-doctor`, `/ccm-brainstorm Q<id>`, `/ccm-plan`, `/ccm-execute`, …)
- ✅ **Lifecycle in dieser Session bewiesen:** Q0001 selbst durchlief `ccm-brainstorm → ccm-plan → ccm-execute` ohne Daemon, ohne Plugin (`audit.log` und Commits `3200372` / `2588f4d` / `c71f495` / `74c70a1` / `f28f4bb` belegen es)
- ❌ **Plugin-Hook `SessionStart` feuert nicht** — Daemon-spezifische Dep-Checks, automatische CLAUDE.md-Pflege etc. müssen alternativ getriggert werden (z.B. via `/ccm-doctor` on-demand)
- ⚠️ **Plugin-Dependencies werden nicht automatisch validiert** — der User aktiviert die Required-Deps (`superpowers`, `code-review`, `claude-md-management`) ohnehin schon manuell in `~/.claude/settings.json`

## 9. Risiken & Mitigationen

| Risiko                                                         | Wahrscheinlichkeit | Mitigation                                                                       |
|----------------------------------------------------------------|--------------------|----------------------------------------------------------------------------------|
| `settings.json`-Edit korrumpiert die Datei (JSON-Syntax-Fehler) | mittel             | Pflicht-Backup `settings.json.bak-q0001` vor jedem Edit; JSON-Lint nach Edit     |
| Plugin lädt, aber Hook bricht ab (Bash-only) und blockt Session | niedrig            | Vorab `chmod +x`-Frage prüfen; falls Session-Blockade: `enabledPlugins`-Eintrag zurücksetzen |
| H2-Junction kollidiert mit zukünftigem Marketplace-Install     | niedrig            | Spec-Output dokumentiert das explizit; `claude-plugins-official`-Source bleibt unangetastet |
| Phase 0 (Doku-Recherche) konsumiert mehr Zeit als der Test selbst | niedrig            | Hard-Cap 30 Min für Phase 0; bei Überschreitung direkt zu Phase 1                |
| Claude-Code-Schema-Validierung wirft cryptische Fehler         | mittel             | Volltext-Logging der Fehlermeldung in §8; Doku-Suche zum Fehlertext nachgelagert |
| `~/.claude/plugins/` ist von Claude-Code reserviert und Symlinks werden überschrieben | niedrig | Junction-Test ist non-destruktiv; falls Claude beim Start löscht, wird das in V2.1 sichtbar |

## 10. Acceptance Criteria

- [ ] Phase 1 (H2) wurde durchgeführt; Verifikations-Ergebnis (Erfolg/Fehlschlag) ist in §8 dokumentiert.
- [ ] Falls H2 fehlgeschlagen: Phase 2 (H1) wurde durchgeführt und dokumentiert.
- [ ] Falls H1 fehlgeschlagen: Phase 3 (H3) ist aktiviert; Parent-Spec D2 ist aktualisiert.
- [ ] §8 enthält entweder ein vollständiges Setup-Rezept (H1/H2) oder einen vollständigen Negativ-Befund (H3) — kein TBD.
- [ ] Queue-Item Q0001 ist auf `status: done` gesetzt mit Verweis auf diese Spec als Artifact.
- [ ] Falls H3: CLAUDE.md hat den Skills-only-Hinweis-Absatz.

## 11. Referenzen

- Queue-Item: [`docs/queue/Q0001-ccm-plugin-discovery-verifizieren.md`](../../queue/Q0001-ccm-plugin-discovery-verifizieren.md)
- Parent-Spec: [`docs/superpowers/specs/2026-05-20-ccm-approvals-ready-design.md`](./2026-05-20-ccm-approvals-ready-design.md) (Abschnitt D2)
- CCM-Plugin-Manifest: `C:\Users\jz\source\ClaudeCodeManager\plugin\plugin.json`
- CCM-Plugin-Hook: `C:\Users\jz\source\ClaudeCodeManager\plugin\hooks\ccm-bootstrap-skill-check.sh` (Bash-only — wird in Sprint B portiert)
- DBlicious-Settings: `C:\Users\jz\source\DBlicious\.claude\settings.json`
- Bestehende Junctions (M0): `~/.claude/skills/ccm-*` → `C:\Users\jz\source\ClaudeCodeManager\skills\ccm-*`
