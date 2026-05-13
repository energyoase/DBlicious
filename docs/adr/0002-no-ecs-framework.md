# ADR-0002: Kein ECS-Framework im Builder-State

- **Status**: Accepted
- **Datum**: 2026-05-13
- **Beteiligt**: solo

## Kontext

Der strategische Blueprint empfiehlt `bevy_ecs` (ohne den Game-Loop) als Metadaten-Engine fuer den Visual Builder. Argumente im Blueprint:

- **Components statt Vererbung**: ein UI-Element = `Transform` + `Style` + `Interactable` + `EventTrigger`-Components.
- **Kolumnare Memory-Layouts** fuer cache-friendly Iteration ueber tausende Knoten.
- **Enum-State-Components** (`Idle`/`Hover`/`Active`) statt sprawling State-Machines.

Diese Argumente sind in Computer-Spielen mit zigtausend Entities valide. Die Frage ist, ob sie fuer dblicious-Builder valide sind.

Beobachtungen:

- In einer Designer-Session sind typischerweise **5–50** UI-Knoten sichtbar (eine Tabelle, ein Form, eine Liste). Selbst mit Edge-Cases (komplexe Dashboards mit verschachtelten Sektionen) selten ueber 500.
- Performance-Vorteile von kolumnarem Memory zahlen sich erst bei vier- bis fuenfstelligen Knotenzahlen aus.
- Bevy/`hecs`/`legion` bringen substantielle Komplexitaet: Systems, Queries, Resources, Schedules, Stages, Component-Storage-Strategien. Lernkurve fuer Mitentwickler und Plugin-Autoren ist hoch.
- Bevy in WASM laeuft, ist aber nicht trivial — eigene Constraints, eigene Debug-Probleme.

## Entscheidung

Wir **uebernehmen das Konzept** "Komposition statt Vererbung" — aber **nicht das Framework**.

Builder-State ist ein `RwSignal<UiTree>` mit `UiTree { nodes: Vec<UiNode> }` (oder `HashMap<NodeId, UiNode>`, falls Indexierung noetig wird). Ein `UiNode` ist eine Plain-Rust-Struct mit optionalen Feldern:

```rust
pub struct UiNode {
    pub id:            NodeId,
    pub transform:     Option<Transform>,
    pub style:         Option<Style>,
    pub bound_field:   Option<BoundField>,
    pub event_trigger: Option<EventTrigger>,
    pub draggable:     bool,
    pub children:      Vec<UiNode>,
}
```

Optionale Felder + Sub-Struct-Komposition liefern die "Components-over-Inheritance"-Eigenschaft, ohne die ECS-Maschinerie.

## Alternativen

- **`bevy_ecs`** als ECS-Engine im Builder. Verworfen: Overkill fuer 5–50 Knoten, hohe Lernkurve, WASM-Debug-Komplexitaet.
- **`hecs`** als minimaler ECS. Verworfen: gleiche Argumente in milderer Form; die Konzept-Vorteile rechtfertigen die Abhaengigkeit nicht.
- **Trait-Object-basierte dynamische Komposition** (`Box<dyn UiComponent>`). Verworfen: schwerer typsicher, schwerer codegen-uebersetzbar.
- **Schemaless JSON im Client mit Reflection** (z.B. `serde_json::Value` als Builder-State). Verworfen: keine Compile-Time-Sicherheit, fehler-anfaellig.

## Konsequenzen

**Positive**

- Builder-Code ist in Stunden lesbar, nicht in Tagen.
- Codegen (Phase 4) hat triviale 1:1-Uebersetzung von `UiNode` auf Leptos-Komponenten-Views.
- Keine zusaetzliche Workspace-Dependency.
- Standard-Rust-Tooling (clippy, rust-analyzer) funktioniert ohne Spezialfaelle.

**Negative**

- Bei Skalierung jenseits 1000 Knoten *in einer Session* ist `Vec<UiNode>`-Iteration O(n) pro Mutation — bei kleinen Designer-Sessions vernachlaessigbar.
- Falls spaeter doch hunderte Tabellen mit hunderten Spalten parallel im Builder noetig sind, muss eine indexierte `HashMap<NodeId, UiNode>` nachgezogen werden (lokaler Refactor, nicht trivial aber ueberschaubar).
- Wir verlieren "automatische" Vorteile wie kolumnaren Memory — sehr unwahrscheinlich, dass das je relevant wird.

## Referenzen

- `ROADMAP.md` Phase 1
- `VISION.md` Sektion 2 "Komposition statt Vererbung im Builder-State"
- [ADR-0001](./0001-codegen-not-runtime.md) — uebergeordnetes Prinzip (Codegen statt Runtime-Engine)
