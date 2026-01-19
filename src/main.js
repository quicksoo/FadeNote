// 获取当前窗口
const { getCurrentWindow } = window.__TAURI__.window;
const win = getCurrentWindow();
const textarea = document.querySelector(".paper-content");

// 便签ID - 将在窗口准备好后从后端获取
let noteId = null;

// 自动保存定时器
let saveTimer = null;
let idleTimer = null;

/* 顶栏按钮功能 - 修复版本 */
document.getElementById("btn-close").addEventListener('click', () => {
  win.close();
});

document.getElementById("btn-min").addEventListener('click', () => {
  win.minimize();
});

// 固定按钮功能
document.getElementById("btn-top").addEventListener('click', async () => {
  try {
    const top = await win.isAlwaysOnTop();
    await win.setAlwaysOnTop(!top);
    
    // 更新按钮样式以反映当前状态
    const topBtn = document.getElementById("btn-top");
    if (top) {
      topBtn.classList.remove('active');
      topBtn.title = "置顶";
    } else {
      topBtn.classList.add('active');
      topBtn.title = "取消置顶";
    }
  } catch (err) {
    console.error('切换置顶状态失败:', err);
  }
});

// 创建新便签
async function createNewNote() {
  const position = await win.innerPosition();
  const size = await win.innerSize();
  
  try {
    noteId = await window.__TAURI__.core.invoke('create_note', {
      x: position.x,
      y: position.y,
      width: size.width,
      height: size.height
    });
    
    console.log("创建新便签:", noteId);
    
    // 更新活动时间
    await window.__TAURI__.core.invoke('update_note_activity', { id: noteId });
  } catch (err) {
    console.error('创建便签失败:', err);
  }
}

/* 双击 textarea → 新建便签窗口 */
textarea.addEventListener("dblclick", async (e) => {
  e.preventDefault();
  e.stopPropagation();

  const label = `note-${Date.now()}`;
  console.log("创建新便签窗口:", label);

  // 获取当前窗口的位置和尺寸
  const position = await win.innerPosition();
  const size = await win.innerSize();

  try {
    // 使用 Tauri v2 的 invoke API 调用Rust命令
    await window.__TAURI__.core.invoke('create_note_window', {
      label: label,
      title: "便签",
      width: size.width,
      height: size.height,
      x: Math.round(position.x + 20),  // 确保坐标是整数
      y: Math.round(position.y + 20)
    });

    console.log("新窗口创建成功:", label);

  } catch (err) {
    console.error('创建窗口失败:', err);
  }
});

// 页面加载完成后初始化
document.addEventListener('DOMContentLoaded', async () => {
  console.log("便签窗口已加载");
  
  // 自动获取AppData目录，不再需要用户手动设置
  try {
    const currentDir = await window.__TAURI__.core.invoke('ensure_notes_directory');
    if (currentDir) {
      document.getElementById("current-dir-display").textContent = `目录: ${currentDir}`;
    } else {
      console.warn('无法获取便签目录');
    }
  } catch (err) {
    console.error('获取便签目录失败:', err);
  }
  
  // 尝试从后端获取便签ID
  try {
    // 创建新的便签
    await createNewNote();
  } catch (err) {
    console.error('获取或创建便签ID失败:', err);
    // 如果获取失败，尝试创建新的便签
    await createNewNote();
  }
  
  // 等待noteId被设置
  if (!noteId) {
    console.error('未能获取或创建便签ID');
    return;
  }
  
  console.log("便签ID:", noteId);
  
  // 加载便签的位置信息
  try {
    const activeNotes = await window.__TAURI__.core.invoke('get_active_notes');
    const noteDetail = activeNotes.find(note => note.id === noteId);
    
    if (noteDetail) {
      // 恢复窗口位置和大小
      await win.setPosition(new window.__TAURI__.window.Position(noteDetail.window.x, noteDetail.window.y));
      await win.setSize(new window.__TAURI__.window.Size(noteDetail.window.width, noteDetail.window.height));
    }
  } catch (err) {
    console.warn('获取便签位置信息失败:', err);
  }
  
  // 尝试加载现有的便签内容
  try {
    const savedContent = await window.__TAURI__.core.invoke('load_note', { id: noteId });
    if (savedContent) {
      textarea.value = savedContent;
    }
  } catch (err) {
    console.warn('加载便签内容失败:', err);
  }
  
  // 设置默认文本（如果没有内容）
  if (textarea && !textarea.value) {
    textarea.value = "";
  }
  
  // 添加焦点/失焦效果
  if (textarea) {
    textarea.placeholder = "写点什么…";
    
    textarea.addEventListener('focus', async () => {
      textarea.style.backgroundColor = "#fffdf5";
      
      // 窗口获得焦点时更新活动时间
      if (noteId) {
        try {
          await window.__TAURI__.core.invoke('update_note_activity', { id: noteId });
        } catch (err) {
          console.error('更新便签活动时间失败:', err);
        }
      }
    });
    
    textarea.addEventListener('blur', () => {
      if (textarea.value.trim() === "") {
        textarea.style.backgroundColor = "transparent";
      }
    });
    
    // 监听文本变化（但不立即保存）
    textarea.addEventListener('input', () => {
      // 清除之前的空闲计时器
      if (idleTimer) {
        clearTimeout(idleTimer);
      }
      
      // 设置新的空闲计时器，3秒无操作后触发
      idleTimer = setTimeout(async () => {
        if (noteId && textarea.value) {
          try {
            await window.__TAURI__.core.invoke('save_note_content', {
              id: noteId,
              content: textarea.value
            });
            console.log(`便签 ${noteId} 内容已保存，长度: ${textarea.value.length}`);
          } catch (err) {
            console.error('保存便签内容失败:', err);
          }
        }
      }, 3000); // 3秒空闲后保存
      
      // 清除之前的保存计时器
      if (saveTimer) {
        clearTimeout(saveTimer);
      }
    });
  }
  
  // 监听窗口位置变化并更新到后端
  let positionUpdateTimer = null;
  
  // 监听鼠标拖拽结束事件来更新位置和活动时间
  document.addEventListener('mouseup', async () => {
    // 防抖处理，避免频繁更新
    if (positionUpdateTimer) {
      clearTimeout(positionUpdateTimer);
    }
    
    positionUpdateTimer = setTimeout(async () => {
      if (noteId) {
        try {
          const position = await win.innerPosition();
          const size = await win.innerSize();
          
          // 更新窗口信息
          await window.__TAURI__.core.invoke('update_note_window', {
            id: noteId,
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height
          });
          
          // 更新活动时间（窗口拖拽时）
          await window.__TAURI__.core.invoke('update_note_activity', { id: noteId });
          
          console.log(`便签 ${noteId} 位置和大小已更新: (${position.x}, ${position.y}, ${size.width}x${size.height})`);
        } catch (err) {
          console.error('更新便签窗口信息失败:', err);
        }
      }
    }, 500); // 500ms延迟更新
  });
});

// 移除监听来自后端的设置目录请求，因为我们不再需要手动设置目录