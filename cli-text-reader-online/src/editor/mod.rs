use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute,
    terminal::{self, Clear, ClearType},
};
use std::io::{self, IsTerminal, Write};
use std::sync::Arc;

use crate::config::load_config;
use crate::server::HyggClient;
use crate::tutorial::get_tutorial_text;

pub mod state;
pub mod display;
pub mod commands;
pub mod search;
pub mod highlight;

pub use state::{EditorMode, EditorState};
use display::render_editor;
use commands::execute_command;
use search::{find_next_match, center_on_match};
use highlight::handle_highlights;

// Re-export for compatibility
pub use commands::execute_command as cmd_execute_command;

pub struct Editor {
    pub lines: Vec<String>,
    pub col: usize,
    pub offset: usize,
    pub width: usize,
    pub height: usize,
    pub show_highlighter: bool,
    pub editor_state: EditorState,
    pub highlights: Vec<usize>, // Store highlighted line numbers
    pub document_hash: u64,
    pub total_lines: usize,
    pub progress_display_until: Option<std::time::Instant>,
    pub show_progress: bool,
    pub progress_callback: Option<Box<dyn Fn(usize) + Send>>,
    pub read_only: bool,
    pub client: Option<Arc<HyggClient>>,
    pub file_path: String,
}

impl Editor {
    pub fn new(lines: Vec<String>, col: usize, file_path: String, client: Option<Arc<HyggClient>>) -> Self {
        let document_hash = crate::progress::generate_hash(&lines);
        let total_lines = lines.len();
        let (width, height) = terminal::size()
            .map(|(w, h)| (w as usize, h as usize))
            .unwrap_or((80, 24));
            
        // Try to load highlights
        let highlights = if client.is_some() {
            // We need to handle async load_highlights_async specially during initialization
            println!("Loading highlights during Editor initialization");
            if let Ok(rt) = tokio::runtime::Handle::try_current() {
                // We're in a Tokio runtime context, use it
                println!("Using existing Tokio runtime for highlight loading");
                match tokio::task::block_in_place(|| {
                    rt.block_on(async {
                        crate::progress::load_highlights_async(document_hash, client.clone()).await
                    })
                }) {
                    Ok(h) => h,
                    Err(e) => {
                        println!("Error loading highlights: {}", e);
                        Vec::new()
                    }
                }
            } else {
                println!("No Tokio runtime available for highlight loading");
                Vec::new()
            }
        } else {
            // For local highlights only, we can use the synchronous version
            match crate::progress::load_highlights(document_hash, None) {
                Ok(h) => h,
                Err(e) => {
                    println!("Error loading highlights: {}", e);
                    Vec::new()
                }
            }
        };

        Self {
            lines,
            col,
            offset: 0,
            width,
            height,
            show_highlighter: true,
            editor_state: EditorState::new(),
            document_hash,
            total_lines,
            progress_display_until: None,
            show_progress: false,
            progress_callback: None,
            read_only: false,
            highlights,
            client,
            file_path,
        }
    }

