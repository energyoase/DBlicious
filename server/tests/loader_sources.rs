//! Loader berücksichtigt sources.toml.

use std::collections::BTreeMap;

#[test]
fn loader_picks_up_sources_toml() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("sources.toml"),
        r#"
[sources.local]
kind = "managed-sqlite"
url  = "sqlite::memory:"

[sources.d2v_legacy]
kind = "foreign-sqlite"
url  = "sqlite::memory:"
        "#,
    ).unwrap();

    let set = server::example::loader::load(tmp.path()).expect("load");
    let names: BTreeMap<_, _> = set.sources.iter().map(|(k, v)| (k.clone(), v.kind.clone())).collect();
    assert_eq!(names.get("local").map(String::as_str), Some("managed-sqlite"));
    assert_eq!(names.get("d2v_legacy").map(String::as_str), Some("foreign-sqlite"));
}

#[test]
fn loader_synthesizes_local_when_no_sources_toml() {
    let tmp = tempfile::tempdir().unwrap();
    // kein sources.toml in tmp
    let set = server::example::loader::load(tmp.path()).expect("load");
    assert_eq!(set.sources.get("local").map(|c| c.kind.as_str()), Some("managed-sqlite"));
}
