//! Headless mode for AI-assisted testing and automation.
//!
//! Provides a way to run the TUI without a terminal, executing scripted events
//! and capturing output for verification.

mod events;
mod output;

pub use events::{Event, EventParser};
#[allow(unused_imports)]
pub use output::OutputFormat;
pub use output::{HeadlessOutput, ScreenRenderer};

use crate::cli::Cli;
use crate::error::{GlanceError, Result};
use crate::tui::app::App;
use crate::tui::ui;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::time::{Duration, Instant};

/// Configuration for headless mode execution.
#[derive(Debug, Clone)]
pub struct HeadlessConfig {
    /// Screen width in columns.
    pub width: u16,
    /// Screen height in rows.
    pub height: u16,
    /// Output format.
    pub output_format: output::OutputFormat,
    /// Whether to stop on first assertion failure.
    pub fail_fast: bool,
    /// Path to write output (None = stdout).
    pub output_file: Option<std::path::PathBuf>,
}

impl HeadlessConfig {
    /// Creates a HeadlessConfig from CLI arguments.
    pub fn from_cli(cli: &Cli) -> Result<Self> {
        let (width, height) = cli
            .parse_screen_size()
            .map_err(GlanceError::config)?;

        let output_format = cli
            .parse_output_format()
            .map_err(GlanceError::config)?;

        Ok(Self {
            width,
            height,
            output_format: match output_format {
                crate::cli::OutputFormat::Text => output::OutputFormat::Text,
                crate::cli::OutputFormat::Json => output::OutputFormat::Json,
                crate::cli::OutputFormat::Frames => output::OutputFormat::Frames,
            },
            fail_fast: cli.fail_fast,
            output_file: cli.output_file.clone(),
        })
    }
}

/// Result of headless execution.
#[derive(Debug)]
pub struct HeadlessResult {
    /// Final screen content as text.
    pub screen: String,
    /// Screen lines for JSON output.
    pub screen_lines: Vec<String>,
    /// Number of events executed.
    pub events_executed: usize,
    /// Total execution duration.
    pub duration: Duration,
    /// Number of assertions passed.
    pub assertions_passed: usize,
    /// Number of assertions failed.
    pub assertions_failed: usize,
    /// Application state snapshot.
    pub state: HeadlessState,
    /// Frame captures (for frames output mode).
    pub frames: Vec<Frame>,
}

/// Snapshot of application state for JSON output.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HeadlessState {
    /// Current input text.
    pub input_text: String,
    /// Current focus panel.
    pub focus: String,
    /// Whether the app is processing.
    pub is_processing: bool,
    /// Number of messages in chat.
    pub message_count: usize,
    /// Whether the app is still running.
    pub running: bool,
}

impl HeadlessState {
    fn from_app(app: &App) -> Self {
        Self {
            input_text: app.input.text.clone(),
            focus: format!("{:?}", app.focus),
            is_processing: app.is_processing,
            message_count: app.messages.len(),
            running: app.running,
        }
    }
}

/// A captured frame (screen state after an event).
#[derive(Debug, Clone)]
pub struct Frame {
    /// Frame number (0 = initial state).
    pub number: usize,
    /// Event that produced this frame (None for initial).
    pub event: Option<String>,
    /// Screen content.
    pub screen: String,
}

/// Runs the TUI in headless mode.
pub struct HeadlessRunner {
    config: HeadlessConfig,
    terminal: Terminal<TestBackend>,
    app: App,
    events: Vec<Event>,
    frames: Vec<Frame>,
    start_time: Instant,
    assertions_passed: usize,
    assertions_failed: usize,
}

impl HeadlessRunner {
    /// Creates a new headless runner with the given configuration.
    pub fn new(config: HeadlessConfig) -> Result<Self> {
        let backend = TestBackend::new(config.width, config.height);
        let terminal = Terminal::new(backend)
            .map_err(|e| GlanceError::internal(format!("Failed to create test terminal: {e}")))?;

        let app = App::new(None);

        Ok(Self {
            config,
            terminal,
            app,
            events: Vec::new(),
            frames: Vec::new(),
            start_time: Instant::now(),
            assertions_passed: 0,
            assertions_failed: 0,
        })
    }

    /// Loads events from a string (comma-separated or newline-separated).
    pub fn load_events(&mut self, input: &str) -> Result<()> {
        let parser = EventParser::new();
        self.events = parser.parse_all(input)?;
        Ok(())
    }

