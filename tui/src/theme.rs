use ratatui::style::Color;

#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub primary: Color,
    pub secondary: Color,
    pub accent: Color,
    pub background: Color,
    pub text: Color,
    pub success: Color,
    pub error: Color,
    pub warning: Color,
    pub thinking: Color,
    pub dimmed: Color,
    pub border_style: String,
    // Activity feed background tints (very dark, subtle)
    pub bg_edit: Color,
    pub bg_read: Color,
    pub bg_bash: Color,
    pub bg_search: Color,
    pub bg_agent: Color,
    pub bg_default: Color,
}

impl Theme {
    pub fn cyberpunk() -> Self {
        Self {
            name: "cyberpunk".into(),
            primary: Color::Rgb(0, 255, 65),
            secondary: Color::Rgb(10, 189, 198),
            accent: Color::Rgb(255, 42, 109),
            background: Color::Rgb(10, 10, 10),
            text: Color::Rgb(209, 209, 209),
            success: Color::Rgb(5, 217, 232),
            error: Color::Rgb(255, 42, 109),
            warning: Color::Rgb(255, 204, 0),
            thinking: Color::Rgb(187, 154, 247),
            dimmed: Color::Rgb(86, 95, 137),
            border_style: "double".into(),
            bg_edit: Color::Rgb(10, 28, 10),
            bg_read: Color::Rgb(10, 14, 28),
            bg_bash: Color::Rgb(28, 22, 8),
            bg_search: Color::Rgb(8, 22, 28),
            bg_agent: Color::Rgb(18, 12, 28),
            bg_default: Color::Rgb(18, 18, 18),
        }
    }

    pub fn clean() -> Self {
        Self {
            name: "clean".into(),
            primary: Color::Rgb(122, 162, 247),
            secondary: Color::Rgb(86, 95, 137),
            accent: Color::Rgb(192, 202, 245),
            background: Color::Rgb(26, 27, 38),
            text: Color::Rgb(192, 202, 245),
            success: Color::Rgb(158, 206, 106),
            error: Color::Rgb(247, 118, 142),
            warning: Color::Rgb(224, 175, 104),
            thinking: Color::Rgb(157, 124, 216),
            dimmed: Color::Rgb(86, 95, 137),
            border_style: "single".into(),
            bg_edit: Color::Rgb(20, 32, 20),
            bg_read: Color::Rgb(20, 22, 36),
            bg_bash: Color::Rgb(32, 28, 18),
            bg_search: Color::Rgb(18, 26, 32),
            bg_agent: Color::Rgb(26, 20, 34),
            bg_default: Color::Rgb(24, 24, 28),
        }
    }

    pub fn retro() -> Self {
        Self {
            name: "retro".into(),
            primary: Color::Rgb(255, 176, 0),
            secondary: Color::Rgb(98, 94, 74),
            accent: Color::Rgb(255, 176, 0),
            background: Color::Rgb(10, 8, 0),
            text: Color::Rgb(255, 176, 0),
            success: Color::Rgb(255, 176, 0),
            error: Color::Rgb(255, 102, 0),
            warning: Color::Rgb(255, 176, 0),
            thinking: Color::Rgb(255, 176, 0),
            dimmed: Color::Rgb(98, 94, 74),
            border_style: "ascii".into(),
            bg_edit: Color::Rgb(20, 16, 0),
            bg_read: Color::Rgb(14, 12, 0),
            bg_bash: Color::Rgb(24, 18, 0),
            bg_search: Color::Rgb(16, 14, 0),
            bg_agent: Color::Rgb(18, 14, 4),
            bg_default: Color::Rgb(12, 10, 0),
        }
    }

