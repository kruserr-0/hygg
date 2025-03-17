use std::sync::Arc;
use axum::{
    routing::{get, post, delete},
    Router, Json, extract::{State, Path},
    response::IntoResponse,
    http::StatusCode,
    body::Body,
};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use async_lock::Mutex;
use tower_http::cors::CorsLayer;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ReadingProgress {
    id: Uuid,
    file_path: String,
    position: usize,
    user_id: String,
    last_accessed: DateTime<Utc>,
    lock_holder: Option<String>,
    lock_expiry: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
struct Highlight {
    id: Option<Uuid>,              // Optional for new highlights
    file_path: String,
    document_hash: String,
    line_number: i64,             // Using i64 instead of usize for SQLx compatibility
    user_id: String,
    created_at: Option<DateTime<Utc>>, // Optional for new highlights
}

#[derive(Debug, Serialize, Deserialize)]
struct ProgressEvent {
    id: Uuid,
    progress_id: Uuid,
    event_type: String,
    position: usize,
    user_id: String,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HighlightRequest {
    file_path: String,
    document_hash: String,
    line_number: i64,  // Using i64 instead of usize for SQLx compatibility
    user_id: String,
}

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
    locks: Arc<Mutex<Vec<(Uuid, String, DateTime<Utc>)>>>,
}

async fn init_db(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    println!("Initializing database tables...");
    
    // Enable foreign key constraints
    sqlx::query("
        PRAGMA foreign_keys = ON;
    ")
    .execute(pool)
    .await?;
    
    // Create reading_progress table if it doesn't exist
    println!("Creating reading_progress table if needed...");
    sqlx::query("
        CREATE TABLE IF NOT EXISTS reading_progress (
            id TEXT PRIMARY KEY,
            file_path TEXT NOT NULL,
            position INTEGER NOT NULL,
            user_id TEXT NOT NULL,
            last_accessed TIMESTAMP NOT NULL,
            lock_holder TEXT,
            lock_expiry TIMESTAMP
        )"
    ).execute(pool).await?;

    // Create progress_events table if it doesn't exist
    println!("Creating progress_events table if needed...");
    sqlx::query("
        CREATE TABLE IF NOT EXISTS progress_events (
            id TEXT PRIMARY KEY,
            progress_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            position INTEGER NOT NULL,
            user_id TEXT NOT NULL,
            timestamp TIMESTAMP NOT NULL,
            FOREIGN KEY(progress_id) REFERENCES reading_progress(id)
        )"
    ).execute(pool).await?;
    
    // Create highlights table if it doesn't exist
    println!("Creating highlights table if needed...");
    sqlx::query("
        CREATE TABLE IF NOT EXISTS highlights (
            id TEXT PRIMARY KEY,
            file_path TEXT NOT NULL,
            document_hash TEXT NOT NULL,
            line_number INTEGER NOT NULL,
            user_id TEXT NOT NULL,
            created_at TIMESTAMP NOT NULL,
            UNIQUE(document_hash, line_number, user_id)
        )"
    ).execute(pool).await?;
    
    println!("Database initialization completed successfully");
    Ok(())
}

async fn get_file_content(Path(file_path): Path<String>) -> impl IntoResponse {
    // For security reasons, restrict to files in the test-data directory
    let safe_path = format!("test-data/{}", file_path.trim_start_matches('/').split('/').last().unwrap_or("sample.txt"));
    
    println!("Attempting to read file: {}", safe_path);
    match tokio::fs::read_to_string(&safe_path).await {
        Ok(content) => (StatusCode::OK, content).into_response(),
        Err(err) => {
            eprintln!("Error reading file {}: {}", safe_path, err);
            (StatusCode::NOT_FOUND, format!("File not found: {}", file_path)).into_response()
        },
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct FileUpload {
    file_path: String,
    content: String,
}

async fn upload_file(
    State(_state): State<AppState>,
    Json(upload): Json<FileUpload>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    println!("FILE UPLOAD: Attempting to save file: {}", upload.file_path);
    
    // For security, only allow files to be saved to test-data directory
    let file_name = upload.file_path.trim_start_matches('/').split('/').last().unwrap_or("unknown.txt");
    let safe_path = format!("test-data/{}", file_name);
    
    // Write the file
    tokio::fs::write(&safe_path, &upload.content).await
        .map_err(|e| {
            println!("ERROR: Failed to write file {:?}: {}", safe_path, e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write file: {}", e))
        })?;
    
    println!("FILE UPLOAD: Successfully saved file: {}", file_name);
    Ok((StatusCode::OK, "File uploaded successfully".to_string()).into_response())
}

async fn acquire_lock(
    State(state): State<AppState>,
    Json(progress): Json<ReadingProgress>,
) -> Result<Json<ReadingProgress>, (axum::http::StatusCode, String)> {
    println!("LOCK REQUEST: User {} is trying to acquire lock for progress {}", progress.user_id, progress.id);
    
    // Check if progress entry exists, create if not
    let exists = sqlx::query("SELECT id FROM reading_progress WHERE id = ?")
        .bind(progress.id.to_string())
        .fetch_optional(&state.db)
        .await
        .map_err(|e| {
            println!("DATABASE ERROR: Failed to query reading_progress: {}", e);
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
        
    if exists.is_none() {
        println!("PROGRESS INIT: Creating new progress entry for {} with id {}", progress.file_path, progress.id);
        sqlx::query("
            INSERT INTO reading_progress (id, file_path, position, user_id, last_accessed, lock_holder, lock_expiry)
            VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(progress.id.to_string())
        .bind(&progress.file_path)
        .bind(progress.position as i64)
        .bind(&progress.user_id)
        .bind(Utc::now())
        .bind::<Option<&str>>(None)
        .bind::<Option<DateTime<Utc>>>(None)
        .execute(&state.db)
        .await
        .map_err(|e| {
            println!("DATABASE ERROR: Failed to create progress entry: {}", e);
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        })?;
    }
    
    let mut locks = state.locks.lock().await;
    
    // Clean expired locks
    let now = Utc::now();
    locks.retain(|(_, _, expiry)| expiry > &now);
    
    // Check if progress is already locked by another user
    if let Some((_, holder, _)) = locks.iter().find(|(id, _, _)| *id == progress.id) {
        // If it's the same user trying to reacquire their lock, allow it
        if holder != &progress.user_id {
            println!("LOCK DENIED: Progress {} is already locked by {}", progress.id, holder);
            return Err((axum::http::StatusCode::CONFLICT, 
                format!("Progress is locked by {}", holder)));
        } else {
            // Same user is reacquiring their lock - remove the old lock entry first
            println!("LOCK REACQUIRE: User {} is reacquiring their lock for progress {}", progress.user_id, progress.id);
            locks.retain(|(id, user, _)| !(*id == progress.id && user == &progress.user_id));
        }
    }
    
    // Acquire new lock
    let expiry = now + chrono::Duration::minutes(15);
    locks.push((progress.id, progress.user_id.clone(), expiry));
    println!("LOCK GRANTED: User {} acquired lock for progress {} until {}", progress.user_id, progress.id, expiry);
    
    // Update database
    sqlx::query("
        UPDATE reading_progress 
        SET lock_holder = ?, lock_expiry = ? 
        WHERE id = ?"
    )
    .bind(&progress.user_id)
    .bind(expiry)
    .bind(progress.id.to_string())
    .execute(&state.db)
    .await
    .map_err(|e| {
        println!("DATABASE ERROR: Failed to update lock in database: {}", e);
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
    
    // Fetch the latest progress including current position from the database
    let db_progress = sqlx::query("
        SELECT id, file_path, position, user_id, last_accessed, lock_holder, lock_expiry 
        FROM reading_progress 
        WHERE id = ?"
    )
    .bind(progress.id.to_string())
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        println!("DATABASE ERROR: Failed to query updated progress: {}", e);
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
    
    // Convert to ReadingProgress
    let id_str: String = db_progress.get(0);
    let id = Uuid::parse_str(&id_str)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Invalid UUID: {}", e)))?;
    
    let mut updated_progress = progress.clone();
    updated_progress.id = id;
    updated_progress.file_path = db_progress.get(1);
    updated_progress.position = db_progress.get::<i64, _>(2) as usize;
    updated_progress.user_id = db_progress.get(3);
    updated_progress.last_accessed = db_progress.get(4);
    updated_progress.lock_holder = Some(progress.user_id.clone()); // We just set this
    updated_progress.lock_expiry = Some(expiry); // We just set this
    
    println!("PROGRESS LOADED: User {} acquired lock with position {}", updated_progress.user_id, updated_progress.position);
    
    Ok(Json(updated_progress))
}

async fn release_lock(
    State(state): State<AppState>,
    Json(progress): Json<ReadingProgress>,
) -> Result<Json<ReadingProgress>, (axum::http::StatusCode, String)> {
    println!("LOCK RELEASE: User {} is trying to release lock for progress {}", progress.user_id, progress.id);
    let mut locks = state.locks.lock().await;
    
    // Verify lock is held by the user
    let lock_idx = locks.iter().position(|(id, holder, _)| 
        *id == progress.id && holder == &progress.user_id
    );
    
    println!("LOCK CHECK: User {} is trying to release lock for progress {}. Lock index: {:?}", progress.user_id, progress.id, lock_idx);
    
    match lock_idx {
        Some(idx) => {
            // Remove the lock
            locks.remove(idx);
            println!("LOCK RELEASED: User {} released lock for progress {}", progress.user_id, progress.id);
            
            // Update database to clear lock holder
            sqlx::query("
                UPDATE reading_progress 
                SET lock_holder = NULL, lock_expiry = NULL 
                WHERE id = ?"
            )
            .bind(progress.id.to_string())
            .execute(&state.db)
            .await
            .map_err(|e| {
                println!("DATABASE ERROR: Failed to update lock in database: {}", e);
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            })?;
            
            let mut updated_progress = progress.clone();
            updated_progress.lock_holder = None;
            updated_progress.lock_expiry = None;
            
            Ok(Json(updated_progress))
        },
        None => {
            println!("LOCK RELEASE DENIED: User {} doesn't hold lock for progress {}", progress.user_id, progress.id);
            Err((axum::http::StatusCode::FORBIDDEN, 
                "You don't hold the lock for this progress".to_string()))
        }
    }
}

async fn update_progress(
    State(state): State<AppState>,
    Json(progress): Json<ReadingProgress>,
) -> Result<Json<ReadingProgress>, (axum::http::StatusCode, String)> {
    println!("PROGRESS UPDATE: User {} is updating progress to position {}", progress.user_id, progress.position);
    let locks = state.locks.lock().await;
    
    // Verify lock
    if !locks.iter().any(|(id, holder, _)| 
        *id == progress.id && holder == &progress.user_id
    ) {
        println!("PROGRESS DENIED: User {} doesn't hold lock for progress {}", progress.user_id, progress.id);
        return Err((axum::http::StatusCode::FORBIDDEN, 
            "You don't hold the lock for this progress".to_string()));
    }
    
    // Record event
    let event = ProgressEvent {
        id: Uuid::new_v4(),
        progress_id: progress.id,
        event_type: "update".to_string(),
        position: progress.position,
        user_id: progress.user_id.clone(),
        timestamp: Utc::now(),
    };
    println!("PROGRESS EVENT: Recording event id {} for progress {}", event.id, event.progress_id);
    
    sqlx::query("
        INSERT INTO progress_events (id, progress_id, event_type, position, user_id, timestamp)
        VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(event.id.to_string())
    .bind(event.progress_id.to_string())
    .bind(&event.event_type)
    .bind(event.position as i64)
    .bind(&event.user_id)
    .bind(event.timestamp)
    .execute(&state.db)
    .await
    .map_err(|e| {
        println!("DATABASE ERROR: Failed to insert progress event: {}", e);
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
    
    // Update progress
    sqlx::query("
        UPDATE reading_progress 
        SET position = ?, last_accessed = ? 
        WHERE id = ?"
    )
    .bind(progress.position as i64)
    .bind(Utc::now())
    .bind(progress.id.to_string())
    .execute(&state.db)
    .await
    .map_err(|e| {
        println!("DATABASE ERROR: Failed to update reading progress: {}", e);
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
    
    // Log detailed information about the progress update
    println!("PROGRESS DETAIL: User {} updated position for file {} (ID: {}) to position {}", 
        progress.user_id, 
        progress.file_path,
        progress.id,
        progress.position);
    
    println!("PROGRESS UPDATED: User {} successfully updated position to {}", progress.user_id, progress.position);
    Ok(Json(progress))
}

async fn get_progress(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ReadingProgress>, (axum::http::StatusCode, String)> {
    println!("PROGRESS GET: Retrieving progress with ID {}", id);
    
    // Query the database for the progress with the given ID
    let progress = sqlx::query("
        SELECT id, file_path, position, user_id, last_accessed, lock_holder, lock_expiry 
        FROM reading_progress 
        WHERE id = ?"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        println!("DATABASE ERROR: Failed to query reading progress: {}", e);
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    })?;
    
    // Convert the database row to a ReadingProgress struct
    match progress {
        Some(row) => {
            // Convert ID to UUID
            let id_str: String = row.get(0);
            let id = match Uuid::parse_str(&id_str) {
                Ok(uuid) => uuid,
                Err(e) => return Err((StatusCode::BAD_REQUEST, format!("Invalid UUID: {}", e))),
            };
            
            // Extract other fields
            let file_path: String = row.get(1);
            let position: i64 = row.get(2);
            let user_id: String = row.get(3);
            let last_accessed: DateTime<Utc> = row.get(4);
            let lock_holder: Option<String> = row.get(5);
            let lock_expiry: Option<DateTime<Utc>> = row.get(6);
            
            let progress = ReadingProgress {
                id,
                file_path,
                position: position as usize,
                user_id,
                last_accessed,
                lock_holder,
                lock_expiry,
            };
            
            println!("PROGRESS FOUND: ID {} at position {}", id, progress.position);
            Ok(Json(progress))
        },
        None => {
            println!("PROGRESS NOT FOUND: ID {}", id);
            Err((StatusCode::NOT_FOUND, format!("Progress with ID {} not found", id)))
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    
    // Ensure the database directory exists with proper permissions
    let db_path = std::path::Path::new("data");
    if !db_path.exists() {
        println!("Creating database directory: {:?}", db_path);
        std::fs::create_dir_all(db_path)?;
        
        // On Unix-like systems, ensure proper permissions (this is a no-op on Windows)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(db_path)?;
            let mut perms = metadata.permissions();
            perms.set_mode(0o755); // rwxr-xr-x permissions
            std::fs::set_permissions(db_path, perms)?;
        }
    }
    
    // Check if the test-data directory exists too and create it if needed
    let test_data_path = std::path::Path::new("test-data");
    if !test_data_path.exists() {
        println!("Creating test data directory: {:?}", test_data_path);
        std::fs::create_dir_all(test_data_path)?;
    }
    
    // Database file path (use absolute path for better reliability)
    let current_dir = std::env::current_dir()?;
    let db_path = current_dir.join("data/hygg.db");
    let db_file = db_path.to_str().unwrap_or("data/hygg.db");
    println!("Using database at: {}", db_file);
    
    // Touch the database file to make sure it exists before trying to connect
    if !std::path::Path::new(db_file).exists() {
        println!("Creating empty database file");
        std::fs::File::create(db_file)?;
    }
    
    // Connect to the database with retry logic in case of temporary failures
    let mut retry_count = 0;
    let max_retries = 3;
    let db = loop {
        match SqlitePoolOptions::new()
            .max_connections(10)
            .connect(db_file)
            .await
        {
            Ok(pool) => {
                println!("Successfully connected to database");
                break pool;
            },
            Err(e) if retry_count < max_retries => {
                retry_count += 1;
                eprintln!("Database connection attempt {} failed: {}", retry_count, e);
                println!("Retrying connection in 1 second...");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            },
            Err(e) => return Err(e.into()),
        }
    };
    
    init_db(&db).await?;
    
    let app_state = AppState {
        db,
        locks: Arc::new(Mutex::new(Vec::new())),
    };
    
    let app = Router::new()
        .route("/file/:path", get(get_file_content))
        .route("/file/upload", post(upload_file))
        .route("/progress/lock", post(acquire_lock))
        .route("/progress/release", post(release_lock))
        .route("/progress/update", post(update_progress))
        .route("/progress/:id", get(get_progress))
        // Highlight management endpoints
        .route("/highlights/:document_hash/:user_id", get(get_highlights))
        .route("/highlights/add", post(add_highlight))
        .route("/highlights/remove", post(remove_highlight))
        .route("/highlights/clear/:document_hash/:user_id", delete(clear_highlights))
        .route("/highlights/undo/:document_hash/:user_id", delete(undo_last_highlight))
        .layer(CorsLayer::permissive())
        .with_state(app_state);
    
    println!("Server running on http://localhost:3001");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

// Highlight management API endpoints

/// Get all highlights for a document and user
async fn get_highlights(
    State(state): State<AppState>,
    Path((document_hash, user_id)): Path<(String, String)>,
) -> Result<Json<Vec<Highlight>>, (StatusCode, String)> {
    println!("HIGHLIGHTS: Getting highlights for document_hash {} and user {}", document_hash, user_id);
    
    let highlights = sqlx::query_as::<_, Highlight>(r"
        SELECT id, file_path, document_hash, line_number, user_id, created_at 
        FROM highlights 
        WHERE document_hash = ? AND user_id = ?
    ")
    .bind(&document_hash)
    .bind(&user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        eprintln!("Database error fetching highlights: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
    })?;
    
    println!("HIGHLIGHTS: Found {} highlights", highlights.len());
    Ok(Json(highlights))
}

/// Add a highlight
async fn add_highlight(
    State(state): State<AppState>,
    Json(highlight_req): Json<HighlightRequest>,
) -> Result<Json<Highlight>, (StatusCode, String)> {
    println!("HIGHLIGHTS: Adding highlight for line {} in document {} for user {}", 
             highlight_req.line_number, highlight_req.document_hash, highlight_req.user_id);
    
    let id = Uuid::new_v4();
    let now = Utc::now();
    
    // First check if this highlight already exists
    let existing = sqlx::query(r"
        SELECT id FROM highlights 
        WHERE document_hash = ? AND line_number = ? AND user_id = ?
    ")
    .bind(&highlight_req.document_hash)
    .bind(highlight_req.line_number)
    .bind(&highlight_req.user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        eprintln!("Database error checking for existing highlight: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
    })?;
    
    if existing.is_some() {
        return Err((StatusCode::CONFLICT, "Highlight already exists".to_string()));
    }
    
    sqlx::query(r"
        INSERT INTO highlights (id, file_path, document_hash, line_number, user_id, created_at)
        VALUES (?, ?, ?, ?, ?, ?)
    ")
    .bind(id.to_string())
    .bind(&highlight_req.file_path)
    .bind(&highlight_req.document_hash)
    .bind(highlight_req.line_number)
    .bind(&highlight_req.user_id)
    .bind(now)
    .execute(&state.db)
    .await
    .map_err(|e| {
        eprintln!("Database error adding highlight: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
    })?;
    
    let new_highlight = Highlight {
        id: Some(id),
        file_path: highlight_req.file_path,
        document_hash: highlight_req.document_hash,
        line_number: highlight_req.line_number,
        user_id: highlight_req.user_id,
        created_at: Some(now),
    };
    
    println!("HIGHLIGHTS: Successfully added highlight");
    Ok(Json(new_highlight))
}

/// Remove a highlight
async fn remove_highlight(
    State(state): State<AppState>,
    Json(highlight_req): Json<HighlightRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    println!("HIGHLIGHTS: Removing highlight for line {} in document {} for user {}", 
             highlight_req.line_number, highlight_req.document_hash, highlight_req.user_id);
    
    let result = sqlx::query(r"
        DELETE FROM highlights 
        WHERE document_hash = ? AND line_number = ? AND user_id = ?
    ")
    .bind(&highlight_req.document_hash)
    .bind(highlight_req.line_number)
    .bind(&highlight_req.user_id)
    .execute(&state.db)
    .await
    .map_err(|e| {
        eprintln!("Database error removing highlight: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
    })?;
    
    if result.rows_affected() == 0 {
        println!("HIGHLIGHTS: No highlights found to remove");
        return Err((StatusCode::NOT_FOUND, "Highlight not found".to_string()));
    }
    
    println!("HIGHLIGHTS: Successfully removed highlight");
    Ok((StatusCode::OK, "Highlight removed".to_string()))
}

/// Clear all highlights for a document and user
async fn clear_highlights(
    State(state): State<AppState>,
    Path((document_hash, user_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    println!("HIGHLIGHTS: Clearing all highlights for document {} and user {}", document_hash, user_id);
    
    let result = sqlx::query(r"
        DELETE FROM highlights 
        WHERE document_hash = ? AND user_id = ?
    ")
    .bind(&document_hash)
    .bind(&user_id)
    .execute(&state.db)
    .await
    .map_err(|e| {
        eprintln!("Database error clearing highlights: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
    })?;
    
    println!("HIGHLIGHTS: Cleared {} highlights", result.rows_affected());
    Ok((StatusCode::OK, format!("Cleared {} highlights", result.rows_affected())))
}

/// Undo the last highlight action for a document and user
async fn undo_last_highlight(
    State(state): State<AppState>,
    Path((document_hash, user_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    println!("HIGHLIGHTS: Undoing last highlight for document {} and user {}", document_hash, user_id);
    
    // Find the most recent highlight for this document and user
    let maybe_highlight = sqlx::query_as::<_, Highlight>(r"
        SELECT * FROM highlights 
        WHERE document_hash = ? AND user_id = ? 
        ORDER BY created_at DESC LIMIT 1
    ")
    .bind(&document_hash)
    .bind(&user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        eprintln!("Database error finding most recent highlight: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
    })?;
    
    if let Some(highlight) = maybe_highlight {
        // Delete this highlight
        let _result = sqlx::query(r"
            DELETE FROM highlights 
            WHERE id = ?
        ")
        .bind(highlight.id)
        .execute(&state.db)
        .await
        .map_err(|e| {
            eprintln!("Database error removing highlight: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
        })?;
        
        println!("HIGHLIGHTS: Undid highlight at line {} for document {}", highlight.line_number, document_hash);
        Ok((StatusCode::OK, format!("Undid highlight at line {}", highlight.line_number)))
    } else {
        println!("HIGHLIGHTS: No highlights found to undo for document {}", document_hash);
        Ok((StatusCode::NO_CONTENT, String::from("No highlights to undo")))
    }
}
