use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct DbConn(pub Arc<Mutex<Connection>>);

pub fn open() -> Result<DbConn, rusqlite::Error> {
    let path = db_path();
    let conn = Connection::open(path)?;
    // WAL mode: readers don't block writers and vice-versa
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")?;
    ensure_schema(&conn)?;
    Ok(DbConn(Arc::new(Mutex::new(conn))))
}

fn db_path() -> PathBuf {
    // Use the platform app data dir in production; fall back to a temp dir in dev
    // so the DB never sits inside src-tauri/ where Tauri's file watcher can see it.
    if let Some(data_dir) = dirs::data_dir() {
        let app_dir = data_dir.join("scholium");
        let _ = std::fs::create_dir_all(&app_dir);
        app_dir.join("scholium.db")
    } else {
        std::env::temp_dir().join("scholium.db")
    }
}

fn ensure_schema(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS threads (
            id TEXT PRIMARY KEY,
            document_id TEXT NOT NULL,
            page_number INTEGER NOT NULL,
            anchor_start INTEGER,
            anchor_end INTEGER,
            anchor_quote TEXT NOT NULL,
            summary TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            thread_id TEXT NOT NULL,
            role TEXT NOT NULL CHECK(role IN ('user','assistant')),
            content TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );",
    )?;
    Ok(())
}

pub fn insert_message(
    conn: &Connection,
    id: &str,
    thread_id: &str,
    role: &str,
    content: &str,
    created_at: i64,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR IGNORE INTO messages (id, thread_id, role, content, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, thread_id, role, content, created_at],
    )?;
    Ok(())
}

pub fn list_messages(
    conn: &Connection,
    thread_id: &str,
) -> Result<Vec<(String, String, String, i64)>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, role, content, created_at FROM messages
         WHERE thread_id = ?1 ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map(params![thread_id], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn insert_thread(
    conn: &Connection,
    id: &str,
    document_id: &str,
    page_number: i64,
    anchor_start: Option<i64>,
    anchor_end: Option<i64>,
    anchor_quote: &str,
    summary: Option<&str>,
    created_at: i64,
    updated_at: i64,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO threads (id, document_id, page_number, anchor_start, anchor_end, anchor_quote, summary, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![id, document_id, page_number, anchor_start, anchor_end, anchor_quote, summary, created_at, updated_at],
    )?;
    Ok(())
}
