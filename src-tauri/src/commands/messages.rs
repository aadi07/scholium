use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{State, Window};
use tauri::Emitter;
use futures_util::StreamExt;
use crate::db::DbConn;

const OLLAMA_URL: &str = "http://localhost:11434/api/chat";
const OLLAMA_MODEL: &str = "gemma4:26b";

#[derive(Serialize)]
pub struct Message {
    pub id: String,
    pub thread_id: String,
    pub role: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
}

#[derive(Serialize, Clone)]
struct OllamaMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct OllamaChunk {
    message: Option<OllamaChunkMessage>,
    done: Option<bool>,
}

#[derive(Deserialize)]
struct OllamaChunkMessage {
    content: String,
}

#[tauri::command]
pub async fn send_message(
    window: Window,
    db: State<'_, DbConn>,
    thread_id: String,
    role: String,
    content: String,
    page_image: Option<String>,
    page_number: Option<u32>,
) -> Result<Message, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();

    // Persist the user message immediately
    {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        crate::db::insert_message(&conn, &id, &thread_id, &role, &content, now)
            .map_err(|e| e.to_string())?;
    }

    let user_msg = Message {
        id: id.clone(),
        thread_id: thread_id.clone(),
        role,
        content: content.clone(),
        created_at: now,
    };

    // Build system prompt
    let page_num = page_number.unwrap_or(0);
    let system_content = format!(
        "You are a sharp, friendly reading companion embedded in a PDF reader called Scholium. \
         The user is viewing page {page_num} of a PDF, provided as an image. \
         Be rigorous and precise, but write the way a knowledgeable friend talks — \
         conversational, direct, no bullet-point walls or excessive headers. \
         Flowing prose is strongly preferred over lists. Skip filler phrases like \
         \"Certainly!\" or \"Great question!\". \
         IMPORTANT: Always format ALL mathematical expressions using LaTeX: \
         use $...$ for inline math and $$...$$ for display (block) math. \
         For example, write $E = mc^2$ not \"E = mc2\", and $$\\int_{{0}}^{{\\infty}} f(x)\\,dx$$ for display equations. \
         Never write bare math symbols — always wrap them in LaTeX delimiters."
    );

    // Strip data-URI prefix from image if present
    let user_images: Option<Vec<String>> = page_image.map(|b64| {
        let raw = if let Some(pos) = b64.find(',') { b64[pos + 1..].to_string() } else { b64 };
        vec![raw]
    });

    // Load prior messages to give the model conversation history.
    // Exclude the message we just inserted (last row) — we'll append it as the final turn.
    let history: Vec<OllamaMessage> = {
        let conn = db.0.lock().map_err(|e| e.to_string())?;
        let rows = crate::db::list_messages(&conn, &thread_id).map_err(|e| e.to_string())?;
        // Drop the last row — that's the user message we just persisted above
        rows.iter().rev().skip(1).rev()
            .map(|(_, role, content, _)| OllamaMessage {
                role: role.clone(),
                content: content.clone(),
                images: None, // don't re-send images for history turns
            })
            .collect()
    };

    let mut ollama_messages: Vec<OllamaMessage> = Vec::with_capacity(2 + history.len());
    ollama_messages.push(OllamaMessage { role: "system".to_string(), content: system_content, images: None });
    ollama_messages.extend(history);
    ollama_messages.push(OllamaMessage { role: "user".to_string(), content: content.clone(), images: user_images });

    let ollama_req = OllamaRequest {
        model: OLLAMA_MODEL.to_string(),
        messages: ollama_messages,
        stream: true,
    };

    let win = window.clone();
    let thread_id_clone = thread_id.clone();
    // Clone the Arc so the spawned task can lock the DB after streaming completes
    let db_arc = Arc::clone(&db.0);

    tauri::async_runtime::spawn(async move {
        let client = reqwest::Client::new();

        let res = match client.post(OLLAMA_URL).json(&ollama_req).send().await {
            Ok(r) => r,
            Err(e) => {
                let _ = win.emit("ai://error", serde_json::json!({
                    "thread_id": thread_id_clone,
                    "error": format!("Cannot reach Ollama at localhost:11434. Is it running? ({})", e)
                }));
                return;
            }
        };

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            let _ = win.emit("ai://error", serde_json::json!({
                "thread_id": thread_id_clone,
                "error": format!("Ollama returned HTTP {}: {}", status, body)
            }));
            return;
        }

        let mut stream = res.bytes_stream();
        let mut full_response = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[ollama] stream read error: {}", e);
                    break;
                }
            };

            let text = String::from_utf8_lossy(&chunk);
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() { continue; }

                if let Ok(parsed) = serde_json::from_str::<OllamaChunk>(line) {
                    if let Some(msg) = parsed.message {
                        full_response.push_str(&msg.content);
                        let _ = win.emit("ai://token", serde_json::json!({
                            "thread_id": thread_id_clone,
                            "token": msg.content
                        }));
                    }
                    if parsed.done.unwrap_or(false) {
                        break;
                    }
                }
            }
        }

        // Persist the completed assistant response
        if !full_response.is_empty() {
            let assistant_id = uuid::Uuid::new_v4().to_string();
            let assistant_now = chrono::Utc::now().timestamp();
            if let Ok(conn) = db_arc.lock() {
                let _ = crate::db::insert_message(
                    &conn,
                    &assistant_id,
                    &thread_id_clone,
                    "assistant",
                    &full_response,
                    assistant_now,
                );
            }
        }

        let _ = win.emit("ai://done", serde_json::json!({ "thread_id": thread_id_clone }));
    });

    Ok(user_msg)
}

#[tauri::command]
pub fn load_messages(
    db: State<'_, DbConn>,
    thread_id: String,
) -> Result<Vec<Message>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    crate::db::list_messages(&conn, &thread_id)
        .map(|rows| {
            rows.into_iter()
                .map(|(id, role, content, created_at)| Message {
                    id,
                    thread_id: thread_id.clone(),
                    role,
                    content,
                    created_at,
                })
                .collect()
        })
        .map_err(|e| e.to_string())
}
