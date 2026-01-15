#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Duration, Utc};
use dirs::data_dir;
use serde::{Deserialize, Serialize};
use tauri::Manager;

// 获取AppData目录
fn get_app_data_dir() -> Result<PathBuf, String> {
    let mut app_data_dir = data_dir().ok_or("无法获取AppData目录")?;
    app_data_dir.push("FadeNote");
    Ok(app_data_dir)
}

// 核心数据模型
#[derive(Serialize, Deserialize, Clone)]
struct NoteMeta {
    id: String,
    #[serde(rename = "createdAt", alias = "created_at")]
    created_at: String,
    #[serde(rename = "expiresAt", alias = "expires_at")]
    expires_at: String,
    x: Option<f64>,
    y: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone)]
struct IndexNoteEntry {
    path: String,
    #[serde(rename = "createdAt", alias = "created_at")]
    created_at: String,
    #[serde(rename = "expiresAt", alias = "expires_at")]
    expires_at: String,
    #[serde(rename = "updatedAt", alias = "updated_at")]
    updated_at: String,
    archived: bool,
}

#[derive(Serialize, Deserialize)]
struct IndexFile {
    version: u32,
    #[serde(rename = "lastUpdatedAt", alias = "last_updated_at")]
    last_updated_at: String,
    notes: HashMap<String, IndexNoteEntry>,
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
fn is_expired(expires_at: &str) -> Result<bool, String> {
    let expires_time = DateTime::parse_from_rfc3339(expires_at)
        .map_err(|e| format!("解析过期时间失败: {}", e))?;
    let now = Utc::now();
    Ok(now > expires_time.naive_utc().and_local_timezone(Utc).unwrap())
}

// 归档便签
fn archive_note(notes_dir: &Path, _note_id: &str, entry: &IndexNoteEntry) -> Result<(), String> {
    let source_path = notes_dir.join(&entry.path);
    if !source_path.exists() {
        return Err("源文件不存在".to_string());
    }

    // 创建archive目录
    let archive_dir = notes_dir.join("archive");
    fs::create_dir_all(&archive_dir).map_err(|e| format!("创建archive目录失败: {}", e))?;

    // 移动文件到archive目录
    let dest_path = archive_dir.join(source_path.file_name().unwrap());
    fs::rename(&source_path, &dest_path).map_err(|e| format!("移动文件到archive失败: {}", e))?;

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
        // 创建新的空索引
        IndexFile {
            version: 1,
            last_updated_at: get_current_iso8601_time(),
            notes: HashMap::new(),
        }
    };

    // 遍历所有notes，检查文件是否存在
    let mut to_remove = Vec::new();
    for (id, entry) in &index.notes {
        let file_path = notes_dir.join(&entry.path);
        if !file_path.exists() && !entry.archived {
            // 文件不存在但标记为未归档，添加到删除列表
            to_remove.push(id.clone());
        }
    }

    // 删除不存在的note记录
    for id in to_remove {
        println!("删除不存在的note记录: {}", id);
        index.notes.remove(&id);
    }

