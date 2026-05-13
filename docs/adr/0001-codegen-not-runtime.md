# ADR-0001: Codegen statt generischer Runtime-Engine

- **Status**: Accepted
- **Datum**: 2026-05-13
- **Beteiligt**: solo

## Kontext

Der strategische Blueprint fuer dblicious schlaegt vor, einen *generischen Runtime-Builder* zu bauen: eine Engine, die zur Laufzeit beliebig konfigurierbare UI-Komponenten, Datenbindings und Event-Handler instanziiert und verwaltet. Konkrete Vorschlaege im Blueprint:

- Bevy ECS als Metadaten-Engine fuer den Visual Builder.
- Reflection-/Trait-basierte dynamische Komposition zur Laufzeit.
- Eine "Components-over-Inheritance"-Engine, die zur Laufzeit Configs interpretiert.

Das wuerde fuer den **Builder** Performance-Optimierungen erfordern (kolumnarer Memory, Cache-friendliness, schnelle Iteration ueber viele Knoten) und fuer das **Produkt** eine permanente Interpreter-Schicht mit Overhead bedeuten.

Beobachtung: die produktive Nutzung der gebauten Anwendungen ist von der Builder-Session getrennt. Der Builder ist Entwicklungsumgebung — er muss interaktiv reagieren, nicht hochlastig. Das Produkt ist eine Endnutzer-Anwendung, die fixe, kompilierte Komponenten braucht und nicht zur Laufzeit konfigurieren koennen muss.

## Entscheidung

Wir bauen **keine generische Runtime-Engine**. Statt dessen:

- Der Builder haelt einen simplen, kodexgen-nahen Datenstruktur (`UiTree` aus `UiNode`s).
- Das **Endprodukt** entsteht via **Codegen** (Phase 4): aus dem finalen `UiTree` wird eine eigene, fixierte Komponenten-Crate generiert. Pro Konfigurations-Variante eine eigene generierte Komponente.
- Austauschbarkeit existiert nur **ueber User-Configs**, nicht zur Laufzeit. Plugins (Phase 2) sind die einzige Ausnahme — sie laufen sandboxed, aber als zusaetzlicher Layer, nicht als Konfigurations-Engine.
- Performance-Vorgaben fuer den Builder: "muss interaktiv reagieren" (~16 ms), nicht "muss tausende Knoten pro Frame iterieren".

## Alternativen

- **Bevy ECS / `hecs` / `legion`** als Runtime-Engine fuer den Builder. Verworfen: Performance-Vorteile zahlen sich erst vier- bis fuenfstellig aus; der Builder zeigt 5–50 Knoten. Siehe [ADR-0002](./0002-no-ecs-framework.md).
- **Reflection-/Trait-Object-basierte Dynamik**: zur Laufzeit Komponenten austauschen. Verworfen: hohe Komplexitaet, schwer testbar, schwer typsicher.
- **Hybrid: Runtime-Engine im Dev-Mode, statischer Codegen-Output im Prod-Mode** (wie SurrealDB Schemaless↔Schemafull). Verworfen: zwei Codepfade fuer dieselbe Funktion, doppelter Wartungsaufwand.

## Konsequenzen

**Positive**

- Builder-Code bleibt simpel und debuggbar (Plain-Rust + Leptos-Signals).
- Produktions-App ist nativer Rust-Code, kompiliert und optimiert — keine Interpreter-Overhead.
- Codegen-Output ist normaler Rust, kann mit Standard-Tools (clippy, cargo bench, IDE) bearbeitet werden.

**Negative**

- Builder-Datenstruktur muss zu jedem Zeitpunkt codegen-uebersetzbar bleiben — eine zusaetzliche Constraint.
- Kein Live-Hot-Swap von Komponenten zur Laufzeit (ausser ueber Plugins).
- Codegen wird in Phase 4 zu einem zentralen Werkzeug — wenn der Codegen-Pfad zerbricht, ist das Produkt blockiert.

**Architektonische Folgen**

- "Codegen ist nicht ein optionales Feature in Phase 4, sondern **das Produkt**." — Leitprinzip in `ROADMAP.md`.
- Builder-Komponenten muessen klein und atomar bleiben (1 `UiNode` ⇒ 1 generierte Komponente).
- Keine impliziten Cross-Cutting-Mechanismen, die zur Laufzeit globale Effekte haben (keine globalen Event-Busse o.ä.).

## Referenzen

- `ROADMAP.md` — Architektur-Leitprinzipien, "Dev/Prod-Asymmetrie"
- `VISION.md` — Sektion 2 "Komposition statt Vererbung im Builder-State"
- `ROADMAP.md` — Phase 1 und Phase 4
- [ADR-0002](./0002-no-ecs-framework.md) — Konkretisierung: kein ECS-Framework
