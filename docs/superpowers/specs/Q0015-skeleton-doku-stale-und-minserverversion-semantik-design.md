# Q0015 — Skeleton-Doku-Stale + minServerVersion-Semantik — Design-Spec

Status: Design (brainstormed)
Datum: 2026-05-30
Quelle: Queue-Item `docs/queue/Q0015-skeleton-doku-stale-und-minserverversion-semantik.md`
Referenzen: `docs/reviews/Q0012-review.md` (F1, F2), `docs/reviews/Q0012-security-review.md` (A4)

## 0. Zweck und Scope

Zwei niedrig-riskante Befunde aus dem Q0012-Review, beide am data-dir-/`[meta]`-Vertrag:

- **Befund 1 (F1 / Sec-A4):** `docs/standalone-projekt-skeleton.md` ist stale — der
  `scripts/`-Block listet 1 Script, real liegen seit Q0013 **3** vor.
- **Befund 2 (F2):** `server/src/example/loader.rs` vergleicht `minServerVersion` per
  Exakt-String-Gleichheit (`min_ver != our_ver`), obwohl der Feldname eine
  `>=`-Mindest-Schwelle impliziert.

Beides ist **Warn-only** — kein harter Boot-Abbruch. Der harte Gate bleibt
`dataDirFormat` gegen `shared::DATA_DIR_FORMAT` (Q0012 §2.2), unverändert.

### Bundling-Entscheidung

Die zwei Befunde bleiben in **einer Spec / einer Ausführung**. Begründung:
beide berühren denselben `[meta]`-Vertrag, sind beide klein und warn-only, und der
Skeleton-Doc-Fix (§2 unten) und der Loader-Fix (§1 unten) sind unabhängig genug,
dass sie in einem Plan als zwei getrennte Tasks sauber nebeneinander laufen. Kein
Split nötig — der Overhead von zwei Queue-Lifecycles übersteigt den Nutzen.

## 1. Befund 2 — minServerVersion: `>=`-Semantik (der Substanz-Teil)

### 1.1 Ist-Zustand (verifiziert)

`server/src/example/loader.rs:101-109`:

```rust
if let Some(min_ver) = meta.min_server_version.as_deref() {
    let our_ver = env!("CARGO_PKG_VERSION");
    if min_ver != our_ver {
        tracing::warn!(
            "data-dir '{}' deklariert minServerVersion = '{min_ver}', dieses Binary ist {our_ver}. \
             Es findet keine harte Pruefung statt — bitte selbst verifizieren.",
            dir.display()
        );
    }
}
```

Problem: Binary `0.2.0` + `minServerVersion = "0.1.0"` warnt, obwohl `0.2.0 >= 0.1.0`
die Anforderung **erfüllt**. Der Feldname (und CLAUDE.md, das von "Warn-Schwelle"
spricht) impliziert eine Untergrenze, nicht einen Exact-Pin.

### 1.2 Soll-Semantik

Warne **nur**, wenn das Binary **älter** als das deklarierte Minimum ist:

| `our_ver` (Binary) | `min_ver` (data-dir) | Verhalten |
|---|---|---|
| `0.2.0` | `0.1.0` | **keine** Warnung (Binary erfüllt Minimum) |
| `0.1.0` | `0.1.0` | **keine** Warnung (genau am Minimum) |
| `0.1.0` | `0.2.0` | **Warnung** (Binary zu alt) |
| `0.1.0` | `0.1.5` | **Warnung** (Binary zu alt) |

Vergleich: SemVer-`<` (Precedence per SemVer 2.0). Kein Stopp in keinem Fall —
weiterhin reine Warn-Schwelle.

### 1.3 SemVer-Implementierung: `semver`-Crate

**Entscheidung: das `semver`-Crate ziehen** (nicht hand-gerollte Tuple-Compare).

Verifiziert via `Cargo.lock`: `semver` **1.0.28** ist bereits als transitive
Dependency im Lockfile (gezogen von cargo-/sea-orm-/typst-Tooling-Crates). Es ist
aber **kein direkter** Dependency des `server`-Crates — `use semver::Version`
verlangt einen Eintrag in `server/Cargo.toml`. Da die exakte Version schon gelockt
ist, fügt `semver = "1"` **keinen neuen Lockfile-Knoten** hinzu und bumpt nichts.

