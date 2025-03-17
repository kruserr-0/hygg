use std::sync::Arc;
use tokio::runtime::Runtime;
use crate::progress::{
    add_highlight, add_highlight_async,
    remove_highlight, remove_highlight_async,
    undo_last_highlight, undo_last_highlight_async,
    clear_highlights, clear_highlights_async
};
use crate::server::HyggClient;

/// Test function to verify local highlighting functionality
#[tokio::test]
async fn test_local_highlighting() {
    // Initialize test values
    let document_hash = 12345u64;
    let line_number = 10;
    let file_path = "test_document.txt";
    
    // Test adding a highlight locally
    println!("Test: Adding a highlight locally");
    let result = add_highlight(document_hash, line_number, file_path, None);
    assert!(result.is_ok(), "Failed to add highlight: {:?}", result);
    
    // Test adding the same highlight again (should not crash)
    println!("Test: Adding the same highlight again");
    let result = add_highlight(document_hash, line_number, file_path, None);
    assert!(result.is_ok(), "Failed when adding same highlight: {:?}", result);
    
    // Test removing a highlight
    println!("Test: Removing a highlight");
    let result = remove_highlight(document_hash, line_number, file_path, None);
    assert!(result.is_ok(), "Failed to remove highlight: {:?}", result);
    
    // Test removing a non-existent highlight (should not crash)
    println!("Test: Removing a non-existent highlight");
    let result = remove_highlight(document_hash, line_number, file_path, None);
    assert!(result.is_ok(), "Failed when removing non-existent highlight: {:?}", result);
    
    // Test undoing the last highlight
    println!("Test: Adding a highlight for undo test");
    let _ = add_highlight(document_hash, line_number, file_path, None);
    
    println!("Test: Undoing the last highlight");
    let result = undo_last_highlight(document_hash, None);
    assert!(result.is_ok(), "Failed to undo last highlight: {:?}", result);
    
    // Test undoing when there's nothing to undo (should not crash)
    println!("Test: Undoing when there's nothing to undo");
    let result = undo_last_highlight(document_hash, None);
    assert!(result.is_ok(), "Failed when undoing with nothing to undo: {:?}", result);
    
    // Test clearing highlights
    println!("Test: Adding multiple highlights for clear test");
    let _ = add_highlight(document_hash, line_number, file_path, None);
    let _ = add_highlight(document_hash, line_number + 1, file_path, None);
    
    println!("Test: Clearing all highlights");
    let result = clear_highlights(document_hash, None);
    assert!(result.is_ok(), "Failed to clear highlights: {:?}", result);
    
    println!("Local highlighting tests completed successfully");
}

/// Test function to verify server-side highlighting functionality
#[tokio::test]
async fn test_server_highlighting() {
    // Skip this test if the server is not running
    let server_url = "http://localhost:3001";
    if let Err(_) = reqwest::get(server_url).await {
        println!("Server not running, skipping server tests");
        return;
    }
    
    // Initialize test values
    let document_hash = 67890u64;
    let line_number = 5;
    let file_path = "test_server_document.txt";
    let user_id = "test_user";
    
    // Create a test client
    let client = Arc::new(HyggClient::new(user_id.to_string()));
    
    // Test adding a highlight via server
    println!("Test: Adding a highlight via server");
    let result = add_highlight_async(document_hash, line_number, file_path, Some(client.clone())).await;
    assert!(result.is_ok(), "Failed to add highlight via server: {:?}", result);
    
    // Test adding the same highlight again (should not crash)
    println!("Test: Adding the same highlight again via server");
    let result = add_highlight_async(document_hash, line_number, file_path, Some(client.clone())).await;
    assert!(result.is_ok(), "Failed when adding same highlight via server: {:?}", result);
    
    // Test removing a highlight
    println!("Test: Removing a highlight via server");
    let result = remove_highlight_async(document_hash, line_number, file_path, Some(client.clone())).await;
    assert!(result.is_ok(), "Failed to remove highlight via server: {:?}", result);
    
    // Test removing a non-existent highlight (should not crash)
    println!("Test: Removing a non-existent highlight via server");
    let result = remove_highlight_async(document_hash, line_number, file_path, Some(client.clone())).await;
    assert!(result.is_ok(), "Failed when removing non-existent highlight via server: {:?}", result);
    
    // Test undoing the last highlight
    println!("Test: Adding a highlight for undo test via server");
    let _ = add_highlight_async(document_hash, line_number, file_path, Some(client.clone())).await;
    
    println!("Test: Undoing the last highlight via server");
    let result = undo_last_highlight_async(document_hash, Some(client.clone())).await;
    assert!(result.is_ok(), "Failed to undo last highlight via server: {:?}", result);
    
    // Test undoing when there's nothing to undo (should not crash)
    println!("Test: Undoing when there's nothing to undo via server");
    let result = undo_last_highlight_async(document_hash, Some(client.clone())).await;
    assert!(result.is_ok(), "Failed when undoing with nothing to undo via server: {:?}", result);
    
    // Test clearing highlights
    println!("Test: Adding multiple highlights for clear test via server");
    let _ = add_highlight_async(document_hash, line_number, file_path, Some(client.clone())).await;
    let _ = add_highlight_async(document_hash, line_number + 1, file_path, Some(client.clone())).await;
    
    println!("Test: Clearing all highlights via server");
    let result = clear_highlights_async(document_hash, Some(client.clone())).await;
    assert!(result.is_ok(), "Failed to clear highlights via server: {:?}", result);
    
    println!("Server highlighting tests completed successfully");
}

