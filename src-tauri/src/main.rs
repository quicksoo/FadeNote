#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{Manager, Emitter};

// 配置结构
#[derive(Serialize, Deserialize)]
struct AppConfig {
    notes_directory: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            notes_directory: None,
        }
    }
}

// 从配置文件加载配置
fn load_config() -> Result<AppConfig, String> {
    // 获取应用安装目录
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("无法获取应用路径: {}", e))?;
    let app_dir = exe_path.parent()
        .ok_or("无法获取应用目录")?
        .to_path_buf();
    
    let config_file = app_dir.join("config.json");
    
    if config_file.exists() {
        let content = std::fs::read_to_string(&config_file)
            .map_err(|e| format!("读取配置文件失败: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("解析配置文件失败: {}", e))
    } else {
        Ok(AppConfig::default())
    }
}

// 保存配置到文件
fn save_config(config: &AppConfig) -> Result<(), String> {
    // 获取应用安装目录
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("无法获取应用路径: {}", e))?;
    let app_dir = exe_path.parent()
        .ok_or("无法获取应用目录")?
        .to_path_buf();
    
    let config_file = app_dir.join("config.json");
    let content = serde_json::to_string_pretty(config)
        .map_err(|e| format!("序列化配置失败: {}", e))?;
    std::fs::write(&config_file, content)
        .map_err(|e| format!("写入配置文件失败: {}", e))?;
    
    Ok(())
}

// 便签元数据结构
#[derive(Serialize, Deserialize, Clone)]
struct NoteMetadata {
    id: String,
    file: String,
    created_at: i64,
    updated_at: i64,
}

// 便签索引结构
#[derive(Serialize, Deserialize)]
struct NotesIndex {
    version: u32,
    notes: Vec<NoteMetadata>,
}

// 应用状态 - 存储用户设置的笔记目录
struct AppState {
    notes_directory: Mutex<Option<PathBuf>>,
}

// 显示原生目录选择对话框
#[tauri::command]
async fn show_directory_picker(_window: tauri::Window) -> Result<Option<String>, String> {
    // 使用 rfd (Rust File Dialog) crate 来实现目录选择
    // 因为Tauri v2的dialog插件可能不直接支持目录选择
    let dialog = rfd::FileDialog::new()
        .set_title("选择便签保存目录");
    
    let selected_path = dialog.pick_folder();

    match selected_path {
        Some(path) => Ok(Some(path.to_string_lossy().to_string())),
        None => Ok(None) // 用户取消了选择
    }
}

// 获取笔记保存目录（如果尚未设置，则提示用户选择）
#[tauri::command]
async fn ensure_notes_directory(window: tauri::Window) -> Result<String, String> {
    let app_state = window.state::<AppState>();
    let mut dir_lock = app_state.notes_directory.lock().unwrap();
    
    if let Some(ref dir) = *dir_lock {
        // 如果已在内存中缓存，直接返回
        Ok(dir.to_string_lossy().to_string())
    } else {
        // 尝试从配置文件加载
        let config = load_config()?;
        if let Some(dir_str) = config.notes_directory {
            let path = PathBuf::from(dir_str);
            if path.exists() {
                *dir_lock = Some(path.clone());
                return Ok(path.to_string_lossy().to_string());
            }
        }
        
        // 如果没有配置或目录不存在，返回错误让前端处理
        Err("需要用户选择保存目录".to_string())
    }
}

// 设置笔记保存目录
#[tauri::command]
async fn set_notes_directory(window: tauri::Window, directory: String) -> Result<String, String> {
    let path = std::path::PathBuf::from(directory);
    
    if !path.exists() {
        return Err("指定的目录不存在".to_string());
    }
    
    // 确保目录结构存在
    let notes_dir = path.join("notes");
    std::fs::create_dir_all(&notes_dir).map_err(|e| format!("创建notes目录失败: {}", e))?;
    
    // 更新内存中的缓存
    let app_state = window.state::<AppState>();
    let mut dir_lock = app_state.notes_directory.lock().unwrap();
    *dir_lock = Some(path.clone());
    
    // 保存到配置文件
    let mut config = load_config().unwrap_or_default();
    config.notes_directory = Some(path.to_string_lossy().to_string());
    save_config(&config).map_err(|e| format!("保存配置失败: {}", e))?;
    
    Ok(path.to_string_lossy().to_string())
}

// 获取当前笔记目录
#[tauri::command]
async fn get_notes_directory(window: tauri::Window) -> Result<Option<String>, String> {
    let app_state = window.state::<AppState>();
    let dir_lock = app_state.notes_directory.lock().unwrap();
    
    Ok(dir_lock.as_ref().map(|path| path.to_string_lossy().to_string()))
}