Begründung gegen Hand-Roll: SemVer-Precedence (Pre-Release-Tags, `0.x`-Sonderregeln,
Build-Metadata-Ignore) korrekt selbst zu implementieren ist fehleranfälliger als ein
~50KB-Crate, das ohnehin schon im Build ist. YAGNI spricht hier *für* das Crate, weil
die Tuple-Variante mehr Code + mehr Edge-Case-Tests bräuchte als der Crate-Aufruf.

`server/Cargo.toml`, neuer Eintrag unter `[dependencies]`:

```toml
semver = "1"
```

### 1.4 Malformed / non-SemVer Strings: warn-and-skip

`min_ver` kommt aus operator-geschriebenem `config.toml` und kann Murks sein
(`"latest"`, `"v1"`, `""`). Policy — **warn-and-skip, kein Boot-Fehler** (das Item
verlangt warn-only):

- `min_ver` parst nicht als `semver::Version` → **eine** Warnung "konnte
  `minServerVersion` nicht als SemVer parsen, Check übersprungen", dann weiter.
- `our_ver` (= `CARGO_PKG_VERSION`) parst nicht → das ist ein interner Build-Defekt,
  sollte nie passieren; ebenfalls warn-and-skip (kein `panic`, kein Boot-Abbruch),
  da ein kaputter Versions-String das Hochfahren des Servers nicht verhindern darf.

Damit gibt es genau drei Warn-Auslöser: (a) Binary zu alt, (b) `min_ver`
unparsbar, (c) `our_ver` unparsbar. Alle anderen Fälle: still.

### 1.5 Testbarkeit — reine Helper-Funktion extrahieren

Heute ist der Vergleich inline mit `tracing::warn!` verwoben; `load()` gibt
`Result<ExampleSet>` zurück, nicht den Log — die Warn-Entscheidung ist ohne
Tracing-Capture-Harness nicht unit-testbar. Lösung (idiomatisch, matcht die
existierende Praxis, Fehlermeldungs-Inhalte zu pinnen):

Eine **pure, freie Funktion** extrahieren, die die Entscheidung trifft und die
fertige Warn-Message (oder `None`) zurückgibt — der Loader ruft sie auf und feuert
`tracing::warn!` nur bei `Some`:

```rust
/// Prueft die optionale `minServerVersion`-Untergrenze gegen die laufende
/// Binary-Version. Reine Warn-Schwelle (kein Stopp): liefert `Some(msg)`, wenn
/// eine Warnung geloggt werden soll, sonst `None`.
///
/// Warn-Faelle:
/// - Binary aelter als das deklarierte Minimum (`our < min`),
/// - `min_ver` ist kein gueltiges SemVer (Check uebersprungen),
/// - `our_ver` ist kein gueltiges SemVer (interner Build-Defekt; Check uebersprungen).
/// Kein Warn-Fall: `our >= min`.
fn server_version_warning(min_ver: &str, our_ver: &str) -> Option<String> {
    let min = match semver::Version::parse(min_ver) {
        Ok(v) => v,
        Err(_) => {
            return Some(format!(
                "minServerVersion = '{min_ver}' ist kein gueltiges SemVer — \
                 Versions-Check uebersprungen."
            ));
        }
    };
    let our = match semver::Version::parse(our_ver) {
        Ok(v) => v,
        Err(_) => {
            return Some(format!(
                "interne Binary-Version '{our_ver}' ist kein gueltiges SemVer — \
                 minServerVersion-Check uebersprungen."
            ));
        }
    };
    if our < min {
        return Some(format!(
            "data-dir verlangt minServerVersion = '{min_ver}', dieses Binary ist \
             {our_ver} (zu alt). Reine Warnung, kein Stopp — bitte ein neueres \
             dblicious-Binary installieren."
        ));
    }
    None
}
```

Call-Site im Loader (ersetzt Zeilen 101-109):

```rust
if let Some(min_ver) = meta.min_server_version.as_deref() {
    if let Some(msg) = server_version_warning(min_ver, env!("CARGO_PKG_VERSION")) {
        tracing::warn!("data-dir '{}': {msg}", dir.display());
    }
}
```

