use owo_colors::Style;
use std::sync::OnceLock;

static THEME: OnceLock<Theme> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct Theme {
    pub header: Style,
    pub success: Style,
    pub error: Style,
    pub warn: Style,
    pub info: Style,
    pub dim: Style,
    pub muted: Style,
}

impl Theme {
    pub fn detect() -> Self {
        if !console::Term::stdout().is_term() {
            return Self::plain();
        }
        Self::colored()
    }

    pub fn colored() -> Self {
        Self {
            header: Style::new().cyan().bold(),
            success: Style::new().green().bold(),
            error: Style::new().red().bold(),
            warn: Style::new().yellow().bold(),
            info: Style::new().magenta(),
            dim: Style::new().white().dimmed(),
            muted: Style::new().bright_black(),
        }
    }

    pub fn plain() -> Self {
        Self {
            header: Style::new(),
            success: Style::new(),
            error: Style::new(),
            warn: Style::new(),
            info: Style::new(),
            dim: Style::new(),
            muted: Style::new(),
        }
    }
}

pub fn theme() -> &'static Theme {
    THEME.get_or_init(Theme::detect)
}
