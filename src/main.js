const { getCurrentWindow } = window.__TAURI__.window;
const win = getCurrentWindow();
const editor = document.querySelector(".paper-content");
const toolbar = document.querySelector(".editor-toolbar");
const paper = document.querySelector(".paper");

async function showCustomConfirm(title, message) {
  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.style.cssText = `
      position:fixed;inset:0;display:flex;justify-content:center;align-items:center;
      background:rgba(0,0,0,0.25);backdrop-filter:blur(3px);z-index:10000;
    `;

    const dialog = document.createElement('div');
    dialog.style.cssText = `
      background:#fffdf5;border-radius:12px;padding:22px 24px;width:280px;max-width:85%;
      box-shadow:0 20px 40px rgba(0,0,0,0.15);border:1px solid #e8e2d6;
      font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;
      animation:fadeInScale 0.18s ease-out;
    `;

    dialog.innerHTML = `
      <div style="text-align:center;margin-bottom:18px;">
        <div style="font-size:16px;font-weight:600;color:#333;margin-bottom:6px;">${title}</div>
        <div style="font-size:13px;color:#888;">${message}</div>
      </div>
      <div style="display:flex;justify-content:flex-end;gap:12px;">
        <button id="cancel-btn" style="background:none;border:none;color:#666;font-size:14px;cursor:pointer;">Cancel</button>
        <button id="confirm-btn" style="background:none;border:none;color:#d9534f;font-size:14px;font-weight:500;cursor:pointer;">Delete</button>
      </div>
    `;

    const style = document.createElement('style');
    style.textContent = `
      @keyframes fadeInScale { from { opacity:0; transform:scale(0.96); } to { opacity:1; transform:scale(1); } }
      #cancel-btn:hover { opacity:0.6; }
      #confirm-btn:hover { opacity:0.7; }
    `;
    document.head.appendChild(style);
    overlay.appendChild(dialog);
    document.body.appendChild(overlay);

    function closeDialog(result) {
      dialog.style.opacity = "0";
      dialog.style.transform = "scale(0.96)";
      overlay.style.opacity = "0";
      setTimeout(() => {
        document.body.removeChild(overlay);
        document.head.removeChild(style);
        resolve(result);
      }, 150);
    }

    dialog.querySelector('#cancel-btn').addEventListener('click', () => closeDialog(false));
    dialog.querySelector('#confirm-btn').addEventListener('click', () => closeDialog(true));
    overlay.addEventListener('click', (e) => {
      if (e.target === overlay) closeDialog(false);
    });
    document.addEventListener('keydown', function escHandler(e) {
      if (e.key === 'Escape') {
        document.removeEventListener('keydown', escHandler);
        closeDialog(false);
      }
    });
  });
}

let noteId = null;
let noteIdSet = false;
let hasInitialized = false;
let saveTimer = null;
let idleTimer = null;
let isPinned = false;
let markdownSource = "";
let isRendering = false;
let isComposing = false;

const urlParams = new URLSearchParams(window.location.search);
const urlNoteId = urlParams.get('noteId');
if (urlNoteId) {
  noteId = urlNoteId;
  noteIdSet = true;
}

function escapeHtml(value) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#039;');
}

function plainTitleFromMarkdown(markdown) {
  const firstLine = markdown
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find(Boolean);

  if (!firstLine) return "FadeNote - New Note";

  const text = firstLine
    .replace(/^[-*]\s+/, '')
    .replace(/^[-*]\s+\[[ xX]\]\s+/, '')
    .replace(/\*\*([^*]+)\*\*/g, '$1')
    .replace(/~~([^~]+)~~/g, '$1')
    .trim()
    .slice(0, 40);

  return `FadeNote - ${text || 'New Note'}`;
}

async function updateWindowTitle() {
  try {
    await win.setTitle(plainTitleFromMarkdown(markdownSource));
  } catch (err) {
    console.warn('Failed to update window title:', err);
  }
}