Die exakte Wortwahl der Messages ist hier verbindlich (deutsch, projekt-konform).
Die `dir.display()`-Präfixierung bleibt an der Call-Site, damit die Helper-Funktion
pfad-agnostisch und damit trivial testbar bleibt.

### 1.6 Doc-Comment-Update

`ConfigMeta::min_server_version` doc-comment (`loader.rs:44-46`) muss von der jetzigen
"lex-vergleichbar reicht uns hier nicht; wir parsen es nicht weiter"-Begründung auf
die neue `>=`-Semantik umgeschrieben werden, z.B.:

```rust
/// Optionale Mindest-Server-Version (SemVer). Reine Warn-Schwelle, kein Stopp:
/// das Binary warnt nur, wenn seine eigene Version *kleiner* als dieser Wert ist
/// (oder der String kein gueltiges SemVer ist). Vergleich via `semver`-Crate.
```

### 1.7 Tests

Neue Test-Datei `server/tests/loader_min_server_version.rs` (oder als
`#[cfg(test)] mod`-Block in `loader.rs`, sofern `server_version_warning` modul-privat
bleibt — bevorzugt **inline `#[cfg(test)]`-Modul in `loader.rs`**, weil die Funktion
`fn`-privat ist und nicht über die `loader`-Surface exportiert werden soll).

Pflicht-Testfälle (DoD aus dem Item: "neuer → keine Warnung", "älter → Warnung"):

| Test | `min_ver` | `our_ver` | Erwartung |
|---|---|---|---|
| `binary_newer_than_min_no_warning` | `"0.1.0"` | `"0.2.0"` | `None` |
| `binary_equal_to_min_no_warning` | `"0.1.0"` | `"0.1.0"` | `None` |
| `binary_older_than_min_warns` | `"0.2.0"` | `"0.1.0"` | `Some(_)`, Message enthält beide Versionen + `"zu alt"` |
| `binary_older_minor_warns` | `"0.1.5"` | `"0.1.0"` | `Some(_)` |
| `malformed_min_ver_warns_and_skips` | `"latest"` | `"0.1.0"` | `Some(_)`, Message enthält `"kein gueltiges SemVer"` |
| `malformed_our_ver_warns_and_skips` | `"0.1.0"` | `"nonsense"` | `Some(_)` |

Die Tests rufen `server_version_warning` direkt auf — kein Tracing-Capture, kein
DB-IO, kein `#[serial]` nötig (rein funktional). Assertions auf
`Option`-Variante + (bei `Some`) auf Message-Inhalt via `contains`, analog zur
existierenden `loader_data_dir_format.rs`-Praxis (Fehlermeldungs-Tokens pinnen).

## 2. Befund 1 — Skeleton-Doku stale (der Doku-Teil)

### 2.1 Ist-Zustand (verifiziert)

`docs/standalone-projekt-skeleton.md`:

- **Zeilen 53-55** (`scripts/`-Block) listen nur
  `d2v_value_type_label.{rhai,manifest.json}` — **1 Script**.
- **Zeile 59** Quellen-Zeile: "verifiziert: 17 Entity-Typen, 1 Script".

Realer Bestand (verifiziert via Glob auf `examples/d2v/scripts/`): **3 Script-Paare**:
- `d2v_value_type_label.{rhai,manifest.json}` (Q0009)
- `d2v_balance_validator.{rhai,manifest.json}` (Q0013 P1)
- `d2v_stack_filter.{rhai,manifest.json}` (Q0013 P3)

Die **17-Entity-Zahl ist korrekt** (verifiziert via Glob auf
`examples/d2v/entities/*/columns.json` → genau 17 Typen, exakt die im Tree gelisteten).
Kein weiterer Drift im Entity-Baum.

### 2.2 Fix-Scope

Genau zwei Stellen ändern, **kein Over-Engineering**:

1. **`scripts/`-Block** (Zeilen 53-55) auf 3 Einträge bringen:

   ```
   └── scripts/
       ├── d2v_balance_validator.{rhai,manifest.json}
       ├── d2v_stack_filter.{rhai,manifest.json}
       └── d2v_value_type_label.{rhai,manifest.json}
   ```

