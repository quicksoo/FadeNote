#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Duration, Utc};
use dirs::data_dir;
use serde::{Deserialize, Serialize};
use tauri::{Manager};
use uuid::Uuid;

// 获取AppData目录
fn get_app_data_dir() -> Result<PathBuf, String> {
    let mut app_data_dir = data_dir().ok_or("无法获取AppData目录")?;
    app_data_dir.push("FadeNote");
    Ok(app_data_dir)
}

// V2规范的数据模型
#[derive(Serialize, Deserialize, Clone)]
struct AppInfo {
    name: String,
    #[serde(rename = "createdAt")]
    created_at: String,
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
    expire_at: String,
    archived: bool,
    window: WindowInfo,
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
    Utc::now().to_rfc3339()
}

// 检查便签是否过期
fn is_expired(expire_at: &str) -> Result<bool, String> {
    let expire_time = DateTime::parse_from_rfc3339(expire_at)
        .map_err(|e| format!("解析过期时间失败: {}", e))?;
    let now = Utc::now();
    Ok(now > expire_time.naive_utc().and_local_timezone(Utc).unwrap())
}

// 归档便签
fn archive_note(notes_dir: &Path, entry: &mut NoteEntry) -> Result<(), String> {
    let source_path = notes_dir.join(&entry.file.relative_path);
    if !source_path.exists() {
        return Err("源文件不存在".to_string());
    }

    // 创建archive目录
    let archive_dir = notes_dir.join("archive");
    fs::create_dir_all(&archive_dir).map_err(|e| format!("创建archive目录失败: {}", e))?;

    // 移动文件到archive目录
    let dest_path = archive_dir.join(source_path.file_name().unwrap());
    fs::rename(&source_path, &dest_path).map_err(|e| format!("移动文件到archive失败: {}", e))?;

    // 更新entry的文件路径
    let archive_relative_path = format!("archive/{}", source_path.file_name().unwrap().to_string_lossy());
    entry.file.relative_path = archive_relative_path;
    entry.archived = true;

    Ok(())
}

// 验证并修复索引
fn validate_and_fix_index(notes_dir: &Path) -> Result<IndexFile, String> {
    let index_path = notes_dir.join("index.json");
    let mut index: IndexFile = if index_path.exists() {
        let content = fs::read_to_string(&index_path)
            .map_err(|e| format!("读取索引文件失败: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("解析索引文件失败: {}", e))?
    } else {
        // 创建新的V2索引
        IndexFile {
            version: 2,
            app: AppInfo {
                name: "FadeNote".to_string(),
                created_at: get_current_iso8601_time(),
            },
            notes: Vec::new(),
        }
    };

    // 遍历所有notes，检查文件是否存在
    index.notes.retain(|entry| {
        let file_path = notes_dir.join(&entry.file.relative_path);
        // 保留即使文件不存在的条目，因为它们可能在其他地方
        // 但如果文件不存在且已归档，则可以考虑删除
        if !file_path.exists() && entry.archived {
            println!("移除已归档且文件不存在的note: {}", entry.id);
            false
        } else {
            true
        }
    });

    // 扫描notes目录下的所有文件，仅添加当前索引中不存在的文件
    // 避免重复添加已存在的便签
    let current_index_ids: std::collections::HashSet<String> = index.notes.iter().map(|note| note.id.clone()).collect();
    
    let notes_path = notes_dir.join("notes");
    if notes_path.exists() {
        scan_directory_for_notes(notes_dir, &mut index, &notes_path, &current_index_ids)?;
    }

    // 检查过期的便签并归档
    for entry in index.notes.iter_mut() {
        if !entry.archived && is_expired(&entry.expire_at)? {
            match archive_note(notes_dir, entry) {
                Ok(()) => {
                    println!("便签 {} 已归档", entry.id);
                },
                Err(e) => {
                    eprintln!("归档便签 {} 失败: {}", entry.id, e);
                    // 即使归档失败也标记为已归档，避免重复尝试
                    entry.archived = true;
                }
            }
        }
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
                        
                        let expires_time = created_time + Duration::days(7);
                        
                        let new_entry = NoteEntry {
                            id: parsed_id.clone(), // 修复：clone值以避免移动
                            created_at: created_time.to_rfc3339(),
                            last_active_at: created_time.to_rfc3339(),
                            expire_at: expires_time.to_rfc3339(),
                            archived: false,
                            window: WindowInfo {
                                x: 100.0,
                                y: 100.0,
                                width: 280.0,
                                height: 360.0,
                            },
                            file: FileInfo {
                                relative_path,
                            },
                        };
                        
                        index.notes.push(new_entry);
                        existing_ids.insert(parsed_id.clone()); // 添加到已知ID集合
                        println!("添加新发现的note到索引: {}", parsed_id); // 修复：使用克隆的值
                    }
                }
            }
        } else if path.is_dir() {
            // 递归扫描子目录
            scan_directory_for_notes_recursive(notes_dir, index, &path, existing_ids)?;
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
    let notes_dir = PathBuf::from(ensure_notes_directory(window).await?);
    let index = validate_and_fix_index(&notes_dir)?;

    let mut active_notes = Vec::new();
    for entry in &index.notes {
        if !entry.archived && !is_expired(&entry.expire_at)? {
            active_notes.push(entry.clone());
        }
    }

    Ok(active_notes)
}

