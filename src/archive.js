// 获取当前窗口
const { getCurrentWindow } = window.__TAURI__.window;
const win = getCurrentWindow();

// 关闭按钮事件
document.getElementById('btn-close').addEventListener('click', () => {
  win.close();
});

// 加载归档便签列表
async function loadArchivedNotes() {
  try {
    const archivedNotes = await window.__TAURI__.core.invoke('get_archived_notes');
    const archiveList = document.getElementById('archive-list');
    
    if (archivedNotes.length === 0) {
      archiveList.innerHTML = '<div class="empty-state">暂无归档便签</div>';
      return;
    }
    
    // 清空列表
    archiveList.innerHTML = '';
    
    // 渲染每个归档便签
    archivedNotes.forEach(note => {
      const noteElement = document.createElement('div');
      noteElement.className = 'note-item';
      
      // 格式化时间
      const lastActiveTime = note.lastActiveAt ? new Date(note.lastActiveAt).toLocaleString() : '未知时间';
      
      // 获取预览内容，如果没有则显示占位符
      const previewText = note.cachedPreview || '(已归档便签)';
      
      noteElement.innerHTML = `
        <div class="note-preview">${previewText}</div>
        <div class="note-meta">
          <span>归档时间: ${lastActiveTime}</span>
          <span>ID: ${note.id.substring(0, 8)}...</span>
        </div>
      `;
      
      // 点击便签恢复它
      noteElement.addEventListener('click', async () => {
        try {
          // 恢复便签
          await window.__TAURI__.core.invoke('restore_note', { id: note.id });
          
          // 创建便签窗口
          const windowInfo = note.window || {
            x: 200,
            y: 200,
            width: 280,
            height: 360
          };
          
          const label = `note-${note.id}`;
          await window.__TAURI__.core.invoke('create_note_window', {
            label: label,
            title: "FadeNote",
            width: Math.round(windowInfo.width),
            height: Math.round(windowInfo.height),
            x: Math.round(windowInfo.x),
            y: Math.round(windowInfo.y)
          });
          
          // 重新加载列表
          loadArchivedNotes();
          
          console.log(`便签 ${note.id} 已恢复`);
        } catch (err) {
          console.error('恢复便签失败:', err);
        }
      });
      
      archiveList.appendChild(noteElement);
    });
  } catch (err) {
    console.error('加载归档便签失败:', err);
    const archiveList = document.getElementById('archive-list');
    archiveList.innerHTML = `<div class="error">加载失败: ${err.message}</div>`;
  }
}

// 页面加载完成后初始化
document.addEventListener('DOMContentLoaded', () => {
  loadArchivedNotes();
});