    // 扫描notes目录下的所有文件，补充缺失的索引项
    let notes_path = notes_dir.join("notes");
    if notes_path.exists() {
        for entry in fs::read_dir(&notes_path).map_err(|e| format!("读取notes目录失败: {}", e))? {
            let entry = entry.map_err(|e| format!("遍历文件失败: {}", e))?;
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |ext| ext == "md") {
                let file_name = path.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                
                if !index.notes.contains_key(&file_name) {
                    // 找到未索引的文件，添加到索引中
                    let metadata = path.metadata().map_err(|e| format!("获取文件元数据失败: {}", e))?;
                    let created_time = DateTime::<Utc>::from(metadata.created()
                        .map_err(|e| format!("获取创建时间失败: {}", e))?);
                    
                    let expires_time = created_time + Duration::days(7);
                    
                    index.notes.insert(
                        file_name.clone(),
                        IndexNoteEntry {
                            path: format!("notes/{}", path.file_name().unwrap().to_string_lossy()),
                            created_at: created_time.to_rfc3339(),
                            expires_at: expires_time.to_rfc3339(),
                            updated_at: get_current_iso8601_time(),
                            archived: false,
                        }
                    );
                    println!("添加新发现的note到索引: {}", file_name);
                }
            } else if path.is_dir() {
                // 扫描子目录（按日期组织的目录）
                for sub_entry in fs::read_dir(&path).map_err(|e| format!("读取子目录失败: {}", e))? {
                    let sub_entry = sub_entry.map_err(|e| format!("遍历子文件失败: {}", e))?;
                    let sub_path = sub_entry.path();
                    
                    if sub_path.is_file() && sub_path.extension().map_or(false, |ext| ext == "md") {
                        let file_name = sub_path.file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        
                        if !index.notes.contains_key(&file_name) {
                            let rel_path = sub_path.strip_prefix(notes_dir)
                                .unwrap_or(&sub_path)
                                .to_string_lossy()
                                .to_string();
                            
                            let metadata = sub_path.metadata().map_err(|e| format!("获取文件元数据失败: {}", e))?;
                            let created_time = DateTime::<Utc>::from(metadata.created()
                                .map_err(|e| format!("获取创建时间失败: {}", e))?);
                            
                            let expires_time = created_time + Duration::days(7);
                            
                            index.notes.insert(
                                file_name.clone(),
                                IndexNoteEntry {
                                    path: rel_path,
                                    created_at: created_time.to_rfc3339(),
                                    expires_at: expires_time.to_rfc3339(),
                                    updated_at: get_current_iso8601_time(),
                                    archived: false,
                                }
                            );
                            println!("添加新发现的note到索引: {}", file_name);
                        }
                    }
                }
            }
        }
    }

    // 检查过期的便签并归档 - 分离读取和更新步骤以避免借用冲突
    let expired_notes: Vec<(String, IndexNoteEntry)> = index.notes
        .iter()
        .filter(|(_, entry)| !entry.archived && is_expired(&entry.expires_at).unwrap_or(true))
        .map(|(id, entry)| (id.clone(), entry.clone()))
        .collect();

    for (id, entry) in expired_notes {
        match archive_note(notes_dir, &id, &entry) {
            Ok(()) => {
                // 更新索引中标记为已归档
                let mut updated_entry = entry.clone();
                updated_entry.archived = true;
                updated_entry.updated_at = get_current_iso8601_time();
                index.notes.insert(id.clone(), updated_entry);
                println!("便签 {} 已归档", id);
            },
            Err(e) => {
                eprintln!("归档便签 {} 失败: {}", id, e);
                // 即使归档失败也标记为已归档，避免重复尝试
                let mut updated_entry = entry.clone();
                updated_entry.archived = true;
                updated_entry.updated_at = get_current_iso8601_time();
                index.notes.insert(id.clone(), updated_entry);
            }
        }
    }

    // 更新最后更新时间
    index.last_updated_at = get_current_iso8601_time();

    // 保存更新后的索引
    let json_content = serde_json::to_string_pretty(&index)
        .map_err(|e| format!("序列化索引失败: {}", e))?;
    fs::write(&index_path, json_content)
        .map_err(|e| format!("写入索引文件失败: {}", e))?;

    Ok(index)
}

// 在内容中更新位置信息
fn update_position_in_content(content: &str, x: f64, y: f64) -> Result<String, String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut result_lines = Vec::<String>::new();
    let mut in_front_matter = false;
    let mut front_matter_processed = false;
    let mut x_updated = false;
    let mut y_updated = false;

    let x_str = format!("x: {}", x);
    let y_str = format!("y: {}", y);

    let mut line_idx = 0;
    while line_idx < lines.len() {
        let line = lines[line_idx];

        if line.trim() == "---" {
            if !in_front_matter {
                in_front_matter = true;
                result_lines.push(line.to_string());
            } else {
                // 结束front matter
                if !x_updated {
                    result_lines.push(x_str.clone());
                }
                if !y_updated {
                    result_lines.push(y_str.clone());
                }
                result_lines.push(line.to_string());
                front_matter_processed = true;
                in_front_matter = false;
            }
        } else if in_front_matter {
            if let Some(pos) = line.find(':') {
                let key = line[..pos].trim();
                match key {
                    "x" => {
                        result_lines.push(x_str.clone());
                        x_updated = true;
                    }
                    "y" => {
                        result_lines.push(y_str.clone());
                        y_updated = true;
                    }
                    _ => {
                        result_lines.push(line.to_string());
                    }
                }
            } else {
                result_lines.push(line.to_string());
            }
        } else {
            // 不在front matter中，直接添加
            if !front_matter_processed {
                // 如果还没处理完front matter就结束了，说明没有找到结束标记
                if !x_updated {
                    result_lines.push(x_str.clone());
                }
                if !y_updated {
                    result_lines.push(y_str.clone());
                }
                result_lines.push("---".to_string()); // 添加front matter结束标记
                front_matter_processed = true;
            }
            result_lines.push(line.to_string());
        }

        line_idx += 1;
    }

    // 如果整个文件都没有front matter，需要添加
    if !front_matter_processed {
        let new_content = format!(
            "---\nx: {}\ny: {}\n---\n{}",
            x,
            y,
            content
        );
        return Ok(new_content);
    }

    Ok(result_lines.join("\n"))
}