// 获取所有未过期的便签
#[tauri::command]
async fn get_all_unexpired_notes(window: tauri::WebviewWindow) -> Result<Vec<NoteEntry>, String> {
    let notes_dir = PathBuf::from(ensure_notes_directory(window).await?);
    let index = validate_and_fix_index(&notes_dir)?;

    let mut unexpired_notes = Vec::new();
    for entry in &index.notes {
        if !is_expired(&entry.expire_at)? {
            unexpired_notes.push(entry.clone());
        }
    }

    Ok(unexpired_notes)
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
            },
            notes: Vec::new(),
        }
    };

    let rel_path = file_path.strip_prefix(&notes_dir)
        .unwrap_or(&file_path)
        .to_string_lossy()
        .to_string();

    let new_entry = NoteEntry {
        id: id.clone(),
        created_at: created_at.clone(),
        last_active_at: created_at.clone(), // 初始last_active_at就是创建时间
        expire_at: expires_at.clone(),
        archived: false,
        window: WindowInfo {
            x,
            y,
            width,
            height,
        },
        file: FileInfo {
            relative_path: rel_path,
        },
    };

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
    if let Some(entry) = index.notes.iter().find(|note| note.id == id && !note.archived) {
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
        // 更新last_active_at和expire_at
        let now = get_current_iso8601_time();
        entry.last_active_at = now.clone();
        
        // 计算新的过期时间：当前时间 + 7天
        let current_time = DateTime::parse_from_rfc3339(&now)
            .map_err(|e| format!("解析当前时间失败: {}", e))?;
        let new_expire_time = (current_time.naive_utc()
            .and_local_timezone(Utc)
            .unwrap() + Duration::days(7)).to_rfc3339();
        entry.expire_at = new_expire_time;

        // 保存更新后的索引
        index.app.name = "FadeNote".to_string(); // 确保app信息存在
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
    
    println!("开始保存便签内容，ID: {}, 内容长度: {}", id, content.len());
    
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

    // 先检查便签是否存在且未归档
    if !index.notes.iter().any(|note| note.id == id && !note.archived) {
        return Err("找不到指定的便签或便签已被归档".to_string());
    }
    
    // 查找并更新活动时间
    if let Some(update_entry) = index.notes.iter_mut().find(|note| note.id == id) {
        if update_entry.archived {
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
        let new_expire_time = (current_time.naive_utc()
            .and_local_timezone(Utc)
            .unwrap() + Duration::days(7)).to_rfc3339();
        update_entry.expire_at = new_expire_time;
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
        entry.window.x = x;
        entry.window.y = y;
        entry.window.width = width;
        entry.window.height = height;

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

// 初始化便签目录结构（通过路径）
pub async fn initialize_notes_directory_by_path(notes_dir: std::path::PathBuf) -> Result<String, String> {
    std::fs::create_dir_all(&notes_dir).map_err(|e| format!("创建AppData目录失败: {}", e))?;

    let notes_subdir = notes_dir.join("notes");
    std::fs::create_dir_all(&notes_subdir).map_err(|e| format!("创建notes目录失败: {}", e))?;

    // 验证并修复索引
    validate_and_fix_index(&notes_dir)?;

    Ok(notes_dir.to_string_lossy().to_string())
}

// 获取所有未过期的便签（通过路径）
pub fn get_all_unexpired_notes_by_path_sync(notes_dir: std::path::PathBuf) -> Result<Vec<NoteEntry>, String> {
    let index = validate_and_fix_index(&notes_dir)?;

    let mut unexpired_notes = Vec::new();
    for entry in &index.notes {
        if !is_expired(&entry.expire_at)? {
            unexpired_notes.push(entry.clone());
        }
    }

    Ok(unexpired_notes)
}

// 创建新的便签（通过路径）
pub async fn create_note_by_path(notes_dir: std::path::PathBuf, x: f64, y: f64, width: f64, height: f64) -> Result<String, String> {
    // 生成UUID作为ID
    let id = Uuid::new_v4().to_string();
    
    // 创建时间信息
    let created_at = get_current_iso8601_time();
    let expires_at = (chrono::DateTime::parse_from_rfc3339(&created_at)
        .map_err(|e| format!("解析时间失败: {}", e))?
        .naive_utc()
        .and_local_timezone(chrono::Utc)
        .unwrap() + chrono::Duration::days(7)).to_rfc3339();
    
    // 创建文件内容
    let content = build_full_content(&id, &created_at, "");
    
    // 创建按日期组织的目录结构
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
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
            },
            notes: Vec::new(),
        }
    };

    let rel_path = file_path.strip_prefix(&notes_dir)
        .unwrap_or(&file_path)
        .to_string_lossy()
        .to_string();

    let new_entry = NoteEntry {
        id: id.clone(),
        created_at: created_at.clone(),
        last_active_at: created_at.clone(), // 初始last_active_at就是创建时间
        expire_at: expires_at.clone(),
        archived: false,
        window: WindowInfo {
            x,
            y,
            width,
            height,
        },
        file: FileInfo {
            relative_path: rel_path,
        },
    };

    index.notes.push(new_entry);

    let json_content = serde_json::to_string_pretty(&index)
        .map_err(|e| format!("序列化索引失败: {}", e))?;
    std::fs::write(&index_path, json_content)
        .map_err(|e| format!("写入索引文件失败: {}", e))?;

    Ok(id)
}

