# ADR-0003: Server als alleinige Autoritaet fuer Permissions, Schema, Implementations

- **Status**: Accepted
- **Datum**: 2026-05-13
- **Beteiligt**: solo

## Kontext

Im aktuellen Code (Pre-Phase-0.7) ist die Permission-Logik primaer client-seitig: `client/src/auth/AuthContext` haelt eine Liste von Permissions und entscheidet `is_allowed(entity_type, op) -> bool`. Server-seitiges Enforcement ist rudimentaer. Aehnliches gilt fuer:

- Schema-Definitionen (in `--data-dir` geladen, Server haelt sie im RwLock, aber Client-Tabellen-Spalten waren bis 0.5.2 hartkodiert).
- Implementations-Wahl (Filter, Editor, Formatter): in 0.5.8 erstmals ueber `ColumnMeta.filter_id` server-getrieben, aber Client koennte theoretisch die ID ignorieren.

Browser-Code ist nicht trustworthy. Jeder reicht-zur-Implementierungsfreiheit eingeraumter Spielraum kann von einem motivierten Angreifer durch DevTools/Skripting umgangen werden.

## Entscheidung

**Server ist die einzige Quelle der Wahrheit** fuer:

- Permissions (Phase 0.7): Resolver `effective(user, resource, op)` laeuft serverseitig. Client bekommt eine **projizierte Sicht**, die nur UI-Vorbedingungen steuert (was wird angezeigt, was ist enabled), niemals als Sicherheits-Garantie.
- Schema-Definitionen: `ColumnMeta`, `EntitySettings`, `EntityEditor` kommen aus `entity_designs`/Loader; Client kennt sie nicht von sich aus.
- Implementations-Wahl (Phase 1.5): Server bestimmt erlaubte IDs und Default. Wenn User aus einer Liste waehlt, validiert der Server die Wahl gegen die Choose-Permission.

Konkret:

- Jeder GraphQL-Resolver fuehrt einen Permission-Check durch — kein Resolver verlaesst sich auf "Client hat schon gefiltert".
- Audit-Log haelt jeden Deny fest (Phase 0.7).
- `whyAllowed(user, resource, op) -> Trace` als Debug-Endpoint, damit man bei verwirrenden Effekten die Herkunfts-Kette nachvollziehen kann.

## Alternativen

- **Client-side Enforcement** (heutige Praxis, ausbauen). Verworfen: Browser nicht trustworthy.
- **Dual Enforcement Client+Server** mit dem Risiko, dass sich beide Implementierungen auseinander entwickeln. Verworfen: zwei Wahrheiten = keine Wahrheit. Client darf *projizieren*, aber niemals *erzwingen*.
- **Capability-basiertes Token-System** (Macaroons o.ä.). Verworfen: zusaetzliche Komplexitaet, fuer dblicious-Scope ueberzogen. Falls je benoetigt, in einer spaeteren Phase einfuehrbar.

## Konsequenzen

**Positive**

- Sicherheits-Argumente sind einfach: "Server prueft, Client zeigt". Ein Angreifer kann das UI manipulieren, aber keine Daten lesen/aendern.
- Permission-Aenderungen wirken sofort und konsistent — keine Out-of-Sync-Lage zwischen Browser-Cache und Server-Realitaet.
- Mehrere Clients (Browser, CLI, kuenftiger Mobile-Client) teilen sich dieselbe Wahrheit.

**Negative**

- Server-Resolver muss performant sein — pro CRUD-Aufruf darf nicht die ganze Permissions-Tabelle gescannt werden. Session-Cache ist Pflicht (siehe Phase 0.7).
- Doppelte Modellierung im Client wirkt redundant ("Server prueft das doch nochmal") — UI-Code muss trotzdem die Permission-Liste lesen, um z.B. Edit-Buttons auszublenden.

**Architektonische Folgen**

- Leitprinzip "Server als Authority" in `ROADMAP.md`.
- Phase 1.5 baut darauf auf: Implementations-Resolution funktioniert nur, weil der Server vorgibt, welche IDs ein User waehlen darf.
- Plugin-Permissions (Phase 2) folgen demselben Muster: Server entscheidet, was ein Plugin darf — Plugin-Code kann es nicht umgehen.

## Referenzen

- `ROADMAP.md` Architektur-Leitprinzipien, "Server als Authority"
- `ROADMAP.md` Phase 0.7 Auth-Modell, Phase 1.5 Implementations-Resolution
- [ADR-0005](./0005-server-and-client-wasm.md) — auch Plugins laufen unter Server-Authority, auch in der Client-Runtime