function getCaretOffset() {
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0 || !editor.contains(selection.anchorNode)) return 0;
  const range = selection.getRangeAt(0).cloneRange();
  range.selectNodeContents(editor);
  range.setEnd(selection.anchorNode, selection.anchorOffset);
  return range.toString().length;
}

function setCaretOffset(offset) {
  const selection = window.getSelection();
  if (!selection) return;

  let remaining = Math.max(0, offset);
  const walker = document.createTreeWalker(editor, NodeFilter.SHOW_TEXT);
  let node = walker.nextNode();

  while (node) {
    if (remaining <= node.nodeValue.length) {
      const range = document.createRange();
      range.setStart(node, remaining);
      range.collapse(true);
      selection.removeAllRanges();
      selection.addRange(range);
      return;
    }
    remaining -= node.nodeValue.length;
    node = walker.nextNode();
  }

  const range = document.createRange();
  range.selectNodeContents(editor);
  range.collapse(false);
  selection.removeAllRanges();
  selection.addRange(range);
}

function getSelectionOffsets() {
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0 || !editor.contains(selection.anchorNode)) {
    const offset = markdownSource.length;
    return { start: offset, end: offset };
  }

  const range = selection.getRangeAt(0);
  const startRange = range.cloneRange();
  startRange.selectNodeContents(editor);
  startRange.setEnd(range.startContainer, range.startOffset);

  const endRange = range.cloneRange();
  endRange.selectNodeContents(editor);
  endRange.setEnd(range.endContainer, range.endOffset);

  return {
    start: startRange.toString().length,
    end: endRange.toString().length
  };
}

function setSelectionOffsets(start, end) {
  setCaretOffset(start);
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0) return;

  const startPosition = findTextPosition(start);
  const endPosition = findTextPosition(end);
  if (!startPosition || !endPosition) return;

  const range = document.createRange();
  range.setStart(startPosition.node, startPosition.offset);
  range.setEnd(endPosition.node, endPosition.offset);
  selection.removeAllRanges();
  selection.addRange(range);
}

function findTextPosition(offset) {
  let remaining = Math.max(0, offset);
  const walker = document.createTreeWalker(editor, NodeFilter.SHOW_TEXT);
  let node = walker.nextNode();

  while (node) {
    if (remaining <= node.nodeValue.length) return { node, offset: remaining };
    remaining -= node.nodeValue.length;
    node = walker.nextNode();
  }

  const last = editor.lastChild;
  if (last && last.nodeType === Node.TEXT_NODE) return { node: last, offset: last.nodeValue.length };
  return null;
}

function renderInlineMarkdown(line) {
  return escapeHtml(line)
    .replace(/\*\*([^*\n]+)\*\*/g, '<span class="md-syntax">**</span><span class="md-bold">$1</span><span class="md-syntax">**</span>')
    .replace(/~~([^~\n]+)~~/g, '<span class="md-syntax">~~</span><span class="md-strike">$1</span><span class="md-syntax">~~</span>');
}

function renderMarkdownLine(line, lineIndex) {
  const taskMatch = line.match(/^(\s*)- \[([ xX])\]\s?(.*)$/);
  if (taskMatch) {
    const checked = taskMatch[2].toLowerCase() === 'x';
    return `${escapeHtml(taskMatch[1])}<span class="md-list-marker">- </span><span class="md-task-marker" data-line="${lineIndex}">[${checked ? 'x' : ' '}]</span> ${renderInlineMarkdown(taskMatch[3])}`;
  }

  const listMatch = line.match(/^(\s*)-\s+(.*)$/);
  if (listMatch) {
    return `${escapeHtml(listMatch[1])}<span class="md-list-marker">- </span>${renderInlineMarkdown(listMatch[2])}`;
  }

  return renderInlineMarkdown(line);
}

function renderMarkdown(preserveCaret = true) {
  const caretOffset = preserveCaret ? getCaretOffset() : 0;
  isRendering = true;
  editor.innerHTML = markdownSource
    .split('\n')
    .map((line, index) => renderMarkdownLine(line, index))
    .join('\n');
  isRendering = false;
  if (preserveCaret) setCaretOffset(Math.min(caretOffset, editor.textContent.length));
}