// 检查是否有未过期的便签
#[tauri::command]
async fn has_unexpired_notes(window: tauri::WebviewWindow) -> Result<bool, String> {
    let unexpired_notes = get_all_unexpired_notes(window).await?;
    Ok(!unexpired_notes.is_empty())
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
            initialize_notes_directory,
            ensure_notes_directory,
            get_active_notes,
            get_all_unexpired_notes,
            has_unexpired_notes,
            create_note,
            load_note,
            update_note_activity,
            save_note_content,
            update_note_window
        ])
        .setup(|app| {
            // 初始化AppData目录
            tauri::async_runtime::block_on(async {
                // 获取应用数据目录
                let app_data_dir = get_app_data_dir().unwrap();
                // 确保目录存在
                std::fs::create_dir_all(&app_data_dir).unwrap();
                // 验证并修复索引
                match validate_and_fix_index(&app_data_dir) {
                    Ok(_) => {
                        println!("成功初始化便签目录: {}", app_data_dir.display());
                        
                        // 获取所有未过期的便签（这会执行一次完整的验证和修复）
                        let unexpired_notes = match get_all_unexpired_notes_by_path_sync(app_data_dir.clone()) {
                            Ok(notes) => {
                                println!("找到 {} 个未过期的便签", notes.len());
                                notes
                            },
                            Err(e) => {
                                eprintln!("获取未过期便签失败: {}", e);
                                vec![]
                            }
                        };
                        
                        if !unexpired_notes.is_empty() {
                            // 如果有未过期的便签，恢复它们的窗口
                            for note in unexpired_notes {
                                if !note.archived {
                                    // 创建对应窗口
                                    let label = format!("note-{}", note.id);
                                    let title = "便签";
                                    
                                    match create_note_window(
                                        app.app_handle().clone(),
                                        label,
                                        title.to_string(),
                                        note.window.width as u32,
                                        note.window.height as u32,
                                        Some(note.window.x as i32),
                                        Some(note.window.y as i32),
                                    ).await {
                                        Ok(_) => println!("恢复便签窗口: {}", note.id),
                                        Err(e) => eprintln!("创建便签窗口失败 {}: {}", note.id, e),
                                    }
                                }
                            }
                        } else {
                            // 如果没有未过期的便签，创建一个新的默认便签窗口
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
                                    },
                                    notes: Vec::new(),
                                })
                            } else {
                                IndexFile {
                                    version: 2,
                                    app: AppInfo {
                                        name: "FadeNote".to_string(),
                                        created_at: get_current_iso8601_time(),
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
                                .unwrap_or_else(|_| chrono::Utc::now().into());
                            let expires_at = (created_datetime.naive_utc()
                                .and_local_timezone(chrono::Utc)
                                .unwrap() + chrono::Duration::days(7)).to_rfc3339();
                            
                            // 创建文件内容
                            let content = build_full_content(&id, &created_at, "");
                            
                            // 创建按日期组织的目录结构
                            let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
                            let dated_dir = app_data_dir.join("notes").join(today);
                            std::fs::create_dir_all(&dated_dir).unwrap();

                            // 创建文件
                            let file_path = dated_dir.join(format!("{}.md", id));
                            std::fs::write(&file_path, content).unwrap();

                            let rel_path = file_path.strip_prefix(&app_data_dir)
                                .unwrap_or(&file_path)
                                .to_string_lossy()
                                .to_string();

                            let new_entry = NoteEntry {
                                id: id.clone(),
                                created_at: created_at.clone(),
                                last_active_at: created_at.clone(), // 初始last_active_at就是创建时间
                                expire_at: expires_at.clone(),
                                archived: false,
                                window: WindowInfo {
                                    x: 100.0,
                                    y: 100.0,
                                    width: 280.0,
                                    height: 360.0,
                                },
                                file: FileInfo {
                                    relative_path: rel_path,
                                },
                            };

                            index.notes.push(new_entry);

                            let json_content = serde_json::to_string_pretty(&index)
                                .unwrap_or_else(|_| "{}".to_string());
                            std::fs::write(&index_path, json_content)
                                .unwrap();
                            
                            // 创建对应的窗口
                            let label = format!("note-{}", id);
                            let title = "便签";
                            
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