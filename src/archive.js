function tr(key, values) {
  return window.FadeNoteI18n?.t(key, values) || key;
}

function escapeHtml(value) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#039;');
}

async function showArchiveConfirm(title, message) {
  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.style.cssText = `
      position:fixed;inset:0;display:flex;justify-content:center;align-items:center;
      background:rgba(0,0,0,0.25);backdrop-filter:blur(3px);z-index:10000;
      opacity:1;transition:opacity 0.15s ease;
    `;

    const dialog = document.createElement('div');
    dialog.style.cssText = `
      background:var(--panel-bg, #fffdf5);border-radius:12px;padding:22px 24px;width:280px;max-width:85%;
      box-shadow:0 20px 40px rgba(0,0,0,0.15);border:1px solid var(--border, #e8e2d6);
      font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;
      animation:fadeInScale 0.18s ease-out;transition:opacity 0.15s ease,transform 0.15s ease;
    `;

    dialog.innerHTML = ''
      + '<div style="text-align:center;margin-bottom:18px;">'
      + '<div style="font-size:16px;font-weight:600;color:var(--text, #333);margin-bottom:6px;">' + escapeHtml(title) + '</div>'
      + '<div style="font-size:13px;color:var(--muted-text, #888);">' + escapeHtml(message) + '</div>'
      + '</div>'
      + '<div style="display:flex;justify-content:flex-end;gap:12px;">'
      + '<button id="archive-cancel-btn" style="background:none;border:none;color:var(--muted-text, #666);font-size:14px;cursor:pointer;">' + tr('common.cancel') + '</button>'
      + '<button id="archive-confirm-btn" style="background:none;border:none;color:#d9534f;font-size:14px;font-weight:500;cursor:pointer;">' + tr('common.delete') + '</button>'
      + '</div>';

    const style = document.createElement('style');
    style.textContent = `
      @keyframes fadeInScale { from { opacity:0; transform:scale(0.96); } to { opacity:1; transform:scale(1); } }
      #archive-cancel-btn:hover { opacity:0.6; }
      #archive-confirm-btn:hover { opacity:0.7; }
    `;
    document.head.appendChild(style);
    overlay.appendChild(dialog);
    document.body.appendChild(overlay);

    let closed = false;
    function closeDialog(result) {
      if (closed) return;
      closed = true;
      document.removeEventListener('keydown', escHandler);
      dialog.style.opacity = '0';
      dialog.style.transform = 'scale(0.96)';
      overlay.style.opacity = '0';
      setTimeout(() => {
        overlay.remove();
        style.remove();
        resolve(result);
      }, 150);
    }

    function escHandler(event) {
      if (event.key === 'Escape') closeDialog(false);
    }

    dialog.querySelector('#archive-cancel-btn').addEventListener('click', () => closeDialog(false));
    dialog.querySelector('#archive-confirm-btn').addEventListener('click', () => closeDialog(true));
    overlay.addEventListener('click', (event) => {
      if (event.target === overlay) closeDialog(false);
    });
    document.addEventListener('keydown', escHandler);
  });
}


// 加载归档便签列表
async function loadArchivedNotes() {
  try {
    const archivedNotes = await window.__TAURI__.core.invoke('get_archived_notes');
    const archiveList = document.getElementById('archive-list');
    
    if (archivedNotes.length === 0) {
      archiveList.innerHTML = '<div class="empty-state">' + tr('archive.empty') + '</div>';
      return;
    }
    
    // 清空列表
    archiveList.innerHTML = '';
    
    // 渲染每个归档便签
    archivedNotes.forEach(note => {
      const noteElement = document.createElement('div');
      noteElement.className = 'note-item';
      
      // 格式化归档时间，缺失时再回退到最后活跃时间
      const archiveTimeSource = note.archivedAt || note.lastActiveAt;
      const archivedTime = archiveTimeSource ? new Date(archiveTimeSource).toLocaleString(document.documentElement.lang) : tr('archive.unknownTime');
      
      // 获取预览内容，如果没有则显示占位符
      const previewText = note.cachedPreview || tr('archive.placeholder');
      
      noteElement.innerHTML = `
        <div class="note-preview">${escapeHtml(previewText)}</div>
        <div class="note-meta">
          <span>${escapeHtml(tr('archive.archived', { time: archivedTime }))}</span>
          <div class="note-actions">
            <button class="archive-action restore-action" type="button">${escapeHtml(tr('archive.restore'))}</button>
            <button class="archive-action delete-action" type="button">${escapeHtml(tr('archive.delete'))}</button>
          </div>
        </div>
      `;
      
      // 点击便签恢复它
      const restoreArchivedNote = async () => {
        try {
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
          
          console.log(`Note ${note.id} restored`);
        } catch (err) {
          console.error('Failed to restore note:', err);
        }
      };

      noteElement.addEventListener('click', (event) => {
        if (event.target.closest('.archive-action')) return;
        restoreArchivedNote();
      });

      noteElement.querySelector('.restore-action').addEventListener('click', (event) => {
        event.stopPropagation();
        restoreArchivedNote();
      });

      const deleteButton = noteElement.querySelector('.delete-action');
      deleteButton.addEventListener('pointerdown', (event) => {
        event.stopPropagation();
      });
      deleteButton.addEventListener('click', async (event) => {
        event.preventDefault();
        event.stopPropagation();
        event.stopImmediatePropagation();

        const shouldDelete = await showArchiveConfirm(tr('archive.deleteTitle'), tr('archive.deleteMessage'));
        if (!shouldDelete) return;

        try {
          await window.__TAURI__.core.invoke('delete_note', { id: note.id });
          loadArchivedNotes();
          console.log('Note ' + note.id + ' deleted');
        } catch (err) {
          console.error('Failed to delete note:', err);
        }
      });
      
      archiveList.appendChild(noteElement);
    });
  } catch (err) {
    console.error('Failed to load archived notes:', err);
    const archiveList = document.getElementById('archive-list');
    archiveList.innerHTML = `<div class="error">${escapeHtml(tr('archive.loadFailed', { message: err.message }))}</div>`;
  }
}

// 页面加载完成后初始化
document.addEventListener('DOMContentLoaded', () => {
  loadArchivedNotes();
  window.__TAURI__?.event?.listen('fadenote://language-changed', () => {
    setTimeout(loadArchivedNotes, 0);
  });
});
