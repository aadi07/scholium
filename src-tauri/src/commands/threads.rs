use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct CreateThreadPayload {
    pub document_id: String,
    pub page_number: u32,
    pub anchor_quote: String,
    pub anchor_start: Option<i64>,
    pub anchor_end: Option<i64>,
}

#[derive(Serialize)]
pub struct Thread {
    pub id: String,
    pub document_id: String,
    pub page_number: u32,
    pub anchor_start: Option<i64>,
    pub anchor_end: Option<i64>,
    pub anchor_quote: String,
    pub summary: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[tauri::command]
pub fn create_thread(payload: CreateThreadPayload) -> Result<Thread, String> {
    // Generate id + timestamps
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();

    // Persist to SQLite
    match crate::db::insert_thread(
        &id,
        &payload.document_id,
        payload.page_number as i64,
        payload.anchor_start,
        payload.anchor_end,
        &payload.anchor_quote,
        None,
        now,
        now,
    ) {
        Ok(()) => {
            let thread = Thread {
                id,
                document_id: payload.document_id,
                page_number: payload.page_number,
                anchor_start: payload.anchor_start,
                anchor_end: payload.anchor_end,
                anchor_quote: payload.anchor_quote,
                summary: None,
                created_at: now,
                updated_at: now,
            };
            Ok(thread)
        }
        Err(e) => Err(format!("DB insert failed: {}", e)),
    }
}
