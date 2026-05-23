//! CSS-in-Rust-Implementierung des Design-Systems.
//!
//! Erzeugt Inline-Style-Strings auf Basis der Tokens. Im Vergleich zu
//! generierten CSS-Klassen ist das pragmatisch und benoetigt keine
//! Build-Step-Integration. Fuer ein Wechsel auf Stylance/Tailwind genuegt
//! es, eine neue `DesignSystem`-Implementierung zu liefern.

use super::{ActionStyle, ButtonVariant, DesignSystem, Style, SurfaceLevel, TextVariant, Tokens};

#[derive(Default, Clone)]
pub struct InlineDesign;

impl DesignSystem for InlineDesign {
    fn root(&self) -> Style {
        Style::inline(format!(
            "font-family: {}; color: {};",
            Tokens::FONT_FAMILY,
            Tokens::COLOR_TEXT_PRIMARY
        ))
    }

    fn surface(&self, level: SurfaceLevel) -> Style {
        let s = match level {
            SurfaceLevel::App => format!("background: {};", Tokens::COLOR_BG_APP),
            SurfaceLevel::Sidebar => format!(
                "background: {}; color: {}; padding: {} 0;",
                Tokens::COLOR_BG_SIDEBAR,
                Tokens::COLOR_TEXT_INVERSE,
                Tokens::SPACE_SM
            ),
            SurfaceLevel::Card => format!(
                "background: {}; border: 1px solid {}; border-radius: {}; padding: {};",
                Tokens::COLOR_BG_SURFACE,
                Tokens::COLOR_BORDER,
                Tokens::RADIUS_MD,
                Tokens::SPACE_LG
            ),
            SurfaceLevel::Toolbar => format!(
                "background: {}; color: {};",
                Tokens::COLOR_BG_TOOLBAR,
                Tokens::COLOR_TEXT_INVERSE
            ),
        };
        Style::inline(s)
    }

    fn text(&self, variant: TextVariant) -> Style {
        let s = match variant {
            TextVariant::H1 => "font-size: 1.75rem; font-weight: 600; margin: 0;".to_string(),
            TextVariant::H2 => "font-size: 1.25rem; font-weight: 600; margin: 0;".to_string(),
            TextVariant::Body => "font-size: 0.95rem;".to_string(),
            TextVariant::Caption => "font-size: 0.8rem;".to_string(),
            TextVariant::Muted => {
                format!("font-size: 0.85rem; color: {};", Tokens::COLOR_TEXT_MUTED)
            }
        };
        Style::inline(s)
    }

    fn button(&self, variant: ButtonVariant) -> Style {
        self.button_action(variant).default.clone()
    }

    fn button_action(&self, variant: ButtonVariant) -> ActionStyle {
        let base = format!(
            "border: 1px solid transparent; border-radius: {}; padding: {} {}; font: inherit; cursor: pointer;",
            Tokens::RADIUS_SM,
            Tokens::SPACE_XS,
            Tokens::SPACE_MD
        );
        let (bg, fg, border) = match variant {
            ButtonVariant::Primary => (Tokens::COLOR_ACCENT, "white", "transparent"),
            ButtonVariant::Secondary => ("white", Tokens::COLOR_TEXT_PRIMARY, Tokens::COLOR_BORDER),
            ButtonVariant::Ghost => ("transparent", "inherit", "rgba(255,255,255,0.2)"),
        };
        let default = format!("{base} background: {bg}; color: {fg}; border-color: {border};");
        // Hover/Pressed/Focus heller bzw. dunkler abtoenen.
        let hover = format!(
            "{base} background: {bg}; color: {fg}; border-color: {border}; filter: brightness(1.08);"
        );
        let pressed = format!(
            "{base} background: {bg}; color: {fg}; border-color: {border}; filter: brightness(0.92);"
        );
        let focused = format!(
            "{base} background: {bg}; color: {fg}; border-color: {accent}; outline: 2px solid {accent}; outline-offset: 1px;",
            accent = Tokens::COLOR_ACCENT
        );
        let disabled = format!(
            "{base} background: {bg}; color: {fg}; border-color: {border}; opacity: 0.5; cursor: not-allowed;"
        );
        ActionStyle {
            default: Style::inline(default),
            hover: Style::inline(hover),
            pressed: Style::inline(pressed),
            focused: Style::inline(focused),
            disabled: Style::inline(disabled),
        }
    }

    fn input(&self) -> Style {
        Style::inline(format!(
            "border: 1px solid {}; border-radius: {}; padding: {} {}; font: inherit;",
            Tokens::COLOR_BORDER,
            Tokens::RADIUS_SM,
            Tokens::SPACE_XS,
            Tokens::SPACE_SM
        ))
    }

    fn nav_item(&self, depth: usize, active: bool) -> Style {
        let padding_left = 0.75 + (depth as f32) * 1.0;
        let bg = if active {
            "rgba(59, 130, 246, 0.25)"
        } else {
            "transparent"
        };
        Style::inline(format!(
            "display: block; padding: {} 1rem {} {}rem; color: inherit; text-decoration: none; background: {};",
            Tokens::SPACE_XS,
            Tokens::SPACE_XS,
            padding_left,
            bg
        ))
    }

    fn nav_group(&self, depth: usize) -> Style {
        let padding_left = 0.75 + (depth as f32) * 1.0;
        Style::inline(format!(
            "display: block; padding: {} 1rem {} {}rem; color: rgba(255,255,255,0.6); font-size: 0.8rem; text-transform: uppercase; letter-spacing: 0.05em;",
            Tokens::SPACE_SM,
            Tokens::SPACE_XS,
            padding_left
        ))
    }

