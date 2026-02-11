// 获取当前窗口
const { getCurrentWindow } = window.__TAURI__.window;
const win = getCurrentWindow();
const textarea = document.querySelector(".paper-content");

// 便签ID - 将在窗口准备好后从后端获取
let noteId = null;
let noteIdSet = false; // 标志，防止noteId被重复设置

// 从URL参数中获取noteId（如果存在）
const urlParams = new URLSearchParams(window.location.search);
const urlNoteId = urlParams.get('noteId');

// 如果URL中包含noteId，直接使用它
if (urlNoteId) {
  noteId = urlNoteId;
  noteIdSet = true;
}

// 全局标志，防止重复初始化
let hasInitialized = false;

// 自动保存定时器
let saveTimer = null;
let idleTimer = null;

// 固定/取消固定便签功能
let isPinned = false; // 本地状态跟踪

// 初始化按钮事件绑定
function initializeButtonEvents() {
  /* 顶栏按钮功能 */
  document.getElementById("btn-close").addEventListener('click', () => {
    win.close();
  });

  document.getElementById("btn-min").addEventListener('click', () => {
    win.minimize();
  });

  // Always on Top按钮功能
  document.getElementById("btn-top").addEventListener('click', async () => {
    try {
      const top = await win.isAlwaysOnTop();
      await win.setAlwaysOnTop(!top);
      
      // Update button style to reflect current state
      const topBtn = document.getElementById("btn-top");
      if (top) {
        topBtn.classList.remove('active');
        topBtn.title = "Always on Top";
      } else {
        topBtn.classList.add('active');
        topBtn.title = "Remove from Top";
      }
    } catch (err) {
      console.error('Failed to toggle always-on-top state:', err);
    }
  });

  // 固定/取消固定按钮点击事件
  document.getElementById("btn-pin").addEventListener('click', async () => {
    if (!noteId) {
      console.error('noteId not set');
      return;
    }
    
    try {
      // 切换固定状态
      isPinned = !isPinned;
      
      // 调用后端API更新固定状态
      await window.__TAURI__.core.invoke('set_note_pinned', {
        id: noteId,
        pinned: isPinned
      });
      
      // 更新按钮样式
      updatePinButtonStyle();
      
      console.log(`Note ${noteId} ${isPinned ? 'pinned' : 'unpinned'}`);
    } catch (err) {
      console.error('Failed to set pin status:', err);
      // 如果失败，恢复按钮状态
      isPinned = !isPinned;
      updatePinButtonStyle();
    }
  });
}

// 从后端获取当前便签的固定状态
async function updatePinStatus() {
  if (noteId) {
    try {
      // 获取便签详情以确定当前固定状态
      const activeNotes = await window.__TAURI__.core.invoke('get_active_notes');
      const noteDetail = activeNotes.find(note => note.id === noteId);
      
      if (noteDetail) {
        isPinned = noteDetail.pinned || false;
        updatePinButtonStyle();
      }
    } catch (err) {
      console.warn('Failed to get note pin status:', err);
    }
  }
}

// 更新固定按钮样式
function updatePinButtonStyle() {
  const pinBtn = document.getElementById("btn-pin");
  if (pinBtn) {
    if (isPinned) {
      pinBtn.classList.add('active');
      pinBtn.title = "Unpin";
    } else {
      pinBtn.classList.remove('active');
      pinBtn.title = "Pin";
    }
  }
}

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
    
    // 创建便签时不更新活动时间
    // Activity time only updates when content changes substantially
  } catch (err) {
    console.error('Failed to create note:', err);
  }
}

/* 双击 textarea → 新建便签窗口 */
textarea.addEventListener("dblclick", async (e) => {
  e.preventDefault();
  e.stopPropagation();

  try {
    // Call Rust to create new note, this will create note file and index entry
    const position = await win.innerPosition();
    const size = await win.innerSize();
    
    const newNoteId = await window.__TAURI__.core.invoke('create_note', {
      x: position.x + 20,
      y: position.y + 20,
      width: size.width,
      height: size.height
    });
    
    // 创建对应的新窗口
    const label = `note-${newNoteId}`;

    await window.__TAURI__.core.invoke('create_note_window', {
      label: label,
      title: "FadeNote",
      width: size.width,
      height: size.height,
      x: Math.round(position.x + 20),
      y: Math.round(position.y + 20)
    });

  } catch (err) {
    console.error('Failed to create new note:', err);
  }
});

