# Q0016 — Diagnose: rustc-ICE in `server/tests/reference_resolver.rs`

- Item: `docs/queue/Q0016-reference-resolver-rustc-ice.md`
- Typ: bug | Prioritaet: medium | Quelle: manual (out-of-scope-Befund, angeblich aus Q0011-Review)
- Datum: 2026-05-30
- Toolchain: `rust-toolchain.toml` → `channel = "stable"`; aktuell aufgeloest `rustc 1.95.0 (59807616e 2026-04-14)`, `cargo 1.95.0`
- Methode: superpowers:systematic-debugging — Reproduktion **deterministisch versucht**
  in incremental UND clean target-dir, statt dem Item zu vertrauen.

> **VERDICT: NOT A BUG** — Der gemeldete rustc-ICE in `reference_resolver.rs`
> reproduziert **nicht**: weder incremental (`target-test`) noch clean
> (frisches `target-ice-repro`). `reference_resolver.rs` kompiliert in beiden
> Modi sauber, alle 5 Tests gruen. Die ICE-artigen Fehler, die beim Bau **anderer**
> Server-Test-Targets auftraten (`E0786 invalid metadata`, `STATUS_STACK_BUFFER_OVERRUN`
> 0xc0000409, „crate X required to be available in rlib format"), sind eine
> **Korruption des wiederverwendeten `target-test/`-Verzeichnisses**, kein
> Source-Level- oder Toolchain-Bug. Im frischen target-dir verschwinden sie restlos.
>
> Zusatzbefund: Die verlinkte Ursprungs-Notiz `docs/reviews/Q0011-review.md`
> **enthaelt keinerlei ICE-/`reference_resolver`-Erwaehnung** (verifiziert per grep).
> Der Q0011-Review behandelt ausschliesslich `script/engine/rhai.rs`,
> `script_gate_integration.rs`, `script_run.rs`, `script_engine.rs`. Die im
> Queue-Item behauptete Herkunft ist damit nicht belegbar.

---

## 1. Reproducer

### 1a. Incremental (wiederverwendetes `target-test`) — KEIN ICE

```
$ RUST_BACKTRACE=1 cargo test -p server --test reference_resolver --target-dir target-test
   Compiling server v0.1.0 (...\dblicious\server)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 13.79s
     Running tests\reference_resolver.rs (...\reference_resolver-21c49c021fc28dd4.exe)
running 5 tests
test gql_settings_carries_display_field_for_customer ... ok
test raw_with_display_field_resolves_label ... ok
test shop_seed_order_customer_label_resolved ... ok
test raw_no_display_field_yields_empty_labels ... ok
test gql_entities_carries_reference_labels_field ... ok
test result: ok. 5 passed; 0 failed; ...
```

### 1b. Clean (frisches `target-ice-repro`, voller Dependency-Rebuild) — KEIN ICE

```
$ RUST_BACKTRACE=1 cargo test -p server --test reference_resolver --target-dir target-ice-repro
   Compiling ... (aws-sdk-s3, sea-orm, server, ...)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3m 07s
     Running tests\reference_resolver.rs (...\reference_resolver-21c49c021fc28dd4.exe)
running 5 tests
test result: ok. 5 passed; 0 failed; ...
```

> Beide Modi: `reference_resolver.rs` kompiliert sauber, kein ICE, alle 5 Tests gruen.
> Es liegt **kein** `rustc-ice-*.txt`-Dump im Repo (`**/rustc-ice-*.txt` → No files found).

### 1c. Wo der ICE-artige Fehler WIRKLICH auftrat — und nur im polluten `target-test`

Beim Bau **aller** Server-Test-Targets im wiederverwendeten `target-test`:

```
$ cargo build -p server --tests --target-dir target-test
... STATUS_STACK_BUFFER_OVERRUN (0xc0000409) in rustc.exe (test "attachments")
error[E0786]: found invalid metadata files for crate `server`
error[E0786]: found invalid metadata files for crate `client`
error[E0786]: found invalid metadata files for crate `sea_orm`
error: crate `aws_smithy_schema` required to be available in rlib format, but was not found in this form
error: crate `aws_sdk_sts`/`aws_sdk_s3`/`toml` required to be available in rlib format, ...
error: could not compile `server` (test "d2v_all_17_listable" / "email_stub" / "script_run" / ...)
error[E0599]: no method named `insert` found for struct `server::entity::permissions::ActiveModel`  (test "state_machine")
```

Gegenprobe im **frischen** target-dir — dieselben Targets bauen fehlerfrei:

```
$ cargo build -p server --test reference_resolver --target-dir target-ice-repro
    Finished `dev` profile ... in 7.90s
$ cargo build -p server --test state_machine --target-dir target-ice-repro
    Finished `dev` profile ... in 14.83s     # E0599 war ebenfalls Korruptions-Artefakt
```

**Ergebnis:** Jeder Fehler (E0786, Stack-Overrun, „rlib not found", sogar das
E0599) ist ein Artefakt des korrupten `target-test/`. Clean → alles gruen.
Keiner davon betrifft `reference_resolver.rs` (das Target, auf das Q0016 zeigt).

---

## 2. Root-Cause-Hypothese

**Kein Source-Level-Bug und kein Toolchain-Bug.** Die beobachteten Symptome sind
**Korruption des Cargo-`target`-Verzeichnisses** (incremental-/metadata-Cache):

- `E0786 found invalid metadata files for crate <X>` = rustc liest eine `.rmeta`/`.rlib`,
  deren Inhalt nicht zum erwarteten Hash/Format passt (halb geschriebene / aus einem
  abgebrochenen Build stehengebliebene Artefakte).
- `crate X required to be available in rlib format, but was not found in this form` =
  Folgefehler derselben Klasse — die rlib-Variante einer Dependency fehlt/ist defekt,
  weil ein vorheriger Build (vermutlich durch den `STATUS_STACK_BUFFER_OVERRUN`-Crash
  von rustc) abbrach und das Verzeichnis in inkonsistentem Zustand hinterliess.
- `STATUS_STACK_BUFFER_OVERRUN (0xc0000409)` in `rustc.exe` = Windows-/MSVC-typischer
  Crash-Code; tritt hier in Verbindung mit dem Lesen der defekten Metadaten auf
  (`install_ctrlc_handler`-Backtrace-Rahmen — generischer rustc-Treiber, kein
  spezifisches Source-Konstrukt).
- `E0599 ...ActiveModel::insert` in `state_machine` = ebenfalls nur im korrupten Dir;
  clean kompiliert es. (Stale `libsea_orm`-rmeta, deren Trait-Impls nicht sichtbar waren.)

Die Korruption entstand sehr wahrscheinlich, weil `target-test/` laut CLAUDE.md das
projektweit wiederverwendete Test-Verzeichnis ist (Windows-`server.exe`-Lock-Workaround)
und ueber viele Sessions/abgebrochene/parallel laufende Builds hinweg beschrieben wurde.

**`reference_resolver.rs` selbst enthaelt kein verdaechtiges Trigger-Konstrukt:** nur
`#[tokio::test(flavor = "current_thread")]` + `#[serial]`, plain async/await,
`async_graphql::Request`, `serde_json::json!`. Nichts davon ist ICE-bekannt; alles
kompiliert in beiden Dirs problemlos. Die im Item gesuchte „Trigger-Zeile" existiert nicht.

---

## 3. Beweis-Strategie

Bereits ausgefuehrt und oben belegt:

1. **Incremental-Repro** (`target-test`) → kein ICE, 5/5 gruen. (§1a)
2. **Clean-Repro** (frisches `target-ice-repro`, voller Rebuild) → kein ICE, 5/5 gruen. (§1b)
   → schliesst incremental-cache-Korruption als *Source*-Erklaerung aus und beweist:
   das File ist sauber.
3. **Differenzbeweis**: dieselben „kaputten" Targets (`state_machine`, …) bauen im
   frischen Dir fehlerfrei → die Fehler hingen am Verzeichnis, nicht am Code. (§1c)
4. **Herkunfts-Check**: `grep -i "ice|reference_resolver"` in `docs/reviews/Q0011-review.md`
   → 0 Treffer. Die behauptete Quelle stuetzt das Item nicht.
5. **Artefakt-Check**: `**/rustc-ice-*.txt` → keine Datei (echte ICEs schreiben einen Dump).

---

## 4. Test-Strategy (fuer einen etwaigen „Fix")

Da kein Defekt vorliegt, ist die einzig sinnvolle „Test"-Massnahme eine **Hygiene-Guard**,
kein neuer Unit-Test:

- Akzeptanz = `cargo test -p server --test reference_resolver --target-dir <FRESH>` gruen
  (bereits erfuellt).
- Optional als Baseline-Schutz (vgl. Q0010): ein dokumentierter „bei E0786/rlib-Fehlern
  zuerst target-dir loeschen"-Hinweis, NICHT ein Code-Test. Ein Regressionstest gegen
  Cache-Korruption ist nicht sinnvoll konstruierbar (nicht-deterministisch, umgebungs-/
  abbruch-abhaengig).

---

## 5. Moegliche Fixes (Trade-offs)

**F1 — Item als `not_a_bug` schliessen + frisches `target-test` (empfohlen).**
Den korrupten Cache verwerfen (`rm -rf target-test`) und das Verzeichnis neu aufbauen
lassen. Loest *alle* beobachteten Symptome (E0786/rlib/Stack-Overrun/E0599) auf einen
Schlag. Kein Code-Change, kein Toolchain-Bump.
+ minimal, adressiert die echte Ursache. − muss bei erneutem Korruptions-Vorfall wiederholt werden.

**F2 — Doku/CLAUDE.md-Hinweis ergaenzen.** Im Test-Abschnitt vermerken: „Bei
`E0786 found invalid metadata` / `... required to be available in rlib format` /
`STATUS_STACK_BUFFER_OVERRUN` in rustc → kein Compiler-Bug, sondern korruptes
`target-test/`; loeschen und neu bauen." Verhindert, dass der naechste solche Vorfall
erneut faelschlich als ICE-Bug getriaged wird.
+ billig, praeventiv. − reine Doku, verhindert die Korruption nicht.

**F3 — Toolchain-Bump (NICHT noetig).** `rust-toolchain.toml` ist bereits auf `stable`
(aktuell 1.95.0). Kein gepinnter alter Compiler, dem ein echter ICE-Fix fehlen wuerde.
Ein Bump waere wirkungslos, da kein echter ICE vorliegt. Verworfen.

Empfehlung: **F1 (+ optional F2)**. Kein Upstream-Issue, da kein reproduzierbarer
Compiler-Crash auf sauberem Stand existiert.

---

## 6. Risiko-Einschaetzung

- **Test-Baseline (Q0010-Analogie):** Aktuell NICHT durch ein Source-/Toolchain-Problem
  gefaehrdet. `reference_resolver` ist gruen. Die zwischenzeitlichen Build-Abbrueche
  betrafen nur das polluted `target-test` und sind durch einen frischen Build behoben.
- **Restrisiko:** Cache-Korruption kann unter Windows bei abgebrochenen/parallelen
  Builds erneut auftreten (umgebungsbedingt, nicht code-bedingt). Symptom-Wiedererkennung
  via F2-Doku senkt den Triage-Aufwand bei Wiederholung.
- **Falsch-Positiv-Risiko des Items:** Das Item verweist auf eine Quelle
  (`Q0011-review.md`), die den Befund nicht enthaelt — Hinweis, dass die ICE-Beobachtung
  aus einem transienten lokalen Build-Zustand stammte (genau das hier reproduzierte
  Cache-Artefakt) und nie ein echter Source-ICE war.
- **Aenderungsrisiko eines „Fixes":** Null bei F1/F2 (kein Produktions-/Testcode angefasst).

---

## Anhang — Befehle & Evidenz

```
rustc 1.95.0 (59807616e 2026-04-14) / cargo 1.95.0   # rust-toolchain.toml: channel = "stable"
RUSTFLAGS: (leer)
grep -i "ice|reference_resolver" docs/reviews/Q0011-review.md   → 0 Treffer
**/rustc-ice-*.txt                                              → No files found
cargo test -p server --test reference_resolver --target-dir target-test       → 5 passed
cargo test -p server --test reference_resolver --target-dir target-ice-repro   → 5 passed (clean, 3m07s rebuild)
cargo build -p server --test state_machine     --target-dir target-ice-repro   → Finished (E0599 war Cache-Artefakt)
cargo build -p server --tests                  --target-dir target-test        → E0786/rlib/STATUS_STACK_BUFFER_OVERRUN (nur polluted dir)
```
