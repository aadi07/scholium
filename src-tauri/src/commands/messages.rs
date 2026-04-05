use serde::{Deserialize, Serialize};
use tauri::Window;
use tauri::Emitter;
use futures_util::StreamExt;

const OLLAMA_URL: &str = "http://localhost:11434/api/chat";
const OLLAMA_MODEL: &str = "qwen2.5:7b";

#[derive(Serialize)]
pub struct Message {
    pub id: String,
    pub thread_id: String,
    pub role: String,
    pub content: String,
    pub created_at: i64,
}

// ── Ollama wire types ────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────────────────────────

/// Tauri v2 maps JS object keys directly to individual Rust parameters (snake_case).
/// Do NOT use a wrapper struct — that requires nesting under the param name on the JS side.
#[tauri::command]
pub async fn send_message(
    window: Window,
    thread_id: String,
    role: String,
    content: String,
    page_context: Option<String>,
    page_number: Option<u32>,
) -> Result<Message, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp();

    // Persist user message
    crate::db::insert_message(&id, &thread_id, &role, &content, now)
        .map_err(|e| format!("DB insert failed: {}", e))?;

    let user_msg = Message {
        id: id.clone(),
        thread_id: thread_id.clone(),
        role: role.clone(),
        content: content.clone(),
        created_at: now,
    };

    // Build conversation history from DB for context
    let history = crate::db::list_messages(&thread_id)
        .unwrap_or_default()
        .into_iter()
        .map(|(_, role, content, _)| OllamaMessage { role, content })
        .collect::<Vec<_>>();

    // Build system prompt including the page context if provided
    let system_content = {
        let mut s = String::from(
            "You are a helpful academic reading assistant embedded in a PDF reader called Scholium. \
             Answer questions clearly and concisely. Use markdown where it aids readability."
        );
        if let Some(ctx) = &page_context {
            if !ctx.trim().is_empty() {
                let page_num = page_number.unwrap_or(0);
                s.push_str(&format!(
                    "\n\nThe user is currently reading page {}. Here is the text content of that page:\n\n---\n{}\n---",
                    page_num, ctx.trim()
                ));
            }
        }
        s
    };

    let mut messages: Vec<OllamaMessage> = vec![
        OllamaMessage { role: "system".to_string(), content: system_content },
    ];
    // Append prior conversation turns
    messages.extend(history);

    let ollama_req = OllamaRequest {
        model: OLLAMA_MODEL.to_string(),
        messages,
        stream: true,
    };

    // Spawn streaming task
    let win = window.clone();
    let thread_id_clone = thread_id.clone();

    tauri::async_runtime::spawn(async move {
        let client = reqwest::Client::new();
        let res = match client.post(OLLAMA_URL).json(&ollama_req).send().await {
            Ok(r) => r,
            Err(e) => {
                let _ = win.emit("ai://error", serde_json::json!({
                    "thread_id": thread_id_clone,
                    "error": format!("Failed to reach Ollama: {}", e)
                }));
                return;
            }
        };

        let mut stream = res.bytes_stream();
        let mut full_reply = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[ollama] stream error: {}", e);
                    break;
                }
            };

            // Each chunk may contain one or more newline-delimited JSON objects
            let text = String::from_utf8_lossy(&chunk);
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() { continue; }

                if let Ok(parsed) = serde_json::from_str::<OllamaChunk>(line) {
                    if let Some(msg) = parsed.message {
                        let token = msg.content;
                        full_reply.push_str(&token);
                        let _ = win.emit("ai://token", serde_json::json!({
                            "thread_id": thread_id_clone,
                            "token": token
                        }));
                    }
                    if parsed.done.unwrap_or(false) {
                        break;
                    }
                }
            }
        }

        // Persist complete assistant reply
        let assistant_id = uuid::Uuid::new_v4().to_string();
        let now2 = chrono::Utc::now().timestamp();
        let _ = crate::db::insert_message(&assistant_id, &thread_id_clone, "assistant", &full_reply, now2);

        let _ = win.emit("ai://done", serde_json::json!({ "thread_id": thread_id_clone }));
    });

    Ok(user_msg)
}

#[tauri::command]
pub fn list_messages(thread_id: String) -> Result<Vec<Message>, String> {
    match crate::db::list_messages(&thread_id) {
        Ok(rows) => {
            let msgs = rows
                .into_iter()
                .map(|(id, role, content, created_at)| Message {
                    id,
                    thread_id: thread_id.clone(),
                    role,
                    content,
                    created_at,
                })
                .collect();
            Ok(msgs)
        }
        Err(e) => Err(format!("DB query failed: {}", e)),
    }
}
