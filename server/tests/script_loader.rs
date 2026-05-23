//! Q0009 Phase 3.2 — Loader-Integration fuer `scripts/<id>.rhai` +
//! `<id>.manifest.{json,toml}`.
//!
//! Reine Datei-Loader-Tests (kein DB-Init, kein `setup_for_tests`) —
//! analog zu `tests/loader.rs`. Daher kein `#[serial]`.

use std::fs;
use std::path::{Path, PathBuf};

use server::example;

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("scripts")
        .join("example")
}

#[test]
fn loader_reads_provider_and_component_scripts_from_fixture() {
    let set = example::load(&fixture_dir()).expect("fixture muss laden");

    // Zwei Skripte erwartet.
    assert_eq!(
        set.scripts.len(),
        2,
        "scripts/ enthaelt zwei .rhai-Dateien, geladen: {:?}",
        set.scripts.keys().collect::<Vec<_>>()
    );
    assert!(
        set.scripts.contains_key("eur_formatter"),
        "Provider 'eur_formatter' fehlt"
    );
    assert!(
        set.scripts.contains_key("dashboard_widget"),
        "Component 'dashboard_widget' fehlt"
    );

    // Provider — JSON-Manifest.
    let provider = &set.scripts["eur_formatter"];
    assert!(provider.manifest.is_some(), "Provider-Manifest soll parsen");
    assert!(provider.manifest_error.is_none());
    match &provider.kind {
        shared::script::ScriptKind::Provider { slot } => {
            assert_eq!(*slot, shared::script::ProviderSlot::Formatter);
        }
        other => panic!("Provider erwartet, war: {other:?}"),
    }
    // Source-Datei wurde gelesen.
    assert!(provider.source.contains("fn fmt"));

    // Component — TOML-Manifest.
    let widget = &set.scripts["dashboard_widget"];
    assert!(widget.manifest.is_some(), "Component-Manifest soll parsen");
    assert!(widget.manifest_error.is_none());
    match &widget.kind {
        shared::script::ScriptKind::Component { entry } => {
            assert_eq!(entry, "render");
        }
        other => panic!("Component erwartet, war: {other:?}"),
    }
    let m = widget.manifest.as_ref().unwrap();
    assert_eq!(m.tier, shared::script::ScriptTier::Author);
}

#[test]
fn loader_marks_missing_manifest_as_draft_error() {
    // Wir bauen ein temporaeres Verzeichnis mit nur einer .rhai-Datei
    // (kein Manifest) und pruefen, dass der Loader das als Draft markiert.
    let dir = std::env::temp_dir().join("dblicious_loader_script_no_manifest");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("scripts")).expect("temp/scripts dir");
    fs::write(dir.join("scripts").join("orphan.rhai"), "fn x() { 1 }")
        .expect("write rhai");

    let set = example::load(&dir).expect("laden");
    assert_eq!(set.scripts.len(), 1);
    let seed = &set.scripts["orphan"];
    assert!(seed.manifest.is_none(), "manifest soll fehlen");
    assert!(
        seed.manifest_error.is_some(),
        "manifest_error soll gesetzt sein"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn loader_marks_unparseable_manifest_as_draft_error() {
    let dir = std::env::temp_dir().join("dblicious_loader_script_bad_manifest");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("scripts")).expect("temp/scripts dir");
    fs::write(dir.join("scripts").join("broken.rhai"), "fn x() { 1 }")
        .expect("write rhai");
    // Bewusst kaputt: kein gueltiges JSON.
    fs::write(
        dir.join("scripts").join("broken.manifest.json"),
        "{ not valid json",
    )
    .expect("write bad manifest");

    let set = example::load(&dir).expect("laden — darf NICHT crashen bei kaputtem Manifest");
    let seed = &set.scripts["broken"];
    assert!(seed.manifest.is_none());
    assert!(seed.manifest_error.is_some());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn missing_scripts_dir_yields_empty_map() {
    // Verzeichnis ohne `scripts/`-Sub-Verzeichnis: empty map, kein Fehler.
    let dir = std::env::temp_dir().join("dblicious_loader_no_scripts_dir");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp dir");

    let set = example::load(&dir).expect("laden");
    assert!(set.scripts.is_empty());

    let _ = fs::remove_dir_all(&dir);
}