    /// Neon noir — hot pink, electric blue, deep black
    pub fn neon() -> Self {
        Self {
            name: "neon".into(),
            primary: Color::Rgb(255, 0, 170),
            secondary: Color::Rgb(0, 180, 255),
            accent: Color::Rgb(255, 255, 0),
            background: Color::Rgb(5, 5, 12),
            text: Color::Rgb(220, 220, 240),
            success: Color::Rgb(0, 255, 136),
            error: Color::Rgb(255, 50, 80),
            warning: Color::Rgb(255, 170, 0),
            thinking: Color::Rgb(180, 100, 255),
            dimmed: Color::Rgb(70, 60, 90),
            border_style: "double".into(),
            bg_edit: Color::Rgb(0, 22, 12),
            bg_read: Color::Rgb(0, 12, 22),
            bg_bash: Color::Rgb(24, 16, 0),
            bg_search: Color::Rgb(0, 16, 24),
            bg_agent: Color::Rgb(16, 8, 24),
            bg_default: Color::Rgb(12, 10, 16),
        }
    }

    /// Dracula — purple-heavy with pastel accents
    pub fn dracula() -> Self {
        Self {
            name: "dracula".into(),
            primary: Color::Rgb(189, 147, 249),
            secondary: Color::Rgb(139, 233, 253),
            accent: Color::Rgb(255, 121, 198),
            background: Color::Rgb(40, 42, 54),
            text: Color::Rgb(248, 248, 242),
            success: Color::Rgb(80, 250, 123),
            error: Color::Rgb(255, 85, 85),
            warning: Color::Rgb(241, 250, 140),
            thinking: Color::Rgb(189, 147, 249),
            dimmed: Color::Rgb(98, 114, 164),
            border_style: "single".into(),
            bg_edit: Color::Rgb(20, 35, 20),
            bg_read: Color::Rgb(20, 25, 38),
            bg_bash: Color::Rgb(35, 30, 18),
            bg_search: Color::Rgb(18, 28, 35),
            bg_agent: Color::Rgb(28, 22, 38),
            bg_default: Color::Rgb(32, 33, 42),
        }
    }

    /// Solarized dark — warm, muted, easy on the eyes
    pub fn solarized() -> Self {
        Self {
            name: "solarized".into(),
            primary: Color::Rgb(38, 139, 210),
            secondary: Color::Rgb(42, 161, 152),
            accent: Color::Rgb(203, 75, 22),
            background: Color::Rgb(0, 43, 54),
            text: Color::Rgb(147, 161, 161),
            success: Color::Rgb(133, 153, 0),
            error: Color::Rgb(220, 50, 47),
            warning: Color::Rgb(181, 137, 0),
            thinking: Color::Rgb(108, 113, 196),
            dimmed: Color::Rgb(88, 110, 117),
            border_style: "single".into(),
            bg_edit: Color::Rgb(8, 38, 12),
            bg_read: Color::Rgb(4, 30, 42),
            bg_bash: Color::Rgb(28, 28, 8),
            bg_search: Color::Rgb(4, 32, 36),
            bg_agent: Color::Rgb(14, 14, 36),
            bg_default: Color::Rgb(0, 36, 44),
        }
    }

    /// Monokai — warm syntax colors on dark gray
    pub fn monokai() -> Self {
        Self {
            name: "monokai".into(),
            primary: Color::Rgb(166, 226, 46),
            secondary: Color::Rgb(102, 217, 239),
            accent: Color::Rgb(249, 38, 114),
            background: Color::Rgb(39, 40, 34),
            text: Color::Rgb(248, 248, 242),
            success: Color::Rgb(166, 226, 46),
            error: Color::Rgb(249, 38, 114),
            warning: Color::Rgb(230, 219, 116),
            thinking: Color::Rgb(174, 129, 255),
            dimmed: Color::Rgb(117, 113, 94),
            border_style: "single".into(),
            bg_edit: Color::Rgb(22, 32, 14),
            bg_read: Color::Rgb(14, 18, 30),
            bg_bash: Color::Rgb(32, 28, 14),
            bg_search: Color::Rgb(14, 24, 30),
            bg_agent: Color::Rgb(22, 16, 32),
            bg_default: Color::Rgb(30, 30, 26),
        }
    }