// 提取内容中的位置信息
fn extract_position_from_content(content: &str) -> (Option<f64>, Option<f64>) {
    let mut x = None;
    let mut y = None;

    // 查找Front Matter中的位置信息
    let lines: Vec<&str> = content.lines().collect();
    let mut in_front_matter = false;
    let mut line_idx = 0;

    while line_idx < lines.len() {
        let line = lines[line_idx];
        if line.trim() == "---" {
            if !in_front_matter {
                in_front_matter = true; // 开始front matter
            } else {
                break; // 结束front matter
            }
        } else if in_front_matter {
            if let Some(pos) = line.find(':') {
                let key = line[..pos].trim();
                let value = line[pos + 1..].trim();
                match key {
                    "x" => {
                        if let Ok(val) = value.parse::<f64>() {
                            x = Some(val);
                        }
                    }
                    "y" => {
                        if let Ok(val) = value.parse::<f64>() {
                            y = Some(val);
                        }
                    }
                    _ => {}
                }
            }
        }
        line_idx += 1;
    }

    (x, y)
}

// 更新内容
fn update_content_in_file(file_path: &Path, content: &str, x: Option<f64>, y: Option<f64>) -> Result<(), String> {
    let existing_content = if file_path.exists() {
        fs::read_to_string(file_path)
            .unwrap_or_else(|_| String::new())
    } else {
        String::new()
    };

    let final_content = if existing_content.starts_with("---") {
        // 如果已有front matter，只更新位置信息
        let updated_content = if let (Some(pos_x), Some(pos_y)) = (x, y) {
            update_position_in_content(&existing_content, pos_x, pos_y)?
        } else {
            existing_content
        };
        
        // 在front matter之后插入实际内容
        insert_content_after_front_matter(&updated_content, content)
    } else {
        // 如果没有front matter，创建新的
        if let (Some(pos_x), Some(pos_y)) = (x, y) {
            format!(
                "---\nx: {}\ny: {}\n---\n{}",
                pos_x, pos_y, content
            )
        } else {
            content.to_string()
        }
    };

    fs::write(file_path, final_content)
        .map_err(|e| format!("写入文件失败: {}", e))
}

