use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct AppInfo {
    pub name: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "rebuildAt")]
    pub rebuild_at: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct WindowInfo {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FileInfo {
    #[serde(rename = "relativePath")]
    pub relative_path: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct NoteEntry {
    pub id: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "lastActiveAt")]
    pub last_active_at: String,
    #[serde(rename = "expireAt")]
    pub expire_at: Option<String>,
    #[serde(rename = "cachedPreview")]
    pub cached_preview: Option<String>,
    pub status: String,
    #[serde(rename = "archivedAt")]
    pub archived_at: Option<String>,
    pub window: Option<WindowInfo>,
    pub pinned: bool,
    pub file: FileInfo,
}

#[derive(Serialize, Deserialize)]
pub struct IndexFile {
    pub version: u32,
    pub app: AppInfo,
    pub notes: Vec<NoteEntry>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ScheduleSettings {
    pub enabled: bool,
    pub time: String,
    pub recurrence: String,
    pub weekdays: Vec<u32>,
    #[serde(rename = "lastTriggeredKey")]
    pub last_triggered_key: Option<String>,
}

impl Default for ScheduleSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            time: "09:00".to_string(),
            recurrence: "daily".to_string(),
            weekdays: vec![1, 2, 3, 4, 5],
            last_triggered_key: None,
        }
    }
}
