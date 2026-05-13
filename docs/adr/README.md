# Architecture Decision Records

Strukturierte Aufzeichnungen architektonischer Entscheidungen. Jeder ADR haelt den Kontext, die Entscheidung, betrachtete Alternativen und die Konsequenzen fest. Ziel: ein neuer Entwickler kann in einem halben Jahr nachvollziehen, **warum** wir Variante X gewaehlt haben — nicht nur was X ist.

## Wann einen ADR schreiben?

- Eine Entscheidung, die mehrere Module/Phasen betrifft.
- Eine Wahl zwischen mehreren plausiblen Alternativen mit unterschiedlichen Trade-Offs.
- Eine bewusst verworfene Option, die spaeter wieder vorgeschlagen werden koennte.
- Ein Leitprinzip, das kuenftige Entscheidungen einschraenkt.

**Nicht** als ADR: Implementierungsdetails, Code-Style, einzelne Bug-Fixes. Dafuer Git-Commits, CLAUDE.md, oder normale Code-Kommentare.

## Format

Wir folgen einer Kurzfassung des Michael-Nygard-Stils. Eine Datei pro ADR, durchnummeriert (`NNNN-short-kebab-title.md`):

```markdown
# ADR-NNNN: Titel in Aussagesatz-Form

- **Status**: Proposed | Accepted | Deprecated | Superseded by ADR-XXXX
- **Datum**: YYYY-MM-DD
- **Beteiligt**: <Namen oder "solo">

## Kontext
Welches Problem loesen wir? Welche Zwaenge gibt es?

## Entscheidung
Was haben wir entschieden, in einem Satz, dann Details.

## Alternativen
Welche anderen Optionen wurden ernsthaft erwogen, mit kurzer Begruendung fuers Verwerfen.

## Konsequenzen
Positive und negative Folgen. Was wird einfacher? Was wird schwerer?

## Referenzen
Links zu ROADMAP-Sektionen, anderen ADRs, externer Doku.
```

Status-Verlauf:
- **Proposed** — vorgeschlagen, noch nicht angenommen.
- **Accepted** — angenommen, aktiv gueltig.
- **Deprecated** — nicht mehr empfohlen, aber noch im Code.
- **Superseded by ADR-XXXX** — durch neueren ADR ersetzt.

## Index

| ADR | Titel | Status | Datum |
|---|---|---|---|
| [0001](./0001-codegen-not-runtime.md) | Codegen statt generischer Runtime-Engine | Accepted | 2026-05-13 |
| [0002](./0002-no-ecs-framework.md) | Kein ECS-Framework im Builder-State | Accepted | 2026-05-13 |
| [0003](./0003-server-as-authority.md) | Server als alleinige Autoritaet fuer Permissions, Schema, Implementations | Accepted | 2026-05-13 |
| [0004](./0004-runtime-both-single-blob.md) | `runtime = "both"`-Plugins sind ein WASM-Blob | Accepted | 2026-05-13 |
| [0005](./0005-server-and-client-wasm.md) | Plugins laufen Server- UND Client-seitig (WASM-in-WASM) | Accepted | 2026-05-13 |
| [0006](./0006-erp-platform-layer.md) | ERP-Plattform-Bausteine als eigene Phase 1.7 (nicht in Phase 2) | Accepted | 2026-05-13 |

## Backlog (zu schreiben)

Weitere in dieser Session getroffene Entscheidungen, fuer die ein ADR-Eintrag aussteht:

- Implementations-Resolution Reihenfolge (`column.id` → entity_type-Default → fieldType-Default → Client-Fallback)
- Glob-Wildcard-Grammatik (`globset`-basiert, nicht single-`*` oder Prefix)
- Cargo-Style SemVer-Range-Intersect fuer Plugin-Dependencies
- Effect-Enum typisiert in `shared` + `Custom`-Erweiterungspunkt
- Append-only `entity_designs` mit Optimistic-Locking statt CRDT
- `projection`-Feld redundant in `entity_designs` (Server-lesbar ohne Builder-Code)
- Loader als Seed, DB als Laufzeit-Source-of-Truth
- Zweiphasige Migration (Expansion/Cutover/Contract) + 24h-Rollback-Fenster
- Dual-Write/Dual-Read in Migration-`Expanded`/`CuttingOver`
- `Approve` ≠ `Contract` als getrennte Migration-Permissions
- Permission-Resolver: Deny-vor-Allow bei gleicher Spezifitaet
- Plugin-Distribution: pull-basierter IndexedDB-Cache (kein Server-Push)
- Phase 0.7 enforced nur `EntityType`+`EntityProperty` (Instance-Level deferred)
- Plugin-Host-Function-Audit pro Call, Client batched bis Page-Unload
- Composable Table-Komponenten (`EntityTableShell` + Children) statt Slot-Props
- `FilterRegistry` pro Shell-Instanz, nicht App-global
- Parallel Groups + Roles statt nur Groups oder nur Roles
- Multi-Tenancy: `tenant_id NULL` Schema-Vorbereitung, kein Enforcement in 0.7
- Anonymous-User als impliziter Subject mit Default-Role, abschaltbar via Config

Reihenfolge des Schreibens: bei Bedarf, nicht alle auf einmal. Wenn jemand auf einen Punkt zurueckkommt und nach dem "warum" fragt, ist das der Anlass.
