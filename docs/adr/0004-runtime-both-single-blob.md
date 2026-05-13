# ADR-0004: `runtime = "both"`-Plugins sind ein WASM-Blob, kein Doppel-Build

- **Status**: Accepted
- **Datum**: 2026-05-13
- **Beteiligt**: solo

## Kontext

dblicious unterstuetzt Plugins, die server-seitig laufen (`runtime = "server"`, z.B. `beforeSave`), client-seitig (`runtime = "client"`, z.B. `onClick`), oder in beiden Runtimes (`runtime = "both"`, z.B. ein Validator, der client-seitig fuer Live-Feedback und server-seitig als Autoritaet laeuft).

Frage: wie wird ein `"both"`-Plugin deployed? Drei Optionen wurden erwogen:

1. **Ein WASM-Blob, ein Code-Pfad** — derselbe Build laeuft in beiden Runtimes; Plugin-Autor pruefen zur Laufzeit, welche Host-Functions verfuegbar sind.
2. **Zwei separate Blobs** im selben Bundle (`server.wasm`, `client.wasm`) — Manifest verweist auf beide, Build-Workflow erzeugt zwei Artefakte.
3. **Ein Blob mit Conditional Compilation** (Rust-Feature-Flags o.ä.) — eine Codebasis, zwei kompilierte Varianten.

## Entscheidung

**Option 1: ein Blob, ein Code-Pfad, Laufzeit-Pruefung.**

Konkret:

- Plugin-Autor schreibt **eine** Codebasis. Daraus entsteht **ein** WASM-Blob.
- Der Host stellt jeweils nur die fuer die Runtime erlaubten Host-Functions zur Verfuegung. Server-Runtime hat `host.db.query`/`host.db.update`; Client-Runtime hat `host.ui.dispatch`/`host.ui.read`.
- Ein Aufruf einer in der aktuellen Runtime nicht-verfuegbaren Host-Function ist ein **Laufzeitfehler** (`host_function_unavailable`).
- `host.runtime() -> "server" | "client"` ist eine immer-verfuegbare Host-Function, mit der das Plugin Fallback-Pfade waehlen kann.

## Alternativen

- **Zwei separate Blobs**. Verworfen: Plugin-Autor-Komplexitaet (zwei Builds, zwei Test-Runs, zwei Versionen synchron halten). Doppelter Storage pro Plugin. Skript-Sprachen-PDKs (TS/Python/Go) haben oft keine native Conditional-Compilation, was die Idee von "zwei Artefakte aus einer Codebasis" erschwert.
- **Conditional Compilation**. Verworfen: erzwingt Build-Workflow-Komplexitaet auf Plugin-Autor-Seite; passt zu Rust, aber nicht zu allen anderen Extism-PDKs.

## Konsequenzen

**Positive**

- Plugin-Autor schreibt minimal: eine Codebasis, ein Test-Setup, ein Build.
- Eine Quelle der Wahrheit pro Plugin — Code in beiden Runtimes ist garantiert identisch (was z.B. `runtime = "both"` Validatoren wertvoll macht).
- Plugin-Storage im Server (`plugins.wasm_blob`) und Client-Cache (IndexedDB nach `(plugin_id, version)`) bleiben einfach.

**Negative**

- Laufzeitfehler statt Compile-Zeit-Fehler: ein Plugin, das `host.db.query` ohne Runtime-Check aufruft, crashed im Client. Mitigation: gute Doku, Beispiel-Patterns, Linter im PDK.
- Plugin-Autor muss aktiv pruefen — vergisst er das, ist die Plugin-UX schlecht.

**Architektonische Folgen**

- `host.runtime()` als immer-verfuegbare Host-Function in beiden Runtimes ist Pflicht.
- Audit-Log loggt `runtime: "server" | "client"` pro Host-Function-Call, damit man "es lief auf Server, nicht im Client"-Probleme nachvollziehen kann.
- Plugin-Beispiele (Phase 2.7) zeigen das Runtime-Check-Pattern.

## Referenzen

- `ROADMAP.md` Architektur-Vertraege §2 "Plugin-Manifest-Schema" — `runtime`-Feld
- `ROADMAP.md` Architektur-Vertraege §5 "Client-WASM-Laufzeit" — runtime-Verhalten
- [ADR-0005](./0005-server-and-client-wasm.md) — uebergeordnete Entscheidung: ueberhaupt Client-WASM-Runtime
