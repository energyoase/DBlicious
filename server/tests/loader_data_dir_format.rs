//! Loader-Tests fuer `[meta] dataDirFormat` (Q0012 §2.2).
//!
//! Vier Faelle: missing (v0-Backcompat), match, mismatch-newer (Abbruch),
//! mismatch-older (laedt, kein Abbruch). Kein DB-IO — reines Loader-Schema.

use std::fs;

use shared::DATA_DIR_FORMAT;

fn write_config_toml(dir: &std::path::Path, content: &str) {
    fs::write(dir.join("config.toml"), content).expect("config.toml schreiben");
}

#[test]
fn loader_accepts_data_dir_without_meta_section() {
    // v0-Backcompat: alle heutigen examples/* (shop, d2v) haben keine
    // [meta]-Sektion und muessen unveraendert laden.
    let tmp = tempfile::tempdir().unwrap();
    write_config_toml(
        tmp.path(),
        r#"
[server]
name = "no-meta"
        "#,
    );
    let set = server::example::loader::load(tmp.path())
        .expect("data-dir ohne [meta] muss laden (Backcompat)");
    assert_eq!(set.config.name, "no-meta");
}

#[test]
fn loader_accepts_data_dir_with_matching_format_version() {
    let tmp = tempfile::tempdir().unwrap();
    write_config_toml(
        tmp.path(),
        &format!(
            r#"
[server]
name = "match"

[meta]
dataDirFormat = {DATA_DIR_FORMAT}
        "#
        ),
    );
    let set = server::example::loader::load(tmp.path()).expect("matching dataDirFormat muss laden");
    assert_eq!(set.config.name, "match");
}

#[test]
fn loader_rejects_data_dir_with_newer_format_version() {
    let tmp = tempfile::tempdir().unwrap();
    let newer = DATA_DIR_FORMAT + 1;
    write_config_toml(
        tmp.path(),
        &format!(
            r#"
[server]
name = "too-new"

[meta]
dataDirFormat = {newer}
        "#
        ),
    );
    let err = server::example::loader::load(tmp.path())
        .expect_err("neuere dataDirFormat-Major muss harten Abbruch ausloesen");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("dataDirFormat"),
        "Fehlermeldung muss 'dataDirFormat' enthalten, war: {msg}"
    );
    assert!(
        msg.contains(&newer.to_string()),
        "Fehlermeldung muss die data-dir-Version ({newer}) nennen, war: {msg}"
    );
    assert!(
        msg.contains(&DATA_DIR_FORMAT.to_string()),
        "Fehlermeldung muss die Binary-Version ({DATA_DIR_FORMAT}) nennen, war: {msg}"
    );
}

#[test]
fn loader_accepts_data_dir_with_older_format_version() {
    // Forward-Compat innerhalb der Major-Familie ist Definition: ein neueres
    // Binary kann ein aelteres Format weiter lesen (Loader ist additiv).
    // Wir testen explizit dataDirFormat=0, weil das die "vor-Q0012"-Aera
    // markiert; jede zukuenftige Erhoehung der Konstante muss diesen Pfad
    // gruen halten.
    let tmp = tempfile::tempdir().unwrap();
    write_config_toml(
        tmp.path(),
        r#"
[server]
name = "older-version"

[meta]
dataDirFormat = 0
        "#,
    );
    let set = server::example::loader::load(tmp.path())
        .expect("aeltere dataDirFormat-Major muss laden (Forward-Compat)");
    assert_eq!(set.config.name, "older-version");
}
