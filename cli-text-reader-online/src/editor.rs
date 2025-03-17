use crate::progress::undo_last_highlight;
use crossterm::{
  cursor::{Hide, MoveTo, Show},
  event::{self, Event as CEvent, KeyCode},
  execute,
  style::{Color, ResetColor, SetBackgroundColor, SetForegroundColor},
  terminal::{self, Clear, ClearType},
};
use std::io::{self, IsTerminal, Write};
use std::sync::Arc;

use crate::config::load_config;
use crate::progress::{generate_hash, load_progress, save_progress, 
                      load_highlights, add_highlight, remove_highlight, 
                      clear_highlights, export_highlights};
use crate::server::HyggClient;
use crate::tutorial::get_tutorial_text;

#[derive(PartialEq)]
pub enum EditorMode {
  Normal,
  Command,
  Search,
  ReverseSearch,
  Visual,
}

pub struct EditorState {
  pub mode: EditorMode,
  pub command_buffer: String,
  pub search_query: String,
  pub search_direction: bool, // true for forward, false for backward
  pub last_search_index: Option<usize>,
  pub current_match: Option<(usize, usize, usize)>, // (line_index, start, end)
  pub visual_start: Option<usize>, // Starting line for visual mode selection
  pub visual_end: Option<usize>,   // Ending line for visual mode selection
}

impl EditorState {
  pub fn new() -> Self {
    Self {
      mode: EditorMode::Normal,
      command_buffer: String::new(),
      search_query: String::new(),
      search_direction: true,
      last_search_index: None,
      current_match: None,
      visual_start: None,
      visual_end: None,
    }
  }
}

pub struct Editor {
    lines: Vec<String>,
    col: usize,
    offset: usize,
    width: usize,
    height: usize,
    show_highlighter: bool,
    editor_state: EditorState,
    highlights: Vec<usize>, // Store highlighted line numbers
    document_hash: u64,
    total_lines: usize,
    progress_display_until: Option<std::time::Instant>,
    show_progress: bool,
    progress_callback: Option<Box<dyn Fn(usize) + Send>>,
    read_only: bool,
    client: Option<Arc<HyggClient>>,
    file_path: String,
}

