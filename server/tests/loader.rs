//! Loader-Tests fuer das `examples/<name>/`-Verzeichnislayout.
//!
//! Diese Tests sind bewusst pur (kein `setup_for_tests`, kein DB-Init),
//! weil sie nur den Datei-Loader pruefen. Sie umgehen den prozessweiten
//! `example::install`-Slot und brauchen daher kein `#[serial_test::serial]`.

use std::fs;
use std::path::{Path, PathBuf};

use server::example;

fn shop_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("examples")
        .join("shop")
}

#[test]
fn shop_example_loads_with_expected_entity_types() {
    let set = example::load(&shop_dir()).expect("examples/shop muss ladbar sein");

    let types: Vec<&str> = set.entity_types().collect();
    for required in &["product", "order", "customer"] {
        assert!(
            types.contains(required),
            "Entity-Type '{required}' fehlt im geladenen Set ({:?})",
            types
        );
    }

    // Navigation darf hier nicht leer sein — das Beispiel definiert eine.
    assert!(
        !set.navigation.is_empty(),
        "examples/shop sollte eine Navigation liefern"
    );

    // Security: das Beispiel hat admin/editor/viewer.
    let usernames: Vec<&str> = set.users.iter().map(|u| u.username.as_str()).collect();
    assert!(
        usernames.contains(&"admin"),
        "Seed-User 'admin' fehlt: {:?}",
        usernames
    );

    // Translatables sind nicht leer (mind. Sprachen + ein paar Einträge).
    assert!(
        !set.translatables.languages.is_empty(),
        "examples/shop sollte Sprachen definieren"
    );
}

#[test]
fn shop_product_has_columns_and_settings() {
    let set = example::load(&shop_dir()).expect("examples/shop laden");
    let product = set
        .entities
        .get("product")
        .expect("Entity 'product' muss vorhanden sein");

    assert!(!product.columns.is_empty(), "product hat keine Spalten");
    let names: Vec<&str> = product.columns.iter().map(|c| c.key.as_str()).collect();
    for required in &["name", "price"] {
        assert!(names.contains(required), "Spalte '{required}' fehlt: {:?}", names);
    }

    assert!(
        product.editor.is_some(),
        "product sollte einen Editor definieren"
    );
    assert!(
        product.settings.is_some(),
        "product sollte Settings definieren"
    );
}

#[test]
fn missing_directory_yields_error() {
    let nowhere = std::env::temp_dir().join("dblicious_loader_test_does_not_exist_xyz");
    // Falls jemand sie zwischenzeitlich angelegt hat: erzwingen, dass sie fehlt.
    let _ = fs::remove_dir_all(&nowhere);

    let res = example::load(&nowhere);
    assert!(res.is_err(), "fehlendes Verzeichnis muss Fehler liefern");
    let msg = format!("{}", res.unwrap_err());
    assert!(
        msg.contains("existiert nicht"),
        "Fehlertext soll auf fehlendes Verzeichnis hinweisen, war: {msg}"
    );
}

#[test]
fn empty_directory_loads_with_defaults() {
    // Komplett leeres Verzeichnis: Loader muss durchlaufen, alle optionalen
    // Felder sind leer/default.
    let dir = std::env::temp_dir().join("dblicious_loader_empty_dir");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp dir anlegen");

    let set = example::load(&dir).expect("leeres Verzeichnis muss laden, nicht failen");
    assert!(set.navigation.is_empty());
    assert!(set.users.is_empty());
    assert!(set.groups.is_empty());
    assert!(set.entities.is_empty());
    assert!(set.translatables.languages.is_empty());
    // Config faellt auf Default zurueck, Name = Verzeichnisname.
    assert_eq!(set.config.name, "dblicious_loader_empty_dir");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn config_toml_overrides_default_bind() {
    // Verzeichnis nur mit config.toml — verifiziert den Format-Dispatch
    // fuer .toml und das Override-Verhalten.
    let dir = std::env::temp_dir().join("dblicious_loader_config_only");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp dir anlegen");
    fs::write(
        dir.join("config.toml"),
        r#"[server]
name = "TestShop"
bind = "0.0.0.0:9999"
"#,
    )
    .expect("config.toml schreiben");

    let set = example::load(&dir).expect("laden");
    assert_eq!(set.config.name, "TestShop");
    assert_eq!(set.config.bind, "0.0.0.0:9999");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn json_navigation_is_parsed() {
    // Verifiziert den .json-Format-Dispatch nebenher.
    let dir = std::env::temp_dir().join("dblicious_loader_json_nav");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp dir anlegen");
    fs::write(
        dir.join("navigation.json"),
        r#"[{"id":"root","labelKey":"nav.dashboard","route":"/","children":[]}]"#,
    )
    .expect("navigation.json schreiben");

    let set = example::load(&dir).expect("laden");
    assert_eq!(set.navigation.len(), 1);
    assert_eq!(set.navigation[0].id, "root");
    assert_eq!(set.navigation[0].label_key, "nav.dashboard");

    let _ = fs::remove_dir_all(&dir);
}