    /// Loads events from a script file.
    pub fn load_script(&mut self, path: &str) -> Result<()> {
        let content = if path == "-" {
            use std::io::Read;
            let mut buffer = String::new();
            std::io::stdin()
                .read_to_string(&mut buffer)
                .map_err(|e| GlanceError::internal(format!("Failed to read stdin: {e}")))?;
            buffer
        } else {
            std::fs::read_to_string(path)
                .map_err(|e| GlanceError::internal(format!("Failed to read script file: {e}")))?
        };

        self.load_events(&content)
    }

    /// Runs the headless execution and returns the result.
    pub fn run(mut self) -> Result<HeadlessResult> {
        self.start_time = Instant::now();

        // Capture initial frame
        self.capture_frame(None)?;

        let events = std::mem::take(&mut self.events);
        let mut events_executed = 0;

        for event in events {
            let event_str = event.to_string();

            // Handle the event
            match &event {
                Event::Key(key_event) => {
                    self.app
                        .handle_event(crate::tui::Event::Key(*key_event));
                }
                Event::Type(text) => {
                    for c in text.chars() {
                        self.app.input.insert(c);
                    }
                }
                Event::Wait(duration) => {
                    std::thread::sleep(*duration);
                }
                Event::Resize(w, h) => {
                    self.terminal
                        .resize(ratatui::layout::Rect::new(0, 0, *w, *h))
                        .map_err(|e| GlanceError::internal(format!("Resize failed: {e}")))?;
                }
                Event::Snapshot(_name) => {
                    // Snapshots are captured as frames
                }
                Event::Assert(assertion) => {
                    let screen = self.render_screen()?;
                    if assertion.check(&screen, &self.app) {
                        self.assertions_passed += 1;
                    } else {
                        self.assertions_failed += 1;
                        if self.config.fail_fast {
                            break;
                        }
                    }
                }
            }

            events_executed += 1;

            // Render after each event
            self.terminal
                .draw(|frame| ui::render(frame, &mut self.app))
                .map_err(|e| GlanceError::internal(format!("Failed to render: {e}")))?;

            // Capture frame if in frames mode
            if self.config.output_format == output::OutputFormat::Frames {
                self.capture_frame(Some(event_str))?;
            }

            // Check if app has exited
            if !self.app.running {
                break;
            }
        }

        // Final render
        self.terminal
            .draw(|frame| ui::render(frame, &mut self.app))
            .map_err(|e| GlanceError::internal(format!("Failed to render: {e}")))?;

        let screen = self.render_screen()?;
        let screen_lines = screen.lines().map(String::from).collect();

        Ok(HeadlessResult {
            screen,
            screen_lines,
            events_executed,
            duration: self.start_time.elapsed(),
            assertions_passed: self.assertions_passed,
            assertions_failed: self.assertions_failed,
            state: HeadlessState::from_app(&self.app),
            frames: self.frames,
        })
    }

    /// Renders the current screen to a string.
    fn render_screen(&self) -> Result<String> {
        let buffer = self.terminal.backend().buffer();
        Ok(ScreenRenderer::render(buffer))
    }

    /// Captures the current frame.
    fn capture_frame(&mut self, event: Option<String>) -> Result<()> {
        // Render first
        self.terminal
            .draw(|frame| ui::render(frame, &mut self.app))
            .map_err(|e| GlanceError::internal(format!("Failed to render: {e}")))?;

        let screen = self.render_screen()?;
        let number = self.frames.len();

        self.frames.push(Frame {
            number,
            event,
            screen,
        });

        Ok(())
    }
}

/// Runs headless mode from CLI arguments.
pub async fn run_headless(cli: &Cli) -> Result<i32> {
    // Validate headless arguments
    cli.validate_headless()
        .map_err(GlanceError::config)?;

    let config = HeadlessConfig::from_cli(cli)?;
    let mut runner = HeadlessRunner::new(config.clone())?;

    // Load events
    if let Some(ref events_str) = cli.events {
        runner.load_events(events_str)?;
    } else if let Some(ref script_path) = cli.script {
        runner.load_script(script_path)?;
    }

    // Run
    let result = runner.run()?;

    // Generate output
    let output = HeadlessOutput::new(config.output_format);
    let output_str = output.format(&result);

    // Write output
    if let Some(ref path) = config.output_file {
        std::fs::write(path, &output_str)
            .map_err(|e| GlanceError::internal(format!("Failed to write output file: {e}")))?;
    } else {
        print!("{}", output_str);
    }

    // Return exit code based on assertions
    if result.assertions_failed > 0 {
        Ok(1)
    } else {
        Ok(0)
    }
}
