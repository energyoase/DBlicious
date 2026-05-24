//! Email-Template-Rendering (Roadmap 1.7.10-Folge).
//!
//! Reine Render-Layer, quellen-agnostisch (Vorbild `crate::pdf`). Ein Template
//! ist ein Buendel dreier Template-Strings (subject/body_text/body_html); der
//! Renderer fuellt sie mit Variablen. Die Template-QUELLE (Loader/DB/Designer)
//! und Locale-Auswahl sind Folge-Items.
//!
//! Sicherheit: Variablen gehen strukturiert (serde) in den Render-Kontext,
//! keine String-Konkatenation. Der HTML-Part wird autoescaped, subject/text
//! nicht (Per-Part-Autoescape ueber Template-Namens-Suffix).
