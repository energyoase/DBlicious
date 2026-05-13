//! Integration-Test fuer `dblicious migrate-security`.
//!
//! Setzt ein Wegwerf-Beispielverzeichnis auf, ruft das CLI ueber
//! `cargo run -p cli --` auf und verifiziert die geschriebenen Dateien.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn bin_path() -> PathBuf {
    // cargo legt das integration-test-Binary unter `target/<profile>/deps/...`
    // an, aber CLI selbst ist `dblicious`. Wir nutzen `cargo run` als
    // Sprungbrett, damit wir nicht selbst den Pfad raten muessen.
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn run_cli(args: &[&str]) -> std::process::Output {
    Command::new("cargo")
        .current_dir(bin_path())
        .arg("run")
        .arg("--quiet")
        .arg("-p")
        .arg("cli")
        .arg("--target-dir")
        .arg("target-test")
        .arg("--")
        .args(args)
        .output()
        .expect("cargo run -p cli")
}

fn setup_minimal_example(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dblicious_migrate_security_{test_name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("security")).expect("security/");
    fs::create_dir_all(dir.join("entities").join("product")).expect("entities/product");
    fs::create_dir_all(dir.join("entities").join("customer")).expect("entities/customer");

    // Minimaler Groups-Datensatz: zwei Rollen, eine mit Wildcard.
    fs::write(
        dir.join("security").join("groups.json"),
        r#"[
            {
                "id": "g-admin",
                "nameKey": "group.admin",
                "descriptionKey": "group.admin.desc",
                "permissions": [
                    { "entityType": "*", "canRead": true, "canCreate": true, "canUpdate": true, "canDelete": true }
                ]
            },
            {
                "id": "g-reader",
                "nameKey": "group.reader",
                "permissions": [
                    { "entityType": "product", "canRead": true }
                ]
            }
        ]"#,
    )
    .expect("groups.json");

    fs::write(
        dir.join("security").join("users.json"),
        r#"[
            { "id": "u-alice", "username": "alice", "displayName": "Alice", "groupIds": ["g-admin"] },
            { "id": "u-bob",   "username": "bob",   "displayName": "Bob",   "groupIds": ["g-reader"] }
        ]"#,
    )
    .expect("users.json");

    dir
}

#[test]
fn migrate_security_writes_three_files_with_expected_shape() {
    let dir = setup_minimal_example("write_three_files");

    let out = run_cli(&[
        "migrate-security",
        "--data-dir",
        dir.to_str().unwrap(),
    ]);
    assert!(
        out.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let perm_path = dir.join("security").join("permissions.json");
    let roles_path = dir.join("security").join("roles.json");
    let ra_path = dir.join("security").join("role_assignments.json");

    assert!(perm_path.is_file(), "permissions.json muss existieren");
    assert!(roles_path.is_file(), "roles.json muss existieren");
    assert!(ra_path.is_file(), "role_assignments.json muss existieren");

    let perms: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&perm_path).unwrap()).unwrap();
    let perms_arr = perms.as_array().unwrap();
    // g-admin mit Wildcard expandiert auf 2 entity-types * 4 ops = 8.
    // g-reader hat 1 entity_type * 1 op = 1.
    // Summe: 9.
    assert_eq!(
        perms_arr.len(),
        9,
        "expected 8 (g-admin) + 1 (g-reader) Permissions, got {}",
        perms_arr.len()
    );

    let roles: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&roles_path).unwrap()).unwrap();
    assert_eq!(roles.as_array().unwrap().len(), 2);

    let ra: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&ra_path).unwrap()).unwrap();
    assert_eq!(ra.as_array().unwrap().len(), 2, "u-alice + u-bob");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn migrate_security_idempotent_with_force() {
    let dir = setup_minimal_example("idempotent");

    let first = run_cli(&[
        "migrate-security",
        "--data-dir",
        dir.to_str().unwrap(),
    ]);
    assert!(first.status.success());

    // Zweiter Lauf ohne --force muss scheitern.
    let second = run_cli(&[
        "migrate-security",
        "--data-dir",
        dir.to_str().unwrap(),
    ]);
    assert!(!second.status.success(), "zweiter Lauf ohne --force muss scheitern");

    // Inhalt der Dateien vor dem dritten Lauf festhalten.
    let perm_before = fs::read_to_string(dir.join("security").join("permissions.json")).unwrap();

    // Dritter Lauf mit --force ueberschreibt — Inhalt bleibt identisch.
    let third = run_cli(&[
        "migrate-security",
        "--data-dir",
        dir.to_str().unwrap(),
        "--force",
    ]);
    assert!(third.status.success());
    let perm_after = fs::read_to_string(dir.join("security").join("permissions.json")).unwrap();
    assert_eq!(
        perm_before, perm_after,
        "Idempotenz: --force-Lauf muss identischen Inhalt schreiben"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn migrate_security_dry_run_does_not_write() {
    let dir = setup_minimal_example("dry_run");

    let out = run_cli(&[
        "migrate-security",
        "--data-dir",
        dir.to_str().unwrap(),
        "--dry-run",
    ]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("dry-run"), "dry-run-Hinweis im Output: {stdout}");

    assert!(
        !dir.join("security").join("permissions.json").exists(),
        "dry-run darf keine permissions.json schreiben"
    );

    let _ = fs::remove_dir_all(&dir);
}
