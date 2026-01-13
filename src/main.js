const win = window.__TAURI__.window.getCurrentWindow();

document.getElementById("btn-close").onclick = () => win.close();
document.getElementById("btn-min").onclick = () => win.minimize();
document.getElementById("btn-top").onclick = async () => {
  const top = await win.isAlwaysOnTop();
  await win.setAlwaysOnTop(!top);
};