function readMarkdownFromEditor() {
  return editor.innerText
    .replace(/\r\n/g, '\n')
    .replace(/\u00a0/g, ' ')
    .replace(/\n$/, '');
}

function setMarkdownSource(value, preserveCaret = false) {
  markdownSource = value || "";
  renderMarkdown(preserveCaret);
  updateWindowTitle();
}

function scheduleAutoSave() {
  if (idleTimer) clearTimeout(idleTimer);

  idleTimer = setTimeout(async () => {
    if (!noteId) return;
    try {
      await saveCurrentNoteContent();
    } catch (err) {
      console.error('Failed to save note content:', err);
    }
  }, 3000);

  if (saveTimer) clearTimeout(saveTimer);
}

async function saveCurrentNoteContent() {
  if (!noteId || !editor) return;

  if (idleTimer) {
    clearTimeout(idleTimer);
    idleTimer = null;
  }

  markdownSource = readMarkdownFromEditor();
  await window.__TAURI__.core.invoke('save_note_content', {
    id: noteId,
    content: markdownSource
  });
  await updateWindowTitle();
}

function replaceSelection(text, caretShift = 0) {
  const { start, end } = getSelectionOffsets();
  markdownSource = readMarkdownFromEditor();
  markdownSource = markdownSource.slice(0, start) + text + markdownSource.slice(end);
  renderMarkdown(false);
  const caret = start + text.length + caretShift;
  setCaretOffset(caret);
  updateWindowTitle();
  scheduleAutoSave();
}

function toggleInlineFormat(marker) {
  markdownSource = readMarkdownFromEditor();
  let { start, end } = getSelectionOffsets();

  if (start === end) {
    replaceSelection(`${marker}${marker}`, -marker.length);
    return;
  }

  const selected = markdownSource.slice(start, end);
  const before = markdownSource.slice(Math.max(0, start - marker.length), start);
  const after = markdownSource.slice(end, end + marker.length);

  if (before === marker && after === marker) {
    markdownSource = markdownSource.slice(0, start - marker.length) + selected + markdownSource.slice(end + marker.length);
    start -= marker.length;
    end -= marker.length;
  } else {
    markdownSource = markdownSource.slice(0, start) + marker + selected + marker + markdownSource.slice(end);
    start += marker.length;
    end += marker.length;
  }

  renderMarkdown(false);
  setSelectionOffsets(start, end);
  updateWindowTitle();
  scheduleAutoSave();
}

function toggleLinePrefix(type) {
  markdownSource = readMarkdownFromEditor();
  const { start, end } = getSelectionOffsets();
  const lines = markdownSource.split('\n');
  let cursor = 0;
  const selectedIndexes = [];

  lines.forEach((line, index) => {
    const lineStart = cursor;
    const lineEnd = cursor + line.length;
    if (lineEnd >= start && lineStart <= end) selectedIndexes.push(index);
    cursor = lineEnd + 1;
  });

  const indexes = selectedIndexes.length ? selectedIndexes : [0];
  const allAlreadyPrefixed = indexes.every((index) => {
    const line = lines[index] || "";
    return type === 'task' ? /^\s*- \[[ xX]\]\s?/.test(line) : /^\s*-\s+/.test(line) && !/^\s*- \[[ xX]\]/.test(line);
  });

  indexes.forEach((index) => {
    const line = lines[index] || "";
    if (type === 'task') {
      lines[index] = allAlreadyPrefixed
        ? line.replace(/^(\s*)- \[[ xX]\]\s?/, '$1')
        : line.replace(/^(\s*)-\s+/, '$1').replace(/^(\s*)/, '$1- [ ] ');
    } else {
      lines[index] = allAlreadyPrefixed
        ? line.replace(/^(\s*)-\s+/, '$1')
        : line.replace(/^(\s*)- \[[ xX]\]\s?/, '$1').replace(/^(\s*)/, '$1- ');
    }
  });

  markdownSource = lines.join('\n');
  renderMarkdown(false);
  setCaretOffset(Math.min(start, markdownSource.length));
  updateWindowTitle();
  scheduleAutoSave();
}

