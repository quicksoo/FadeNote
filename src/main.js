// 获取当前窗口
const { getCurrentWindow } = window.__TAURI__.window;
const win = getCurrentWindow();
const textarea = document.querySelector(".paper-content");

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
document.addEventListener('DOMContentLoaded', () => {
  console.log("便签窗口已加载");
  
  // 设置默认文本
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
  }
});