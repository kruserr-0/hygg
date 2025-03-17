use crossterm::{
    cursor::MoveTo,
    execute,
    style::{Color, SetForegroundColor, ResetColor},
};
use std::io::{self, Write};
use crate::progress::{
    add_highlight_async, remove_highlight_async, 
    clear_highlights_async, export_highlights_async,
    load_highlights_async
};
use crate::progress_compat::{undo_last_highlight, undo_last_highlight_async};
use super::{Editor, EditorMode};
use std::path::PathBuf;

pub async fn execute_command(
    editor: &mut Editor,
    stdout: &mut io::Stdout,
) -> Result<bool, Box<dyn std::error::Error>> {
    match editor.editor_state.command_buffer.trim() {
        "p" => {
            editor.show_progress = !editor.show_progress;
            if editor.show_progress {
                println!("Progress display enabled");
                editor.progress_display_until = Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
            } else {
                println!("Progress display disabled");
                editor.progress_display_until = None;
            }
            Ok(false)
        },
        "h" => {
            editor.show_highlighter = !editor.show_highlighter;
            if editor.show_highlighter {
                println!("Highlighter enabled");
            } else {
                println!("Highlighter disabled");
            }
            // Load highlights
            editor.highlights = match load_highlights_async(editor.document_hash, editor.client.clone()).await {
                Ok(highlights) => highlights,
                Err(e) => {
                    eprintln!("Error loading highlights: {}", e);
                    Vec::new()
                }
            };
            Ok(false)
        },
        "q" | "quit" | "exit" => {
            // Save position before exiting
            if let Some(ref callback) = editor.progress_callback {
                callback(editor.offset);
            }
            
            // Clean up and exit
            execute!(
                stdout,
                MoveTo(0, editor.height as u16),
                SetForegroundColor(Color::White),
                ResetColor
            )?;
            println!("Exiting...");
            Ok(true)
        },
        "help" => {
            execute!(
                stdout,
                MoveTo(0, editor.height as u16 - 10),
                SetForegroundColor(Color::White)
            )?;
            println!("Commands:");
            println!("  h - Toggle highlighting mode");
            println!("  p - Toggle progress display");
            println!("  q/quit/exit - Exit the reader");
            println!("  clear - Clear all highlights");
            println!("  export - Export highlights to file");
            println!("  o [line] - Jump to line number");
            println!("  / - Search forward");
            println!("  ? - Search backward");
            println!("  v - Visual mode for selecting text (use j/k to select lines)");
            Ok(false)
        },
        "clear" => {
            // Clear all highlights
            match clear_highlights_async(editor.document_hash, editor.client.clone()).await {
                Ok(_) => {
                    // Update the highlights array
                    editor.highlights.clear();
                    println!("All highlights cleared");
                },
                Err(e) => {
                    eprintln!("Error clearing highlights: {}", e);
                }
            }
            Ok(false)
        },
        "export" => {
            // Export highlights to a file
            match export_highlights_async(editor.document_hash, &editor.lines, editor.client.clone()).await {
                Ok(path_str) => {
                    println!("Highlights exported to: {}", path_str);
                },
                Err(e) => {
                    eprintln!("Error exporting highlights: {}", e);
                }
            }
            Ok(false)
        },
        "u" | "undo" => {
            // Undo last highlight
            match undo_last_highlight(editor.document_hash, &editor.file_path) {
                Ok(true) => {
                    println!("Undid last highlight action");
                    // Reload highlights
                    editor.highlights = match load_highlights_async(editor.document_hash, editor.client.clone()).await {
                        Ok(highlights) => highlights,
                        Err(e) => {
                            eprintln!("Error loading highlights: {}", e);
                            Vec::new()
                        }
                    };
                },
                Ok(false) => {
                    println!("Nothing to undo");
                },
                Err(e) => {
                    eprintln!("Error undoing highlight: {}", e);
                }
            }
            Ok(false)
        },
        cmd if cmd.starts_with("o ") => {
            // Go to line number
            if let Some(line_str) = cmd.strip_prefix("o ") {
                if let Ok(line) = line_str.trim().parse::<usize>() {
                    if line > 0 && line <= editor.lines.len() {
                        editor.offset = line - 1;
                        println!("Jumping to line {}", line);
                    } else {
                        println!("Line number out of range (1-{})", editor.lines.len());
                    }
                } else {
                    println!("Invalid line number");
                }
            }
            Ok(false)
        },
        _ => {
            if !handle_command(&editor.editor_state.command_buffer, &mut editor.show_highlighter) {
                println!("Unknown command: {}", editor.editor_state.command_buffer);
            }
            Ok(false)
        }
    }
}

pub fn handle_command(command: &str, show_highlighter: &mut bool) -> bool {
    match command {
        "highlight on" | "h on" => {
            *show_highlighter = true;
            println!("Highlighting enabled");
            true
        },
        "highlight off" | "h off" => {
            *show_highlighter = false;
            println!("Highlighting disabled");
            true
        },
        _ => false,
    }
}