function toggleTaskLine(lineIndex) {
  const lines = markdownSource.split('\n');
  const line = lines[lineIndex];
  if (!line) return;
  lines[lineIndex] = line.replace(/^(\s*)- \[([ xX])\]/, (_, indent, state) => `${indent}- [${state.toLowerCase() === 'x' ? ' ' : 'x'}]`);
  setMarkdownSource(lines.join('\n'), false);
  scheduleAutoSave();
}

async function createNewNoteWindow(offset = 20) {
  const position = await win.innerPosition();
  const size = await win.innerSize();
  const newNoteId = await window.__TAURI__.core.invoke('create_note', {
    x: position.x + offset,
    y: position.y + offset,
    width: size.width,
    height: size.height
  });

  await window.__TAURI__.core.invoke('create_note_window', {
    label: `note-${newNoteId}`,
    title: "FadeNote - New Note",
    width: size.width,
    height: size.height,
    x: Math.round(position.x + offset),
    y: Math.round(position.y + offset)
  });
}

function initializeButtonEvents() {
  document.getElementById("btn-close").addEventListener('click', async () => {
    try {
      await saveCurrentNoteContent();
    } catch (err) {
      console.error('Failed to save note before close:', err);
    }
    win.close();
  });

  document.getElementById("btn-delete").addEventListener('click', async () => {
    if (!noteId) return;

    const userConfirmed = await showCustomConfirm("Delete this note?", "This cannot be undone.");
    if (!userConfirmed) return;

    try {
      if (idleTimer) {
        clearTimeout(idleTimer);
        idleTimer = null;
      }

      await window.__TAURI__.core.invoke('delete_note', { id: noteId });

      const deletedNoteId = noteId;
      noteId = null;
      try {
        await win.destroy();
      } catch (destroyErr) {
        console.warn('Failed to destroy window, falling back to close:', destroyErr);
        await win.close();
      }

      console.log(`Note ${deletedNoteId} deleted`);
    } catch (err) {
      console.error('Failed to delete note:', err);
      alert('删除便签失败：' + err);
    }
  });

  document.getElementById("btn-top").addEventListener('click', async () => {
    try {
      const top = await win.isAlwaysOnTop();
      await win.setAlwaysOnTop(!top);
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

  document.getElementById("btn-pin").addEventListener('click', async () => {
    if (!noteId) return;

    try {
      isPinned = !isPinned;
      await window.__TAURI__.core.invoke('set_note_pinned', { id: noteId, pinned: isPinned });
      updatePinButtonStyle();
    } catch (err) {
      console.error('Failed to set pin status:', err);
      isPinned = !isPinned;
      updatePinButtonStyle();
    }
  });
}

async function updatePinStatus() {
  if (!noteId) return;
  try {
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

function updatePinButtonStyle() {
  const pinBtn = document.getElementById("btn-pin");
  if (!pinBtn) return;

  if (isPinned) {
    pinBtn.classList.add('active');
    pinBtn.title = "Unpin";
  } else {
    pinBtn.classList.remove('active');
    pinBtn.title = "Pin";
  }
}

function initializeMarkdownEditor() {
  if (!editor) return;

  editor.contentEditable = "true";

  editor.addEventListener('compositionstart', () => {
    isComposing = true;
  });

  editor.addEventListener('compositionend', () => {
    isComposing = false;
    markdownSource = readMarkdownFromEditor();
    renderMarkdown(true);
    scheduleAutoSave();
  });

  editor.addEventListener('input', () => {
    if (isRendering || isComposing) return;
    markdownSource = readMarkdownFromEditor();
    renderMarkdown(true);
    updateWindowTitle();
    scheduleAutoSave();
  });

  editor.addEventListener('focus', () => {
    editor.style.backgroundColor = "#fffdf5";
  });

  editor.addEventListener('blur', () => {
    if (markdownSource.trim() === "") editor.style.backgroundColor = "transparent";
  });

  editor.addEventListener('click', (event) => {
    const taskMarker = event.target.closest('.md-task-marker');
    if (!taskMarker) return;
    event.preventDefault();
    toggleTaskLine(Number(taskMarker.dataset.line));
  });

  editor.addEventListener('keydown', (event) => {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'b') {
      event.preventDefault();
      toggleInlineFormat('**');
    }

    if ((event.ctrlKey || event.metaKey) && event.shiftKey && event.key.toLowerCase() === 'x') {
      event.preventDefault();
      toggleInlineFormat('~~');
    }
  });

  toolbar?.addEventListener('click', (event) => {
    const button = event.target.closest('.toolbar-btn');
    if (!button) return;

    editor.focus();
    const format = button.dataset.format;
    if (format === 'bold') toggleInlineFormat('**');
    if (format === 'strike') toggleInlineFormat('~~');
    if (format === 'list') toggleLinePrefix('list');
    if (format === 'task') toggleLinePrefix('task');
  });
}

function initializeNewNoteGesture() {
  paper?.addEventListener('dblclick', async (event) => {
    if (event.target.closest('.markdown-editor, .editor-toolbar, .toolbar-btn, .dot')) return;

    event.preventDefault();
    event.stopPropagation();

    try {
      await createNewNoteWindow(20);
    } catch (err) {
      console.error('Failed to create new note:', err);
    }
  });
}

document.addEventListener('DOMContentLoaded', async () => {
  if (hasInitialized) return;
  hasInitialized = true;

  initializeButtonEvents();
  initializeMarkdownEditor();
  initializeNewNoteGesture();

  try {
    const currentDir = await window.__TAURI__.core.invoke('ensure_notes_directory');
    const currentDirDisplay = document.getElementById("current-dir-display");
    if (currentDir && currentDirDisplay) {
      currentDirDisplay.textContent = `Directory: ${currentDir}`;
    }
  } catch (err) {
    console.error('Failed to get notes directory:', err);
  }

  let windowLabel = '';
  if (noteId && noteIdSet) {
    windowLabel = `note-${noteId}`;
  } else {
    try {
      const currentWindow = window.__TAURI__.window.getCurrentWindow();
      windowLabel = currentWindow.label;
      const match = windowLabel.match(/note-(.*)/);
      if (match && match[1]) {
        noteId = match[1];
        noteIdSet = true;
      }
    } catch (err) {
      console.warn('Unable to get window label:', err);
      return;
    }
  }

  if (windowLabel === 'main') return;
  if (!noteId) {
    console.error('Failed to get note ID');
    return;
  }

  try {
    const activeNotes = await window.__TAURI__.core.invoke('get_active_notes');
    const noteDetail = activeNotes.find(note => note.id === noteId);
    if (noteDetail?.window) {
      await win.setPosition(new window.__TAURI__.window.Position(noteDetail.window.x, noteDetail.window.y));
      await win.setSize(new window.__TAURI__.window.Size(noteDetail.window.width, noteDetail.window.height));
    }
    await updatePinStatus();
  } catch (err) {
    console.warn('Failed to get note position info:', err);
  }

  try {
    const savedContent = await window.__TAURI__.core.invoke('load_note', { id: noteId });
    setMarkdownSource(savedContent || "", false);
  } catch (err) {
    console.warn('Failed to load note content:', err);
    setMarkdownSource("", false);
  }

  window.addEventListener('beforeunload', async () => {
    try {
      await saveCurrentNoteContent();
    } catch (err) {
      console.error('Failed to save note content:', err);
    }
  });

  let positionUpdateTimer = null;
  document.addEventListener('mouseup', async () => {
    if (positionUpdateTimer) clearTimeout(positionUpdateTimer);

    positionUpdateTimer = setTimeout(async () => {
      if (!noteId) return;
      try {
        const position = await win.innerPosition();
        const size = await win.innerSize();
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
    }, 500);
  });

  try {
    const top = await win.isAlwaysOnTop();
    const topBtn = document.getElementById("btn-top");
    if (top) {
      topBtn.classList.add("active");
      topBtn.title = "Remove from Top";
    }
  } catch {}
});
