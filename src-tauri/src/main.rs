#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Duration, Utc, Local};
use dirs::data_dir;
use serde::{Deserialize, Serialize};
use tauri::{Manager, menu::{MenuBuilder, MenuItem}, tray::TrayIconBuilder};
use uuid::Uuid;

// 获取AppData目录
fn get_app_data_dir() -> Result<PathBuf, String> {
    let mut app_data_dir = data_dir().ok_or("无法获取AppData目录")?;
    app_data_dir.push("FadeNote");
    Ok(app_data_dir)
}

// 检查是否为首次启动
// 条件：index.json不存在或为空，且notes目录下没有任何md文件
fn is_first_launch(app_data_dir: &Path) -> bool {
    let index_path = app_data_dir.join("index.json");
    let notes_path = app_data_dir.join("notes");
    
    // 如果index.json不存在，则可能是首次启动
    if !index_path.exists() {
        return true;
    }
    
    // 如果index.json存在但无法解析或为空，则是首次启动
    if let Ok(content) = std::fs::read_to_string(&index_path) {
        if content.trim().is_empty() {
            return true;
        }
        // 尝试解析index.json
        if let Ok(index_file) = serde_json::from_str::<IndexFile>(&content) {
            if index_file.notes.is_empty() {
                // 检查notes目录下是否有md文件
                if notes_path.exists() {
                    if let Ok(entries) = std::fs::read_dir(&notes_path) {
                        for entry in entries.flatten() {
                            if let Ok(file_type) = entry.file_type() {
                                if file_type.is_dir() {
                                    // 检查子目录中的md文件
                                    if let Ok(sub_entries) = std::fs::read_dir(entry.path()) {
                                        for sub_entry in sub_entries.flatten() {
                                            if let Ok(sub_file_type) = sub_entry.file_type() {
                                                if sub_file_type.is_file() {
                                                    if let Some(ext) = sub_entry.path().extension() {
                                                        if ext == "md" {
                                                            return false; // 找到md文件，不是首次启动
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else if file_type.is_file() {
                                    if let Some(ext) = entry.path().extension() {
                                        if ext == "md" {
                                            return false; // 找到md文件，不是首次启动
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                return true; // 没有找到任何md文件
            } else {
                return false; // index.json中有便签记录，不是首次启动
            }
        } else {
            return true; // 无法解析index.json，视为首次启动
        }
    }
    
    true // 默认视为首次启动
}

// 获取首次启动欢迎文案
fn get_welcome_content() -> String {
    "写点什么吧。

这张便签会自动保存。
关掉窗口，也不会立刻消失。

一段时间后，
它会悄悄淡出。

需要的时候，
可以从托盘里再叫回来。".to_string()
}

// V2规范的数据模型
#[derive(Serialize, Deserialize, Clone)]
struct AppInfo {
    name: String,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "rebuildAt")]
    rebuild_at: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct WindowInfo {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Serialize, Deserialize, Clone)]
struct FileInfo {
    #[serde(rename = "relativePath")]
    relative_path: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct NoteEntry {
    id: String,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "lastActiveAt")]
    last_active_at: String,
    #[serde(rename = "expireAt")]
    expire_at: Option<String>,
    #[serde(rename = "cachedPreview")]
    cached_preview: Option<String>,
    status: String,
    #[serde(rename = "archivedAt")]
    archived_at: Option<String>,
    window: Option<WindowInfo>,
    pinned: bool,  // 是否固定，固定便签不会过期
    file: FileInfo,
}

#[derive(Serialize, Deserialize)]
struct IndexFile {
    version: u32,
    app: AppInfo,
    notes: Vec<NoteEntry>,
}

// 应用状态
struct AppState {
    notes_directory: Mutex<Option<PathBuf>>,
}

// 获取当前ISO 8601时间戳
fn get_current_iso8601_time() -> String {
    Local::now().to_rfc3339()
}

// Fix 1: 引入「Domain Query 层」（纯判断）
// 判断便签是否已归档
fn is_archived(entry: &NoteEntry) -> bool {
    entry.archived_at.is_some()
}

// 判断便签是否过期
fn is_expired_check(entry: &NoteEntry, now: &DateTime<Local>) -> bool {
    // 如果便签被固定，则永远不会过期
    if entry.pinned {
        return false;
    }
    
    match &entry.expire_at {
        Some(time_str) => {
            match DateTime::parse_from_rfc3339(time_str) {
                Ok(expire_time) => *now > expire_time.naive_local().and_local_timezone(Local).unwrap(),
                Err(_) => false, // 如果无法解析时间，默认不过期
            }
        },
        None => false, // 如果没有过期时间，则认为不过期
    }
}

// 判断便签是否活跃
fn is_active(entry: &NoteEntry) -> bool {
    entry.archived_at.is_none()
}



// Fix 2: archive_note 作为唯一状态迁移入口
fn archive_note(entry: &mut NoteEntry, now: &DateTime<Local>) -> Result<(), String> {
    // 只更新entry的归档状态和过期时间
    entry.archived_at = Some(now.to_rfc3339());
    entry.expire_at = None; // 归档后不再需要过期时间

    Ok(())
}

// 派生状态字段
fn derive_status(entry: &mut NoteEntry) {
    entry.status = if entry.archived_at.is_some() {
        "archived".to_string()
    } else {
        "active".to_string()
    };
}

// RULE: lifecycle mutation only here
// Fix 3: 新增明确的生命周期阶段 —— expire pass
fn apply_expire_pass(index: &mut IndexFile, now: &DateTime<Local>) {
    for entry in index.notes.iter_mut() {
        if entry.archived_at.is_none() && is_expired_check(entry, now) {
            // 调用唯一的归档入口
            if let Err(e) = archive_note(entry, now) {
                eprintln!("归档便签 {} 失败: {}", entry.id, e);
                // 即使归档失败也标记为已归档，避免重复尝试
                entry.archived_at = Some(now.to_rfc3339());
            }
        }
    }
}

// Fix 5: 重建索引 - 不得重置生命周期
fn rebuild_index(notes_dir: &Path) -> Result<IndexFile, String> {
    let index_path = notes_dir.join("index.json");
    
    // 加载现有的索引以保留状态信息
    let mut existing_entries_map: std::collections::HashMap<String, NoteEntry> = std::collections::HashMap::new();
    let old_index: Option<IndexFile> = if index_path.exists() {
        if let Ok(content) = fs::read_to_string(&index_path) {
            if let Ok(existing_index) = serde_json::from_str::<IndexFile>(&content) {
                for entry in &existing_index.notes {
                    existing_entries_map.insert(entry.id.clone(), entry.clone());
                }
                Some(existing_index)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };
    
    // 创建新的V2索引 - 这是重建操作，需要设置rebuildAt
    let app_created_at = old_index
        .as_ref()
        .map(|i| i.app.created_at.clone())
        .unwrap_or_else(get_current_iso8601_time);
    
    let mut index = IndexFile {
        version: 2,
        app: AppInfo {
            name: "FadeNote".to_string(),
            created_at: app_created_at,
            rebuild_at: Some(get_current_iso8601_time()), // 仅在重建时设置rebuildAt
        },
        notes: Vec::new(),
    };

    // 扫描notes目录下的所有文件并添加到索引中
    let notes_path = notes_dir.join("notes");
    if notes_path.exists() {
        scan_directory_for_notes_rebuild(notes_dir, &mut index, &notes_path, &existing_entries_map)?;
    }

    // 派生所有条目的状态
    for entry in &mut index.notes {
        derive_status(entry);
    }
    
    // 保存重建后的索引
    let json_content = serde_json::to_string_pretty(&index)
        .map_err(|e| format!("序列化索引失败: {}", e))?;
    fs::write(&index_path, json_content)
        .map_err(|e| format!("写入索引文件失败: {}", e))?;

    Ok(index)
}

// 扫描目录中的便签文件用于重建 - 递归辅助函数
fn scan_directory_for_notes_rebuild_recursive(notes_dir: &Path, index: &mut IndexFile, scan_path: &Path, existing_entries: &std::collections::HashMap<String, NoteEntry>) -> Result<(), String> {
    for entry in fs::read_dir(scan_path).map_err(|e| format!("读取目录失败: {}", e))? {
        let entry = entry.map_err(|e| format!("遍历文件失败: {}", e))?;
        let path = entry.path();
        
        if path.is_file() && path.extension().map_or(false, |ext| ext == "md") {
            // 解析文件内容获取ID和其他信息
            if let Ok(content) = fs::read_to_string(&path) {
                if let Some(parsed_id) = parse_id_from_content(&content) {
                    let metadata = path.metadata().map_err(|e| format!("获取文件元数据失败: {}", e))?;
                    let created_time = DateTime::<Utc>::from(metadata.created()
                        .map_err(|e| format!("获取创建时间失败: {}", e))?);
                    
                    let relative_path = path.strip_prefix(notes_dir)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string();
                    
                    // 从现有条目中获取状态信息，如果不存在则为新条目设置默认值
                    let (archived_at, expire_at, created_at, last_active_at) = if let Some(existing_entry) = existing_entries.get(&parsed_id) {
                        (
                            existing_entry.archived_at.clone(),
                            existing_entry.expire_at.clone(), // 保留现有expireAt
                            existing_entry.created_at.clone(), // 保留原始创建时间
                            existing_entry.last_active_at.clone(), // 保留上次活跃时间
                        )
                    } else {
                        (
                            None, // 如果是新文件，archived_at为None
                            None, // ❗ rebuild 不生成 expire
                            created_time.to_rfc3339(), // 使用文件创建时间
                            created_time.to_rfc3339(), // 初始last_active_at就是创建时间
                        )
                    };
                    
                    let mut new_entry = NoteEntry {
                        id: parsed_id.clone(),
                        created_at,
                        last_active_at,
                        expire_at,
                        cached_preview: None,
                        status: String::new(), // 禁止手写，将在派生时设置
                        archived_at,
                        window: None,    // 重建时所有window都是null
                        pinned: false,  // 默认不固定
                        file: FileInfo {
                            relative_path,
                        },
                    };
                    
                    // 重建索引时应该保留所有note，无论是否活跃
                    index.notes.push(new_entry);
                    println!("重建时添加note到索引: {}", parsed_id);
                }
            }
        } else if path.is_dir() {
            // 递归扫描子目录
            scan_directory_for_notes_rebuild_recursive(notes_dir, index, &path, existing_entries)?;
        }
    }
    
    Ok(())
}

// 扫描目录中的便签文件用于重建
fn scan_directory_for_notes_rebuild(notes_dir: &Path, index: &mut IndexFile, scan_path: &Path, existing_entries: &std::collections::HashMap<String, NoteEntry>) -> Result<(), String> {
    scan_directory_for_notes_rebuild_recursive(notes_dir, index, scan_path, existing_entries)
}

// 规范化索引 - 修正非法状态
fn normalize_index(mut index: IndexFile) -> IndexFile {
    // archived=true 的 note 不得出现在桌面
    // 这意味着这些note不应该被当作活跃的便签处理
    // 我们保留它们在索引中，但它们不会在正常操作中被使用
    
    // window=null 的 note 不创建窗口
    // 在窗口恢复逻辑中已经处理了这一点
    
    // 修正非法字段值
    for entry in &mut index.notes {
        // 确保ID有效
        if entry.id.is_empty() {
            entry.id = Uuid::new_v4().to_string();
        }
        
        // 确保时间字段格式正确
        if entry.created_at.is_empty() {
            entry.created_at = get_current_iso8601_time();
        }
        
        if entry.last_active_at.is_empty() {
            entry.last_active_at = get_current_iso8601_time();
        }
        
        // 确保文件路径有效
        if entry.file.relative_path.is_empty() {
            entry.file.relative_path = format!("notes/unknown/{}.md", entry.id);
        }
        
        // 确保pinned字段存在（默认为false）
        // 注意：这里不需要设置默认值，因为我们已经在创建NoteEntry时设置了
    }
    
    index
}

// 验证并修复索引
fn validate_and_fix_index(notes_dir: &Path) -> Result<IndexFile, String> {
    let index_path = notes_dir.join("index.json");
    let mut index: IndexFile = if index_path.exists() {
        let content = fs::read_to_string(&index_path)
            .map_err(|e| format!("读取索引文件失败: {}", e))?;
        match serde_json::from_str::<IndexFile>(&content) {
            Ok(parsed_index) => parsed_index,
            Err(_) => {
                // 如果解析失败，执行重建
                println!("索引文件解析失败，执行重建...");
                return rebuild_index(notes_dir);
            }
        }
    } else {
        // 如果不存在，执行重建
        println!("索引文件不存在，执行重建...");
        return rebuild_index(notes_dir);
    };

    // 保留原有的rebuildAt值，不进行修改（V2规范：普通启动/更新禁止写入rebuildAt）
    let original_rebuild_at = index.app.rebuild_at.clone();

    // 不再检查文件是否存在，保留所有条目
    // 文件不会被移动，所以不需要检查文件是否存在

    // 扫描notes目录下的所有文件，仅添加当前索引中不存在的文件
    // 避免重复添加已存在的便签
    let current_index_ids: std::collections::HashSet<String> = index.notes.iter().map(|note| note.id.clone()).collect();
    
    let notes_path = notes_dir.join("notes");
    if notes_path.exists() {
        scan_directory_for_notes(notes_dir, &mut index, &notes_path, &current_index_ids)?;
    }



    // 应用过期检查
    let now = Local::now();
    apply_expire_pass(&mut index, &now);
    
    // 应用规范化规则
    index = normalize_index(index);

    // 恢复原始的rebuildAt值，确保不会在普通更新时修改它
    index.app.rebuild_at = original_rebuild_at;

    // 派生所有条目的状态（在函数结束前统一派生）
    for entry in &mut index.notes {
        derive_status(entry);
    }
    
    // 保存更新后的索引
    let json_content = serde_json::to_string_pretty(&index)
        .map_err(|e| format!("序列化索引失败: {}", e))?;
    fs::write(&index_path, json_content)
        .map_err(|e| format!("写入索引文件失败: {}", e))?;

    Ok(index)
}

// 扫描目录中的便签文件 - 递归辅助函数
fn scan_directory_for_notes_recursive(notes_dir: &Path, index: &mut IndexFile, scan_path: &Path, existing_ids: &mut std::collections::HashSet<String>) -> Result<(), String> {
    // 加载现有的索引以保留状态信息
    let index_path = notes_dir.join("index.json");
    let mut existing_entries_map: std::collections::HashMap<String, NoteEntry> = std::collections::HashMap::new();
    if index_path.exists() {
        if let Ok(content) = fs::read_to_string(&index_path) {
            if let Ok(existing_index) = serde_json::from_str::<IndexFile>(&content) {
                for entry in existing_index.notes {
                    existing_entries_map.insert(entry.id.clone(), entry);
                }
            }
        }
    }
    
    scan_directory_for_notes_recursive_with_existing(notes_dir, index, scan_path, existing_ids, &existing_entries_map)
}

// 扫描目录中的便签文件 - 递归辅助函数（实际实现）
fn scan_directory_for_notes_recursive_with_existing(
    notes_dir: &Path, 
    index: &mut IndexFile, 
    scan_path: &Path, 
    existing_ids: &mut std::collections::HashSet<String>,
    existing_entries: &std::collections::HashMap<String, NoteEntry>
) -> Result<(), String> {
    for entry in fs::read_dir(scan_path).map_err(|e| format!("读取目录失败: {}", e))? {
        let entry = entry.map_err(|e| format!("遍历文件失败: {}", e))?;
        let path = entry.path();
        
        if path.is_file() && path.extension().map_or(false, |ext| ext == "md") {
            // 解析文件内容获取ID和其他信息
            if let Ok(content) = fs::read_to_string(&path) {
                if let Some(parsed_id) = parse_id_from_content(&content) {
                    // 检查这个ID是否已在索引中，如果不在则添加
                    if !existing_ids.contains(&parsed_id) {
                        let metadata = path.metadata().map_err(|e| format!("获取文件元数据失败: {}", e))?;
                        let created_time = DateTime::<Utc>::from(metadata.created()
                            .map_err(|e| format!("获取创建时间失败: {}", e))?);
                        
                        let relative_path = path.strip_prefix(notes_dir)
                            .unwrap_or(&path)
                            .to_string_lossy()
                            .to_string();
                        
                        // 从现有条目中获取状态信息，如果不存在则为新条目设置默认值
                        let (archived_at, expire_at) = if let Some(existing_entry) = existing_entries.get(&parsed_id) {
                            (existing_entry.archived_at.clone(), existing_entry.expire_at.clone())
                        } else {
                            // 新文件：扫描时不设置过期时间，archived_at为None
                            (None, None)
                        };
                        
                        let mut new_entry = NoteEntry {
                            id: parsed_id.clone(), // 修复：clone值以避免移动
                            created_at: created_time.to_rfc3339(),
                            last_active_at: created_time.to_rfc3339(),
                            expire_at,
                            cached_preview: None,
                            status: String::new(), // 禁止手写，将在派生时设置
                            archived_at,
                            window: Some(WindowInfo {
                                x: 100.0,
                                y: 100.0,
                                width: 280.0,
                                height: 360.0,
                            }),
                            pinned: false,  // 默认不固定
                            file: FileInfo {
                                relative_path,
                            },
                        };
                        
                        // 添加note到索引中（扫描时保留所有note，不管是否活跃）
                        index.notes.push(new_entry);
                        existing_ids.insert(parsed_id.clone()); // 添加到已知ID集合
                        println!("添加新发现的note到索引: {}", parsed_id); // 修复：使用克隆的值
                    }
                }
            }
        } else if path.is_dir() {
            // 递归扫描子目录
            scan_directory_for_notes_recursive_with_existing(notes_dir, index, &path, existing_ids, existing_entries)?;
        }
    }
    
    Ok(())
}

// 扫描目录中的便签文件
fn scan_directory_for_notes(notes_dir: &Path, index: &mut IndexFile, scan_path: &Path, current_index_ids: &std::collections::HashSet<String>) -> Result<(), String> {
    // 使用传入的当前索引ID集合，避免重复添加
    let mut existing_ids = current_index_ids.clone();

    scan_directory_for_notes_recursive(notes_dir, index, scan_path, &mut existing_ids)
}

// 从文件内容解析ID
fn parse_id_from_content(content: &str) -> Option<String> {
    // 查找Front Matter中的id
    let lines: Vec<&str> = content.lines().collect();
    let mut in_front_matter = false;
    
    for line in &lines {
        if line.trim() == "---" {
            if !in_front_matter {
                in_front_matter = true;
            } else {
                break; // 结束front matter
            }
        } else if in_front_matter {
            if line.starts_with("id:") {
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                if parts.len() == 2 {
                    return Some(parts[1].trim().to_string());
                }
            }
        }
    }
    
    None
}

// 提取纯文本内容（去除Front Matter）
fn extract_content_only(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    
    let mut content_start = 0;
    
    // 循环处理可能存在的多个Front Matter块
    while content_start < lines.len() {
        // 寻找Front Matter的开始（---）
        if let Some(start_idx) = lines[content_start..].iter().position(|line| line.trim() == "---") {
            let actual_start_idx = content_start + start_idx;
            
            // 从开始位置之后寻找Front Matter的结束（下一个 ---）
            if let Some(end_idx) = lines[actual_start_idx + 1..].iter().position(|line| line.trim() == "---") {
                let actual_end_idx = actual_start_idx + 1 + end_idx;
                
                // 检查这个 --- 块之间是否包含标准的id和createdAt字段
                let mut found_id = false;
                let mut found_created_at = false;
                
                for i in actual_start_idx + 1..actual_end_idx {
                    let line = lines[i].trim();
                    if line.starts_with("id:") {
                        found_id = true;
                    } else if line.starts_with("createdAt:") {
                        found_created_at = true;
                    }
                }
                
                // 只有当找到标准的id和createdAt字段时，才认为这是Front Matter
                if found_id && found_created_at {
                    // 跳过这个Front Matter块和紧接着的空行（如果有的话）
                    content_start = if actual_end_idx + 1 < lines.len() && lines[actual_end_idx + 1].is_empty() {
                        actual_end_idx + 2
                    } else {
                        actual_end_idx + 1
                    };
                    
                    // 继续循环，检查是否还有更多的Front Matter
                    continue;
                }
            }
        }
        
        // 如果没有找到更多有效的Front Matter，返回剩余内容
        break;
    }
    
    // 返回从content_start开始的剩余内容
    if content_start < lines.len() {
        lines[content_start..].join("\n")
    } else {
        String::new() // 如果content_start超出了lines范围，返回空字符串
    }
}

// 构建带Front Matter的完整内容
fn build_full_content(id: &str, created_at: &str, content: &str) -> String {
    format!(
        "---\nid: {}\ncreatedAt: {}\n---\n{}",
        id, created_at, content
    )
}

// 初始化便签目录结构
#[tauri::command]
async fn initialize_notes_directory(window: tauri::WebviewWindow) -> Result<String, String> {
    // 使用AppData目录而不是让用户选择
    let app_data_dir = get_app_data_dir()?;
    fs::create_dir_all(&app_data_dir).map_err(|e| format!("创建AppData目录失败: {}", e))?;

    let notes_dir = app_data_dir.join("notes");
    fs::create_dir_all(&notes_dir).map_err(|e| format!("创建notes目录失败: {}", e))?;

    // 更新应用状态
    let app_state = window.state::<AppState>();
    {
        let mut dir_lock = app_state.notes_directory.lock().unwrap();
        *dir_lock = Some(app_data_dir.clone());
    }

    // 验证并修复索引
    validate_and_fix_index(&app_data_dir)?;

    Ok(app_data_dir.to_string_lossy().to_string())
}

// 获取笔记保存目录
#[tauri::command]
async fn ensure_notes_directory(window: tauri::WebviewWindow) -> Result<String, String> {
    let app_state = window.state::<AppState>();
    let dir_option = {
        let dir_lock = app_state.notes_directory.lock().unwrap();
        dir_lock.clone()
    }; // 在异步操作之前释放锁

    if let Some(ref dir) = dir_option {
        Ok(dir.to_string_lossy().to_string())
    } else {
        // 初始化目录
        initialize_notes_directory(window).await
    }
}

// 获取活跃的便签列表（非归档的便签）
#[tauri::command]
async fn get_active_notes(window: tauri::WebviewWindow) -> Result<Vec<NoteEntry>, String> {
    get_all_active_notes(window).await
}

// 获取所有活跃的便签
#[tauri::command]
async fn get_all_active_notes(window: tauri::WebviewWindow) -> Result<Vec<NoteEntry>, String> {
    let notes_dir = PathBuf::from(ensure_notes_directory(window).await?);
    let index = validate_and_fix_index(&notes_dir)?;

    let mut active_notes = Vec::new();
    for entry in &index.notes {
        if is_active(entry) {
            active_notes.push(entry.clone());
        }
    }

    Ok(active_notes)
}

// 获取所有归档的便签
#[tauri::command]
async fn get_archived_notes(window: tauri::WebviewWindow) -> Result<Vec<NoteEntry>, String> {
    let notes_dir = PathBuf::from(ensure_notes_directory(window).await?);
    let index = validate_and_fix_index(&notes_dir)?;

    let mut archived_notes = Vec::new();
    for entry in &index.notes {
        if !is_active(entry) {  // 归档的便签是不活跃的
            archived_notes.push(entry.clone());
        }
    }

    Ok(archived_notes)
}

// 获取存在但当前没有窗口的便签（即隐藏的便签）
#[tauri::command]
async fn get_notes_without_windows(window: tauri::WebviewWindow) -> Result<Vec<NoteEntry>, String> {
    // 克隆window以便后面使用
    let window_clone = window.clone();
    let app_handle = window.app_handle().clone();
    let all_windows = app_handle.webview_windows();
    
    let notes_dir = PathBuf::from(ensure_notes_directory(window_clone).await?);
    let index = validate_and_fix_index(&notes_dir)?;
    
    let mut hidden_notes = Vec::new();
    for entry in &index.notes {
        if is_active(entry) && entry.window.is_some() {  // 活跃且应该有窗口
            let label = format!("note-{}", entry.id);
            
            // 检查该标签的窗口是否存在
            if let Some(note_window) = all_windows.get(&label) {
                // 检查窗口是否可见
                if let Ok(is_visible) = note_window.is_visible() {
                    if !is_visible {
                        // 窗口存在但不可见，需要恢复
                        hidden_notes.push(entry.clone());
                    }
                } else {
                    // 如果无法获取可见性状态，也认为是隐藏的
                    hidden_notes.push(entry.clone());
                }
            } else {
                // 窗口不存在，需要创建
                hidden_notes.push(entry.clone());
            }
        } else if is_active(entry) && entry.window.is_none() {  // 活跃但没有窗口配置
            hidden_notes.push(entry.clone());
        }
    }

    Ok(hidden_notes)
}

// 恢复没有窗口的便签（为它们创建窗口）
#[tauri::command]
async fn restore_notes_without_windows(window: tauri::WebviewWindow) -> Result<(), String> {
    let notes_without_windows = get_notes_without_windows(window.clone()).await?;
    
    let app_handle = window.app_handle().clone();
    for note in notes_without_windows {
        // 为便签创建默认窗口位置
        let default_x = 100.0 + (note.id.as_bytes()[0] as f64 * 20.0) % 200.0;
        let default_y = 100.0 + (note.id.as_bytes()[1] as f64 * 20.0) % 200.0;
        
        let window_info = note.window.unwrap_or(WindowInfo {
            x: default_x,
            y: default_y,
            width: 280.0,
            height: 360.0,
        });
        
        let label = format!("note-{}", note.id);
        let _ = create_note_window(
            app_handle.clone(),
            label,
            "FadeNote".to_string(),
            window_info.width as u32,
            window_info.height as u32,
            Some(window_info.x as i32),
            Some(window_info.y as i32),
        ).await;
    }
    
    Ok(())
}

// 创建新的便签
#[tauri::command]
async fn create_note(window: tauri::WebviewWindow, x: f64, y: f64, width: f64, height: f64) -> Result<String, String> {
    let notes_dir = PathBuf::from(ensure_notes_directory(window).await?);
    
    // 生成UUID作为ID
    let id = Uuid::new_v4().to_string();
    
    // 创建时间信息
    let created_at = get_current_iso8601_time();
    let expires_at = (DateTime::parse_from_rfc3339(&created_at)
        .map_err(|e| format!("解析时间失败: {}", e))?
        .naive_utc()
        .and_local_timezone(Utc)
        .unwrap() + Duration::days(7)).to_rfc3339();
    
    // 创建文件内容
    let content = build_full_content(&id, &created_at, "");
    
    // 创建按日期组织的目录结构
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let dated_dir = notes_dir.join("notes").join(today);
    fs::create_dir_all(&dated_dir).map_err(|e| format!("创建日期目录失败: {}", e))?;

    // 创建文件
    let file_path = dated_dir.join(format!("{}.md", id));
    fs::write(&file_path, content).map_err(|e| format!("创建便签文件失败: {}", e))?;

    // 更新索引
    let index_path = notes_dir.join("index.json");
    let mut index: IndexFile = if index_path.exists() {
        let content = fs::read_to_string(&index_path)
            .map_err(|e| format!("读取索引文件失败: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("解析索引文件失败: {}", e))?
    } else {
        IndexFile {
            version: 2,
            app: AppInfo {
                name: "FadeNote".to_string(),
                created_at: get_current_iso8601_time(),
                rebuild_at: None,
            },
            notes: Vec::new(),
        }
    };

    let rel_path = file_path.strip_prefix(&notes_dir)
        .unwrap_or(&file_path)
        .to_string_lossy()
        .to_string();

    let mut new_entry = NoteEntry {
        id: id.clone(),
        created_at: created_at.clone(),
        last_active_at: created_at.clone(), // 初始last_active_at就是创建时间
        expire_at: Some(expires_at.clone()),
        cached_preview: None,
        status: String::new(), // 禁止手写，将在派生时设置
        archived_at: None,
        window: Some(WindowInfo {
            x,
            y,
            width,
            height,
        }),
        pinned: false,  // 默认不固定
        file: FileInfo {
            relative_path: rel_path,
        },
    };
    
    // 派生状态
    derive_status(&mut new_entry);
    
    index.notes.push(new_entry);

    let json_content = serde_json::to_string_pretty(&index)
        .map_err(|e| format!("序列化索引失败: {}", e))?;
    std::fs::write(&index_path, json_content)
        .map_err(|e| format!("写入索引文件失败: {}", e))?;

    Ok(id)
}

// 读取便签内容
#[tauri::command]
async fn load_note(window: tauri::WebviewWindow, id: String) -> Result<Option<String>, String> {
    let notes_dir = PathBuf::from(ensure_notes_directory(window).await?);
    
    let index_path = notes_dir.join("index.json");
    if !index_path.exists() {
        return Ok(None);
    }

    let index: IndexFile = {
        let content = fs::read_to_string(&index_path)
            .map_err(|e| format!("读取索引文件失败: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("解析索引文件失败: {}", e))?
    };

    // 在索引中查找该ID的便签
    let note = index.notes.iter().find(|note| note.id == id);
    
    if let Some(entry) = note {
        if !is_active(entry) {
            return Ok(None);
        }
        let file_path = notes_dir.join(&entry.file.relative_path);
        if file_path.exists() {
            let full_content = fs::read_to_string(&file_path)
                .map_err(|e| format!("读取便签文件失败: {}", e))?;
            let pure_content = extract_content_only(&full_content);
            Ok(Some(pure_content))
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

// 更新便签的活动时间
#[tauri::command]
async fn update_note_activity(window: tauri::WebviewWindow, id: String) -> Result<(), String> {
    let notes_dir = PathBuf::from(ensure_notes_directory(window).await?);
    
    // 从索引中获取文件路径
    let index_path = notes_dir.join("index.json");
    if !index_path.exists() {
        return Err("索引文件不存在".to_string());
    }

    let mut index: IndexFile = {
        let content = fs::read_to_string(&index_path)
            .map_err(|e| format!("读取索引文件失败: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("解析索引文件失败: {}", e))?
    };

    // 查找并更新指定ID的便签
    if let Some(entry) = index.notes.iter_mut().find(|note| note.id == id) {
        if !is_active(entry) {
            return Err("note archived".to_string());
        }
        // 更新last_active_at和expire_at
        let now = get_current_iso8601_time();
        entry.last_active_at = now.clone();
        
        // 计算新的过期时间：当前时间 + 7天
        let current_time = DateTime::parse_from_rfc3339(&now)
            .map_err(|e| format!("解析当前时间失败: {}", e))?;
        let new_expire_time = (current_time.naive_local()
            .and_local_timezone(Local)
            .unwrap() + Duration::days(7)).to_rfc3339();
        entry.expire_at = Some(new_expire_time);

        // 保存更新后的索引
        index.app.name = "FadeNote".to_string(); // 确保app信息存在
        // 不修改rebuildAt字段（V2规范：普通启动/更新禁止写入rebuildAt）
        let json_content = serde_json::to_string_pretty(&index)
            .map_err(|e| format!("序列化索引失败: {}", e))?;
        fs::write(&index_path, json_content)
            .map_err(|e| format!("写入索引文件失败: {}", e))?;

        Ok(())
    } else {
        Err("找不到指定的便签".to_string())
    }
}

// 恢复便签 - 统一入口
fn internal_restore_note(entry: &mut NoteEntry, now: &DateTime<Local>) {
    entry.archived_at = None;
    entry.last_active_at = now.to_rfc3339();
    let new_expire_time = now.with_timezone(&chrono::Utc) + Duration::days(7);
    entry.expire_at = Some(new_expire_time.to_rfc3339());
}

// 设置便签固定状态
#[tauri::command]
async fn set_note_pinned(window: tauri::WebviewWindow, id: String, pinned: bool) -> Result<(), String> {
    let notes_dir = PathBuf::from(ensure_notes_directory(window).await?);
    
    // 从索引中获取文件路径
    let index_path = notes_dir.join("index.json");
    if !index_path.exists() {
        return Err("索引文件不存在".to_string());
    }

    let mut index: IndexFile = {
        let content = fs::read_to_string(&index_path)
            .map_err(|e| format!("读取索引文件失败: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("解析索引文件失败: {}", e))?
    };

    // 查找并更新指定ID的便签
    if let Some(entry) = index.notes.iter_mut().find(|note| note.id == id) {
        entry.pinned = pinned;
        
        // 保存更新后的索引
        index.app.name = "FadeNote".to_string(); // 确保app信息存在
        // 不修改rebuildAt字段（V2规范：普通启动/更新禁止写入rebuildAt）
        let json_content = serde_json::to_string_pretty(&index)
            .map_err(|e| format!("序列化索引失败: {}", e))?;
        fs::write(&index_path, json_content)
            .map_err(|e| format!("写入索引文件失败: {}", e))?;

        Ok(())
    } else {
        Err("找不到指定的便签".to_string())
    }
}

// 恢复归档的便签
#[tauri::command]
async fn restore_note(window: tauri::WebviewWindow, id: String) -> Result<(), String> {
    let notes_dir = PathBuf::from(ensure_notes_directory(window).await?);
    
    // 从索引中获取文件路径
    let index_path = notes_dir.join("index.json");
    if !index_path.exists() {
        return Err("索引文件不存在".to_string());
    }

    let mut index: IndexFile = {
        let content = fs::read_to_string(&index_path)
            .map_err(|e| format!("读取索引文件失败: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("解析索引文件失败: {}", e))?
    };

    // 查找并恢复指定ID的便签
    if let Some(entry) = index.notes.iter_mut().find(|note| note.id == id) {
        if entry.archived_at.is_some() {
            let now = Local::now();
            internal_restore_note(entry, &now);
        }

        // 保存更新后的索引
        index.app.name = "FadeNote".to_string(); // 确保app信息存在
        // 不修改rebuildAt字段（V2规范：普通启动/更新禁止写入rebuildAt）
        let json_content = serde_json::to_string_pretty(&index)
            .map_err(|e| format!("序列化索引失败: {}", e))?;
        fs::write(&index_path, json_content)
            .map_err(|e| format!("写入索引文件失败: {}", e))?;

        Ok(())
    } else {
        Err("找不到指定的便签".to_string())
    }
}

// 保存便签内容
#[tauri::command]
async fn save_note_content(window: tauri::WebviewWindow, id: String, content: String) -> Result<(), String> {
    let notes_dir = PathBuf::from(ensure_notes_directory(window).await?);
    
    
    
    // 从索引中获取文件路径
    let index_path = notes_dir.join("index.json");
    if !index_path.exists() {
        return Err("索引文件不存在".to_string());
    }

    let mut index: IndexFile = {
        let content_str = fs::read_to_string(&index_path)
            .map_err(|e| format!("读取索引文件失败: {}", e))?;
        serde_json::from_str(&content_str)
            .map_err(|e| format!("解析索引文件失败: {}", e))?
    };

    // 查找并更新活动时间
    if let Some(update_entry) = index.notes.iter_mut().find(|note| note.id == id) {
        if !is_active(update_entry) {
            return Err("便签已被归档，无法更新".to_string());
        }
        
        let file_path = notes_dir.join(&update_entry.file.relative_path);
        
        if !file_path.exists() {
            return Err("便签文件不存在".to_string());
        }

        // 读取现有Front Matter信息
        let existing_content = fs::read_to_string(&file_path)
            .unwrap_or_default();
        
        // 提取Front Matter中的ID和创建时间
        let existing_id = if let Some(parsed_id) = parse_id_from_content(&existing_content) {
            parsed_id
        } else {
            return Err("无法从文件中解析ID".to_string());
        };
        
        // 保留原始的创建时间
        let created_at = extract_created_at_from_content(&existing_content)
            .unwrap_or_else(|| get_current_iso8601_time());

        // 构建新内容
        let full_content = build_full_content(&existing_id, &created_at, &content);

        // 写入文件
        fs::write(&file_path, full_content)
            .map_err(|e| format!("写入便签文件失败: {}", e))?;

        // 更新活动时间
        let now = get_current_iso8601_time();
        update_entry.last_active_at = now.clone();
        
        // 计算新的过期时间：当前时间 + 7天
        let current_time = DateTime::parse_from_rfc3339(&now)
            .map_err(|e| format!("解析当前时间失败: {}", e))?;
        let new_expire_time = (current_time.naive_local()
            .and_local_timezone(Local)
            .unwrap() + Duration::days(7)).to_rfc3339();
        update_entry.expire_at = Some(new_expire_time);
        
        // 更新cachedPreview：从内容中提取第一行作为预览
        update_entry.cached_preview = extract_first_line_preview(&content);
        
        // 保存更新后的索引
        let json_content = serde_json::to_string_pretty(&index)
            .map_err(|e| format!("序列化索引失败: {}", e))?;
        fs::write(&index_path, json_content)
            .map_err(|e| format!("写入索引文件失败: {}", e))?;

        Ok(())
    } else {
        Err("找不到指定的便签".to_string())
    }
}

// 提取内容预览：从内容中提取第一行作为预览
fn extract_first_line_preview(content: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    
    // 跳过空行，找到第一个非空行
    for line in lines {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            // 限制预览长度为50个字符
            return Some(trimmed.chars().take(50).collect());
        }
    }
    
    // 如果没有找到非空行，返回None
    None
}

// 从内容中提取创建时间
fn extract_created_at_from_content(content: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut in_front_matter = false;
    
    for line in &lines {
        if line.trim() == "---" {
            if !in_front_matter {
                in_front_matter = true;
            } else {
                break; // 结束front matter
            }
        } else if in_front_matter {
            if line.starts_with("createdAt:") {
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                if parts.len() == 2 {
                    return Some(parts[1].trim().to_string());
                }
            }
        }
    }
    
    None
}

// 更新窗口位置和大小
#[tauri::command]
async fn update_note_window(window: tauri::WebviewWindow, id: String, x: f64, y: f64, width: f64, height: f64) -> Result<(), String> {
    let notes_dir = PathBuf::from(ensure_notes_directory(window).await?);
    
    // 从索引中更新窗口信息
    let index_path = notes_dir.join("index.json");
    if !index_path.exists() {
        return Err("索引文件不存在".to_string());
    }

    let mut index: IndexFile = {
        let content = fs::read_to_string(&index_path)
            .map_err(|e| format!("读取索引文件失败: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("解析索引文件失败: {}", e))?
    };

    if let Some(entry) = index.notes.iter_mut().find(|note| note.id == id) {
        if let Some(ref mut window_info) = entry.window {
            window_info.x = x;
            window_info.y = y;
            window_info.width = width;
            window_info.height = height;
        } else {
            // 如果窗口信息不存在，创建一个新的
            entry.window = Some(WindowInfo {
                x,
                y,
                width,
                height,
            });
        }
        
        // 保存更新后的索引
        let json_content = serde_json::to_string_pretty(&index)
            .map_err(|e| format!("序列化索引失败: {}", e))?;
        fs::write(&index_path, json_content)
            .map_err(|e| format!("写入索引文件失败: {}", e))?;

        Ok(())
    } else {
        Err("找不到指定的便签".to_string())
    }
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
        tauri::WebviewUrl::App(format!("index.html?noteId={}", &label.replace("note-", "")).into()),
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

// 创建归档列表窗口
#[tauri::command]
async fn create_archive_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    let window = tauri::WebviewWindowBuilder::new(
        &app_handle,
        "archive",
        tauri::WebviewUrl::App("archive.html".into()),
    )
    .title("归档便签")
    .inner_size(800.0, 600.0)
    .resizable(true)
    .decorations(true)
    .visible(true);

    let _window = window.build().map_err(|e| e.to_string())?;

    Ok(())
}

// 初始化便签目录结构（通过路径）
pub async fn initialize_notes_directory_by_path(notes_dir: std::path::PathBuf) -> Result<String, String> {
    std::fs::create_dir_all(&notes_dir).map_err(|e| format!("创建AppData目录失败: {}", e))?;

    let notes_subdir = notes_dir.join("notes");
    std::fs::create_dir_all(&notes_subdir).map_err(|e| format!("创建notes目录失败: {}", e))?;

    // 验证并修复索引
    validate_and_fix_index(&notes_dir)?;

    Ok(notes_dir.to_string_lossy().to_string())
}



// 创建新的便签（通过路径）
pub async fn create_note_by_path(notes_dir: std::path::PathBuf, x: f64, y: f64, width: f64, height: f64) -> Result<String, String> {
    // 生成UUID作为ID
    let id = Uuid::new_v4().to_string();
    
    // 创建时间信息
    let created_at = get_current_iso8601_time();
    let expires_at = (chrono::DateTime::parse_from_rfc3339(&created_at)
        .map_err(|e| format!("解析时间失败: {}", e))?
        .naive_local()
        .and_local_timezone(chrono::Local)
        .unwrap() + chrono::Duration::days(7)).to_rfc3339();
    
    // 创建文件内容
    let content = build_full_content(&id, &created_at, "");
    
    // 创建按日期组织的目录结构
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let dated_dir = notes_dir.join("notes").join(today);
    std::fs::create_dir_all(&dated_dir).map_err(|e| format!("创建日期目录失败: {}", e))?;

    // 创建文件
    let file_path = dated_dir.join(format!("{}.md", id));
    std::fs::write(&file_path, content).map_err(|e| format!("创建便签文件失败: {}", e))?;

    // 更新索引
    let index_path = notes_dir.join("index.json");
    let mut index: IndexFile = if index_path.exists() {
        let content = std::fs::read_to_string(&index_path)
            .map_err(|e| format!("读取索引文件失败: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("解析索引文件失败: {}", e))?
    } else {
        IndexFile {
            version: 2,
            app: AppInfo {
                name: "FadeNote".to_string(),
                created_at: get_current_iso8601_time(),
                rebuild_at: None,
            },
            notes: Vec::new(),
        }
    };

    let rel_path = file_path.strip_prefix(&notes_dir)
        .unwrap_or(&file_path)
        .to_string_lossy()
        .to_string();

    let mut new_entry = NoteEntry {
        id: id.clone(),
        created_at: created_at.clone(),
        last_active_at: created_at.clone(), // 初始last_active_at就是创建时间
        expire_at: Some(expires_at.clone()),
        cached_preview: None,
        status: String::new(), // 禁止手写，将在派生时设置
        archived_at: None,
        window: Some(WindowInfo {
            x,
            y,
            width,
            height,
        }),
        pinned: false,  // 默认不固定
        file: FileInfo {
            relative_path: rel_path,
        },
    };
    
    // 派生状态
    derive_status(&mut new_entry);
    
    index.notes.push(new_entry);

    let json_content = serde_json::to_string_pretty(&index)
        .map_err(|e| format!("序列化索引失败: {}", e))?;
    std::fs::write(&index_path, json_content)
        .map_err(|e| format!("写入索引文件失败: {}", e))?;

    Ok(id)
}

// 检查是否有活跃的便签
#[tauri::command]
async fn has_unexpired_notes(window: tauri::WebviewWindow) -> Result<bool, String> {
    let active_notes = get_all_active_notes(window).await?;
    Ok(!active_notes.is_empty())
}

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            notes_directory: Mutex::new(None),
        })
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // 隐藏窗口而不是关闭它
                let _ = window.hide();
                // 阻止默认的关闭行为
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            create_note_window,
            initialize_notes_directory,
            ensure_notes_directory,
            get_active_notes,
            get_all_active_notes,
            get_archived_notes,
            get_notes_without_windows,
            restore_notes_without_windows,
            has_unexpired_notes,
            create_note,
            load_note,
            update_note_activity,
            save_note_content,
            update_note_window,
            restore_note,
            set_note_pinned,
            create_archive_window
        ])
        .setup(|app| {
            // 为应用设置防止退出行为
            let app_handle = app.handle().clone();
            
            // 创建系统托盘菜单项
            let new_note_item = MenuItem::with_id(app, "new_note", "New Note", true, None::<&str>).unwrap();
            let show_notes_item = MenuItem::with_id(app, "show_notes", "Show Notes", true, None::<&str>).unwrap();
            let archive_item = MenuItem::with_id(app, "archive", "Archive", true, None::<&str>).unwrap();
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>).unwrap();
            
            // 创建系统托盘菜单
            let tray_menu = MenuBuilder::new(app)
                .item(&new_note_item)
                .item(&show_notes_item)
                .separator()
                .item(&archive_item)
                .separator()
                .item(&quit_item)
                .build().unwrap();
            
            // 创建托盘图标（注意：Windows 必须提供 icon）
            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone()) // 使用窗口图标
                .menu(&tray_menu)
                .on_menu_event(|_app, event| {
                    match event.id().as_ref() {
                        "new_note" => {
                            // 创建新便签
                            let app_handle = _app.clone();
                            tauri::async_runtime::spawn(async move {
                                // 创建新便签
                                let id = match create_note_by_path(
                                    get_app_data_dir().unwrap(),
                                    200.0,  // 默认X坐标
                                    200.0,  // 默认Y坐标
                                    280.0,  // 默认宽度
                                    360.0,  // 默认高度
                                ).await {
                                    Ok(id) => id,
                                    Err(e) => {
                                        eprintln!("创建新便签失败: {}", e);
                                        return;
                                    }
                                };
                                
                                // 为新便签创建窗口
                                let label = format!("note-{}", id);
                                if let Err(e) = create_note_window(
                                    app_handle.clone(),
                                    label,
                                    "FadeNote".to_string(),
                                    280,
                                    360,
                                    Some(200),
                                    Some(200),
                                ).await {
                                    eprintln!("创建便签窗口失败: {}", e);
                                }
                            });
                        },
                        "show_notes" => {
                            // 恢复没有窗口或隐藏的便签
                            let app_handle = _app.clone();
                            tauri::async_runtime::spawn(async move {
                                // 获取当前所有窗口及其可见性状态
                                let all_windows = app_handle.webview_windows();
                                
                                // 获取所有活跃便签
                                let app_data_dir = get_app_data_dir().unwrap();
                                let index = validate_and_fix_index(&app_data_dir).unwrap_or_else(|_| {
                                    IndexFile {
                                        version: 2,
                                        app: AppInfo {
                                            name: "FadeNote".to_string(),
                                            created_at: get_current_iso8601_time(),
                                            rebuild_at: None,
                                        },
                                        notes: Vec::new(),
                                    }
                                });
                                
                                // 找出需要恢复的活跃便签（没有窗口或窗口隐藏）
                                for entry in &index.notes {
                                    if is_active(entry) && entry.window.is_some() {
                                        let label = format!("note-{}", entry.id);
                                        
                                        // 检查窗口是否存在且是否可见
                                        if let Some(note_window) = all_windows.get(&label) {
                                            // 窗口存在，检查是否可见
                                            if let Ok(is_visible) = note_window.is_visible() {
                                                if !is_visible {
                                                    // 窗口存在但不可见，显示它
                                                    let _ = note_window.show();
                                                    let _ = note_window.set_focus();
                                                }
                                            } else {
                                                // 无法获取可见性，尝试显示
                                                let _ = note_window.show();
                                                let _ = note_window.set_focus();
                                            }
                                        } else {
                                            // 窗口不存在，创建新窗口
                                            let window_info = entry.window.as_ref().unwrap();
                                            if let Err(e) = create_note_window(
                                                app_handle.clone(),
                                                label,
                                                "FadeNote".to_string(),
                                                window_info.width as u32,
                                                window_info.height as u32,
                                                Some(window_info.x as i32),
                                                Some(window_info.y as i32),
                                            ).await {
                                                eprintln!("恢复便签窗口失败 {}: {}", entry.id, e);
                                            }
                                        }
                                    }
                                }
                            });
                        },
                        "archive" => {
                            // 打开归档窗口
                            let app_handle = _app.clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = create_archive_window(app_handle).await;
                            });
                        },
                        "quit" => {
                            // 退出前确保所有状态持久化
                            tauri::async_runtime::spawn(async move {
                                // 确保index.json是最新的
                                let app_data_dir = get_app_data_dir().unwrap();
                                let _ = validate_and_fix_index(&app_data_dir);
                                
                                // 安全退出
                                std::process::exit(0);
                            });
                        },
                        _ => {}
                    }
                })
                .build(app).unwrap();
            
            // 初始化AppData目录
            tauri::async_runtime::block_on(async {
                // 获取应用数据目录
                let app_data_dir = get_app_data_dir().unwrap();
                // 确保目录存在
                std::fs::create_dir_all(&app_data_dir).unwrap();
                
                // 检查是否为首次启动
                let first_launch = is_first_launch(&app_data_dir);
                
                // 验证并修复索引
                match validate_and_fix_index(&app_data_dir) {
                    Ok(_) => {
                        println!("成功初始化便签目录: {}", app_data_dir.display());
                        
                        // Fix 6: 启动流程遵循正确顺序
                        // 1. Load index
                        let mut index = match validate_and_fix_index(&app_data_dir) {
                            Ok(idx) => idx,
                            Err(e) => {
                                eprintln!("验证和修复索引失败: {}", e);
                                IndexFile {
                                    version: 2,
                                    app: AppInfo {
                                        name: "FadeNote".to_string(),
                                        created_at: get_current_iso8601_time(),
                                        rebuild_at: None,
                                    },
                                    notes: Vec::new(),
                                }
                            }
                        };
                        
                        // 2. Apply expire pass 已在 validate_and_fix_index 内执行
                        // 3. Save index
                        let index_path = app_data_dir.join("index.json");
                        if let Ok(json_content) = serde_json::to_string_pretty(&index) {
                            let _ = std::fs::write(&index_path, json_content);
                        }
                        
                        // 4. Get active notes for restoration
                        let mut active_notes = Vec::new();
                        for entry in &index.notes {
                            if is_active(entry) {  // 使用统一的is_active函数
                                active_notes.push(entry.clone());
                            }
                        }
                                                
                        let unexpired_notes = active_notes;
                                                 
                        let mut restored_count = 0;
                        if !unexpired_notes.is_empty() {
                            // 如果有未过期的便签，恢复它们的窗口
                            for note in unexpired_notes {
                                if is_active(&note) && note.window.is_some() { // note是owned value，&note取引用
                                    let window_info = note.window.as_ref().unwrap();
                                    // 创建对应窗口
                                    let label = format!("note-{}", note.id);
                                    let title = "FadeNote";
                                    
                                    match create_note_window(
                                        app.app_handle().clone(),
                                        label,
                                        title.to_string(),
                                        window_info.width as u32,
                                        window_info.height as u32,
                                        Some(window_info.x as i32),
                                        Some(window_info.y as i32),
                                    ).await {
                                        Ok(_) => {
                                            println!("恢复便签窗口: {}", note.id);
                                            restored_count += 1;
                                        },
                                        Err(e) => eprintln!("创建便签窗口失败 {}: {}", note.id, e),
                                    }
                                }
                            }
                        }
                        
                        // 首次启动逻辑
                        if first_launch {
                            println!("首次启动，创建欢迎便签");
                            
                            // 创建欢迎便签
                            let welcome_id = Uuid::new_v4().to_string();
                            let created_at = get_current_iso8601_time();
                            let expires_at = (chrono::DateTime::parse_from_rfc3339(&created_at)
                                .unwrap_or_else(|_| chrono::Local::now().into())
                                .naive_local()
                                .and_local_timezone(chrono::Local)
                                .unwrap() + chrono::Duration::days(7)).to_rfc3339();
                            
                            // 创建欢迎内容
                            let welcome_content = get_welcome_content();
                            let full_content = build_full_content(&welcome_id, &created_at, &welcome_content);
                            
                            // 创建按日期组织的目录结构
                            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                            let dated_dir = app_data_dir.join("notes").join(today);
                            std::fs::create_dir_all(&dated_dir).unwrap();

                            // 创建文件
                            let file_path = dated_dir.join(format!("{}.md", welcome_id));
                            std::fs::write(&file_path, full_content).unwrap();

                            let rel_path = file_path.strip_prefix(&app_data_dir)
                                .unwrap_or(&file_path)
                                .to_string_lossy()
                                .to_string();

                            let mut welcome_entry = NoteEntry {
                                id: welcome_id.clone(),
                                created_at: created_at.clone(),
                                last_active_at: created_at.clone(),
                                expire_at: Some(expires_at.clone()),
                                cached_preview: Some("写点什么吧...".to_string()),
                                status: String::new(),
                                archived_at: None,
                                window: Some(WindowInfo {
                                    x: 200.0,
                                    y: 200.0,
                                    width: 300.0,
                                    height: 380.0,
                                }),
                                pinned: false,  // 欢迎便签默认不固定
                                file: FileInfo {
                                    relative_path: rel_path,
                                },
                            };
                            
                            // 派生状态
                            derive_status(&mut welcome_entry);
                            index.notes.push(welcome_entry);

                            // 保存索引
                            let json_content = serde_json::to_string_pretty(&index)
                                .unwrap_or_else(|_| "{}".to_string());
                            std::fs::write(&index_path, json_content)
                                .unwrap();
                            
                            // 创建欢迎便签窗口
                            let label = format!("note-{}", welcome_id);
                            let title = "FadeNote";
                            
                            match create_note_window(
                                app.app_handle().clone(),
                                label,
                                title.to_string(),
                                300,
                                380,
                                Some(200),
                                Some(200),
                            ).await {
                                Ok(_) => {
                                    println!("创建欢迎便签窗口: {}", welcome_id);
                                    restored_count += 1;
                                },
                                Err(e) => eprintln!("创建欢迎便签窗口失败 {}: {}", welcome_id, e),
                            }
                        }
                        // 如果不是首次启动且没有恢复任何窗口，创建默认便签
                        else if restored_count == 0 {
                            // 直接创建便签和窗口，而不使用临时窗口
                            // 创建便签
                            let index_path = app_data_dir.join("index.json");
                            let mut index: IndexFile = if index_path.exists() {
                                let content = std::fs::read_to_string(&index_path).unwrap_or_else(|_| "{}".to_string());
                                serde_json::from_str(&content).unwrap_or(IndexFile {
                                    version: 2,
                                    app: AppInfo {
                                        name: "FadeNote".to_string(),
                                        created_at: get_current_iso8601_time(),
                                        rebuild_at: None,
                                    },
                                    notes: Vec::new(),
                                })
                            } else {
                                IndexFile {
                                    version: 2,
                                    app: AppInfo {
                                        name: "FadeNote".to_string(),
                                        created_at: get_current_iso8601_time(),
                                        rebuild_at: None,
                                    },
                                    notes: Vec::new(),
                                }
                            };
                            
                            // 生成UUID作为ID
                            let id = Uuid::new_v4().to_string();
                            
                            // 创建时间信息
                            let created_at = get_current_iso8601_time();
                            // 解析创建时间并计算过期时间
                            let created_datetime = DateTime::parse_from_rfc3339(&created_at)
                                .unwrap_or_else(|_| chrono::Local::now().into());
                            let expires_at = (created_datetime.naive_local()
                                .and_local_timezone(chrono::Local)
                                .unwrap() + chrono::Duration::days(7)).to_rfc3339();
                            
                            // 创建文件内容
                            let content = build_full_content(&id, &created_at, "");
                            
                            // 创建按日期组织的目录结构
                            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                            let dated_dir = app_data_dir.join("notes").join(today);
                            std::fs::create_dir_all(&dated_dir).unwrap();

                            // 创建文件
                            let file_path = dated_dir.join(format!("{}.md", id));
                            std::fs::write(&file_path, content).unwrap();

                            let rel_path = file_path.strip_prefix(&app_data_dir)
                                .unwrap_or(&file_path)
                                .to_string_lossy()
                                .to_string();

                            let mut new_entry = NoteEntry {
                                id: id.clone(),
                                created_at: created_at.clone(),
                                last_active_at: created_at.clone(), // 初始last_active_at就是创建时间
                                expire_at: Some(expires_at.clone()),
                                cached_preview: None,
                                status: String::new(), // 禁止手写，将在派生时设置
                                archived_at: None,
                                window: Some(WindowInfo {
                                    x: 100.0,
                                    y: 100.0,
                                    width: 280.0,
                                    height: 360.0,
                                }),
                                pinned: false,  // 默认不固定
                                file: FileInfo {
                                    relative_path: rel_path,
                                },
                            };
                            
                            // 派生状态
                            derive_status(&mut new_entry);

                            index.notes.push(new_entry);

                            let json_content = serde_json::to_string_pretty(&index)
                                .unwrap_or_else(|_| "{}".to_string());
                            std::fs::write(&index_path, json_content)
                                .unwrap();
                            
                            // 创建对应的窗口
                            let label = format!("note-{}", id);
                            let title = "FadeNote";
                            
                            match create_note_window(
                                app.app_handle().clone(),
                                label,
                                title.to_string(),
                                280,
                                360,
                                Some(100),
                                Some(100),
                            ).await {
                                Ok(_) => println!("创建默认便签窗口: {}", id),
                                Err(e) => eprintln!("创建默认便签窗口失败 {}: {}", id, e),
                            }
                        }
                    },
                    Err(e) => eprintln!("初始化便签目录失败: {}", e),
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}