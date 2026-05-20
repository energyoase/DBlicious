# ccm-approvals-ready — DBlicious-Adoption von ClaudeCodeManager

**Status:** Design v0.1 (2026-05-20)
**Konsument:** DBlicious
**Lieferant:** ClaudeCodeManager (`C:\Users\jz\source\ClaudeCodeManager`)
**Brainstorm-Session:** /superpowers:brainstorming, 2026-05-20

## Problem

DBlicious arbeitet heute mit Legacy-Triage (`.claude/triage-config.toml`, `docs/triage/T*.md`) und einer Hand voll Superpowers-Skills. ClaudeCodeManager (CCM) ist als universelles Lifecycle-Orchester konzipiert, kompiliert sauber, hat aber funktional nur **Iteration 1** (IPC, Hook-Binary, Storage, `private`-WS, Tray-Smoke) abgeschlossen. Iter 2–7 sind ausgeplant, aber nicht implementiert. Außerdem ist CCM:

- Nicht im Claude-Plugin-Marketplace verfügbar
- Bootstrap-Hook ist Bash-only (`.sh`) — Windows-nutzlos
- Hat keine etablierte Praxis für lokale Plugin-Installation
- Stack-Detection kennt Rust-Multi-Crate-Workspaces noch nicht

DBlicious soll der **erste produktive Konsument** von CCM werden. Dazu muss CCM bis Iter 3 fertiggebaut werden (Approvals + Telegram), die Plugin-Distribution Windows-tauglich gemacht und die DBlicious-Adoption (Triage→Queue) sauber durchgezogen werden.

## Ziele (End-Zustand)

1. DBlicious-Daily-Workflow läuft via CCM-Skills: `/ccm-brainstorm Q<id>` → `/ccm-plan` → `/ccm-execute` → optional `/ccm-review` → done. Status-Frontmatter in `docs/queue/Q*.md` führt durch.
2. `PreToolUse`-Approvals laufen end-to-end: Hook blockt, Daemon broadcastet `Frame::Wizard` an `private` (Tray) **und** `telegram` (Handy), Multi-Channel-Sync schließt Wizard auf allen Kanälen bei Antwort, Audit-Hash-Chain persistiert tamper-evident.
3. CCM-Daemon läuft als Hintergrundprozess auf Win11 (detached console, restart-bei-crash via Wrapper, log-rotiert), überlebt User-Logoff nur bedingt (Service-Migration ist Post-M3).
4. Plugin lokal installiert in `DBlicious/.claude/settings.json` → zeigt auf `C:\Users\jz\source\ClaudeCodeManager\plugin`. Kein Marketplace nötig.

## Non-Goals

- `ccm-loop`-Autopilot (braucht Iter 7 Orchestrator — separater Track)
- Dashboard (Iter 6 — schön, nicht produktionskritisch)
- Wizard-Engine (Iter 5 — interaktive Multi-Step-Telegram-Wizards, nicht für Approvals nötig)
- Scaffolder `ccm new` (Iter 4 — DBlicious existiert schon)
- Echte Service-Installation auf Windows (ADR-0009 sagt Console-First)
- Linux/macOS-Daemon-Lifecycle (Plattform-Scope laut CCM-Architektur: Windows-Primary)
- Marketplace-Veröffentlichung des CCM-Plugins (erst nach M3)

## Architektur — 5 Arbeitsströme

```
A1. CCM Iter-2 (Approvals+Audit-Chain)         A2. CCM Iter-3 (Telegram+Multi-Sync)
    Plan: ClaudeCodeManager/                       Plan: ClaudeCodeManager/
    docs/superpowers/plans/                        docs/superpowers/plans/
    2026-05-19-iter-2-approvals-                   2026-05-19-iter-3-telegram-
    audit-chain.md                                 multichannel.md
    ~1-2 Wochen                                    ~1 Woche, needs A1
                  │                                              │
                  └──────────────────┬───────────────────────────┘
                                     ▼
A3. Daemon-Background-Robustheit (~3-5 Tage)
   - Detach-Modus reliability (CREATE_NEW_PROCESS_GROUP-Pfad)
   - Restart-on-Crash Wrapper (kleines PowerShell-Script + Scheduled-Task)
   - Lock-File + Health-Endpoint
   - Log-Rotation via tracing-appender

                                     │
                                     ▼
B. Plugin-Distribution Windows-tauglich (~3-5 Tage, parallel zu A möglich)
   - PowerShell-Variante von ccm-bootstrap-skill-check
   - Lokale Plugin-Install verifizieren (settings.json-Pfad-Variante)
   - Skill-Symlinks ~/.claude/skills/ccm-* via PowerShell automatisiert

                                     │
                                     ▼
C. DBlicious-Adoption one-off (~2-3 Tage)
   - triage-config.toml → ccm.toml Migration
   - docs/triage/T*.md → docs/queue/Q*.md (Frontmatter-Mapping)
   - Plugin in settings.json eintragen
   - Telegram-Bot via @BotFather, Token in Keyring
   - ccm-doctor durchlaufen lassen, bis grün
   - 1 echter End-to-End-Run (Brainstorm → Plan → Execute mit Telegram-Approval)

                                     │
                                     ▼
D. Stack-Detection für Rust-Workspace (~1 Tag, isoliert)
   In CCM: skills/_shared/ccm-stack-detection.md erweitern um
   Multi-Crate-Workspace + WASM-Client + cargo-Targets.
```