2. **Quellen-Zeile** (Zeile 59) auf
   `"verifiziert: 17 Entity-Typen, 3 Scripts (Stand 2026-05-30)"` aktualisieren.

### 2.3 Anti-Drift-Hinweis (klein, kein neuer Mechanismus)

Statt nur die Zahl zu syncen (die beim nächsten Q0013-Folgepilot wieder driftet),
**einen einzeiligen Hinweis** an die Quellen-Zeile hängen, dass `scripts/` der
lebende Bestand von `examples/d2v/scripts/` ist und mit jedem Script-Pilot mitwächst —
also "Liste synchron halten, Bestand via Glob verifizieren". Das ist eine Doku-Notiz,
**kein** automatisierter Check (kein CI-Job, kein Test gegen die Markdown-Datei —
das wäre Over-Engineering für eine Handoff-Doku, die Stage 2 noch nicht aktiv
konsumiert). Bewusst eine Zeile, nicht mehr.

Das `§7 Stdlib-Naht`-Kapitel braucht **keine** Änderung — seine Aussage ("Scripts
aus eigenem `scripts/`-Ordner") ist strukturell korrekt und war nur durch die
Single-Script-Annahme im Tree optisch überlagert.

## 3. Was NICHT geändert wird (Abgrenzung)

- **`dataDirFormat`-Boot-Check** (`loader.rs:86-100`) — unverändert, bleibt der harte Gate.
- **`shared::DATA_DIR_FORMAT`** — unverändert (= 1); diese Spec ist additiv/korrektiv,
  keine breaking data-dir-Vertragsänderung, also **kein** Konstanten-Bump.
- **`examples/d2v/` und `examples/shop/`** — keine `config.toml`-Mutation; beide haben
  weiterhin kein `[meta]`.
- **CLAUDE.md** — der bestehende Satz beschreibt `minServerVersion` bereits korrekt als
  "Warn-Schwelle"; nach diesem Fix stimmt Code *und* Doku überein. Keine
  CLAUDE.md-Änderung nötig (optional: das Wort "Schwelle" ist jetzt präzise korrekt).
- **Feld-Umbenennung** (`requiredServerVersion` etc., Review-Variante 1) — **verworfen**.
  Das Item verlangt explizit `>=`-Semantik unter dem bestehenden Namen
  `minServerVersion`; Umbenennen würde dem Feldnamen-Intent widersprechen.

## 4. Definition of Done

- [ ] `server/Cargo.toml`: `semver = "1"` als direkte Dependency (löst auf gelockte 1.0.28, kein Bump).
- [ ] `loader.rs`: pure `server_version_warning(min_ver, our_ver) -> Option<String>`-Helper, Call-Site nutzt sie, `>=`-Semantik, deutsche Messages wie in §1.5.
- [ ] `loader.rs`: `ConfigMeta::min_server_version` doc-comment auf SemVer-`>=`-Semantik umgeschrieben (§1.6).
- [ ] Tests: alle 6 Fälle aus §1.7 grün; "Binary neuer → keine Warnung" und "Binary älter → Warnung" beide explizit abgedeckt.
- [ ] `docs/standalone-projekt-skeleton.md`: `scripts/`-Block + Quellen-Zeile auf 3 Scripts (§2.2), Ein-Zeilen-Anti-Drift-Hinweis (§2.3).
- [ ] `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` grün (Git-Hooks-Baseline).
- [ ] `cargo test -p server` grün (ggf. `--target-dir target-test` bei laufendem Dev-Server / Windows-File-Lock).
- [ ] Keine `examples/**`-Mutation, kein `DATA_DIR_FORMAT`-Bump.

## 5. Risiko

Gering. Beide Teile sind warn-only bzw. reine Doku. Die einzige Code-Substanz
(`semver`-Vergleich) ist durch eine pure Funktion + 6 Unit-Tests vollständig
gepinnt. Kein neuer Boot-Pfad, keine neue Threat-Surface (Sec-Review A4/NB10: reines
Lesen einer Versionszahl + Vergleich).
