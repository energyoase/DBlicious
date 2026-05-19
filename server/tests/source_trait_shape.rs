//! Statische Garantien an die `Source`-Trait. Wenn diese Datei kompiliert,
//! ist die API-Form korrekt.

use server::source::{Capabilities, PageQuery, Source, SourceError};

fn _assert_object_safe(_: &dyn Source) {}

fn _assert_send_sync<T: Send + Sync>() {}

#[test]
fn trait_is_object_safe_and_threadsafe() {
    _assert_send_sync::<Capabilities>();
    _assert_send_sync::<PageQuery>();
    _assert_send_sync::<SourceError>();
    // Object-Safety wird durch `_assert_object_safe` zur Compile-Zeit geprueft.
}