impl Editor {
    pub fn new(lines: Vec<String>, col: usize, file_path: String, client: Option<Arc<HyggClient>>) -> Self {
        let document_hash = generate_hash(&lines);
        let total_lines = lines.len();
        let (width, height) = terminal::size()
            .map(|(w, h)| (w as usize, h as usize))
            .unwrap_or((80, 24));
            
        // Try to load highlights
        let highlights = load_highlights(document_hash, client.clone()).unwrap_or_default();

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

    pub fn run_with_progress<F>(&mut self, callback: F) -> Result<(), Box<dyn std::error::Error>>
    where
        F: Fn(usize) + Send + 'static,
    {
        self.progress_callback = Some(Box::new(callback));
        self.run()
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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

        self.main_loop(&mut stdout)?;

        self.cleanup(&mut stdout)?;
        Ok(())
  }

  pub fn show_tutorial(
    &self,
    stdout: &mut io::Stdout,
  ) -> Result<(), Box<dyn std::error::Error>> {
    let tutorial_lines = get_tutorial_text();

    if std::io::stdout().is_terminal() {
      // Save current state
      // let was_alternate = terminal::is_alternate_screen_active()?;
      let was_raw = terminal::is_raw_mode_enabled()?;

      // Setup tutorial display
      // if !was_alternate {
      //     execute!(stdout, terminal::EnterAlternateScreen)?;
      // }
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
        match event::read()? {
          CEvent::Key(key_event) => match key_event.code {
            KeyCode::Char('j') | KeyCode::Down => {
              if tutorial_offset + self.height < tutorial_lines.len() {
                tutorial_offset += 1;
              }
            }
            KeyCode::Char('k') | KeyCode::Up => {
              if tutorial_offset > 0 {
                tutorial_offset -= 1;
              }
            }
            KeyCode::PageDown => {
              tutorial_offset = (tutorial_offset + self.height)
                .min(tutorial_lines.len().saturating_sub(self.height));
            }
            KeyCode::PageUp => {
              tutorial_offset = tutorial_offset.saturating_sub(self.height);
            }
            _ => break,
          },
          _ => {}
        }
      }

      // Restore original state
      execute!(stdout, Clear(ClearType::All))?;
      // if !was_alternate {
      //     execute!(stdout, terminal::LeaveAlternateScreen)?;
      // }
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

    fn main_loop(
        &mut self,
        stdout: &mut io::Stdout,
    ) -> Result<(), Box<dyn std::error::Error>> {
    loop {
      if std::io::stdout().is_terminal() {
        execute!(stdout, MoveTo(0, 0), Clear(ClearType::All))?;
      }

      let center = true;
      let term_width = terminal::size()?.0 as u16;
      let center_offset =
        if self.width > self.col { (self.width / 2) - self.col / 2 } else { 0 };
      let center_offset_string =
        if center { " ".repeat(center_offset) } else { "".to_string() };

      for (i, line_orig) in
        self.lines.iter().skip(self.offset).take(self.height).enumerate()
      {
        let line = line_orig.clone();
        let absolute_line_number = self.offset + i;
        execute!(stdout, MoveTo(0, i as u16))?;
        
        // First, always clear the background for this line to avoid any color bleeding
        execute!(stdout, ResetColor)?;
        
        // Check if line is in visual selection - only if we're actually in Visual mode
        let is_visual_selected = if self.editor_state.mode == EditorMode::Visual {
          if let (Some(start), Some(end)) = (self.editor_state.visual_start, self.editor_state.visual_end) {
            let (min, max) = if start <= end { (start, end) } else { (end, start) };
            absolute_line_number >= min && absolute_line_number <= max
          } else {
            false
          }
        } else {
          false
        };
        
        // Check if this is a highlighted line
        let is_highlighted = self.highlights.contains(&absolute_line_number);

        // Apply visual selection highlight
        if self.editor_state.mode == EditorMode::Visual && is_visual_selected {
          // Clear line first to ensure clean rendering
          execute!(stdout, SetBackgroundColor(Color::Reset))?;
          print!("{}", " ".repeat(term_width as usize));
          execute!(stdout, MoveTo(0, i as u16))?;
          
          // Now apply the visual selection highlight
          execute!(
            stdout,
            SetBackgroundColor(Color::Rgb { r: 50, g: 50, b: 100 })
          )?;
          print!("{}", " ".repeat(term_width as usize));
          execute!(stdout, MoveTo(0, i as u16))?;
        }
        // Apply permanent highlight (yellow background)
        else if is_highlighted {
          // Clear line first to ensure clean rendering
          execute!(stdout, SetBackgroundColor(Color::Reset))?;
          print!("{}", " ".repeat(term_width as usize));
          execute!(stdout, MoveTo(0, i as u16))?;
          
          // Now apply the highlight
          execute!(
            stdout,
            SetBackgroundColor(Color::Rgb { r: 100, g: 100, b: 0 })
          )?;
          print!("{}", " ".repeat(term_width as usize));
          execute!(stdout, MoveTo(0, i as u16))?;
        }
        // Current line indicator
        else if self.show_highlighter && i == self.height / 2 {
          // Clear line first to ensure clean rendering
          execute!(stdout, SetBackgroundColor(Color::Reset))?;
          print!("{}", " ".repeat(term_width as usize));
          execute!(stdout, MoveTo(0, i as u16))?;
          
          // Now apply the current line highlight
          execute!(
            stdout,
            SetBackgroundColor(Color::Rgb { r: 40, g: 40, b: 40 })
          )?;
          print!("{}", " ".repeat(term_width as usize));
          execute!(stdout, MoveTo(0, i as u16))?;
        }

        // Handle search highlight
        if let Some((line_idx, start, end)) = self.editor_state.current_match {
          if line_idx == self.offset + i {
            print!("{}", center_offset_string);
            print!("{}", &line[..start]);
            execute!(
              stdout,
              SetBackgroundColor(Color::Yellow),
              SetForegroundColor(Color::Black)
            )?;
            print!("{}", &line[start..end]);
            execute!(stdout, ResetColor)?;
            println!("{}", &line[end..]);
            continue;
          }
        }

        println!("{}{}", center_offset_string, line);
        
        // Always reset ALL colors (foreground and background) after printing each line
        // This is critical to prevent color bleeding when scrolling
        execute!(stdout, ResetColor)?;
      }

      if self.editor_state.mode == EditorMode::Command {
        execute!(stdout, MoveTo(0, (self.height - 1) as u16))?;
        print!(":{}", self.editor_state.command_buffer);
      } else if self.editor_state.mode == EditorMode::Search {
        execute!(stdout, MoveTo(0, (self.height - 1) as u16))?;
        print!("/{}", self.editor_state.command_buffer);
      } else if self.editor_state.mode == EditorMode::ReverseSearch {
        execute!(stdout, MoveTo(0, (self.height - 1) as u16))?;
        print!("?{}", self.editor_state.command_buffer);
      } else if self.editor_state.mode == EditorMode::Visual {
        // Show visual mode status with selection info
        execute!(stdout, MoveTo(0, (self.height - 1) as u16))?;
        if let (Some(start), Some(end)) = (self.editor_state.visual_start, self.editor_state.visual_end) {
          let (min, max) = if start <= end { (start, end) } else { (end, start) };
          let line_count = max - min + 1;
          print!("-- VISUAL LINE -- {} line{} selected", line_count, if line_count == 1 { "" } else { "s" });
        } else {
          print!("-- VISUAL LINE --");
        }
      }

      // Show highlight count in normal mode
      if self.editor_state.mode == EditorMode::Normal && !self.highlights.is_empty() {
        execute!(stdout, MoveTo(0, (self.height - 2) as u16))?;
        print!("Highlights: {} lines | Press V for Visual mode, :hl to highlight, :rm-hl to remove, :export-hl to export", self.highlights.len());
      }
      
      // Show progress if enabled
      if self.show_progress {
        let progress =
          (self.offset as f64 / self.total_lines as f64 * 100.0).round();
        let message = format!("{}%", progress);
        let x = self.width as u16 - message.len() as u16 - 2;
        let y = self.height as u16 - 2;
        execute!(stdout, MoveTo(x, y))?;
        print!("{}", message);
      }
      
      // Show read-only indicator if in read-only mode
      if self.read_only {
        let message = "READ-ONLY";
        let x = 2;
        let y = self.height as u16 - 2;
        execute!(stdout, MoveTo(x, y), SetForegroundColor(Color::Yellow))?;
        print!("{}", message);
        execute!(stdout, ResetColor)?;
      }

      stdout.flush()?;

      if std::io::stdout().is_terminal() {
        match event::read()? {
          CEvent::Key(key_event) => match self.editor_state.mode {
            EditorMode::Normal => match key_event.code {
              KeyCode::Char(':') => {
                self.editor_state.mode = EditorMode::Command;
                self.editor_state.command_buffer.clear();
              }
              KeyCode::Char('/') => {
                self.editor_state.mode = EditorMode::Search;
                self.editor_state.command_buffer.clear();
                self.editor_state.search_direction = true;
              }
              KeyCode::Char('?') => {
                self.editor_state.mode = EditorMode::ReverseSearch;
                self.editor_state.command_buffer.clear();
                self.editor_state.search_direction = false;
              }
              KeyCode::Char('V') => {
                // Enter Visual Line mode (like Vim's SHIFT+V)
                self.editor_state.mode = EditorMode::Visual;
                // Start selection at current line
                let current_line = self.offset + self.height / 2;
                self.editor_state.visual_start = Some(current_line);
                self.editor_state.visual_end = Some(current_line);
              }
              KeyCode::Char('n') => {
                if !self.editor_state.search_query.is_empty() {
                  // Use the original search direction
                  self.find_next_match(self.editor_state.search_direction);
                  self.center_on_match();
                }
              }
              KeyCode::Char('N') => {
                if !self.editor_state.search_query.is_empty() {
                  // Use opposite of original search direction
                  self.find_next_match(!self.editor_state.search_direction);
                  self.center_on_match();
                }
              }
              KeyCode::Char('j') | KeyCode::Down => {
                if self.offset + self.height < self.total_lines {
                  self.offset += 1;
                  
                  // If we have a progress callback and we're not in read-only mode, call it
                  if let Some(callback) = &self.progress_callback {
                    if !self.read_only {
                      callback(self.offset);
                    }
                  }
                }
              }
              KeyCode::Char('k') | KeyCode::Up => {
                if self.offset > 0 {
                  self.offset -= 1;
                  
                  // If we have a progress callback and we're not in read-only mode, call it
                  if let Some(callback) = &self.progress_callback {
                    if !self.read_only {
                      callback(self.offset);
                    }
                  }
                }
              }
              KeyCode::PageDown => {
                if self.offset + self.height < self.total_lines {
                  self.offset += self.height - 3;
                  
                  // If we have a progress callback and we're not in read-only mode, call it
                  if let Some(callback) = &self.progress_callback {
                    if !self.read_only {
                      callback(self.offset);
                    }
                  }
                }
              }
              KeyCode::PageUp => {
                if self.offset as i32 - self.height as i32 > 0 {
                  self.offset -= self.height - 3;
                } else {
                  self.offset = 0;
                }
                
                // If we have a progress callback and we're not in read-only mode, call it
                if let Some(callback) = &self.progress_callback {
                  if !self.read_only {
                    callback(self.offset);
                  }
                }
              }
              _ => {}
            },
            EditorMode::Search | EditorMode::ReverseSearch => {
              match key_event.code {
                KeyCode::Esc => {
                  self.editor_state.mode = EditorMode::Normal;
                  self.editor_state.command_buffer.clear();
                }
                KeyCode::Enter => {
                  self.editor_state.search_query =
                    self.editor_state.command_buffer.clone();
                  // Start from current position
                  self.find_next_match(
                    self.editor_state.mode == EditorMode::Search,
                  );
                  self.center_on_match();
                  self.editor_state.mode = EditorMode::Normal;
                  self.editor_state.command_buffer.clear();
                }
                KeyCode::Backspace => {
                  self.editor_state.command_buffer.pop();
                }
                KeyCode::Char(c) => {
                  self.editor_state.command_buffer.push(c);
                }
                _ => {}
              }
            }
            EditorMode::Visual => match key_event.code {
              KeyCode::Esc => {
                // Exit Visual mode
                self.editor_state.mode = EditorMode::Normal;
                self.editor_state.visual_start = None;
                self.editor_state.visual_end = None;
              }
              KeyCode::Char(':') => {
                // Enter command mode from visual mode
                self.editor_state.mode = EditorMode::Command;
                self.editor_state.command_buffer.clear();
              }
              KeyCode::Char('j') | KeyCode::Down => {
                // Move visual selection down
                if let Some(end) = self.editor_state.visual_end {
                  if end < self.total_lines - 1 {
                    self.editor_state.visual_end = Some(end + 1);
                    
                    // Ensure the selection is visible
                    if end + 1 >= self.offset + self.height {
                      self.offset += 1;
                    }
                  }
                }
              }
              KeyCode::Char('k') | KeyCode::Up => {
                // Move visual selection up
                if let Some(end) = self.editor_state.visual_end {
                  if end > 0 {
                    self.editor_state.visual_end = Some(end - 1);
                    
                    // Ensure the selection is visible
                    if end - 1 < self.offset {
                      self.offset -= 1;
                    }
                  }
                }
              }
              _ => {}
            },
            EditorMode::Command => match key_event.code {
              KeyCode::Esc => {
                self.editor_state.mode = EditorMode::Normal;
                self.editor_state.command_buffer.clear();
                self.editor_state.visual_start = None;
                self.editor_state.visual_end = None;
              }
              KeyCode::Enter => {
                if self.execute_command(stdout)? {
                  return Ok(());
                }
                self.editor_state.mode = EditorMode::Normal;
                self.editor_state.command_buffer.clear();
                self.editor_state.visual_start = None;
                self.editor_state.visual_end = None;
              }
              KeyCode::Backspace => {
                self.editor_state.command_buffer.pop();
              }
              KeyCode::Char(c) => {
                self.editor_state.command_buffer.push(c);
              }
              _ => {}
            },
          },
          CEvent::Resize(w, h) => {
            self.width = w as usize;
            self.height = h as usize;
          }
          _ => {}
        }
      } else {
        break;
      }

      save_progress(self.document_hash, self.offset, self.total_lines)?;
    }

    Ok(())
  }

  fn execute_command(
    &mut self,
    stdout: &mut io::Stdout,
  ) -> Result<bool, Box<dyn std::error::Error>> {
    match self.editor_state.command_buffer.trim() {
      "p" => {
        self.show_progress = !self.show_progress;
        self.editor_state.mode = EditorMode::Normal;
        self.editor_state.command_buffer.clear();
        Ok(false)
      }
      "help" | "tutorial" => {
        self.show_tutorial(stdout)?;
        self.editor_state.mode = EditorMode::Normal;
        self.editor_state.command_buffer.clear();
        Ok(false)
      }
      "hl" => {
        // Highlight the selected line(s) in visual mode
        if let (Some(start), Some(end)) = (self.editor_state.visual_start, self.editor_state.visual_end) {
          let (min, max) = if start <= end { (start, end) } else { (end, start) };
          
          // Add highlights for all lines in the selection
          for line_num in min..=max {
            // Check if the line is already highlighted
            if !self.highlights.contains(&line_num) {
              // Add to the local highlights list
              self.highlights.push(line_num);
              // Save to server database or local storage
              add_highlight(self.document_hash, line_num, &self.file_path, self.client.clone())?;
            }
          }
          
          println!("Added highlights to {} line(s)", max - min + 1);
        } else {
          // Highlight current line if not in visual mode
          let current_line = self.offset + self.height / 2;
          if !self.highlights.contains(&current_line) {
            self.highlights.push(current_line);
            add_highlight(self.document_hash, current_line, &self.file_path, self.client.clone())?;
          }
        }
        
        self.editor_state.mode = EditorMode::Normal;
        self.editor_state.visual_start = None;
        self.editor_state.visual_end = None;
        self.editor_state.command_buffer.clear();
        Ok(false)
      }
      "rm-hl" => {
        // Remove highlights from selected lines
        if let (Some(start), Some(end)) = (self.editor_state.visual_start, self.editor_state.visual_end) {
          let (min, max) = if start <= end { (start, end) } else { (end, start) };
          
          // Remove highlights for all lines in the selection
          for line_num in min..=max {
            if self.highlights.contains(&line_num) {
              // Remove from local highlights
              self.highlights.retain(|&l| l != line_num);
              // Save to server database or local storage
              remove_highlight(self.document_hash, line_num, &self.file_path, self.client.clone())?;
            }
          }
        } else {
          // Remove highlight from current line if not in visual mode
          let current_line = self.offset + self.height / 2;
          if self.highlights.contains(&current_line) {
            self.highlights.retain(|&l| l != current_line);
            remove_highlight(self.document_hash, current_line, &self.file_path, self.client.clone())?;
          }
        }
        
        self.editor_state.mode = EditorMode::Normal;
        self.editor_state.visual_start = None;
        self.editor_state.visual_end = None;
        self.editor_state.command_buffer.clear();
        Ok(false)
      }
      "clear-hl" => {
        // Clear all highlights
        self.highlights.clear();
        clear_highlights(self.document_hash, self.client.clone())?;
        
        self.editor_state.mode = EditorMode::Normal;
        self.editor_state.visual_start = None;
        self.editor_state.visual_end = None;
        self.editor_state.command_buffer.clear();
        Ok(false)
      }
      "export-hl" => {
        // Export all highlights to a file
        let highlights_text = export_highlights(self.document_hash, &self.lines, self.client.clone())?;
        
        // Create highlights directory if it doesn't exist
        let mut highlights_dir = dirs::home_dir().unwrap_or_default();
        highlights_dir.push(".hygg");
        highlights_dir.push("highlights");
        std::fs::create_dir_all(&highlights_dir)?;
        
        // Generate filename based on document hash
        let filename = format!("{}.txt", self.document_hash);
        highlights_dir.push(filename);
        
        // Write to file
        std::fs::write(&highlights_dir, highlights_text)?;
        
        // Show confirmation message
        execute!(stdout, MoveTo(0, (self.height - 2) as u16), SetForegroundColor(Color::Green))?;
        print!("Highlights exported to: {}", highlights_dir.display());
        execute!(stdout, ResetColor)?;
        
        self.editor_state.mode = EditorMode::Normal;
        self.editor_state.command_buffer.clear();
        Ok(false)
      }
      "hlu" => {
        // Undo the last highlight action
        match undo_last_highlight(self.document_hash, &self.file_path, self.client.clone())? {
          true => {
            // Reload highlights after the undo
            self.highlights = load_highlights(self.document_hash, self.client.clone())?;
            
            // Show confirmation message
            execute!(stdout, MoveTo(0, (self.height - 2) as u16), SetForegroundColor(Color::Green))?;
            print!("Last highlight action undone");
            execute!(stdout, ResetColor)?;
          },
          false => {
            // Show message that there was nothing to undo
            execute!(stdout, MoveTo(0, (self.height - 2) as u16), SetForegroundColor(Color::Yellow))?;
            print!("No highlight actions to undo");
            execute!(stdout, ResetColor)?;
          }
        }
        
        self.editor_state.mode = EditorMode::Normal;
        self.editor_state.command_buffer.clear();
        Ok(false)
      }
      cmd => Ok(handle_command(cmd, &mut self.show_highlighter)),
    }
  }

  fn find_next_match(&mut self, forward: bool) {
    if self.editor_state.search_query.is_empty() {
      return;
    }

    let query = self.editor_state.search_query.to_lowercase();
    let start_idx = if let Some((idx, _, _)) = self.editor_state.current_match {
      idx
    } else {
      self.offset
    };

    let find_in_line = |line: &str, query: &str| -> Option<(usize, usize)> {
      line.to_lowercase().find(&query).map(|start| (start, start + query.len()))
    };

    if forward {
      // Forward search
      for i in start_idx + 1..self.lines.len() {
        if let Some((start, end)) = find_in_line(&self.lines[i], &query) {
          self.editor_state.current_match = Some((i, start, end));
          return;
        }
      }
      // Wrap around to beginning
      for i in 0..=start_idx {
        if let Some((start, end)) = find_in_line(&self.lines[i], &query) {
          self.editor_state.current_match = Some((i, start, end));
          return;
        }
      }
    } else {
      // Backward search
      for i in (0..start_idx).rev() {
        if let Some((start, end)) = find_in_line(&self.lines[i], &query) {
          self.editor_state.current_match = Some((i, start, end));
          return;
        }
      }
      // Wrap around to end
      for i in (start_idx..self.lines.len()).rev() {
        if let Some((start, end)) = find_in_line(&self.lines[i], &query) {
          self.editor_state.current_match = Some((i, start, end));
          return;
        }
      }
    }
  }

  fn center_on_match(&mut self) {
    if let Some((line_idx, _, _)) = self.editor_state.current_match {
      let half_height = (self.height / 2) as i32;
      let new_offset = line_idx as i32 - half_height;
      self.offset = if new_offset < 0 {
        0
      } else if new_offset + self.height as i32 > self.total_lines as i32 {
        self.total_lines - self.height
      } else {
        new_offset as usize
      };
    }
  }
}

pub fn handle_command(command: &str, show_highlighter: &mut bool) -> bool {
  match command.trim() {
    "q" => true,
    "z" => {
      *show_highlighter = !*show_highlighter;
      false
    }
    "p" | "help" | "tutorial" => false,
    _ => false,
  }
}
