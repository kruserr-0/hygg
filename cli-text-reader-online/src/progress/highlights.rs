use chrono::Utc;
use std::sync::Arc;
use crate::server::HyggClient;

use super::events::Event;
use super::local::{read_events_from_file, write_events_to_file};
use super::server;

/// Add a highlight for a specific line in a document
/// 
/// This function handles both local and server-side highlights.
/// For server-side highlights, you should use the `add_highlight_async` function instead.
pub fn add_highlight(
    document_hash: u64,
    line_number: usize,
    _file_path: &str,
    client: Option<Arc<HyggClient>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // For server-side highlights, notify the user to use the async version
    if client.is_some() {
        println!("Error: Cannot call add_highlight with client from a synchronous context");
        println!("Use add_highlight_async instead");
        return Err("Cannot use server-side highlights from a synchronous context. Use add_highlight_async instead".into());
    }

    // Fallback to local storage for offline mode
    handle_local_add_highlight(document_hash, line_number)
}

/// Async version of add_highlight for server-side highlights
pub async fn add_highlight_async(
    document_hash: u64,
    line_number: usize,
    file_path: &str,
    client: Option<Arc<HyggClient>>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("add_highlight_async called for document_hash={}, line={}", document_hash, line_number);
    
    // If we have a client, use the server API
    if let Some(client) = client.clone() {
        return server::add_highlight_to_server(document_hash, line_number, file_path, client).await;
    }
    
    // Fallback to local storage for offline mode
    handle_local_add_highlight(document_hash, line_number)
}

/// Helper function to handle local highlight addition
fn handle_local_add_highlight(document_hash: u64, line_number: usize) -> Result<(), Box<dyn std::error::Error>> {
    // First check if this line is already highlighted by loading existing highlights
    let highlights = handle_local_load_highlights(document_hash)?;
    
    if highlights.contains(&line_number) {
        return Ok(());  // Line is already highlighted
    }
    
    // Add the highlight event to the progress file
    let event = Event::AddHighlight {
        timestamp: Utc::now(),
        document_hash,
        line_number,
    };
    
    let mut events = read_events_from_file().unwrap_or_default();
    events.push(event);
    write_events_to_file(&events)?;
    
    Ok(())
}

/// Remove a highlight for a specific line in a document
/// 
/// This function handles both local and server-side highlights.
/// For server-side highlights, you should use the `remove_highlight_async` function instead.
pub fn remove_highlight(
    document_hash: u64,
    line_number: usize,
    _file_path: &str,
    client: Option<Arc<HyggClient>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // For server-side highlights, notify the user to use the async version
    if client.is_some() {
        println!("Error: Cannot call remove_highlight with client from a synchronous context");
        println!("Use remove_highlight_async instead");
        return Err("Cannot use server-side highlights from a synchronous context. Use remove_highlight_async instead".into());
    }

    // Fallback to local storage for offline mode
    handle_local_remove_highlight(document_hash, line_number)
}

/// Async version of remove_highlight for server-side highlights
pub async fn remove_highlight_async(
    document_hash: u64,
    line_number: usize,
    file_path: &str,
    client: Option<Arc<HyggClient>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // If we have a client, use the server API
    if let Some(client) = client.clone() {
        return server::remove_highlight_from_server(document_hash, line_number, file_path, client).await;
    }
    
    // Fallback to local storage for offline mode
    handle_local_remove_highlight(document_hash, line_number)
}

/// Helper function to handle local highlight removal
fn handle_local_remove_highlight(document_hash: u64, line_number: usize) -> Result<(), Box<dyn std::error::Error>> {
    // Add the remove highlight event to the progress file
    let event = Event::RemoveHighlight {
        timestamp: Utc::now(),
        document_hash,
        line_number,
    };
    
    let mut events = read_events_from_file().unwrap_or_default();
    events.push(event);
    write_events_to_file(&events)?;
    
    Ok(())
}

/// Clear all highlights for a document
/// 
/// This function handles both local and server-side highlights.
/// For server-side highlights, you should use the `clear_highlights_async` function instead.
pub fn clear_highlights(
    document_hash: u64,
    client: Option<Arc<HyggClient>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // For server-side highlights, notify the user to use the async version
    if client.is_some() {
        println!("Error: Cannot call clear_highlights with client from a synchronous context");
        println!("Use clear_highlights_async instead");
        return Err("Cannot use server-side highlights from a synchronous context. Use clear_highlights_async instead".into());
    }

    handle_local_clear_highlights(document_hash)
}