## Milestones

| M  | Inhalt                    | Aufwand   | DBlicious kann dann                             | Es fehlt noch                  |
|----|---------------------------|-----------|--------------------------------------------------|--------------------------------|
| M0 | Sprint A-light (Quick-Win) | 1-3 Tage  | Skills `brainstorm/plan/execute/doctor` als Wrapper, kein Daemon | Tool-Approvals                 |
| M1 | A1 + B + C-light          | ~3 Wochen | Tool-Approvals **lokal im Tray**, Audit-Chain, Triage→Queue migriert | Mobile-Push                    |
| M2 | + A2                      | ~+1 Wo    | Telegram-Approvals aufs Handy                   | Daemon-Background-Robustheit   |
| M3 | + A3 + D                  | ~+1 Wo    | Daemon läuft im Hintergrund, log-rotated, Stack-Detect kennt DBlicious | (= produktionsreif)            |

**Gesamtaufwand: ~5 Wochen Solo**, davon ~90% gegen existierende Iter-2/Iter-3-Pläne in CCM.

## Quick-Win: Sprint A-light (sofort, 1-3 Tage)

Bevor irgendetwas Großes losgeht:

1. `~/.claude/skills/ccm-*` Symlinks auf `C:\Users\jz\source\ClaudeCodeManager\skills\ccm-*` via PowerShell `New-Item -ItemType SymbolicLink`. Pro Skill ein Symlink, 23 Stück.
2. Minimales `DBlicious/.claude/ccm.toml`:
   ```toml
   [ccm.project]
   name = "DBlicious"
   roles = ["dev"]
   mode = "solo"
   default_branch_main = "main"
   default_branch_dev = "dev"

   [ccm.meta]
   schema_version = 1
   created_by = "manual-bootstrap@2026-05-20"
   created_at = "2026-05-20T00:00:00Z"

   [ccm.questions]
   fallback_mode = "block"
   ```
3. `docs/queue/.gitkeep` + `docs/queue/done/.gitkeep` + `docs/queue/archived/.gitkeep` anlegen
4. Erste 1-2 Triage-Items als Test umkopieren: `T0023-iter7-...` → `Q0001-iter7-...` mit Frontmatter-Mapping (Triage `status: open` → CCM `status: new`; `created` ISO bleibt; `touches` bleibt; `severity: high` → CCM `priority: high`)
5. Probelauf: `/ccm-doctor` → Errors lesen → in iter-2/3 abarbeiten

**Was M0 nicht hat:** Channels, Daemon, Push, Audit-Hash-Chain, Auto-Loop. Skills laufen im "Daemon-Fallback"-Pfad, `ccm-ask` fragt in-Session via `AskUserQuestion`.

## Design-Entscheidungen

### D1 — Branch-Strategie in CCM

Zwei Feature-Branches in CCM-Repo: `feat/iter-2-approvals` und `feat/iter-3-telegram` (depend). Sequential mergen auf `dev`. Begründung: Iter-3 baut auf Iter-2-Tabellen auf; separate PRs erhalten Review-Granularität.

### D2 — Plugin-Install-Mechanik (Windows, lokal) — **Plan-B aktiv**

**Status (Update 2026-05-20 nach Q0001-Investigation):** H1 (settings.json `plugins.sources`-Block) und H2 (Junction in `~/.claude/plugins/<name>/`) **schlagen beide fehl**. Schema-Validation akzeptiert die Einträge, aber Claude Code aktiviert das Plugin nicht — Plugin-Hooks feuern nicht. Vollständiger Befund: [`Q0001-ccm-plugin-lokale-discovery-in-claude-design.md`](./Q0001-ccm-plugin-lokale-discovery-in-claude-design.md) §8.

