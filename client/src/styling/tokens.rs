//! Design-Tokens – semantische Konstanten, die in Implementierungen
//! verwendet werden. Bei einem Tailwind-Backend kann dieselbe Datei
//! als Quelle der Wahrheit fuer eine `tailwind.config.js` dienen.

pub struct Tokens;

impl Tokens {
    // Farben
    pub const COLOR_BG_APP: &'static str = "#f7f8fa";
    pub const COLOR_BG_SURFACE: &'static str = "#ffffff";
    pub const COLOR_BG_SIDEBAR: &'static str = "#1f2937";
    pub const COLOR_BG_TOOLBAR: &'static str = "#111827";
    pub const COLOR_TEXT_PRIMARY: &'static str = "#111827";
    pub const COLOR_TEXT_INVERSE: &'static str = "#f9fafb";
    pub const COLOR_TEXT_MUTED: &'static str = "#6b7280";
    pub const COLOR_ACCENT: &'static str = "#3b82f6";
    pub const COLOR_BORDER: &'static str = "#e5e7eb";
    pub const COLOR_TABLE_ALT: &'static str = "#f9fafb";

    // Abstaende
    pub const SPACE_XS: &'static str = "0.25rem";
    pub const SPACE_SM: &'static str = "0.5rem";
    pub const SPACE_MD: &'static str = "0.75rem";
    pub const SPACE_LG: &'static str = "1rem";

    // Radien
    pub const RADIUS_SM: &'static str = "4px";
    pub const RADIUS_MD: &'static str = "6px";

    // Schrift
    pub const FONT_FAMILY: &'static str =
        r#"system-ui, -apple-system, "Segoe UI", Roboto, sans-serif"#;

    // Tabellen-Scroll-Container: max-height limitiert die Tabelle so, dass der
    // horizontale Scrollbalken am Tabellen-Boden im Viewport sichtbar bleibt
    // (Vertikal-Scroll wandert in den Container). Der Abzug deckt grob
    // Sidebar-Top, Page-H1, Toolbar, Pager und Card-Padding ab.
    pub const TABLE_SCROLL_MAX_HEIGHT: &'static str = "calc(100vh - 18rem)";
}
