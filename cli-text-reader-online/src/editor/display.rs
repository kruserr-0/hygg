use crossterm::{
    cursor::MoveTo,
    execute,
    style::{Color, ResetColor, SetBackgroundColor, SetForegroundColor},
    terminal::{Clear, ClearType},
};
use std::io::{self, Write};

use super::{Editor, EditorMode};

pub fn render_editor(
    editor: &Editor,
    stdout: &mut io::Stdout,
) -> Result<(), Box<dyn std::error::Error>> {
    let center = true;
    let term_width = crossterm::terminal::size()?.0 as u16;
    let center_offset =
        if editor.width > editor.col { (editor.width / 2) - editor.col / 2 } else { 0 };
    let center_offset_string =
        if center { " ".repeat(center_offset) } else { "".to_string() };

    for (i, line_orig) in
        editor.lines.iter().skip(editor.offset).take(editor.height).enumerate()
    {
        let line = line_orig.clone();
        let absolute_line_number = editor.offset + i;
        execute!(stdout, MoveTo(0, i as u16))?;
        
        // First, always clear the background for this line to avoid any color bleeding
        execute!(stdout, ResetColor)?;
        
        // Check if line is in visual selection - only if we're actually in Visual mode
        let is_visual_selected = if editor.editor_state.mode == EditorMode::Visual {
            if let (Some(start), Some(end)) = (editor.editor_state.visual_start, editor.editor_state.visual_end) {
                let (min, max) = if start <= end { (start, end) } else { (end, start) };
                absolute_line_number >= min && absolute_line_number <= max
            } else {
                false
            }
        } else {
            false
        };
        
        // Check if this is a highlighted line
        let is_highlighted = editor.highlights.contains(&absolute_line_number);

        // Apply visual selection highlight
        if editor.editor_state.mode == EditorMode::Visual && is_visual_selected {
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
        else if editor.show_highlighter && i == editor.height / 2 {
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
        if let Some((line_idx, start, end)) = editor.editor_state.current_match {
            if line_idx == editor.offset + i {
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

    // Display status line
    match editor.editor_state.mode {
        EditorMode::Command => {
            execute!(stdout, MoveTo(0, (editor.height - 1) as u16))?;
            print!(":{}", editor.editor_state.command_buffer);
        },
        EditorMode::Search => {
            execute!(stdout, MoveTo(0, (editor.height - 1) as u16))?;
            print!("/{}", editor.editor_state.command_buffer);
        },
        EditorMode::ReverseSearch => {
            execute!(stdout, MoveTo(0, (editor.height - 1) as u16))?;
            print!("?{}", editor.editor_state.command_buffer);
        },
        EditorMode::Visual => {
            // Show visual mode status with selection info
            execute!(stdout, MoveTo(0, (editor.height - 1) as u16))?;
            if let (Some(start), Some(end)) = (editor.editor_state.visual_start, editor.editor_state.visual_end) {
                let (min, max) = if start <= end { (start, end) } else { (end, start) };
                print!("VISUAL MODE ({} lines selected)", max - min + 1);
            } else {
                print!("VISUAL MODE");
            }
        },
        EditorMode::Normal => {
            // In normal mode, show progress percentage if enabled
            execute!(stdout, MoveTo(0, (editor.height - 1) as u16))?;
            if editor.show_progress {
                let percentage = (editor.offset as f64 / editor.total_lines as f64) * 100.0;
                let middle_line = editor.offset + editor.height / 2;
                let middle_percentage = (middle_line as f64 / editor.total_lines as f64) * 100.0;
                print!("Line: {} / {} ({:.1}%) | Current: {} ({:.1}%)", 
                       editor.offset + 1, editor.total_lines, percentage,
                       middle_line + 1, middle_percentage);
            }
        }
    }
    
    stdout.flush()?;
    Ok(())
}
