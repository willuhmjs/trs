//! Metadata for trash operations

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;
use serde_json;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TrashItem {
    pub path: String,
    pub is_dir: bool,
}

/// Load metadata from file
pub fn load_metadata(metadata_file: &Path) -> io::Result<HashMap<String, String>> {
    if metadata_file.exists() {
        let content = fs::read_to_string(metadata_file)?;
        Ok(serde_json::from_str(&content).unwrap_or_default())
    } else {
        Ok(HashMap::new())
    }
}

/// Save metadata to file
pub fn save_metadata(metadata_file: &Path, metadata: &HashMap<String, String>) -> io::Result<()> {
    let content = serde_json::to_string(metadata)?;
    fs::write(metadata_file, content)?;
    Ok(())
}
