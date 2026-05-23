//! Symmetry-Anker zwischen Server- und Client-`HostApiRegistry`
//! (Q0009 Spec §5.3).
//!
//! Diese Datei lebt im `server/tests`-Ordner, weil dort beide Crates
//! gleichzeitig verfuegbar sind: `server` als Eltern-Crate und `client`
//! ueber den `rlib`-Pfad im dev-dep. Schlaegt der `symmetry_check` Fehler,
//! ist der Build im Workspace rot — genauso wie ein gebrochenes
//! Wire-Format.
//!
//! Was geprueft wird:
//!   - Jede nicht-`server_only` Funktion auf der Server-Seite hat auf der
//!     Client-Seite ein Pendant mit identischem Namen *und* identischem
//!     Capability-Token.
//!   - Es gibt keine Client-Funktion ohne Server-Pendant (auch nicht
//!     server_only-Eintraege duerfen einseitig fehlen).
//!
//! Spec §5.3 ist damit operationalisiert: Drift zwischen den beiden Seiten
//! produziert sofortigen Test-Fail, nicht erst zur Laufzeit.

use shared::script::HostApiRegistry;

#[test]
fn server_and_client_host_api_registries_are_symmetric() {
    let server = server::script::ServerHostApiRegistry::functions();
    let client = client::script::ClientHostApiRegistry::functions();
    let errors = <server::script::ServerHostApiRegistry as HostApiRegistry>::symmetry_check(
        &server, &client,
    );
    assert!(
        errors.is_empty(),
        "Symmetry-Drift Server/Client erkannt:\n{}",
        errors.join("\n")
    );
}

#[test]
fn server_only_functions_are_marked_consistently_on_both_sides() {
    // Eintraege mit `server_only=true` sollen *auch auf dem Client* mit
    // demselben Flag deklariert sein — sonst koennte der Client glauben,
    // er duerfe sie unbeschraenkt aufrufen.
    let server = server::script::ServerHostApiRegistry::functions();
    let client = client::script::ClientHostApiRegistry::functions();
    for s in &server {
        if !s.server_only {
            continue;
        }
        let c = client
            .iter()
            .find(|c| c.name == s.name)
            .unwrap_or_else(|| panic!("Client kennt '{}' nicht", s.name));
        assert_eq!(
            c.server_only, s.server_only,
            "server_only-Flag fuer '{}' weicht ab",
            s.name
        );
    }
}
