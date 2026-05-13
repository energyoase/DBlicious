//! Property-Filter-Komponenten und Filter-Registry.
//!
//! Skelett heute, vollausgebaut in Phase 0.5.8. Die Registry akzeptiert
//! Filter-Komponenten unter String-IDs; konkrete Standard-Filter
//! (`text-contains`, `number-range`, …) werden in einem eigenen Schritt
//! eingezogen.

pub mod registry;

pub use registry::{FilterContext, FilterFactory, FilterRegistry};
