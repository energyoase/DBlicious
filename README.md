# DBlicious (Iteration 1)

Erweiterbarer Rust-WebAssembly-Client auf Basis von **Leptos**, der über
**GraphQL** mit einem in Rust implementierten **Mock-Server** kommuniziert.

## Workspace-Aufbau

```
.
├── shared/   # Geteilte Typen (NavigationNode, ColumnMeta, FieldType, ...)
├── server/   # axum + async-graphql, liefert statische Mock-Antworten
└── client/   # Leptos CSR + WASM, Trunk-Build
    ├── locales/         # Project-Fluent-Dateien (de, en)
    └── src/
        ├── app.rs
        ├── components/  # Navigation, generische Tabelle
        ├── graphql/     # GraphQL-Client + Queries
        ├── i18n/        # Fluent-basierte Lokalisierung
        ├── routes/      # Routen-Komponenten
        └── styling/     # DesignSystem-Trait + InlineDesign
```

## Voraussetzungen

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk
```

## Starten

```bash
# Terminal 1 – Mock-Server (Port 8000, GraphiQL unter /)
cargo run -p server

# Terminal 2 – Client (Port 8080, proxiert /graphql auf 8000)
cd client
trunk serve
```

Anschließend `http://127.0.0.1:8080` öffnen.

## Architekturentscheidungen

### 1. Trennung von Struktur und Design

Komponenten kennen ausschließlich das `DesignSystem`-Trait
(`client/src/styling/mod.rs`). Sie greifen weder auf CSS-Klassen noch
auf Style-Strings direkt zu, sondern fragen semantisch nach `surface(...)`,
`text(...)`, `nav_item(...)` usw. Aktuelle Implementierung: `InlineDesign`
mit CSS-in-Rust auf Token-Basis.

**Wechsel auf Tailwind / Stylance / anderes System:**

1. Neue Implementierung des Traits anlegen, z.B. `TailwindDesign`.
2. `Style::class("...")` statt `Style::inline("...")` zurückgeben.
3. In `app.rs` `provide_design_system()` auf die neue Implementierung
   umstellen – einzige Änderungsstelle in der Codebasis.

### 2. Generische Tabelle

`<EntityTable>` arbeitet ausschließlich auf:

- `Vec<ColumnMeta>` – Spaltenbeschreibung mit `FieldType` aus `shared`
- `Rc<dyn DataSource>` – Datenbeschaffung als Trait

`column_set_for("product")` ist heute hartkodiert, hat aber dieselbe
Signatur wie die GraphQL-Query `fetch_columns("product")`. Der Wechsel
erfordert in `routes/mod.rs` nur den Austausch der Datenherkunft.

### 3. Sortierung, Filterung, Pagination

`TableState` hält reaktive Signale für `page`, `page_size`, `sort` und
`filter`. Die UI-Elemente (Spaltenklick, Suchfeld, Pagination-Buttons)
sind voll verdrahtet und triggern Reloads. **In dieser Iteration ignoriert
der Server `sort` und `filter`** – die Datenstrukturen werden aber bereits
mitgeschickt. Sobald der Server die Argumente auswertet (oder eine
`LocalSource` Sortierung clientseitig übernimmt), funktioniert die UI
automatisch.

### 4. Internationalisierung

Project Fluent über `fluent` (kein Code-Generator). Die `.ftl`-Dateien
werden mit `include_str!` zur Compile-Zeit eingebettet. Reaktiver
Locale-Wechsel über einen Leptos-`RwSignal`. Komplexe Plural- und
Selektor-Beispiele finden sich in `table.placeholder.collection`
(siehe `locales/de/main.ftl`). Zahl-, Datum- und Währungsformatierung
nutzt die `Intl`-API des Browsers für korrekte Lokalisierung.

### 5. GraphQL-Vertrag

`shared` definiert die fachlichen Typen. Server und Client benutzen
dieselben Strukturen über `serde`. Auf Serverseite werden sie nochmals
mit `async-graphql`-Annotationen umgewickelt, damit das Schema passt.

## Erweiterungspunkte

| Erweiterung                          | Anzupassende Stelle                                  |
|--------------------------------------|------------------------------------------------------|
| Tailwind statt CSS-in-Rust           | Neue `DesignSystem`-Impl + `provide_design_system()` |
| Spalten-Metadaten vom Server         | `EntityListPage`: `column_set_for` → `fetch_columns` |
| Server-seitiges Sortieren/Filtern    | `server/src/data.rs` Argumente auswerten             |
| Client-seitiges Sortieren/Filtern    | Neue `LocalSource` als `DataSource`-Implementierung  |
| Echte Reference-/Collection-Anzeige  | `components/table/formatters.rs` erweitern           |
| Weitere Sprachen                     | `locales/<code>/main.ftl` + `Locale`-Enum erweitern  |
| Datenbankanbindung                   | `server/src/data.rs` durch echten Datenzugriff ersetzen |