// 页面加载完成后初始化
document.addEventListener('DOMContentLoaded', async () => {
  // 防止重复初始化
  if (hasInitialized) {
    return;
  }
  hasInitialized = true;
  
  // 初始化按钮事件
  initializeButtonEvents();
  
  // Auto-get AppData directory, no manual setting needed
  try {
    const currentDir = await window.__TAURI__.core.invoke('ensure_notes_directory');
    if (currentDir) {
      document.getElementById("current-dir-display").textContent = `Directory: ${currentDir}`;
    } else {
      console.warn('Unable to get notes directory');
    }
  } catch (err) {
    console.error('Failed to get notes directory:', err);
  }
  
  // 检查当前窗口标签，如果是main窗口则不执行便签逻辑
  let windowLabel = '';
  
  // 如果noteId已经通过URL参数设置，说明这是从Rust传递过来的，直接使用它
  if (noteId && noteIdSet) {
    // Got noteId from URL parameter, no need to get from window label
    windowLabel = `note-${noteId}`;

  } else {
    // 如果noteId未通过URL参数设置，尝试获取窗口标签
    try {
      // Tauri v2中获取窗口标签
      const currentWindow = window.__TAURI__.window.getCurrentWindow();
      windowLabel = currentWindow.label;

      
      // 从窗口标签中提取noteId
      const match = windowLabel.match(/note-(.*)/);
      if (match && match[1]) {
        // This is a note window, use ID from label
        noteId = match[1];
        noteIdSet = true;

      }
    } catch (err) {
      console.warn('Unable to get window label:', err);
      
      // If nothing can be obtained, create new note

      await createNewNote();
      noteIdSet = true;
      return; // 不继续执行可能出错的逻辑
    }
  }
  
  if (windowLabel === 'main') {

    return; // 主窗口不执行便签逻辑
  }
  
  // 现在我们已经有了noteId，继续后续初始化

  
  // 等待noteId被设置
  if (!noteId) {
    console.error('Failed to get or create note ID');
    return;
  }
  

  
  // Load note position information
  try {
    const activeNotes = await window.__TAURI__.core.invoke('get_active_notes');
    const noteDetail = activeNotes.find(note => note.id === noteId);
    
    if (noteDetail) {
      // Restore window position and size
      await win.setPosition(new window.__TAURI__.window.Position(noteDetail.window.x, noteDetail.window.y));
      await win.setSize(new window.__TAURI__.window.Size(noteDetail.window.width, noteDetail.window.height));
    }
    
    // Initialize pin status
    await updatePinStatus();
  } catch (err) {
    console.warn('Failed to get note position info:', err);
  }
  
  // 尝试加载现有的便签内容
  try {
    const savedContent = await window.__TAURI__.core.invoke('load_note', { id: noteId });
    if (savedContent) {
      textarea.value = savedContent;
    }
  } catch (err) {
    console.warn('Failed to load note content:', err);
  }
  
  // Set default text (if no content)
  if (textarea && !textarea.value) {
    textarea.value = "";
  }
  
  // Listen to window close event to ensure content is saved
  // Use beforeUnload event as backup
  window.addEventListener('beforeunload', async () => {
    if (noteId && textarea) {
      try {
        // Save current content immediately
        await window.__TAURI__.core.invoke('save_note_content', {
          id: noteId,
          content: textarea.value
        });

      } catch (err) {
        console.error('Failed to save note content:', err);
      }
    }
  });
  
  // 添加焦点/失焦效果
  if (textarea) {
    textarea.placeholder = "Write something...";
    
    textarea.addEventListener('focus', async () => {
      textarea.style.backgroundColor = "#fffdf5";
      
      // Don't update activity time when window gains focus
      // Activity time only updates when content changes substantially
    });
    
    textarea.addEventListener('blur', () => {
      if (textarea.value.trim() === "") {
        textarea.style.backgroundColor = "transparent";
      }
    });
    
    // Listen to text changes (but don't save immediately)
    textarea.addEventListener('input', () => {
      // Clear previous idle timer
      if (idleTimer) {
        clearTimeout(idleTimer);
      }
      
      // Set new idle timer, triggers after 3 seconds of inactivity
      idleTimer = setTimeout(async () => {
        if (noteId !== null) {  // 只要noteId存在就保存，无论内容是否为空

          try {
            await window.__TAURI__.core.invoke('save_note_content', {
              id: noteId,
              content: textarea.value  // 保存当前值，即使是空字符串
            });

          } catch (err) {
            console.error('Failed to save note content:', err);
          }
        } else {

        }
      }, 3000); // 3秒空闲后保存
      
      // Clear previous save timer
      if (saveTimer) {
        clearTimeout(saveTimer);
      }
    });
  }

  // 初始化 Always on Top 状态
try {
  const top = await win.isAlwaysOnTop();
  const topBtn = document.getElementById("btn-top");
  if (top) {
    topBtn.classList.add("active");
    topBtn.title = "Remove from Top";
  }
} catch {}
  
  // Listen to window position changes and update to backend
  let positionUpdateTimer = null;
  
  // Listen to mouse drag end event to update position and activity time
  document.addEventListener('mouseup', async () => {
    // Debounce processing, avoid frequent updates
    if (positionUpdateTimer) {
      clearTimeout(positionUpdateTimer);
    }
    
    positionUpdateTimer = setTimeout(async () => {
      if (noteId) {
        try {
          const position = await win.innerPosition();
          const size = await win.innerSize();
          
          // Update window info
          await window.__TAURI__.core.invoke('update_note_window', {
            id: noteId,
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height
          });
          

        } catch (err) {
          console.error('Failed to update note window info:', err);
        }
      }
    }, 500); // 500ms delay update
  });
});