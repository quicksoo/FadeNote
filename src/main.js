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
  
  // 检查是否已设置保存目录，如果没有则立即提示用户设置
  try {
    const currentDir = await window.__TAURI__.core.invoke('get_notes_directory');
    if (!currentDir || currentDir === undefined) {
      // 如果没有设置目录，使用原生目录选择对话框
      const directory = await window.__TAURI__.core.invoke('show_directory_picker');
      if (directory) {
        const result = await window.__TAURI__.core.invoke('set_notes_directory', {
          directory: directory
        });
        document.getElementById("current-dir-display").textContent = `目录: ${result}`;
        console.log("便签目录已设置:", result);
      } else {
        alert("您必须选择一个目录来保存便签！");
        // 可以考虑递归调用或循环直到用户选择一个目录
        // 但为了简单起见，这里只是提醒用户
      }
    } else {
      document.getElementById("current-dir-display").textContent = `目录: ${currentDir}`;
    }
  } catch (err) {
    console.warn('获取或设置便签目录失败:', err);
    // 如果出错，也提示用户设置目录
    const directory = await window.__TAURI__.core.invoke('show_directory_picker');
    if (directory) {
      try {
        const result = await window.__TAURI__.core.invoke('set_notes_directory', {
          directory: directory
        });
        document.getElementById("current-dir-display").textContent = `目录: ${result}`;
        console.log("便签目录已设置:", result);
      } catch (setErr) {
        console.error('设置目录失败:', setErr);
      }
    }
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
    
    textarea.addEventListener('focus', () => {
      textarea.style.backgroundColor = "#fffdf5";
    });
    
    textarea.addEventListener('blur', () => {
      if (textarea.value.trim() === "") {
        textarea.style.backgroundColor = "transparent";
      }
    });
    
    // 监听文本变化并自动保存
    textarea.addEventListener('input', () => {
      // 清除之前的定时器
      if (saveTimer) {
        clearTimeout(saveTimer);
      }
      
      // 设置新的定时器，在用户停止输入1秒后保存
      saveTimer = setTimeout(async () => {
        try {
          await window.__TAURI__.core.invoke('save_note', {
            id: noteId,
            content: textarea.value
          });
          console.log(`便签 ${noteId} 已保存`);
        } catch (err) {
          console.error('保存便签失败:', err);
        }
      }, 1000); // 1秒延迟保存
    });
  }
});

// 监听来自后端的设置目录请求
window.__TAURI__.event.listen('request_set_directory', async () => {
  const directory = await window.__TAURI__.core.invoke('show_directory_picker');
  if (directory) {
    try {
      const result = await window.__TAURI__.core.invoke('set_notes_directory', {
        directory: directory
      });
      document.getElementById("current-dir-display").textContent = `目录: ${result}`;
      console.log("便签目录已设置:", result);
    } catch (err) {
      console.error('设置目录失败:', err);
    }
  } else {
    alert("您必须选择一个目录来保存便签！");
  }
});