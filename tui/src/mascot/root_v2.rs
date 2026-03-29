use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootState {
    Idle,
    Thinking,
    Coding,
    Reviewing,
    Committing,
    Streaming,
    Done,
    Error,
    Deep,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeVariant {
    Cyberpunk,
    Clean,
    Retro,
}

#[derive(Debug, Clone, Copy)]
pub struct RootTheme {
    pub variant: ThemeVariant,
    pub frame: Color,
    pub cue: Color,
    pub optics: Color,
    pub core: Color,
    pub alert: Color,
}

#[derive(Debug, Clone, Copy)]
struct RootFrame {
    cue: &'static str,
    left_optic: &'static str,
    right_optic: &'static str,
    core: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct ThemeGlyphs {
    top_left: &'static str,
    top_bar: &'static str,
    top_right: &'static str,
    side_left: &'static str,
    side_right: &'static str,
    bottom_left: &'static str,
    bottom_bar: &'static str,
    bottom_right: &'static str,
}

pub fn root_lines(theme: RootTheme, state: RootState, tick: u64) -> Vec<Line<'static>> {
    let frame = root_frame(state, tick);
    let glyphs = theme_glyphs(theme.variant);
    let frame_style = Style::default().fg(theme.frame);
    let cue_style = cue_style(theme, state);
    let optic_style = optic_style(theme, state);
    let core_style = core_style(theme, state);

    vec![
        Line::from(vec![
            Span::raw("    "),
            Span::styled(frame.cue, cue_style),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(glyphs.top_left, frame_style),
            Span::styled(glyphs.top_bar, frame_style),
            Span::styled(glyphs.top_right, frame_style),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(glyphs.side_left, frame_style),
            Span::raw(" "),
            Span::styled(frame.left_optic, optic_style),
            Span::raw("  "),
            Span::styled(frame.right_optic, optic_style),
            Span::raw(" "),
            Span::styled(glyphs.side_right, frame_style),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(glyphs.side_left, frame_style),
            Span::raw(" "),
            Span::styled(frame.core, core_style),
            Span::raw(" "),
            Span::styled(glyphs.side_right, frame_style),
        ]),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(glyphs.bottom_left, frame_style),
            Span::styled(glyphs.bottom_bar, frame_style),
            Span::styled(glyphs.bottom_right, frame_style),
        ]),
    ]
}

fn root_frame(state: RootState, tick: u64) -> RootFrame {
    match state {
        RootState::Idle => idle_frame(tick),
        RootState::Thinking => thinking_frame(tick),
        RootState::Coding => coding_frame(tick),
        RootState::Reviewing => reviewing_frame(tick),
        RootState::Committing => committing_frame(tick),
        RootState::Streaming => streaming_frame(tick),
        RootState::Done => done_frame(tick),
        RootState::Error => error_frame(tick),
        RootState::Deep => deep_frame(tick),
        RootState::Critical => critical_frame(tick),
    }
}

fn idle_frame(tick: u64) -> RootFrame {
    if rare_glare(tick) {
        return RootFrame {
            cue: "<::>",
            left_optic: "█",
            right_optic: "█",
            core: "·──·",
        };
    }

    if idle_scan(tick) {
        return match (tick / 2) % 4 {
            0 => RootFrame { cue: "<::>", left_optic: "⌐", right_optic: "◈", core: "·──·" },
            1 => RootFrame { cue: "<:·>", left_optic: "⌐", right_optic: "⌐", core: "·──·" },
            2 => RootFrame { cue: "<·:>", left_optic: "◈", right_optic: "⌐", core: "·──·" },
            _ => RootFrame { cue: "<::>", left_optic: "◈", right_optic: "◈", core: "·──·" },
        };
    }

    let (left_optic, right_optic) = if blink(tick, 61, 1) { ("-", "-") } else { ("◈", "◈") };
    let cue = match (tick / 18) % 3 {
        0 => "<::>",
        1 => "<:·>",
        _ => "<·:>",
    };
    let core = "·──·";

    RootFrame {
        cue,
        left_optic,
        right_optic,
        core,
    }
}

fn thinking_frame(tick: u64) -> RootFrame {
    let cue = match (tick / 6) % 4 {
        0 => "<:·>",
        1 => "<··>",
        2 => "<·:>",
        _ => "<··>",
    };
    let (left_optic, right_optic) = match (tick / 9) % 4 {
        0 => ("¬", "¬"),
        1 => ("⌐", "⌐"),
        2 => ("¬", "¬"),
        _ => ("⌐", "⌐"),
    };
    // Waveform scrolling
    let core = match (tick / 4) % 4 {
        0 => "▁▂▃▂",
        1 => "▂▃▂▁",
        2 => "▃▂▁▂",
        _ => "▂▁▂▃",
    };

    RootFrame {
        cue,
        left_optic,
        right_optic,
        core,
    }
}

fn coding_frame(tick: u64) -> RootFrame {
    let cue = match (tick / 4) % 4 {
        0 => "<+  >",
        1 => "<++ >",
        2 => "<+++>",
        _ => "< ++>",
    };
    let (left_optic, right_optic) = if (tick / 8) % 2 == 0 {
        ("◈", "◈")
    } else {
        ("◎", "◎")
    };
    // Scanning bar bouncing
    let core = match (tick / 3) % 4 {
        0 => "▓───",
        1 => "─▓──",
        2 => "──▓─",
        _ => "─▓──",
    };

    RootFrame {
        cue,
        left_optic,
        right_optic,
        core,
    }
}

