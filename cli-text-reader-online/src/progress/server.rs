use std::sync::Arc;
use crate::server::HyggClient;
use super::events::Event;
use super::local::{read_events_from_file, write_events_to_file};
use chrono::Utc;

pub async fn add_highlight_to_server(
    document_hash: u64, 
    line_number: usize,
    file_path: &str,
    client: Arc<HyggClient>
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Adding highlight for line {} via server API", line_number);
    
    // First check if this line is already highlighted to avoid server errors
    log::info!("Checking if line {} is already highlighted in document {}", line_number, document_hash);
    match client.get_highlights(&document_hash.to_string()).await {
        Ok(existing_highlights) => {
            // Check if the line is already highlighted
            let already_exists = existing_highlights.iter().any(|h| h.line_number == line_number);
            
            if already_exists {
                log::info!("Line {} is already highlighted on the server for document {}, skipping", line_number, document_hash);
                return Ok(());
            }
        },
        Err(e) => {
            // If we can't check existing highlights, log the error but continue with the add attempt
            log::warn!("Failed to check existing highlights: {}", e);
        }
    }
    
    // Try to add the highlight
    match client.add_highlight(file_path, &document_hash.to_string(), line_number).await {
        Ok(_) => {
            log::info!("Successfully added highlight for line {} in document {}", line_number, document_hash);
            return Ok(());
        },
        Err(e) => {
            // Handle common error cases
            let error_msg = e.to_string();
            if error_msg.contains("decoding response body") || error_msg.contains("expected value") {
                // This is likely a duplicate highlight that the server rejected
                log::info!("Server rejected highlight addition, likely because it already exists: {}", error_msg);
                return Ok(());  // Return success to avoid crashing the application
            }
            // For other errors, propagate them
            return Err(e.into());
        }
    }
}

pub async fn remove_highlight_from_server(
    document_hash: u64,
    line_number: usize,
    file_path: &str,
    client: Arc<HyggClient>
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Removing highlight for line {} via server API", line_number);
    
    // First check if this line is actually highlighted on the server
    log::info!("Checking if line {} is highlighted in document {}", line_number, document_hash);
    match client.get_highlights(&document_hash.to_string()).await {
        Ok(existing_highlights) => {
            // Check if the line is highlighted
            let highlight_exists = existing_highlights.iter().any(|h| h.line_number == line_number);
            
            if !highlight_exists {
                log::info!("Line {} is not highlighted on the server for document {}, nothing to remove", line_number, document_hash);
                return Ok(());
            }
            
            // Line is highlighted, proceed with removal
            match client.remove_highlight(file_path, &document_hash.to_string(), line_number).await {
                Ok(_) => {
                    log::info!("Successfully removed highlight for line {} in document {}", line_number, document_hash);
                    return Ok(());
                },
                Err(e) => {
                    // Handle server errors
                    log::error!("Server error removing highlight: {}", e);
                    return Err(e.into());
                }
            }
        },
        Err(e) => {
            // If we can't check existing highlights, log the error but continue with local tracking
            log::warn!("Failed to check existing highlights: {}", e);
            // Still record the removal event locally
            let event = Event::RemoveHighlight {
                timestamp: Utc::now(),
                document_hash,
                line_number,
            };
            
            let mut events = read_events_from_file().unwrap_or_default();
            events.push(event);
            write_events_to_file(&events)?;
            
            return Ok(());
        }
    }
}

pub async fn clear_highlights_on_server(
    document_hash: u64,
    client: Arc<HyggClient>
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Clearing all highlights via server API");
    
    // Try to clear highlights on the server
    match client.clear_highlights(&document_hash.to_string()).await {
        Ok(_) => {
            log::info!("Successfully cleared all highlights for document {}", document_hash);
            return Ok(());
        },
        Err(e) => {
            // Handle server errors
            log::error!("Server error clearing highlights: {}", e);
            return Err(e.into());
        }
    }
}

pub async fn load_highlights_from_server(
    document_hash: u64,
    client: Arc<HyggClient>
) -> Result<Vec<usize>, Box<dyn std::error::Error>> {
    log::info!("Loading highlights from server for document {}", document_hash);
    
    // Try to get highlights from the server
    match client.get_highlights(&document_hash.to_string()).await {
        Ok(server_highlights) => {
            // Convert server highlights to line numbers
            let highlights: Vec<usize> = server_highlights.iter()
                .map(|h| h.line_number)
                .collect();
            
            log::info!("Successfully loaded {} highlights from server for document {}", 
                      highlights.len(), document_hash);
            return Ok(highlights);
        },
        Err(e) => {
            // Handle server errors
            log::error!("Server error getting highlights: {}", e);
            return Err(e.into());
        }
    }
}

pub async fn undo_last_highlight_on_server(
    document_hash: u64,
    client: Arc<HyggClient>
) -> Result<bool, Box<dyn std::error::Error>> {
    println!("Undoing last highlight action via server API");
    
    // Get current server highlights
    let current_highlights = match client.get_highlights(&document_hash.to_string()).await {
        Ok(highlights) => highlights,
        Err(e) => {
            log::error!("Failed to get current highlights from server: {}", e);
            return Err(e.into());
        }
    };
    
    // If there are no highlights, nothing to undo
    if current_highlights.is_empty() {
        return Ok(false);
    }
    
    // Check if there are valid highlights with timestamps
    // Note: This check is kept for validation even though we use the client's undo_last_highlight method
    match current_highlights.iter().max_by_key(|h| h.created_at) {
        Some(_) => { /* At least one highlight exists with a timestamp */ },
        None => {
            log::info!("No highlights with valid timestamps found");
            return Ok(false);
        }
    };
    
    // Use client's undo_last_highlight method directly instead of remove_highlight
    // This properly handles the server-side undo endpoint
    match client.undo_last_highlight(&document_hash.to_string()).await {
        Ok(undone) => {
            if undone {
                log::info!("Successfully undid last highlight in document {}", document_hash);
            } else {
                log::info!("Nothing to undo for document {}", document_hash);
            }
            return Ok(undone);
        },
        Err(e) => {
            log::error!("Server error undoing highlight: {}", e);
            return Err(e.into());
        }
    }
}
