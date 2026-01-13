const win = window.__TAURI__.window.getCurrentWindow();
const textarea = document.querySelector(".paper-content");

/* 顶栏按钮 */
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
  console.log("create:", label);

  await window.__TAURI__.core.invoke("plugin:window|create", {
    url: "index.html",
    options: {
      label,               // ✅ 关键在这里
      width: 320,
      height: 420,
      decorations: false,
      transparent: false,
      resizable: true,
      alwaysOnTop: false
    }
  });
});
