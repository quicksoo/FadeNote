use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use dirs::data_dir;
use uuid::Uuid;

pub fn get_app_data_dir() -> Result<PathBuf, String> {
    let mut app_data_dir = data_dir().ok_or("无法获取AppData目录")?;
    app_data_dir.push("FadeNote");
    Ok(app_data_dir)
}

pub fn write_file_safely(path: impl AsRef<Path>, content: impl AsRef<[u8]>) -> Result<(), String> {
    let path = path.as_ref();
    let parent = path.parent().ok_or_else(|| format!("invalid file path: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|e| format!("create parent directory failed {}: {}", parent.display(), e))?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("invalid file name: {}", path.display()))?;
    let temp_path = parent.join(format!(".{}.{}.tmp", file_name, Uuid::new_v4()));

    let write_result = (|| -> Result<(), String> {
        let mut temp_file = fs::File::create(&temp_path)
            .map_err(|e| format!("create temp file failed {}: {}", temp_path.display(), e))?;
        temp_file
            .write_all(content.as_ref())
            .map_err(|e| format!("write temp file failed {}: {}", temp_path.display(), e))?;
        temp_file
            .sync_all()
            .map_err(|e| format!("sync temp file failed {}: {}", temp_path.display(), e))?;
        drop(temp_file);

        let backup_path = if path.exists() {
            let backup_path = parent.join(format!(".{}.{}.bak", file_name, Uuid::new_v4()));
            fs::rename(path, &backup_path)
                .map_err(|e| format!("backup old file failed {}: {}", path.display(), e))?;
            Some(backup_path)
        } else {
            None
        };

        fs::rename(&temp_path, path)
            .map_err(|e| {
                if let Some(backup_path) = backup_path.as_ref() {
                    let _ = fs::rename(backup_path, path);
                }
                format!("replace file failed {}: {}", path.display(), e)
            })?;

        if let Some(backup_path) = backup_path {
            let _ = fs::remove_file(backup_path);
        }
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    write_result
}
