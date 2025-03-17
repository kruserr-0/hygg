use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use anyhow::Result;
use thiserror::Error;

const SERVER_URL: &str = "http://localhost:3001";

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Lock error: {0}")]
    Lock(String),
    #[error("Upload error: {0}")]
    Upload(String),
    #[error("Highlight error: {0}")]
    Highlight(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReadingProgress {
    pub id: Uuid,
    pub file_path: String,
    pub position: usize,
    pub user_id: String,
    pub last_accessed: DateTime<Utc>,
    pub lock_holder: Option<String>,
    pub lock_expiry: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Highlight {
    pub id: Option<Uuid>,
    pub file_path: String,
    pub document_hash: String,
    pub line_number: usize,
    pub user_id: String,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HighlightRequest {
    pub file_path: String,
    pub document_hash: String,
    pub line_number: usize,
    pub user_id: String,
}

#[derive(Clone)]
pub struct HyggClient {
    client: reqwest::Client,
    user_id: String,
}

impl HyggClient {
    pub fn new(user_id: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            user_id,
        }
    }
    
    // Highlight management methods
    
    /// Get all highlights for a document
    pub async fn get_highlights(&self, document_hash: &str) -> Result<Vec<Highlight>> {
        let url = format!("{}/highlights/{}/{}", SERVER_URL, document_hash, self.user_id);
        let highlights = self.client.get(&url)
            .send()
            .await?
            .json::<Vec<Highlight>>()
            .await?;
        Ok(highlights)
    }
    
    /// Add a highlight to a specific line
    pub async fn add_highlight(&self, file_path: &str, document_hash: &str, line_number: usize) -> Result<Highlight> {
        let url = format!("{}/highlights/add", SERVER_URL);
        
        let request = HighlightRequest {
            file_path: file_path.to_string(),
            document_hash: document_hash.to_string(),
            line_number,
            user_id: self.user_id.clone(),
        };
        
        let highlight = self.client.post(&url)
            .json(&request)
            .send()
            .await?
            .json::<Highlight>()
            .await?;
            
        Ok(highlight)
    }
    
    /// Remove a highlight from a specific line
    pub async fn remove_highlight(&self, file_path: &str, document_hash: &str, line_number: usize) -> Result<()> {
        let url = format!("{}/highlights/remove", SERVER_URL);
        
        let request = HighlightRequest {
            file_path: file_path.to_string(),
            document_hash: document_hash.to_string(),
            line_number,
            user_id: self.user_id.clone(),
        };
        
        let response = self.client.post(&url)
            .json(&request)
            .send()
            .await?;
            
        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(ClientError::Highlight(format!("Failed to remove highlight: {}", error_text)).into());
        }
        
        Ok(())
    }
    
    /// Clear all highlights for a document
    pub async fn clear_highlights(&self, document_hash: &str) -> Result<()> {
        let url = format!("{}/highlights/clear/{}/{}", SERVER_URL, document_hash, self.user_id);
        
        let response = self.client.delete(&url)
            .send()
            .await?;
            
        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(ClientError::Highlight(format!("Failed to clear highlights: {}", error_text)).into());
        }
        
        Ok(())
    }
    
    /// Undo the last highlight action for a document
    pub async fn undo_last_highlight(&self, document_hash: &str) -> Result<bool> {
        let url = format!("{}/highlights/undo/{}/{}", SERVER_URL, document_hash, self.user_id);
        
        let response = self.client.delete(&url)
            .send()
            .await?;
            
        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(ClientError::Highlight(format!("Failed to undo highlight: {}", error_text)).into());
        }
        
        // If status is 200 OK, something was undone
        // If status is 204 No Content, nothing was undone
        Ok(response.status().as_u16() == 200)
    }

    pub async fn get_file_content(&self, file_path: &str) -> Result<String> {
        // Extract just the filename for server request
        let file_name = std::path::Path::new(file_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("sample.txt");
            
        let url = format!("{}/file/{}", SERVER_URL, file_name);
        println!("Requesting file from: {}", url);
        let content = self.client.get(&url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        Ok(content)
    }

    pub async fn acquire_lock(&self, progress: &ReadingProgress) -> Result<ReadingProgress> {
        let url = format!("{}/progress/lock", SERVER_URL);
        let response = self.client.post(&url)
            .json(progress)
            .send()
            .await?;

        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(ClientError::Lock(error).into());
        }

        let progress = response.json().await?;
        Ok(progress)
    }

    pub async fn update_progress(&self, progress: &ReadingProgress) -> Result<ReadingProgress> {
        let url = format!("{}/progress/update", SERVER_URL);
        let response = self.client.post(&url)
            .json(progress)
            .send()
            .await?;

        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(ClientError::Lock(error).into());
        }

        let progress = response.json().await?;
        Ok(progress)
    }
    
    pub async fn get_progress(&self, progress_id: &str) -> Result<ReadingProgress> {
        let url = format!("{}/progress/{}", SERVER_URL, progress_id);
        let response = self.client.get(&url)
            .send()
            .await?;

        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(ClientError::Lock(format!("Failed to get progress: {}", error)).into());
        }

        let progress = response.json().await?;
        Ok(progress)
    }
    
    pub async fn release_lock(&self, progress: &ReadingProgress) -> Result<ReadingProgress> {
        let url = format!("{}/progress/release", SERVER_URL);
        let response = self.client.post(&url)
            .json(progress)
            .send()
            .await?;

        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(ClientError::Lock(format!("Failed to release lock: {}", error)).into());
        }

        let progress = response.json().await?;
        Ok(progress)
    }
    
    pub async fn upload_file(&self, file_path: &str, content: &str) -> Result<()> {
        let url = format!("{}/file/upload", SERVER_URL);
        
        #[derive(Serialize)]
        struct FileUpload {
            file_path: String,
            content: String,
        }
        
        let upload = FileUpload {
            file_path: file_path.to_string(),
            content: content.to_string(),
        };
        
        let response = self.client.post(&url)
            .json(&upload)
            .send()
            .await?;

        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(ClientError::Upload(format!("Failed to upload file: {}", error)).into());
        }

        Ok(())
    }
}