    pub fn set_position(&mut self, position: usize) {
        self.offset = position.min(self.total_lines.saturating_sub(1));
    }
    
    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
    }

    pub async fn run_with_progress<F>(&mut self, callback: F) -> Result<(), Box<dyn std::error::Error>>
    where
        F: Fn(usize) + Send + 'static,
    {
        self.progress_callback = Some(Box::new(callback));
        self.run().await
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut stdout = io::stdout();
        let config = load_config();

        self.show_highlighter = config.enable_line_highlighter.unwrap_or(true);

        let show_tutorial = match config.enable_tutorial {
            Some(false) => false,
            _ => self.lines.is_empty(),
        };

        if show_tutorial {
            self.show_tutorial(&mut stdout)?;
        }

        // If the file is empty, exit after tutorial
        if self.lines.is_empty() {
            self.cleanup(&mut stdout)?;
            return Ok(());
        }

        if std::io::stdout().is_terminal() {
            execute!(stdout, terminal::EnterAlternateScreen, Hide)?;
            terminal::enable_raw_mode()?;
        }

        self.main_loop(&mut stdout).await?;

        self.cleanup(&mut stdout)?;
        Ok(())
    }

    pub fn show_tutorial(
        &self,
        stdout: &mut io::Stdout,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tutorial_lines = get_tutorial_text();

        if std::io::stdout().is_terminal() {
            let was_raw = terminal::is_raw_mode_enabled()?;

            if !was_raw {
                terminal::enable_raw_mode()?;
            }
            execute!(stdout, Hide)?;

            let mut tutorial_offset = 0;
            loop {
                // Display tutorial with scrolling
                execute!(stdout, Clear(ClearType::All))?;
                let center_offset = if self.width > self.col {
                    (self.width / 2) - self.col / 2
                } else {
                    0
                };

                for (i, line) in tutorial_lines
                    .iter()
                    .skip(tutorial_offset)
                    .take(self.height)
                    .enumerate()
                {
                    execute!(stdout, MoveTo(center_offset as u16, i as u16))?;
                    println!("{}", line);
                }

                stdout.flush()?;

                // Handle scrolling input
                match crossterm::event::read()? {
                    crossterm::event::Event::Key(key_event) => match key_event.code {
                        crossterm::event::KeyCode::Char('j') | crossterm::event::KeyCode::Down => {
                            if tutorial_offset + self.height < tutorial_lines.len() {
                                tutorial_offset += 1;
                            }
                        }
                        crossterm::event::KeyCode::Char('k') | crossterm::event::KeyCode::Up => {
                            if tutorial_offset > 0 {
                                tutorial_offset -= 1;
                            }
                        }
                        crossterm::event::KeyCode::PageDown => {
                            tutorial_offset = (tutorial_offset + self.height)
                                .min(tutorial_lines.len().saturating_sub(self.height));
                        }
                        crossterm::event::KeyCode::PageUp => {
                            tutorial_offset = tutorial_offset.saturating_sub(self.height);
                        }
                        _ => break,
                    },
                    _ => {}
                }
            }

            // Restore original state
            execute!(stdout, Clear(ClearType::All))?;
            if !was_raw {
                terminal::disable_raw_mode()?;
            }
        }

        Ok(())
    }

    fn cleanup(
        &self,
        stdout: &mut io::Stdout,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if std::io::stdout().is_terminal() {
            execute!(stdout, Show, terminal::LeaveAlternateScreen)?;
            terminal::disable_raw_mode()?;
        }
        Ok(())
    }

    async fn main_loop(
        &mut self,
        stdout: &mut io::Stdout,
    ) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            if std::io::stdout().is_terminal() {
                execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
            }

            // Render the editor content
            render_editor(self, stdout)?;

            // Process input
            if let Ok(event) = crossterm::event::read() {
                match event {
                    crossterm::event::Event::Key(key_event) => {
                        match self.editor_state.mode {
                            EditorMode::Normal => {
                                match key_event.code {
                                    crossterm::event::KeyCode::Char('j') | crossterm::event::KeyCode::Down => {
                                        self.offset = (self.offset + 1).min(self.total_lines.saturating_sub(1));
                                    }
                                    crossterm::event::KeyCode::Char('k') | crossterm::event::KeyCode::Up => {
                                        self.offset = self.offset.saturating_sub(1);
                                    }
                                    crossterm::event::KeyCode::Char('d') => {
                                        if key_event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                                            let page_size = self.height / 2;
                                            self.offset = (self.offset + page_size).min(self.total_lines.saturating_sub(1));
                                        }
                                    }
                                    crossterm::event::KeyCode::Char('u') => {
                                        if key_event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                                            let page_size = self.height / 2;
                                            self.offset = self.offset.saturating_sub(page_size);
                                        }
                                    }
                                    crossterm::event::KeyCode::PageDown => {
                                        self.offset = (self.offset + self.height).min(self.total_lines.saturating_sub(1));
                                    }
                                    crossterm::event::KeyCode::PageUp => {
                                        self.offset = self.offset.saturating_sub(self.height);
                                    }
                                    crossterm::event::KeyCode::Home => {
                                        self.offset = 0;
                                    }
                                    crossterm::event::KeyCode::End => {
                                        self.offset = self.total_lines.saturating_sub(1);
                                    }
                                    crossterm::event::KeyCode::Char('/') => {
                                        self.editor_state.mode = EditorMode::Search;
                                        self.editor_state.command_buffer.clear();
                                        self.editor_state.search_direction = true;
                                    }
                                    crossterm::event::KeyCode::Char('?') => {
                                        self.editor_state.mode = EditorMode::ReverseSearch;
                                        self.editor_state.command_buffer.clear();
                                        self.editor_state.search_direction = false;
                                    }
                                    crossterm::event::KeyCode::Char('n') => {
                                        find_next_match(self, true);
                                    }
                                    crossterm::event::KeyCode::Char('N') => {
                                        find_next_match(self, false);
                                    }
                                    crossterm::event::KeyCode::Char('v') => {
                                        self.editor_state.mode = EditorMode::Visual;
                                        self.editor_state.visual_start = Some(self.offset + self.height / 2);
                                        self.editor_state.visual_end = self.editor_state.visual_start;
                                    }
                                    crossterm::event::KeyCode::Char(':') => {
                                        self.editor_state.mode = EditorMode::Command;
                                        self.editor_state.command_buffer.clear();
                                    }
                                    crossterm::event::KeyCode::Char('q') => {
                                        return Ok(());
                                    }
                                    _ => {}
                                }
                            }
                            EditorMode::Visual => {
                                match key_event.code {
                                    crossterm::event::KeyCode::Char('j') | crossterm::event::KeyCode::Down => {
                                        self.offset = (self.offset + 1).min(self.total_lines.saturating_sub(1));
                                        self.editor_state.visual_end = Some(self.offset + self.height / 2);
                                    }
                                    crossterm::event::KeyCode::Char('k') | crossterm::event::KeyCode::Up => {
                                        self.offset = self.offset.saturating_sub(1);
                                        self.editor_state.visual_end = Some(self.offset + self.height / 2);
                                    }
                                    crossterm::event::KeyCode::Esc => {
                                        self.editor_state.mode = EditorMode::Normal;
                                        self.editor_state.visual_start = None;
                                        self.editor_state.visual_end = None;
                                    }
                                    crossterm::event::KeyCode::Char(':') => {
                                        self.editor_state.mode = EditorMode::Command;
                                        self.editor_state.command_buffer.clear();
                                    }
                                    _ => {}
                                }
                            }
                            EditorMode::Search | EditorMode::ReverseSearch | EditorMode::Command => {
                                match key_event.code {
                                    crossterm::event::KeyCode::Esc => {
                                        self.editor_state.mode = EditorMode::Normal;
                                        self.editor_state.command_buffer.clear();
                                        self.editor_state.visual_start = None;
                                        self.editor_state.visual_end = None;
                                    }
                                    crossterm::event::KeyCode::Enter => {
                                        if self.editor_state.mode == EditorMode::Search || self.editor_state.mode == EditorMode::ReverseSearch {
                                            self.editor_state.search_query = self.editor_state.command_buffer.clone();
                                            find_next_match(self, self.editor_state.search_direction);
                                            if self.editor_state.current_match.is_some() {
                                                center_on_match(self);
                                            }
                                            self.editor_state.mode = EditorMode::Normal;
                                            self.editor_state.command_buffer.clear();
                                        } else if self.editor_state.mode == EditorMode::Command {
                                            if commands::execute_command(self, stdout).await? {
                                                return Ok(());
                                            }
                                            self.editor_state.mode = EditorMode::Normal;
                                            self.editor_state.command_buffer.clear();
                                            self.editor_state.visual_start = None;
                                            self.editor_state.visual_end = None;
                                        }
                                    }
                                    crossterm::event::KeyCode::Backspace => {
                                        self.editor_state.command_buffer.pop();
                                    }
                                    crossterm::event::KeyCode::Char(c) => {
                                        self.editor_state.command_buffer.push(c);
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    crossterm::event::Event::Resize(w, h) => {
                        self.width = w as usize;
                        self.height = h as usize;
                    }
                    _ => {}
                }
            } else {
                break;
            }

            crate::progress::save_progress(self.document_hash, self.offset, self.total_lines)?;
        }

        Ok(())
    }
}