    /// Gruvbox — earthy, warm retro with strong contrast
    pub fn gruvbox() -> Self {
        Self {
            name: "gruvbox".into(),
            primary: Color::Rgb(184, 187, 38),
            secondary: Color::Rgb(131, 165, 152),
            accent: Color::Rgb(251, 73, 52),
            background: Color::Rgb(40, 40, 40),
            text: Color::Rgb(235, 219, 178),
            success: Color::Rgb(184, 187, 38),
            error: Color::Rgb(251, 73, 52),
            warning: Color::Rgb(250, 189, 47),
            thinking: Color::Rgb(211, 134, 155),
            dimmed: Color::Rgb(146, 131, 116),
            border_style: "single".into(),
            bg_edit: Color::Rgb(24, 30, 14),
            bg_read: Color::Rgb(18, 22, 28),
            bg_bash: Color::Rgb(32, 26, 12),
            bg_search: Color::Rgb(16, 24, 26),
            bg_agent: Color::Rgb(26, 18, 24),
            bg_default: Color::Rgb(32, 30, 28),
        }
    }

    /// Nord — cool blue arctic palette
    pub fn nord() -> Self {
        Self {
            name: "nord".into(),
            primary: Color::Rgb(136, 192, 208),
            secondary: Color::Rgb(129, 161, 193),
            accent: Color::Rgb(191, 97, 106),
            background: Color::Rgb(46, 52, 64),
            text: Color::Rgb(216, 222, 233),
            success: Color::Rgb(163, 190, 140),
            error: Color::Rgb(191, 97, 106),
            warning: Color::Rgb(235, 203, 139),
            thinking: Color::Rgb(180, 142, 173),
            dimmed: Color::Rgb(76, 86, 106),
            border_style: "single".into(),
            bg_edit: Color::Rgb(22, 30, 22),
            bg_read: Color::Rgb(20, 24, 34),
            bg_bash: Color::Rgb(30, 28, 18),
            bg_search: Color::Rgb(18, 26, 32),
            bg_agent: Color::Rgb(26, 22, 30),
            bg_default: Color::Rgb(38, 42, 50),
        }
    }

    /// Catppuccin Mocha — pastel on dark, cozy
    pub fn catppuccin() -> Self {
        Self {
            name: "catppuccin".into(),
            primary: Color::Rgb(137, 180, 250),
            secondary: Color::Rgb(148, 226, 213),
            accent: Color::Rgb(245, 194, 231),
            background: Color::Rgb(30, 30, 46),
            text: Color::Rgb(205, 214, 244),
            success: Color::Rgb(166, 227, 161),
            error: Color::Rgb(243, 139, 168),
            warning: Color::Rgb(249, 226, 175),
            thinking: Color::Rgb(203, 166, 247),
            dimmed: Color::Rgb(88, 91, 112),
            border_style: "single".into(),
            bg_edit: Color::Rgb(20, 30, 20),
            bg_read: Color::Rgb(18, 20, 34),
            bg_bash: Color::Rgb(30, 26, 16),
            bg_search: Color::Rgb(16, 24, 30),
            bg_agent: Color::Rgb(24, 18, 32),
            bg_default: Color::Rgb(24, 24, 36),
        }
    }

    /// Tokyo Night — blue-purple modern IDE vibes
    pub fn tokyo_night() -> Self {
        Self {
            name: "tokyo-night".into(),
            primary: Color::Rgb(122, 162, 247),
            secondary: Color::Rgb(125, 207, 255),
            accent: Color::Rgb(255, 158, 100),
            background: Color::Rgb(26, 27, 38),
            text: Color::Rgb(169, 177, 214),
            success: Color::Rgb(115, 218, 202),
            error: Color::Rgb(247, 118, 142),
            warning: Color::Rgb(224, 175, 104),
            thinking: Color::Rgb(187, 154, 247),
            dimmed: Color::Rgb(68, 75, 106),
            border_style: "single".into(),
            bg_edit: Color::Rgb(16, 28, 18),
            bg_read: Color::Rgb(14, 16, 30),
            bg_bash: Color::Rgb(28, 24, 14),
            bg_search: Color::Rgb(12, 22, 28),
            bg_agent: Color::Rgb(22, 16, 30),
            bg_default: Color::Rgb(22, 22, 30),
        }
    }

