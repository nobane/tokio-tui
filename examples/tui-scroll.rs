// tokio-tui/examples/tui-scroll.rs
use anyhow::Result;
use rand::{Rng, seq::SliceRandom};
use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Layout},
    style::{Color, Style},
};
use std::time::Instant;
use tokio_tui::{ScrollbackWidget, StyledText, Tui, TuiApp, TuiWidget as _};

// Define both constant files
const TITLE: &str = "Scrollbox Demo";
// Main app state implementing TuiApp trait
struct ScrollingDemoApp {
    scrolling: ScrollbackWidget,
    refresh: bool,
    quit: bool,
    last_styled_update: Instant,
    init_line_count: usize,
    append_during_render: bool, // Flag to control appending during render
    entry_counter: usize,       // Counter for unique entries
}

impl ScrollingDemoApp {
    fn new() -> Self {
        Self {
            scrolling: ScrollbackWidget::new(TITLE, 99999).wrap_indent(27),
            refresh: false,
            quit: false,
            last_styled_update: Instant::now(),
            init_line_count: 10,
            append_during_render: false,
            entry_counter: 0,
        }
    }

    fn add_styled_demo_entry(&mut self) {
        self.entry_counter += 1;
        let timestamp = chrono::Local::now();
        let mut rng = rand::thread_rng();

        // List of possible colors for variety
        let colors = [
            Color::Red,
            Color::Green,
            Color::Yellow,
            Color::Blue,
            Color::Magenta,
            Color::Cyan,
            Color::Gray,
            Color::DarkGray,
            Color::LightRed,
            Color::LightGreen,
            Color::LightYellow,
            Color::LightBlue,
            Color::LightMagenta,
            Color::LightCyan,
            Color::Rgb(255, 128, 0),
            Color::Rgb(128, 0, 255),
            Color::Rgb(0, 255, 128),
            Color::Rgb(255, 0, 128),
        ];

        // List of possible words/phrases for varying content
        let words = [
            "short",
            "medium length",
            "this is a bit longer",
            "extremely long text to test how wrapping works with really extended content extremely long text to test how wrapping works with really extended content extremely long text to test how wrapping works with really extended content extremely long text to test how wrapping works with really extended content",
            "testing",
            "scrollback",
            "rendering",
            "artifacts",
            "TUI",
            "terminal",
            "user",
            "interface",
            "visualization",
            "debugging",
        ];

        // Randomly determine line length (1-8 segments)
        let segments = rng.gen_range(1..=8);

        // Start building the styled text
        let mut styled_text = StyledText::default();
        styled_text
            .append(
                timestamp.format("%H:%M:%S.%3f").to_string(),
                Style::default().fg(*colors.choose(&mut rng).unwrap()),
            )
            .append_spaces(2)
            .append(
                format!("ENTRY #{:04}", self.entry_counter),
                Style::default().fg(*colors.choose(&mut rng).unwrap()),
            )
            .append_spaces(2);

        // Add random number of segments with random colors and text
        for i in 0..segments {
            // Choose a word/phrase and a color
            let word = words.choose(&mut rng).unwrap();

            // Add segment with space before (except for first segment)
            if i > 0 {
                styled_text.append_space();
            }

            // styled_text.append(word, style);
            styled_text.append_string(word);
        }

        self.scrolling.add_styled_line(styled_text.to_owned());
        self.last_styled_update = Instant::now();
    }

    fn initialize_styled_demo(&mut self) {
        self.scrolling.clear();
        self.entry_counter = 0;

        // Add initial styled entries based on the configured count
        for _ in 0..self.init_line_count {
            self.add_styled_demo_entry();
        }
    }

    fn toggle_append_mode(&mut self, append_during_render: bool) {
        self.append_during_render = append_during_render;

        // Update the title to reflect the current status
        let mode_text = if self.append_during_render {
            format!("{TITLE} [Append ON]")
        } else {
            TITLE.to_string()
        };

        self.scrolling.set_title(&mode_text);
    }
}

impl TuiApp for ScrollingDemoApp {
    fn should_draw(&mut self) -> bool {
        self.scrolling.need_draw()
    }

    fn render(&mut self, frame: &mut tokio_tui::TerminalFrame) {
        if self.refresh {
            self.refresh = false;
            return;
        }

        // Check if we should add content during render
        if self.append_during_render {
            self.add_styled_demo_entry();
        }

        let area = frame.area();
        let buf = frame.buffer_mut();
        let [main_area] = Layout::vertical([Constraint::Fill(1)]).areas(area);
        self.scrolling.draw(main_area, buf);
    }
    fn handle_mouse_events(&mut self, mouse_events: Vec<crossterm::event::MouseEvent>) {
        for event in mouse_events {
            self.scrolling.mouse_event(event);
        }
    }

    fn handle_key_events(&mut self, keys: Vec<KeyEvent>) {
        for key in keys {
            if !self.scrolling.key_event(key) {
                match key.code {
                    KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.refresh = true;
                    }
                    KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.add_styled_demo_entry();
                    }
                    KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.toggle_append_mode(!self.append_during_render);
                    }
                    KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.quit = true;
                    }
                    _ => {}
                }
            }
        }
    }

    fn should_quit(&self) -> bool {
        self.quit
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Create app with default settings
    let mut app = ScrollingDemoApp::new();

    let args: Vec<String> = std::env::args().collect();

    // Check for the append-during-render flag
    if args
        .iter()
        .any(|arg| arg == "--append-render" || arg == "-ar")
    {
        app.toggle_append_mode(true);
    }

    if let Some(lines) = args
        .iter()
        .find(|arg| arg.starts_with("--lines="))
        .and_then(|l| l.trim_start_matches("--lines=").parse::<usize>().ok())
    {
        // Initialize the styled demo with the specified number of lines
        app.init_line_count = lines;
    }
    app.initialize_styled_demo();

    Tui::new()?.run(app)?;

    Ok(())
}
