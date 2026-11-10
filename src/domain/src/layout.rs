//! Window layout: how much room the browser chrome takes and where the website card sits.
//! The chrome webview always spans the window; the website card is inset by `inset`.

/// Gap between the website card and the window edge (logical px).
pub const CARD_GAP: f64 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Layout {
    /// Full sidebar on the left (300 px).
    #[default]
    Sidebar,
    /// Narrower sidebar (240 px): same content, tighter.
    Compact,
    /// Icon-only rail on the left (64 px).
    Rail,
    /// One slim strip along the top: nav, address and tabs. The page gets the full width.
    Top,
}

impl Layout {
    pub const ALL: [Layout; 4] = [Layout::Sidebar, Layout::Compact, Layout::Rail, Layout::Top];

    /// Parses a saved name; anything unknown falls back to the default sidebar.
    pub fn parse(name: &str) -> Layout {
        Self::ALL.into_iter().find(|l| l.name() == name.trim()).unwrap_or_default()
    }

    pub fn name(self) -> &'static str {
        match self {
            Layout::Sidebar => "sidebar",
            Layout::Compact => "compact",
            Layout::Rail => "rail",
            Layout::Top => "top",
        }
    }

    /// Left and top edge of the website card, in logical px.
    pub fn inset(self) -> (f64, f64) {
        match self {
            Layout::Sidebar => (300.0, CARD_GAP),
            Layout::Compact => (240.0, CARD_GAP),
            Layout::Rail => (64.0, CARD_GAP),
            Layout::Top => (CARD_GAP, 48.0),
        }
    }

    /// Where the card goes while the chrome "peeks" open: the rail widens to a full sidebar to
    /// type an address, the top bar drops a 312 px panel (site, archive, closed tabs, profiles).
    pub fn peek_inset(self) -> (f64, f64) {
        match self {
            Layout::Rail => Layout::Sidebar.inset(),
            Layout::Top => (312.0, 48.0),
            other => other.inset(),
        }
    }

    /// The website card's (x, y, width, height) in a window of `width` × `height`.
    pub fn card(self, width: f64, height: f64, peek: bool) -> (f64, f64, f64, f64) {
        let (x, y) = if peek { self.peek_inset() } else { self.inset() };
        (x, y, (width - x - CARD_GAP).max(0.0), (height - y - CARD_GAP).max(0.0))
    }
}