**Root-Cause:** Claude-Code-Plugin-Discovery liest aus zwei Registry-Dateien (`~/.claude/plugins/known_marketplaces.json` + `~/.claude/plugins/installed_plugins.json`) und lädt aus `~/.claude/plugins/cache/<marketplace>/<plugin>/<version>/`. Ohne Marketplace-Registry-Eintrag wird `"ccm@local"` als dangling-Reference still ignoriert.

**Plan-B aktiv:** Skills-only via `~/.claude/skills/ccm-*`-Junctions (M0-Modus) bleibt der primäre Pfad. **Plugin-Lifecycle-Hooks (`SessionStart`, künftig `PreToolUse`) feuern nicht.** Konsequenzen für M1+:

- **M1-AC** `ccm-bootstrap-skill-check.ps1 läuft im SessionStart-Hook` wird **gestrichen** bzw. auf "manuell triggerbar via `/ccm-doctor`" reformuliert.
- **Sprint B** re-scoped: aus "Bootstrap-Hook portieren" wird **"H4-Tooling: `ccm-install-local`-Script"** — Marketplace-Entry + Plugin-Entry in den Registry-JSONs + Junction ins korrekte Cache-Layout, mit Backup + Rollback + Idempotenz. Q0001-Spec §8 listet die genauen Schritte.
- **Alternativ-Pfad zu Sprint B:** CCM via PR an `anthropics/claude-plugins-official` einreichen → Marketplace-Push (Post-M3-Track, langsamer).
- Bis dahin: Plugin-Dependencies werden nicht automatisch validiert; User aktiviert `superpowers`/`code-review`/`claude-md-management` ohnehin schon manuell.

### D3 — Triage→Queue-Migrations-Tool

Einmaliges PowerShell-Script unter `DBlicious/cli/scripts/migrate-triage-to-queue.ps1`, kein Skill. Mapping-Tabelle:

| Triage-Frontmatter           | CCM-Queue-Frontmatter         |
|------------------------------|-------------------------------|
| `id: T0023`                  | `id: Q0001` (neu nummeriert, monoton)  |
| `created: <ISO>`             | `created: <ISO>`              |
| `source: manual`             | `source: manual`              |
| `severity: high/medium/low`  | `priority: high/medium/low`   |
| `status: open`               | `status: new`                 |
| `status: done`               | `status: done` + move zu `done/` |
| `parallel: false`            | (entfällt; CCM hat `requirements`-Block) |
| `touches: [...]`             | `touches: [...]`              |
| `artifacts: [...]`           | (entfällt — wird im CCM-Lifecycle gefüllt) |
| `issue: null`                | (entfällt) |
| `brainstorm_required: true`  | `status: new` impliziert brainstorm-Phase |
| `# Title`                    | `title: "Title"` ins Frontmatter PLUS bleibt im Body |

Body wird 1:1 übernommen. Script schreibt einen Migrations-Bericht (welches T → welches Q).

### D4 — Telegram-Bot anlegen

User legt selbst via @BotFather einen Bot an (`/newbot`, Name z.B. `dblicious_ccm_bot`). Bot-Token landet in Windows Credential Manager:
```powershell
cmdkey /add:ccm /user:channel_telegram_token /pass:<token>
```
oder via `keyring`-CLI. CCM dokumentiert das Verfahren in der Spec, kein Skill dafür.

### D5 — DBlicious-Verification-Log-Format

`ccm-execute` verlangt einen JSON-Block mit `cmd/exit_code/stdout_tail` pro Verification-Schritt. Für DBlicious gilt Default:
```json
{
  "verification": [
    {"cmd": "cargo test --target-dir target-test --workspace", "exit_code": 0, "stdout_tail": "..."},
    {"cmd": "cargo clippy --target-dir target-test -- -D warnings", "exit_code": 0, "stdout_tail": "..."},
    {"cmd": "cargo fmt --check", "exit_code": 0, "stdout_tail": "..."}
  ]
}
```
Das `--target-dir target-test` ist Pflicht (CLAUDE.md sagt Server-Datei-Lock auf Windows). Pro Queue-Item kann das Frontmatter `verification_ops` weitere Schritte hinzufügen (z.B. `trunk build` für Client-Changes).

### D6 — Daemon-Background-Mechanik

