// 获取当前窗口
const { getCurrentWindow } = window.__TAURI__.window;
const win = getCurrentWindow();
const textarea = document.querySelector(".paper-content");

// 便签ID - 从窗口标签中提取
const noteId = win.label.startsWith('note-') ? win.label : `note-${Date.now()}`;

// 自动保存定时器
let saveTimer = null;

/* 顶栏按钮功能 */
document.getElementById("btn-close").onclick = () => win.close();
document.getElementById("btn-min").onclick = () => win.minimize();
document.getElementById("btn-top").onclick = async () => {
  const top = await win.isAlwaysOnTop();
  await win.setAlwaysOnTop(!top);
};

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
  console.log("便签ID:", noteId);
  
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
  
  // 加载便签的位置信息
  try {
    const activeNotes = await window.__TAURI__.core.invoke('get_active_notes');
    const noteDetail = activeNotes.find(note => note.id === noteId);
    
    if (noteDetail && noteDetail.x !== undefined && noteDetail.y !== undefined) {
      // 恢复窗口位置
      await win.setPosition(new window.__TAURI__.window.Position(noteDetail.x, noteDetail.y));
    }
  } catch (err) {
    console.warn('获取便签位置信息失败:', err);
  }
  
  // 尝试加载现有的便签内容
  try {
    const savedContent = await window.__TAURI__.core.invoke('load_note', { id: noteId });
    if (savedContent) {
      // 如果内容包含Front Matter，去除它只保留实际内容
      if (savedContent.startsWith('---')) {
        const lines = savedContent.split('\n');
        let contentStart = 0;
        let frontMatterFound = false;
        
        for (let i = 0; i < lines.length; i++) {
          if (lines[i].trim() === '---') {
            if (!frontMatterFound) {
              frontMatterFound = true;
            } else {
              contentStart = i + 1;
              break;
            }
          }
        }
        
        if (contentStart > 0) {
          // 提取内容部分
          let actualContent = lines.slice(contentStart).join('\n');
          // 去除开头和结尾的空行
          actualContent = actualContent.replace(/^\s+|\s+$/g, '');
          textarea.value = actualContent;
        } else {
          textarea.value = savedContent;
        }
      } else {
        textarea.value = savedContent;
      }
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
    
    textarea.addEventListener('focus', () => {
      textarea.style.backgroundColor = "#fffdf5";
    });
    
    textarea.addEventListener('blur', () => {
      if (textarea.value.trim() === "") {
        textarea.style.backgroundColor = "transparent";
      }
    });
    
    // 监听文本变化并自动保存内容
    textarea.addEventListener('input', () => {
      // 清除之前的定时器
      if (saveTimer) {
        clearTimeout(saveTimer);
      }
      
      // 设置新的定时器，在用户停止输入1秒后保存
      saveTimer = setTimeout(async () => {
        try {
          // 获取当前位置
          const position = await win.innerPosition();
          
          // 先确保便签存在（创建或更新）
          await window.__TAURI__.core.invoke('create_note', {
            id: noteId,
            x: Math.round(position.x),  // 当前窗口x坐标
            y: Math.round(position.y)   // 当前窗口y坐标
          });
          
          // 然后保存内容到该便签
          await window.__TAURI__.core.invoke('save_note_content', {
            id: noteId,
            content: textarea.value,
            x: Math.round(position.x),
            y: Math.round(position.y)
          });
          
          console.log(`便签 ${noteId} 内容已保存，长度: ${textarea.value.length}`);
        } catch (err) {
          console.error('保存便签内容失败:', err);
        }
      }, 1000); // 1秒延迟保存
    });
  }
  
  // 监听窗口位置变化并更新到后端
  let positionUpdateTimer = null;
  
  // 监听鼠标拖拽结束事件来更新位置
  document.addEventListener('mouseup', async () => {
    // 防抖处理，避免频繁更新
    if (positionUpdateTimer) {
      clearTimeout(positionUpdateTimer);
    }
    
    positionUpdateTimer = setTimeout(async () => {
      try {
        const position = await win.innerPosition();
        await window.__TAURI__.core.invoke('update_note_position', {
          id: noteId,
          x: Math.round(position.x),
          y: Math.round(position.y)
        });
        console.log(`便签 ${noteId} 位置已更新: (${position.x}, ${position.y})`);
      } catch (err) {
        console.error('更新便签位置失败:', err);
      }
    }, 500); // 500ms延迟更新
  });
});

// 移除监听来自后端的设置目录请求，因为我们不再需要手动设置目录