/// Test function to verify error handling when trying to use sync functions with server client
#[test]
fn test_sync_functions_with_server_client() {
    let document_hash = 11111u64;
    let line_number = 7;
    let file_path = "test_error_document.txt";
    let server_url = "http://localhost:3001";
    let user_id = "test_user";
    
    // Create a test client
    let client = Arc::new(HyggClient::new(user_id.to_string()));
    
    // Test add_highlight with server client (should return proper error)
    println!("Test: Calling sync add_highlight with server client");
    let result = add_highlight(document_hash, line_number, file_path, Some(client.clone()));
    assert!(result.is_err(), "Expected error when using sync add_highlight with server client");
    
    // Test remove_highlight with server client (should return proper error)
    println!("Test: Calling sync remove_highlight with server client");
    let result = remove_highlight(document_hash, line_number, file_path, Some(client.clone()));
    assert!(result.is_err(), "Expected error when using sync remove_highlight with server client");
    
    // Test undo_last_highlight with server client (should return proper error)
    println!("Test: Calling sync undo_last_highlight with server client");
    let result = undo_last_highlight(document_hash, Some(client.clone()));
    assert!(result.is_err(), "Expected error when using sync undo_last_highlight with server client");
    
    // Test clear_highlights with server client (should return proper error)
    println!("Test: Calling sync clear_highlights with server client");
    let result = clear_highlights(document_hash, Some(client.clone()));
    assert!(result.is_err(), "Expected error when using sync clear_highlights with server client");
    
    println!("Error handling tests completed successfully");
}

/// Test function to verify error cases and handling for the highlight functions
#[tokio::test]
async fn test_error_cases() {
    // Initialize test values
    let document_hash = 99999u64;
    let line_number = 15;
    let file_path = "test_error_document.txt";
    
    // Test edge cases for local operations
    
    // Add a highlight
    println!("Test: Adding a highlight for error tests");
    let _ = add_highlight(document_hash, line_number, file_path, None);
    
    // Try adding it again (should not crash, should be idempotent)
    println!("Test: Adding the same highlight again (idempotence)");
    let result = add_highlight(document_hash, line_number, file_path, None);
    assert!(result.is_ok(), "Failed during idempotent add operation: {:?}", result);
    
    // Remove it
    println!("Test: Removing the highlight");
    let result = remove_highlight(document_hash, line_number, file_path, None);
    assert!(result.is_ok(), "Failed to remove highlight: {:?}", result);
    
    // Try removing it again (should not crash, should be idempotent)
    println!("Test: Removing the same highlight again (idempotence)");
    let result = remove_highlight(document_hash, line_number, file_path, None);
    assert!(result.is_ok(), "Failed during idempotent remove operation: {:?}", result);
    
    // Clear highlights when there are none (should not crash)
    println!("Test: Clearing highlights when there are none");
    let result = clear_highlights(document_hash, None);
    assert!(result.is_ok(), "Failed during clear with no highlights: {:?}", result);
    
    println!("Error case tests completed successfully");
}

// Integration test that simulates a real user workflow
#[tokio::test]
async fn test_highlight_workflow() {
    // Initialize test values
    let document_hash = 54321u64;
    let file_path = "test_workflow_document.txt";
    
    // Simulating a user reading a document and highlighting multiple lines
    println!("Test: Simulating user workflow");
    
    // Add highlights to several lines
    println!("Adding highlights to lines 5, 10, 15");
    assert!(add_highlight(document_hash, 5, file_path, None).is_ok());
    assert!(add_highlight(document_hash, 10, file_path, None).is_ok());
    assert!(add_highlight(document_hash, 15, file_path, None).is_ok());
    
    // Try to add duplicate highlights (should not crash)
    println!("Adding duplicate highlights to lines 5 and 15");
    assert!(add_highlight(document_hash, 5, file_path, None).is_ok());
    assert!(add_highlight(document_hash, 15, file_path, None).is_ok());
    
    // Remove a highlight
    println!("Removing highlight from line 10");
    assert!(remove_highlight(document_hash, 10, file_path, None).is_ok());
    
    // Try removing a non-existent highlight
    println!("Removing non-existent highlight from line 20");
    assert!(remove_highlight(document_hash, 20, file_path, None).is_ok());
    
    // Undo the last highlight action
    println!("Undoing last highlight action");
    assert!(undo_last_highlight(document_hash, None).is_ok());
    
    // Clear all highlights
    println!("Clearing all highlights");
    assert!(clear_highlights(document_hash, None).is_ok());
    
    // Make sure we can still add highlights after clearing
    println!("Adding highlight after clearing");
    assert!(add_highlight(document_hash, 25, file_path, None).is_ok());
    
    println!("User workflow test completed successfully");
}
