# ADR-0005: Plugins laufen Server- UND Client-seitig (WASM-in-WASM)

- **Status**: Accepted
- **Datum**: 2026-05-13
- **Beteiligt**: solo

## Kontext

Wenn ein UI-Trigger feuert (User klickt, tippt, submitted), wo laeuft die Plugin-Logik, die darauf reagiert? Drei Optionen:

1. **Nur server-seitig**: UI-Trigger erzeugt einen GraphQL-Aufruf, Plugin laeuft in der Server-Sandbox, Antwort kommt zurueck. UI rendered die Antwort.
2. **Server- UND Client-seitig**: Plugins mit `runtime = "client"` oder `"both"` laufen in einer Extism-Runtime **innerhalb** des Browser-WASM-Clients — WASM-in-WASM.
3. **Nur server-seitig, UI-Trigger entfallen aus dem Vertrag**: keine `Click`/`Change`/`Submit`-EventKinds; alles geht durch Save-Lifecycle.

## Entscheidung

**Option 2: Server- UND Client-WASM.**

Konkret:

- Plugin-Manifest deklariert `runtime = "server" | "client" | "both"`.
- Client-Plugins werden bei Page-Load vom Server geliefert (capability-gegated) und in einer Extism-WASM-Instanz innerhalb des Leptos-WASM-Clients ausgefuehrt.
- UI-Triggers (`onClick`, `onChange`, `onSubmit`) laufen client-seitig und koennen `Effect`-Werte zurueckgeben, die der Host als deklarative UI-Mutationen ausfuehrt.
- CRUD-Triggers (`beforeSave`, `afterSave`, `beforeDelete`) bleiben server-seitig.
- `validate`, `deriveField`, `customAction` koennen je nach Plugin auf einer der beiden Runtimes laufen.

## Alternativen

- **Option 1 (nur server)**: pro UI-Event ein GraphQL-Roundtrip. Verworfen: schlechte UX fuer Live-Feedback (Tastatur-Echtzeit-Validierung, dynamisch berechnete Felder). Latenz pro Tastendruck mehrere hundert ms ueber Internet ist nicht akzeptabel.
- **Option 3 (UI-Triggers entfallen)**: Verworfen: Builder verliert UI-Reaktivitaet ohne Server-Roundtrip; Plugin-Autoren koennen keine Live-Reaktion bauen.

## Konsequenzen

**Positive**

- Live-Feedback fuer User: Validatoren reagieren mit Tastatur-Geschwindigkeit, abgeleitete Felder aktualisieren sofort.
- Plugin-Autoren bekommen volle Reaktivitaet: sie koennen UI-Logik in derselben Sprache und mit denselben Mustern schreiben wie Server-Logik.
- `runtime = "both"` erlaubt das doppelte Pattern (Live-Feedback im Client, Autoritaet im Server) **mit derselben Codebasis** (siehe [ADR-0004](./0004-runtime-both-single-blob.md)).

**Negative**

- WASM-in-WASM: Extism im Browser-WASM-Client ist messbar (mehrere ms pro Aufruf). Fuer Hot-Path-Triggers (`onChange` waehrend Tastatur-Eingabe) muss das Plugin idempotent und schnell bleiben — Empfehlung <5 ms.
- **Doppelte Sandbox-Boundary**: zwei Stellen, an denen Capability-Checks durchgesetzt werden. Mehr Pruefcode, mehr Audit-Logs. Aber: notwendig, weil Browser-Code nicht trustworthy ist.
- **Plugin-Distribution-Komplexitaet**: Server muss Plugin-Blobs an erlaubte Clients ausliefern (`/plugins/manifest`-Endpoint mit Capability-Filter), Client cached in IndexedDB.
- **Capability-Doppelpruefung**: Server entscheidet, welche Plugins ein User bekommt (Auslieferungs-Filter). Innerhalb des Client-WASM prueft der Host nochmals jede Capability — Doppel-Check ist Pflicht.

**Architektonische Folgen**

- `host.runtime()`-Host-Function ist immer verfuegbar — siehe [ADR-0004](./0004-runtime-both-single-blob.md).
- `Effect`-Enum als Plugin-zu-Host-Kommunikationsformat fuer UI-Mutationen (Plugin manipuliert nie direkt DOM, siehe Architektur-Vertraege §3).
- Client-Plugin-Cache braucht Versionsverwaltung — pull-basiert ueber `/plugins/manifest` bei Page-Load (kein Server-Push, kein WebSocket).
- Reentrancy-Schutz auch im Client (`onChange → host.ui.dispatch({setField}) → onChange → …`).
- Audit-Log loggt `runtime: "server" | "client"` pro Call (Client-Audits batched bis Page-Unload).

## Referenzen

- `ROADMAP.md` Architektur-Vertraege §3 "Trigger-Point-Vertraege"
- `ROADMAP.md` Architektur-Vertraege §5 "Client-WASM-Laufzeit"
- [ADR-0003](./0003-server-as-authority.md) — Server bleibt Autoritaet, auch ueber Client-Plugins
- [ADR-0004](./0004-runtime-both-single-blob.md) — ein Blob fuer `runtime = "both"`
