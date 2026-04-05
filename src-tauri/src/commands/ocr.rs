#[allow(unused_imports)]
use serde::{Deserialize, Serialize};

#[tauri::command]
pub async fn extract_vision_ocr(file_path: String, page_number: u32) -> Result<String, String> {
    let mut helper_path = std::env::current_dir().unwrap_or_default();
    
    // In dev mode, CWD is usually src-tauri. Check if bin/ocr-helper exists here.
    helper_path.push("bin");
    helper_path.push("ocr-helper");
    
    if !helper_path.exists() {
        // Try fallback for different dev contexts
        helper_path = std::env::current_dir().unwrap_or_default();
        helper_path.push("src-tauri");
        helper_path.push("bin");
        helper_path.push("ocr-helper");
    }

    if !helper_path.exists() {
        return Err(format!("Could not locate ocr-helper binary at {:?}", helper_path));
    }

    let output = std::process::Command::new(&helper_path)
        .arg(&file_path)
        .arg(page_number.to_string())
        .output()
        .map_err(|e| format!("Failed to spawn ocr-helper at {:?}: {}", helper_path, e))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("OCR helper failed: {}", err));
    }

    let json = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    Ok(json)
}