A3 implementiert:
- `ccm daemon start --detach` nutzt `CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS` (Windows) wie ADR-0009 vorgegeben
- Lock-File `%LOCALAPPDATA%\ccm\daemon.lock` mit PID; `ccm daemon status` liest den Lock + macht Health-Probe auf der Pipe
- Restart-bei-Crash-Wrapper als PowerShell-Script `ccm-daemon-watchdog.ps1`, registrierbar als Windows-Scheduled-Task (Trigger: At Startup); Wrapper macht Loop `while ($true) { Start-Process ccmd.exe -Wait; Start-Sleep 5 }`
- Log-Rotation via `tracing-appender::rolling::daily` ins `%LOCALAPPDATA%\ccm\logs\`

Echte Windows-Service-Installation (`sc create`) ist Post-M3. Bis dahin: User muss sich nach Reboot einmalig per Start-Menü oder Auto-Run anmelden.

## Acceptance Criteria

### M0 (Sprint A-light)

- [ ] `~/.claude/skills/ccm-brainstorm/SKILL.md` ist via Symlink lesbar
- [ ] `DBlicious/.claude/ccm.toml` existiert und parsed von `ccm-doctor` ohne Schema-Errors
- [ ] `/ccm-brainstorm Q0001` legt eine Spec-Datei unter `DBlicious/docs/superpowers/specs/` an und updated Frontmatter auf `status: brainstormed`
- [ ] Mindestens 1 Triage-Item ist nach Q-Format migriert und in `docs/queue/Q*.md` sichtbar
- [ ] `ccm-doctor` läuft durch, listet Channels-fehlen als INFO (Solo-Mode), nicht WARN

### M1 (CCM Iter-2 + Plugin + C-light)

- [ ] CCM-Repo: `feat/iter-2-approvals` ist auf `dev` gemergt, `cargo test --workspace` grün
- [ ] ~~DBlicious: `ccm-bootstrap-skill-check.ps1` läuft im SessionStart-Hook~~ — **gestrichen** nach Q0001: Plugin-Hooks feuern nicht ohne H4-Tooling. Ersatz: `/ccm-doctor` manuell on-demand. Sprint B baut H4-Install-Script.
- [ ] DBlicious: Triage-Verzeichnis ist stillgelegt (Hinweis-README, da `docs/triage/` heute eh leer ist)
- [ ] Manueller Smoke-Test: `claude code` in DBlicious → versuche `Bash(rm -rf target-test)` → Tray-Toast erscheint mit Approve/Deny → Klick wird im Audit-Log persistiert
- [ ] `ccm-doctor` warnt nicht mehr

### M2 (+ CCM Iter-3)

- [ ] CCM-Repo: `feat/iter-3-telegram` ist auf `dev` gemergt
- [ ] DBlicious-Telegram-Bot ist via @BotFather angelegt, Token im Keyring
- [ ] `ccm channels add telegram` in DBlicious läuft den Pairing-Wizard durch (Bot-Username-Lookup, QR/Deep-Link, Chat-ID-Verify)
- [ ] Manueller Smoke-Test: Bash-Approval kommt parallel als Tray-Toast und Telegram-InlineKeyboard; Approve auf Telegram → Tray-Toast verschwindet, Tool wird ausgeführt, Audit zeigt `decided_by: telegram`

### M3 (+ A3 + D)

- [ ] `ccm daemon start --detach` lebt nach Schließen der Console
- [ ] Scheduled-Task `CCM-Daemon-Watchdog` ist installiert, startet bei User-Login
- [ ] Logs rotieren in `%LOCALAPPDATA%\ccm\logs\ccm.YYYY-MM-DD.log`
- [ ] `ccm-doctor --rescan-ops` erkennt DBlicious als Rust-Multi-Crate-Workspace und schlägt `cargo`-Verification-Ops vor
- [ ] Crash-Test: `taskkill /F /IM ccmd.exe` → Wrapper startet innerhalb 10s neu, Tray reconnected

## Risiken

| Risiko                                                  | Wahrscheinlichkeit | Mitigation                                                              |
|---------------------------------------------------------|--------------------|-------------------------------------------------------------------------|
| Claude-Code-Plugin-Discovery nimmt keinen lokalen Pfad  | **eingetreten** (Q0001 bestätigt) | Plan-B aktiv: Skills-only (M0-Mode dauerhaft). H4-Tooling oder Marketplace-Push als Lösungspfade — siehe D2 + Q0001-Spec §8 |
| Iter-2-Plan-Drift während Implementation               | niedrig            | TDD-Schritte fangen Drift; subagent-driven-development hält synchron    |
| Telegram-Bot-Token leakt in Logs/Config                | niedrig            | Keyring-only, in Spec hart festgehalten; `secrecy::SecretString` im Code |
| Daemon-Lifecycle auf Win11 (User schließt Console)     | hoch ohne A3       | A3 ist nicht optional; mindestens Detach + Lock-File + Watchdog          |
| 5-Wochen-Aufwand zu hoch                               | hoch               | M1 (3 Wo) liefert 80% Wert; M2/M3 sind Stop-Punkte                       |
| Iter-2 deckt subtile Race-Conditions im Approval-Roundtrip nicht ab | niedrig | Iter-2-Plan hat dedizierten `approval_roundtrip.rs` Integration-Test    |
| Triage-Migrations-Mapping verliert Information         | niedrig            | Migrations-Bericht macht jeden Schritt sichtbar; rollback via Git revert |
| Mehrere parallele Sessions in DBlicious erzeugen Hook-Konflikte | niedrig          | CCM-Daemon ist Single-Tenant pro User-Account; Hook-Requests sind ULID-getagged |

## Test-Plan

Da Spec auf existierende CCM-Pläne aufsetzt, testen sich A1/A2 über deren TDD-Schritte. DBlicious-spezifische Tests:

- **Integrationstest C1**: `migrate-triage-to-queue.ps1` auf Backup von `docs/triage/`, dann Diff-Vergleich der erzeugten `docs/queue/*.md` gegen erwartetes Frontmatter-Mapping
- **Integrationstest M1.1**: Mock-PreToolUse-Event über `ccm-hook` → erwartet Tray-Toast mit Buttons + Audit-Chain-Eintrag
- **Integrationstest M2.1**: PreToolUse-Event → erwartet sowohl Tray-Toast als auch Telegram-Message; Reply auf Telegram → Tray verschwindet
- **Smoke-Test M3.1**: Watchdog-Loop mit absichtlichem Daemon-Crash (taskkill) und Reconnect-Verify

## Implementation-Reihenfolge (Vorschlag für Plan-Phase)

Reihenfolge des Implementation-Plans (separater `writing-plans`-Lauf):

1. **Tag 1-2 (M0):** Sprint A-light. Symlinks, ccm.toml, Triage→Queue-Script, erste 1-2 Items migrieren. Echte Skill-Nutzung beginnt.
2. **Tag 3-5 (B):** Plugin-Discovery klären, Windows-Bootstrap-Hook portieren, Symlink-Automatisierung in eigenes PS1-Setup-Script. Parallel: erste Tasks aus Iter-2-Plan.
3. **Woche 2-3 (A1):** Iter-2-Plan abarbeiten in `feat/iter-2-approvals`. Per-Task TDD via `superpowers:subagent-driven-development`.
4. **Woche 4 (A2 + C):** Iter-3 + Telegram-Bot-Setup + Migration der restlichen Triage-Items + erster End-to-End-Run.
5. **Woche 5 (A3 + D):** Daemon-Robustheit + Stack-Detection. Smoke-Test bis grün.

## Referenzen

- CCM-Architektur: `C:\Users\jz\source\ClaudeCodeManager\docs\architecture.md`
- CCM-Iter-2-Plan: `C:\Users\jz\source\ClaudeCodeManager\docs\superpowers\plans\2026-05-19-iter-2-approvals-audit-chain.md`
- CCM-Iter-3-Plan: `C:\Users\jz\source\ClaudeCodeManager\docs\superpowers\plans\2026-05-19-iter-3-telegram-multichannel.md`
- CCM-Triage-Items: `C:\Users\jz\source\ClaudeCodeManager\docs\triage\T0016-iter2-approvals-audit.md`, `T0017-iter3-telegram-channel.md`
- ADR-0009 (Daemon-Lifecycle): `…\docs\decisions\0009-daemon-lifecycle-console-first.md`
- ADR-0007 (Audit-Hash-Chain): `…\docs\decisions\0007-audit-hash-chain.md`
- ADR-0003 (IPC Named Pipe): `…\docs\decisions\0003-ipc-named-pipe.md`
- DBlicious-CLAUDE.md (Verification-Konventionen): `C:\Users\jz\source\DBlicious\CLAUDE.md`
- DBlicious-ROADMAP.md (für Reihenfolgen-Abgleich): `C:\Users\jz\source\DBlicious\ROADMAP.md`