    fn table(&self) -> Style {
        Style::inline("width: 100%; border-collapse: separate; border-spacing: 0;")
    }

    fn table_scroll_container(&self) -> Style {
        Style::inline(format!(
            "overflow: auto; max-height: {};",
            Tokens::TABLE_SCROLL_MAX_HEIGHT
        ))
    }

    fn table_header_row(&self) -> Style {
        Style::inline(format!(
            "background: {}; text-align: left;",
            Tokens::COLOR_TABLE_ALT
        ))
    }

    fn table_header_cell(&self) -> Style {
        Style::inline(format!(
            "padding: {} {}; border-bottom: 1px solid {}; font-weight: 600;",
            Tokens::SPACE_SM,
            Tokens::SPACE_MD,
            Tokens::COLOR_BORDER
        ))
    }

    fn table_row(&self, even: bool) -> Style {
        let bg = if even {
            Tokens::COLOR_TABLE_ALT
        } else {
            Tokens::COLOR_BG_SURFACE
        };
        Style::inline(format!("background: {bg};"))
    }

    fn table_cell(&self) -> Style {
        Style::inline(format!(
            "padding: {} {}; border-bottom: 1px solid {};",
            Tokens::SPACE_SM,
            Tokens::SPACE_MD,
            Tokens::COLOR_BORDER
        ))
    }

    fn placeholder(&self) -> Style {
        Style::inline(format!(
            "color: {}; font-style: italic;",
            Tokens::COLOR_TEXT_MUTED
        ))
    }

    fn pagination_bar(&self) -> Style {
        Style::inline(format!(
            "display: flex; justify-content: space-between; align-items: center; padding: {} 0;",
            Tokens::SPACE_SM
        ))
    }

    fn toolbar(&self) -> Style {
        Style::inline(format!(
            "display: flex; gap: {}; align-items: center; padding: {} 0;",
            Tokens::SPACE_SM,
            Tokens::SPACE_SM
        ))
    }

    fn designer_canvas(&self) -> Style {
        // Sanftes Punktraster auf hellem Surface-Hintergrund.
        Style::inline(format!(
            "background-color: {bg}; \
             background-image: radial-gradient(circle, {dot} 1px, transparent 1px); \
             background-size: 24px 24px; \
             border: 1px solid {border}; \
             border-radius: {radius}; \
             position: relative; overflow: auto;",
            bg = Tokens::COLOR_BG_SURFACE,
            dot = Tokens::COLOR_BORDER,
            border = Tokens::COLOR_BORDER,
            radius = Tokens::RADIUS_MD,
        ))
    }

    fn designer_table(&self, selected: bool) -> Style {
        let border_color = if selected {
            Tokens::COLOR_ACCENT
        } else {
            Tokens::COLOR_BORDER
        };
        let shadow = if selected {
            "0 0 0 2px rgba(59,130,246,0.25), 0 8px 18px rgba(15,23,42,0.12)"
        } else {
            "0 6px 14px rgba(15,23,42,0.08)"
        };
        Style::inline(format!(
            "background: {bg}; border: 1px solid {border}; border-radius: {radius}; \
             box-shadow: {shadow}; min-width: 220px; max-width: 260px; overflow: hidden; \
             font-size: 0.85rem; color: {text};",
            bg = Tokens::COLOR_BG_SURFACE,
            border = border_color,
            radius = Tokens::RADIUS_MD,
            shadow = shadow,
            text = Tokens::COLOR_TEXT_PRIMARY,
        ))
    }

    fn designer_table_header(&self) -> Style {
        Style::inline(format!(
            "background: {bg}; color: {fg}; padding: {sp} {sm}; \
             display: flex; align-items: center; justify-content: space-between; \
             cursor: grab; user-select: none; font-weight: 600;",
            bg = Tokens::COLOR_BG_SIDEBAR,
            fg = Tokens::COLOR_TEXT_INVERSE,
            sp = Tokens::SPACE_XS,
            sm = Tokens::SPACE_SM,
        ))
    }

    fn designer_column_row(&self, selected: bool) -> Style {
        let bg = if selected {
            "rgba(59,130,246,0.12)"
        } else {
            "transparent"
        };
        Style::inline(format!(
            "display: grid; grid-template-columns: 12px 1fr auto 12px; \
             align-items: center; gap: {sp}; padding: {sp} {sm}; \
             border-top: 1px solid {border}; background: {bg};",
            sp = Tokens::SPACE_XS,
            sm = Tokens::SPACE_SM,
            border = Tokens::COLOR_BORDER,
            bg = bg,
        ))
    }

    fn designer_port(&self, active: bool) -> Style {
        let bg = if active {
            Tokens::COLOR_ACCENT
        } else {
            "transparent"
        };
        let border = if active {
            Tokens::COLOR_ACCENT
        } else {
            Tokens::COLOR_TEXT_MUTED
        };
        Style::inline(format!(
            "width: 10px; height: 10px; border-radius: 50%; \
             border: 1.5px solid {border}; background: {bg}; \
             cursor: pointer; box-sizing: border-box;",
            border = border,
            bg = bg,
        ))
    }

    fn designer_status(&self, ok: bool) -> Style {
        let (bg, fg, border) = if ok {
            ("#ecfdf5", "#047857", "#a7f3d0")
        } else {
            ("#fef2f2", "#b91c1c", "#fecaca")
        };
        Style::inline(format!(
            "background: {bg}; color: {fg}; border: 1px solid {border}; \
             border-radius: {radius}; padding: {sp} {sm}; font-size: 0.85rem;",
            radius = Tokens::RADIUS_SM,
            sp = Tokens::SPACE_XS,
            sm = Tokens::SPACE_SM,
        ))
    }
}