// 在front matter之后插入内容
fn insert_content_after_front_matter(full_content: &str, new_content: &str) -> String {
    let lines: Vec<&str> = full_content.lines().collect();
    let mut result_lines = Vec::<String>::new();
    let mut in_front_matter = false;
    let mut front_matter_ended = false;
    let mut line_idx = 0;

    // 复制front matter部分
    while line_idx < lines.len() {
        let line = lines[line_idx];
        result_lines.push(line.to_string());

        if line.trim() == "---" {
            if !in_front_matter {
                in_front_matter = true;
            } else {
                // front matter结束
                front_matter_ended = true;
                break;
            }
        }
        line_idx += 1;
    }

    // 如果front matter已经结束，添加新内容
    if front_matter_ended {
        // 跳过结束的 ---
        line_idx += 1;
        
        // 添加新内容，替换掉原有的内容部分
        result_lines.push(String::new()); // 添加空行
        result_lines.push(new_content.to_string());
    } else {
        // 如果没找到front matter结束，添加内容
        result_lines.push(String::new());
        result_lines.push(new_content.to_string());
    }

    result_lines.join("\n")
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

// 获取活跃的便签列表
#[tauri::command]
async fn get_active_notes(window: tauri::WebviewWindow) -> Result<Vec<NoteMeta>, String> {
    let notes_dir = PathBuf::from(ensure_notes_directory(window).await?);
    let index = validate_and_fix_index(&notes_dir)?;

    let mut active_notes = Vec::new();
    for (id, entry) in &index.notes {
        if !entry.archived && !is_expired(&entry.expires_at)? {
            // 从文件中读取位置信息
            let file_path = notes_dir.join(&entry.path);
            let content = fs::read_to_string(&file_path)
                .unwrap_or_default(); // 如果读取失败则继续，不影响其他便签
            
            let (x, y) = extract_position_from_content(&content);

            active_notes.push(NoteMeta {
                id: id.clone(),
                created_at: entry.created_at.clone(),
                expires_at: entry.expires_at.clone(),
                x,
                y,
            });
        }
    }

    Ok(active_notes)
}

// 创建新的便签
#[tauri::command]
async fn create_note(window: tauri::WebviewWindow, id: String, x: Option<f64>, y: Option<f64>) -> Result<(), String> {
    let notes_dir = PathBuf::from(ensure_notes_directory(window).await?);
    
    // 创建带Front Matter的初始内容
    let created_at = get_current_iso8601_time();
    let expires_at = (DateTime::parse_from_rfc3339(&created_at)
        .map_err(|e| format!("解析时间失败: {}", e))?
        .naive_utc()
        .and_local_timezone(Utc)
        .unwrap() + Duration::days(7)).to_rfc3339();
    
    let content = format!(
        "---\nid: {}\ncreatedAt: \"{}\"\nexpiresAt: \"{}\"\nx: {}\ny: {}\n---\n",
        id,
        created_at,
        expires_at,
        x.unwrap_or(100.0),
        y.unwrap_or(100.0)
    );

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
            version: 1,
            last_updated_at: get_current_iso8601_time(),
            notes: HashMap::new(),
        }
    };

    let rel_path = file_path.strip_prefix(&notes_dir)
        .unwrap_or(&file_path)
        .to_string_lossy()
        .to_string();

    index.notes.insert(
        id.clone(),
        IndexNoteEntry {
            path: rel_path,
            created_at: created_at.clone(),
            expires_at: expires_at.clone(),
            updated_at: get_current_iso8601_time(),
            archived: false,
        }
    );

    index.last_updated_at = get_current_iso8601_time();

    let json_content = serde_json::to_string_pretty(&index)
        .map_err(|e| format!("序列化索引失败: {}", e))?;
    fs::write(&index_path, json_content)
        .map_err(|e| format!("写入索引文件失败: {}", e))?;

    Ok(())
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

    if let Some(entry) = index.notes.get(&id) {
        let file_path = notes_dir.join(&entry.path);
        if file_path.exists() {
            let content = fs::read_to_string(&file_path)
                .map_err(|e| format!("读取便签文件失败: {}", e))?;
            Ok(Some(content))
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

// 更新便签位置
#[tauri::command]
async fn update_note_position(window: tauri::WebviewWindow, id: String, x: f64, y: f64) -> Result<(), String> {
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

    if let Some(entry) = index.notes.get(&id) {
        let file_path = notes_dir.join(&entry.path);
        
        if !file_path.exists() {
            return Err("便签文件不存在".to_string());
        }

        // 读取当前文件内容
        let content = fs::read_to_string(&file_path)
            .map_err(|e| format!("读取便签文件失败: {}", e))?;

        // 更新Front Matter中的位置信息
        let updated_content = update_position_in_content(&content, x, y)?;

        // 写回文件
        fs::write(&file_path, updated_content)
            .map_err(|e| format!("写入便签文件失败: {}", e))?;

        // 更新索引中的更新时间
        if let Some(indexed_note) = index.notes.get_mut(&id) {
            indexed_note.updated_at = get_current_iso8601_time();
        }

        // 保存更新后的索引
        index.last_updated_at = get_current_iso8601_time();
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
async fn save_note_content(window: tauri::WebviewWindow, id: String, content: String, x: Option<f64>, y: Option<f64>) -> Result<(), String> {
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

    if let Some(entry) = index.notes.get(&id) {
        let file_path = notes_dir.join(&entry.path);
        
        if !file_path.exists() {
            return Err("便签文件不存在".to_string());
        }

        // 更新文件内容
        update_content_in_file(&file_path, &content, x, y)
            .map_err(|e| format!("更新内容失败: {}", e))?;

        // 更新索引中的更新时间
        if let Some(indexed_note) = index.notes.get_mut(&id) {
            indexed_note.updated_at = get_current_iso8601_time();
        }

        // 保存更新后的索引
        index.last_updated_at = get_current_iso8601_time();
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
            initialize_notes_directory,
            ensure_notes_directory,
            get_active_notes,
            create_note,
            load_note,
            update_note_position,
            save_note_content
        ])
        .setup(|app| {
            // 应用启动时初始化目录
            let window = app.get_webview_window("main").unwrap();
            
            // 初始化AppData目录
            tauri::async_runtime::block_on(async move {
                match initialize_notes_directory(window).await {
                    Ok(_) => println!("成功初始化便签目录"),
                    Err(e) => eprintln!("初始化便签目录失败: {}", e),
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}