fn reviewing_frame(tick: u64) -> RootFrame {
    let cue = if (tick / 16) % 2 == 0 { "<··>" } else { "<::>" };
    let (left_optic, right_optic) = match (tick / 8) % 4 {
        0 => ("⌐", "¬"),
        1 => ("¬", "¬"),
        2 => ("¬", "⌐"),
        _ => ("⌐", "⌐"),
    };
    // Slow pulse
    let core = match (tick / 15) % 4 {
        0 => "─▪──",
        1 => "─•──",
        2 => "─●──",
        _ => "─•──",
    };

    RootFrame {
        cue,
        left_optic,
        right_optic,
        core,
    }
}

fn committing_frame(tick: u64) -> RootFrame {
    let cue = if (tick / 14) % 2 == 0 { "<::>" } else { "<:·>" };
    let (left_optic, right_optic) = if (tick / 10) % 2 == 0 {
        ("◎", "◎")
    } else {
        ("◈", "◈")
    };
    // Heartbeat
    let core = match (tick / 5) % 5 {
        0 => "────",
        1 => "─▂──",
        2 => "─▅──",
        3 => "─▂──",
        _ => "────",
    };

    RootFrame {
        cue,
        left_optic,
        right_optic,
        core,
    }
}

fn streaming_frame(tick: u64) -> RootFrame {
    let cue = match (tick / 8) % 4 {
        0 => "<~\u{00B7}>",
        1 => "<~~>",
        2 => "<\u{00B7}~>",
        _ => "<~~>",
    };
    let (left_optic, right_optic) = if (tick / 12) % 2 == 0 {
        ("\u{25C8}", "\u{25C8}")
    } else {
        ("\u{25CE}", "\u{25CE}")
    };
    let core = match (tick / 6) % 4 {
        0 => "\u{2581}\u{2582}\u{2583}\u{2582}",
        1 => "\u{2582}\u{2583}\u{2582}\u{2581}",
        2 => "\u{2583}\u{2582}\u{2581}\u{2582}",
        _ => "\u{2582}\u{2581}\u{2582}\u{2583}",
    };
    RootFrame { cue, left_optic, right_optic, core }
}

fn done_frame(tick: u64) -> RootFrame {
    let (left_optic, right_optic) = if blink(tick, 73, 1) { ("-", "-") } else { ("◈", "◈") };

    RootFrame {
        cue: "<  >",
        left_optic,
        right_optic,
        core: "────",
    }
}

fn error_frame(tick: u64) -> RootFrame {
    let cue = if (tick / 3) % 2 == 0 { "<!!!>" } else { "< ! >" };
    let (left_optic, right_optic) = if (tick / 3) % 2 == 0 {
        ("!", "!")
    } else {
        ("◈", "◈")
    };
    let core = if (tick / 4) % 2 == 0 { "─××─" } else { "×──×" };

    RootFrame {
        cue,
        left_optic,
        right_optic,
        core,
    }
}

fn deep_frame(tick: u64) -> RootFrame {
    let cue = if (tick / 12) % 2 == 0 { "<::|>" } else { "<|::>" };
    let (left_optic, right_optic) = if (tick / 14) % 2 == 0 {
        ("◎", "◎")
    } else {
        ("◈", "◈")
    };
    // Slow breathe
    let core = match (tick / 10) % 4 {
        0 => "─··─",
        1 => "··─·",
        2 => "·─··",
        _ => "──··",
    };

    RootFrame {
        cue,
        left_optic,
        right_optic,
        core,
    }
}

fn critical_frame(tick: u64) -> RootFrame {
    let cue = if (tick / 2) % 2 == 0 { "<###>" } else { "<!!!>" };
    let (left_optic, right_optic) = if (tick / 2) % 2 == 0 {
        ("◈", "◈")
    } else {
        ("!", "!")
    };
    let core = if (tick / 2) % 2 == 0 { "××××" } else { "─××─" };

    RootFrame {
        cue,
        left_optic,
        right_optic,
        core,
    }
}

fn blink(tick: u64, period: u64, duration: u64) -> bool {
    tick % period < duration
}

fn idle_scan(tick: u64) -> bool {
    let phase = tick % 300;
    (280..288).contains(&phase)
}

fn rare_glare(tick: u64) -> bool {
    tick % 1800 == 0 && tick != 0
}

fn cue_style(theme: RootTheme, state: RootState) -> Style {
    let color = match state {
        RootState::Error | RootState::Critical => theme.alert,
        _ => theme.cue,
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn optic_style(theme: RootTheme, state: RootState) -> Style {
    let color = match state {
        RootState::Error | RootState::Critical => theme.alert,
        _ => theme.optics,
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn core_style(theme: RootTheme, state: RootState) -> Style {
    let color = match state {
        RootState::Error | RootState::Critical => theme.alert,
        _ => theme.core,
    };
    Style::default().fg(color)
}

fn theme_glyphs(variant: ThemeVariant) -> ThemeGlyphs {
    match variant {
        ThemeVariant::Cyberpunk => ThemeGlyphs {
            top_left: "╔",
            top_bar: "══════",
            top_right: "╗",
            side_left: "║",
            side_right: "║",
            bottom_left: "╚",
            bottom_bar: "══════",
            bottom_right: "╝",
        },
        ThemeVariant::Clean => ThemeGlyphs {
            top_left: "┌",
            top_bar: "──────",
            top_right: "┐",
            side_left: "│",
            side_right: "│",
            bottom_left: "└",
            bottom_bar: "──────",
            bottom_right: "┘",
        },
        ThemeVariant::Retro => ThemeGlyphs {
            top_left: "+",
            top_bar: "------",
            top_right: "+",
            side_left: "|",
            side_right: "|",
            bottom_left: "+",
            bottom_bar: "------",
            bottom_right: "+",
        },
    }
}