// 保存便签到文件
#[tauri::command]
async fn save_note(window: tauri::Window, id: String, content: String) -> Result<(), String> {
    // 确保目录已设置
    let dir_result = ensure_notes_directory(window).await;
    let notes_dir = match dir_result {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            // 如果目录未设置，返回错误让前端处理
            return Err("请先设置便签保存目录".to_string());
        }
    };

    // 确保notes子目录存在
    let notes_subdir = notes_dir.join("notes");
    std::fs::create_dir_all(&notes_subdir).map_err(|e| format!("创建notes子目录失败: {}", e))?;
    
    // 创建markdown文件
    let file_path = notes_subdir.join(format!("{}.md", id));
    std::fs::write(&file_path, content).map_err(|e| format!("写入文件失败: {}", e))?;
    
    // 更新索引
    update_notes_index(&notes_dir, &id, &file_path)?;
    
    Ok(())
}

// 读取便签内容
#[tauri::command]
async fn load_note(window: tauri::Window, id: String) -> Result<Option<String>, String> {
    // 确保目录已设置
    let dir_result = ensure_notes_directory(window).await;
    let notes_dir = match dir_result {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            // 如果目录未设置，返回None（表示还没有设置目录，也就没有保存的便签）
            return Ok(None);
        }
    };

    let file_path = notes_dir.join("notes").join(format!("{}.md", id));
    
    if file_path.exists() {
        let content = std::fs::read_to_string(&file_path).map_err(|e| format!("读取文件失败: {}", e))?;
        Ok(Some(content))
    } else {
        Ok(None)
    }
}

// 更新便签索引
fn update_notes_index(notes_dir: &Path, id: &str, file_path: &Path) -> Result<(), String> {
    let index_path = notes_dir.join("index.json");
    let mut index: NotesIndex = if index_path.exists() {
        let content = std::fs::read_to_string(&index_path).map_err(|e| format!("读取索引文件失败: {}", e))?;
        serde_json::from_str(&content).unwrap_or_else(|_| NotesIndex {
            version: 1,
            notes: Vec::new(),
        })
    } else {
        NotesIndex {
            version: 1,
            notes: Vec::new(),
        }
    };
    
    // 检查是否已存在该笔记
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("获取时间失败: {}", e))?
        .as_millis() as i64;
    
    if let Some(note) = index.notes.iter_mut().find(|n| n.id == id) {
        // 更新现有笔记
        note.updated_at = now;
    } else {
        // 添加新笔记
        let relative_path = pathdiff::diff_paths(file_path, notes_dir)
            .unwrap_or_else(|| PathBuf::from(file_path.file_name().unwrap_or_default()));
        
        let new_note = NoteMetadata {
            id: id.to_string(),
            file: relative_path.to_string_lossy().to_string(),
            created_at: now,
            updated_at: now,
        };
        index.notes.push(new_note);
    }
    
    // 写入索引文件
    let json_content = serde_json::to_string_pretty(&index).map_err(|e| format!("序列化索引失败: {}", e))?;
    std::fs::write(&index_path, json_content).map_err(|e| format!("写入索引文件失败: {}", e))?;
    
    Ok(())
}

// 新增创建窗口的命令
#[tauri::command]
async fn create_note_window(
    app_handle: tauri::AppHandle,
    label: String,
    title: String,
    width: u32,
    height: u32,
    x: Option<i32>,
    y: Option<i32>,
) -> Result<(), String> {
    let window = tauri::WebviewWindowBuilder::new(
        &app_handle,
        &label,
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title(&title)
    .inner_size(width as f64, height as f64)
    .resizable(true)
    .decorations(false)
    .transparent(false)
    .always_on_top(false)
    .visible(true);

    let _window = if let (Some(x_pos), Some(y_pos)) = (x, y) {
        window.position(x_pos as f64, y_pos as f64).build()
    } else {
        window.center().build()
    }.map_err(|e| e.to_string())?;

    Ok(())
}

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            notes_directory: Mutex::new(None),
        })
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            create_note_window,
            show_directory_picker,
            ensure_notes_directory,
            set_notes_directory,
            get_notes_directory,
            save_note,
            load_note
        ])
        .setup(|app| {
            // 应用启动时检查配置
            let window = app.get_webview_window("main").unwrap();
            
            // 检查是否已设置保存目录
            let config = load_config().unwrap_or_default();
            if config.notes_directory.is_none() {
                // 如果没有设置保存目录，发送事件通知前端
                println!("配置中未找到目录设置，发送request_set_directory事件");
                window.emit("request_set_directory", ()).unwrap();
            } else {
                println!("已找到目录设置: {:?}", config.notes_directory);
            }
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}