    /// Synthwave — 80s retro, hot purple and cyan glow
    pub fn synthwave() -> Self {
        Self {
            name: "synthwave".into(),
            primary: Color::Rgb(255, 110, 199),
            secondary: Color::Rgb(114, 247, 238),
            accent: Color::Rgb(254, 215, 102),
            background: Color::Rgb(22, 18, 32),
            text: Color::Rgb(230, 225, 255),
            success: Color::Rgb(114, 247, 180),
            error: Color::Rgb(254, 72, 104),
            warning: Color::Rgb(254, 215, 102),
            thinking: Color::Rgb(192, 132, 252),
            dimmed: Color::Rgb(80, 68, 110),
            border_style: "double".into(),
            bg_edit: Color::Rgb(12, 26, 16),
            bg_read: Color::Rgb(10, 16, 28),
            bg_bash: Color::Rgb(28, 22, 10),
            bg_search: Color::Rgb(8, 22, 26),
            bg_agent: Color::Rgb(20, 12, 30),
            bg_default: Color::Rgb(18, 14, 26),
        }
    }

    /// Midnight — pure dark with icy highlights
    pub fn midnight() -> Self {
        Self {
            name: "midnight".into(),
            primary: Color::Rgb(100, 200, 255),
            secondary: Color::Rgb(160, 180, 210),
            accent: Color::Rgb(255, 140, 100),
            background: Color::Rgb(8, 8, 16),
            text: Color::Rgb(200, 210, 230),
            success: Color::Rgb(120, 230, 150),
            error: Color::Rgb(255, 90, 90),
            warning: Color::Rgb(255, 200, 80),
            thinking: Color::Rgb(170, 140, 240),
            dimmed: Color::Rgb(60, 65, 85),
            border_style: "single".into(),
            bg_edit: Color::Rgb(8, 22, 10),
            bg_read: Color::Rgb(6, 10, 22),
            bg_bash: Color::Rgb(22, 18, 6),
            bg_search: Color::Rgb(4, 16, 22),
            bg_agent: Color::Rgb(14, 8, 22),
            bg_default: Color::Rgb(10, 10, 14),
        }
    }

    pub fn all_names() -> &'static [&'static str] {
        &[
            "cyberpunk", "clean", "retro", "neon", "dracula", "solarized",
            "monokai", "gruvbox", "nord", "catppuccin", "tokyo-night",
            "synthwave", "midnight",
        ]
    }

    pub fn by_name(name: &str) -> Self {
        match name {
            "clean" => Self::clean(),
            "retro" => Self::retro(),
            "neon" => Self::neon(),
            "dracula" => Self::dracula(),
            "solarized" => Self::solarized(),
            "monokai" => Self::monokai(),
            "gruvbox" => Self::gruvbox(),
            "nord" => Self::nord(),
            "catppuccin" => Self::catppuccin(),
            "tokyo-night" => Self::tokyo_night(),
            "synthwave" => Self::synthwave(),
            "midnight" => Self::midnight(),
            _ => Self::cyberpunk(),
        }
    }

    pub fn border_set(&self) -> ratatui::symbols::border::Set {
        match self.border_style.as_str() {
            "double" => ratatui::symbols::border::DOUBLE,
            "ascii" => ratatui::symbols::border::Set {
                top_left: "+",
                top_right: "+",
                bottom_left: "+",
                bottom_right: "+",
                vertical_left: "|",
                vertical_right: "|",
                horizontal_top: "-",
                horizontal_bottom: "-",
            },
            _ => ratatui::symbols::border::PLAIN,
        }
    }
}
