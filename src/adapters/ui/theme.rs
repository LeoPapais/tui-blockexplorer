//! Palette data model + preset list.
//!
//! Ships the semantic-token struct and a static list of presets so
//! the Settings screen can surface a choice of palettes without a
//! full RGB picker. Migrating each widget to consume these tokens
//! is tracked as a separate follow-up (plan/10-settings.md §12.9).

use ratatui::style::Color;

/// Semantic colour tokens used by the TUI. Widgets pick values off a
/// `Palette` rather than hard-coding `Color` enums.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub accent: Color,
    pub warning: Color,
    pub success: Color,
    pub muted: Color,
    pub background: Color,
    pub foreground: Color,
}

/// Built-in palette presets the user can pick from on Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PalettePreset {
    DarkDefault,
    Light,
    HighContrast,
    Solarized,
}

impl PalettePreset {
    /// Human-readable label shown on the Settings screen.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            PalettePreset::DarkDefault => "Dark (default)",
            PalettePreset::Light => "Light",
            PalettePreset::HighContrast => "High contrast",
            PalettePreset::Solarized => "Solarized",
        }
    }

    /// Return every built-in preset in display order.
    #[must_use]
    pub const fn all() -> [PalettePreset; 4] {
        [
            PalettePreset::DarkDefault,
            PalettePreset::Light,
            PalettePreset::HighContrast,
            PalettePreset::Solarized,
        ]
    }

    /// Resolve the preset into a concrete palette.
    #[must_use]
    pub const fn palette(self) -> Palette {
        match self {
            PalettePreset::DarkDefault => Palette {
                accent: Color::Cyan,
                warning: Color::Yellow,
                success: Color::Green,
                muted: Color::DarkGray,
                background: Color::Black,
                foreground: Color::White,
            },
            PalettePreset::Light => Palette {
                accent: Color::Blue,
                warning: Color::Rgb(255, 140, 0),
                success: Color::Rgb(0, 128, 0),
                muted: Color::Gray,
                background: Color::Rgb(250, 250, 250),
                foreground: Color::Black,
            },
            PalettePreset::HighContrast => Palette {
                accent: Color::Rgb(255, 255, 0),
                warning: Color::Rgb(255, 80, 80),
                success: Color::Rgb(0, 255, 0),
                muted: Color::White,
                background: Color::Black,
                foreground: Color::White,
            },
            PalettePreset::Solarized => Palette {
                accent: Color::Rgb(38, 139, 210),
                warning: Color::Rgb(181, 137, 0),
                success: Color::Rgb(133, 153, 0),
                muted: Color::Rgb(147, 161, 161),
                background: Color::Rgb(0, 43, 54),
                foreground: Color::Rgb(238, 232, 213),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_has_a_label() {
        for preset in PalettePreset::all() {
            assert!(!preset.label().is_empty());
        }
    }

    #[test]
    fn high_contrast_diverges_from_dark_default() {
        let dark = PalettePreset::DarkDefault.palette();
        let hc = PalettePreset::HighContrast.palette();
        assert_ne!(dark.accent, hc.accent);
        assert_ne!(dark.warning, hc.warning);
    }
}