/// Async version of clear_highlights for server-side highlights
pub async fn clear_highlights_async(
    document_hash: u64,
    client: Option<Arc<HyggClient>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // If we have a client, use the server API
    if let Some(client) = client.clone() {
        return server::clear_highlights_on_server(document_hash, client).await;
    }
    
    // Fallback to local storage for offline mode
    handle_local_clear_highlights(document_hash)
}

/// Helper function to handle local highlight clearing
fn handle_local_clear_highlights(document_hash: u64) -> Result<(), Box<dyn std::error::Error>> {
    // Add the clear highlights event to the progress file
    let event = Event::ClearHighlights {
        timestamp: Utc::now(),
        document_hash,
    };
    
    let mut events = read_events_from_file().unwrap_or_default();
    events.push(event);
    write_events_to_file(&events)?;
    
    Ok(())
}

/// Load all highlighted lines for a document
/// 
/// This function handles both local and server-side highlights.
/// For server-side highlights, you should call this in an async context
/// or use the `load_highlights_async` function instead.
pub fn load_highlights(
    document_hash: u64,
    client: Option<Arc<HyggClient>>,
) -> Result<Vec<usize>, Box<dyn std::error::Error>> {
    // For server-side highlights, notify the user to use the async version
    if client.is_some() {
        log::warn!("Warning: load_highlights called with client from a synchronous context");
        log::warn!("For best results with a server connection, use load_highlights_async");
        // Continue with local fallback anyway, since we can still return local highlights
    }

    handle_local_load_highlights(document_hash)
}

/// Async version of load_highlights for server-side highlights
pub async fn load_highlights_async(
    document_hash: u64,
    client: Option<Arc<HyggClient>>,
) -> Result<Vec<usize>, Box<dyn std::error::Error>> {
    if let Some(client) = client.clone() {
        // Try to load from server first
        match server::load_highlights_from_server(document_hash, client).await {
            Ok(highlights) => {
                return Ok(highlights);
            },
            Err(e) => {
                log::warn!("Failed to load highlights from server: {}, falling back to local highlights", e);
                // Fall back to local highlights
            }
        }
    }
    
    // Fallback to local storage for offline mode
    handle_local_load_highlights(document_hash)
}

/// Helper function to handle local highlight loading logic
fn handle_local_load_highlights(document_hash: u64) -> Result<Vec<usize>, Box<dyn std::error::Error>> {
    let events = read_events_from_file().unwrap_or_default();
    
    // Process all events to determine the current state of highlights
    let mut highlights = Vec::new();
    
    for event in events {
        match event {
            Event::AddHighlight { document_hash: hash, line_number, .. } => {
                if hash == document_hash && !highlights.contains(&line_number) {
                    highlights.push(line_number);
                }
            },
            Event::RemoveHighlight { document_hash: hash, line_number, .. } => {
                if hash == document_hash {
                    highlights.retain(|&l| l != line_number);
                }
            },
            Event::ClearHighlights { document_hash: hash, .. } => {
                if hash == document_hash {
                    highlights.clear();
                }
            },
            _ => {} // Ignore other event types
        }
    }
    
    Ok(highlights)
}

/// Export all highlights for a document as text
/// 
/// This function handles both local and server-side highlights.
/// For server-side highlights, you should use the `export_highlights_async` function instead.
pub fn export_highlights(
    document_hash: u64,
    lines: &[String],
    client: Option<Arc<HyggClient>>,
) -> Result<String, Box<dyn std::error::Error>> {
    // Load all highlights
    let highlights = load_highlights(document_hash, client)?;
    
    // Format the result
    format_highlights_result(highlights, lines)
}

/// Async version of export_highlights for server-side highlights
pub async fn export_highlights_async(
    document_hash: u64,
    lines: &[String],
    client: Option<Arc<HyggClient>>,
) -> Result<String, Box<dyn std::error::Error>> {
    // Load all highlights (async version)
    let highlights = load_highlights_async(document_hash, client).await?;
    
    // Format the result
    format_highlights_result(highlights, lines)
}

/// Helper function to format highlights result
fn format_highlights_result(highlights: Vec<usize>, lines: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    let mut result = String::new();
    result.push_str("# Exported Highlights\n\n");
    
    for &line_num in &highlights {
        if line_num < lines.len() {
            result.push_str(&format!("Line {}: {}\n", line_num + 1, lines[line_num]));
        }
    }
    
    Ok(result)
}
