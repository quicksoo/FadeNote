const { getCurrentWindow } = window.__TAURI__.window;
const win = getCurrentWindow();
const editor = document.querySelector(".paper-content");
const toolbar = document.querySelector(".editor-toolbar");
const paper = document.querySelector(".paper");
const saveStatus = document.getElementById("note-save-status");
const lifecycleStatus = document.getElementById("note-lifecycle-status");

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
let currentNoteDetail = null;

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
    .replace(/^[-*]\s+\[[ xX]\]\s+/, '')
    .replace(/^[-*]\s+/, '')
    .replace(/\*\*([^*]+)\*\*/g, '$1')
    .trim()
    .slice(0, 40);

  return `FadeNote - ${text || 'New Note'}`;
}

function setSaveStatus(status, label) {
  if (!saveStatus) return;
  saveStatus.textContent = label;
  saveStatus.className = status;
}

function formatRemainingTime(expireAt) {
  if (!expireAt) return "Auto archive";
  const remainingMs = new Date(expireAt).getTime() - Date.now();
  if (!Number.isFinite(remainingMs)) return "Auto archive";
  if (remainingMs <= 0) return "Archiving soon";

  const hours = Math.ceil(remainingMs / (1000 * 60 * 60));
  if (hours < 24) return `${hours}h left`;
  return `${Math.ceil(hours / 24)}d left`;
}

function updateLifecycleStatus() {
  if (!lifecycleStatus) return;
  if (isPinned || currentNoteDetail?.pinned) {
    lifecycleStatus.textContent = "Pinned · stays";
    return;
  }
  lifecycleStatus.textContent = formatRemainingTime(currentNoteDetail?.expireAt);
}

async function updateWindowTitle() {
  try {
    await win.setTitle(plainTitleFromMarkdown(markdownSource));
  } catch (err) {
    console.warn('Failed to update window title:', err);
  }
}

function parseMarkdownLine(line) {
  const taskMatch = line.match(/^(\s*)- \[([ xX])\]\s?(.*)$/);
  if (taskMatch) {
    return {
      type: 'task',
      checked: taskMatch[2].toLowerCase() === 'x',
      content: taskMatch[3] || ''
    };
  }

  const listMatch = line.match(/^\s*-\s?(.*)$/);
  if (listMatch) {
    return {
      type: 'list',
      checked: false,
      content: listMatch[1] || ''
    };
  }

  return {
    type: 'paragraph',
    checked: false,
    content: line || ''
  };
}

function buildMarkdownLine(type, content, checked = false) {
  if (type === 'task') return `- [${checked ? 'x' : ' '}] ${content}`;
  if (type === 'list') return `- ${content}`;
  return content;
}

function parseInlineMarkdown(content) {
  const fragments = [];
  let index = 0;

  while (index < content.length) {
    const boldStart = content.indexOf('**', index);
    const next = boldStart;

    if (next === -1) {
      fragments.push({ type: 'text', text: content.slice(index) });
      break;
    }

    if (next > index) {
      fragments.push({ type: 'text', text: content.slice(index, next) });
    }

    const marker = '**';
    const close = content.indexOf(marker, next + 2);
    if (close === -1) {
      fragments.push({ type: 'text', text: marker });
      index = next + 2;
      continue;
    }

    fragments.push({
      type: 'bold',
      text: content.slice(next + 2, close)
    });
    index = close + 2;
  }

  return fragments;
}

function renderInlineMarkdown(content) {
  return parseInlineMarkdown(content)
    .map((fragment) => {
      const text = escapeHtml(fragment.text);
      if (fragment.type === 'bold') return `<strong class="md-bold">${text || '<br>'}</strong>`;
      return text;
    })
    .join('') || '<br>';
}

function renderMarkdownLine(line, lineIndex) {
  const parsed = parseMarkdownLine(line);
  const checkedClass = parsed.checked ? ' checked' : '';
  const checkedAttr = parsed.checked ? 'true' : 'false';
  const content = renderInlineMarkdown(parsed.content);

  if (parsed.type === 'task') {
    return `<div class="editor-line task-line${checkedClass}" data-line="${lineIndex}" data-type="task" data-checked="${checkedAttr}"><button type="button" class="task-toggle" contenteditable="false" data-line="${lineIndex}" aria-label="Toggle task">${parsed.checked ? '✓' : ''}</button><span class="line-content" contenteditable="true">${content}</span></div>`;
  }

  if (parsed.type === 'list') {
    return `<div class="editor-line list-line" data-line="${lineIndex}" data-type="list" data-checked="false"><span class="list-bullet" contenteditable="false"></span><span class="line-content" contenteditable="true">${content}</span></div>`;
  }

  return `<div class="editor-line paragraph-line" data-line="${lineIndex}" data-type="paragraph" data-checked="false"><span class="line-content" contenteditable="true">${content}</span></div>`;
}

function getMarkdownLines() {
  return markdownSource.split('\n');
}

function renderMarkdown(preserveCaret = true, nextCaret = null) {
  const caret = nextCaret || (preserveCaret ? getCaretPosition() : { line: 0, offset: 0 });
  const lines = getMarkdownLines();
  isRendering = true;
  editor.innerHTML = lines.map((line, index) => renderMarkdownLine(line, index)).join('');
  editor.classList.toggle('is-empty', markdownSource.trim() === '');
  isRendering = false;
  if (preserveCaret || nextCaret) setCaretPosition(caret.line, caret.offset);
}

function serializeInlineNode(node) {
  if (!node) return '';
  if (node.nodeType === Node.TEXT_NODE) return node.nodeValue || '';
  if (node.nodeType !== Node.ELEMENT_NODE) return '';

  if (node.classList.contains('task-toggle') || node.classList.contains('list-bullet')) return '';
  if (node.tagName === 'BR') return '';

  const inner = Array.from(node.childNodes).map(serializeInlineNode).join('');
  if (node.classList.contains('md-bold') || node.tagName === 'STRONG' || node.tagName === 'B') return `**${inner}**`;
  return inner;
}

function serializeLine(lineElement) {
  const type = lineElement.dataset.type || 'paragraph';
  const checked = lineElement.dataset.checked === 'true';
  const contentElement = lineElement.querySelector('.line-content');
  const content = Array.from(contentElement?.childNodes || []).map(serializeInlineNode).join('');
  return buildMarkdownLine(type, content.replace(/\u00a0/g, ' '), checked);
}

function readMarkdownFromEditor() {
  const lineElements = Array.from(editor.querySelectorAll('.editor-line'));
  if (lineElements.length === 0) return '';
  return lineElements.map(serializeLine).join('\n');
}

function setMarkdownSource(value, preserveCaret = false) {
  markdownSource = value || '';
  renderMarkdown(preserveCaret);
  updateWindowTitle();
}

function getLineElement(node) {
  if (!node) return editor.querySelector('.editor-line');
  const element = node.nodeType === Node.ELEMENT_NODE ? node : node.parentElement;
  return element?.closest('.editor-line') || editor.querySelector('.editor-line');
}

function getLineContentElement(lineElement) {
  return lineElement?.querySelector('.line-content') || null;
}

function getTextOffsetWithin(element, container, offset) {
  const range = document.createRange();
  range.selectNodeContents(element);
  try {
    range.setEnd(container, offset);
  } catch {
    range.collapse(false);
  }
  return range.toString().length;
}

function getCaretPosition() {
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0 || !editor.contains(selection.anchorNode)) {
    return { line: Math.max(0, getMarkdownLines().length - 1), offset: 0 };
  }

  const lineElement = getLineElement(selection.anchorNode);
  const contentElement = getLineContentElement(lineElement);
  const line = Number(lineElement?.dataset.line || 0);
  if (!contentElement) return { line, offset: 0 };

  return {
    line,
    offset: Math.max(0, getTextOffsetWithin(contentElement, selection.anchorNode, selection.anchorOffset))
  };
}

function findTextPositionInElement(element, offset) {
  let remaining = Math.max(0, offset);
  const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
  let node = walker.nextNode();

  while (node) {
    if (remaining <= node.nodeValue.length) return { node, offset: remaining };
    remaining -= node.nodeValue.length;
    node = walker.nextNode();
  }

  return null;
}

function scrollLineIntoView(lineElement) {
  if (!lineElement || !editor) return;

  const editorRect = editor.getBoundingClientRect();
  const lineRect = lineElement.getBoundingClientRect();
  const topPadding = 12;
  const bottomPadding = 14;

  if (lineRect.bottom > editorRect.bottom - bottomPadding) {
    editor.scrollTop += lineRect.bottom - editorRect.bottom + bottomPadding;
  } else if (lineRect.top < editorRect.top + topPadding) {
    editor.scrollTop -= editorRect.top + topPadding - lineRect.top;
  }
}

function setCaretPosition(lineIndex, offset) {
  const lines = Array.from(editor.querySelectorAll('.editor-line'));
  const lineElement = lines[Math.min(Math.max(0, lineIndex), Math.max(0, lines.length - 1))];
  const contentElement = getLineContentElement(lineElement);
  if (!contentElement) return;

  const selection = window.getSelection();
  const range = document.createRange();
  const textPosition = findTextPositionInElement(contentElement, offset);

  if (textPosition) {
    range.setStart(textPosition.node, textPosition.offset);
  } else {
    range.selectNodeContents(contentElement);
    range.collapse(false);
  }

  range.collapse(true);
  selection.removeAllRanges();
  selection.addRange(range);
  requestAnimationFrame(() => scrollLineIntoView(lineElement));
}

function getSelectionPositions() {
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0 || !editor.contains(selection.anchorNode)) {
    const caret = getCaretPosition();
    return { start: caret, end: caret, collapsed: true };
  }

  const range = selection.getRangeAt(0);
  const startLine = getLineElement(range.startContainer);
  const endLine = getLineElement(range.endContainer);
  const startContent = getLineContentElement(startLine);
  const endContent = getLineContentElement(endLine);

  const start = {
    line: Number(startLine?.dataset.line || 0),
    offset: startContent ? getTextOffsetWithin(startContent, range.startContainer, range.startOffset) : 0
  };
  const end = {
    line: Number(endLine?.dataset.line || start.line),
    offset: endContent ? getTextOffsetWithin(endContent, range.endContainer, range.endOffset) : start.offset
  };

  return { start, end, collapsed: range.collapsed };
}

function visibleToMarkdownOffset(content, visibleOffset) {
  let visible = 0;
  let index = 0;
  const activeMarkers = [];

  while (index < content.length) {
    const marker = content.slice(index, index + 2);
    if (marker === '**') {
      const isClosingMarker = activeMarkers.at(-1) === marker;
      if (isClosingMarker && visible >= visibleOffset) return index;

      const close = content.indexOf(marker, index + 2);
      if (!isClosingMarker && close !== -1) {
        activeMarkers.push(marker);
        index += 2;
        continue;
      }

      if (isClosingMarker) activeMarkers.pop();
      index += 2;
      continue;
    }

    if (visible >= visibleOffset) return index;
    visible += 1;
    index += 1;
  }
  return content.length;
}

function markdownToVisibleOffset(content, markdownOffset) {
  let visible = 0;
  let index = 0;
  while (index < Math.min(markdownOffset, content.length)) {
    const marker = content.slice(index, index + 2);
    if (marker === '**') {
      index += 2;
      continue;
    }
    visible += 1;
    index += 1;
  }
  return visible;
}

function scheduleAutoSave() {
  if (idleTimer) clearTimeout(idleTimer);
  setSaveStatus('saving', 'Saving...');

  idleTimer = setTimeout(async () => {
    if (!noteId) return;
    try {
      await saveCurrentNoteContent();
    } catch (err) {
      console.error('Failed to save note content:', err);
      setSaveStatus('error', 'Save failed');
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
  setSaveStatus('saving', 'Saving...');
  await window.__TAURI__.core.invoke('save_note_content', {
    id: noteId,
    content: markdownSource
  });
  await updateWindowTitle();
  setSaveStatus('saved', 'Saved');
}

function syncEditorFromDom(preserveCaret = true) {
  const caret = getCaretPosition();
  markdownSource = readMarkdownFromEditor();
  renderMarkdown(preserveCaret, caret);
  updateWindowTitle();
  scheduleAutoSave();
}

function toggleInlineFormat(marker) {
  markdownSource = readMarkdownFromEditor();
  const selection = getSelectionPositions();

  if (selection.collapsed) {
      document.execCommand('bold');
    return;
  }

  const lines = getMarkdownLines();
  const lineIndex = selection.start.line;
  const parsed = parseMarkdownLine(lines[lineIndex] || '');

  let start = visibleToMarkdownOffset(parsed.content, selection.start.offset);
  let end = selection.end.line === lineIndex
    ? visibleToMarkdownOffset(parsed.content, selection.end.offset)
    : start;

  if (start > end) [start, end] = [end, start];

  if (start === end) {
    parsed.content = parsed.content.slice(0, start) + marker + marker + parsed.content.slice(end);
    lines[lineIndex] = buildMarkdownLine(parsed.type, parsed.content, parsed.checked);
    markdownSource = lines.join('\n');
    renderMarkdown(false, { line: lineIndex, offset: markdownToVisibleOffset(parsed.content, start + marker.length) });
  } else {
    const selected = parsed.content.slice(start, end);
    const before = parsed.content.slice(Math.max(0, start - marker.length), start);
    const after = parsed.content.slice(end, end + marker.length);
    if (before === marker && after === marker) {
      parsed.content = parsed.content.slice(0, start - marker.length) + selected + parsed.content.slice(end + marker.length);
      start -= marker.length;
      end -= marker.length;
    } else {
      parsed.content = parsed.content.slice(0, start) + marker + selected + marker + parsed.content.slice(end);
      start += marker.length;
      end += marker.length;
    }
    lines[lineIndex] = buildMarkdownLine(parsed.type, parsed.content, parsed.checked);
    markdownSource = lines.join('\n');
    renderMarkdown(false, { line: lineIndex, offset: markdownToVisibleOffset(parsed.content, end) });
  }

  updateWindowTitle();
  scheduleAutoSave();
}

function selectedLineIndexes() {
  const selection = getSelectionPositions();
  const start = Math.min(selection.start.line, selection.end.line);
  const end = Math.max(selection.start.line, selection.end.line);
  const indexes = [];
  for (let index = start; index <= end; index += 1) indexes.push(index);
  return indexes;
}

function toggleLinePrefix(type) {
  markdownSource = readMarkdownFromEditor();
  const lines = getMarkdownLines();
  const indexes = selectedLineIndexes();
  const allAlreadyTyped = indexes.every((index) => parseMarkdownLine(lines[index] || '').type === type);

  indexes.forEach((index) => {
    const parsed = parseMarkdownLine(lines[index] || '');
    if (allAlreadyTyped) {
      lines[index] = parsed.content;
    } else if (type === 'task') {
      lines[index] = buildMarkdownLine('task', parsed.content, false);
    } else {
      lines[index] = buildMarkdownLine('list', parsed.content, false);
    }
  });

  const caret = getCaretPosition();
  markdownSource = lines.join('\n');
  renderMarkdown(false, caret);
  updateWindowTitle();
  scheduleAutoSave();
}

function toggleTaskLine(lineIndex) {
  markdownSource = readMarkdownFromEditor();
  const lines = getMarkdownLines();
  const parsed = parseMarkdownLine(lines[lineIndex] || '');
  if (parsed.type !== 'task') return;
  lines[lineIndex] = buildMarkdownLine('task', parsed.content, !parsed.checked);
  markdownSource = lines.join('\n');
  renderMarkdown(false, { line: lineIndex, offset: 0 });
  scheduleAutoSave();
}

function splitCurrentLine() {
  markdownSource = readMarkdownFromEditor();
  const caret = getCaretPosition();
  const lines = getMarkdownLines();
  const parsed = parseMarkdownLine(lines[caret.line] || '');
  const markdownOffset = visibleToMarkdownOffset(parsed.content, caret.offset);
  const before = parsed.content.slice(0, markdownOffset);
  const after = parsed.content.slice(markdownOffset);

  if ((parsed.type === 'list' || parsed.type === 'task') && parsed.content.trim() === '') {
    lines[caret.line] = '';
    markdownSource = lines.join('\n');
    renderMarkdown(false, { line: caret.line, offset: 0 });
    updateWindowTitle();
    scheduleAutoSave();
    return;
  }

  lines[caret.line] = buildMarkdownLine(parsed.type, before, parsed.checked);
  const nextType = parsed.type === 'task' ? 'task' : parsed.type === 'list' ? 'list' : 'paragraph';
  lines.splice(caret.line + 1, 0, buildMarkdownLine(nextType, after, false));
  markdownSource = lines.join('\n');
  renderMarkdown(false, { line: caret.line + 1, offset: 0 });
  updateWindowTitle();
  scheduleAutoSave();
}

function handleBackspaceAtLineStart(event) {
  const selection = getSelectionPositions();
  if (!selection.collapsed) return false;

  const caret = getCaretPosition();
  if (caret.offset !== 0) return false;
  markdownSource = readMarkdownFromEditor();
  const lines = getMarkdownLines();
  const parsed = parseMarkdownLine(lines[caret.line] || '');

  if (parsed.type === 'list' || parsed.type === 'task') {
    event.preventDefault();
    lines[caret.line] = parsed.content;
    markdownSource = lines.join('\n');
    renderMarkdown(false, { line: caret.line, offset: 0 });
    updateWindowTitle();
    scheduleAutoSave();
    return true;
  }

  if (caret.line > 0) {
    event.preventDefault();
    const previousParsed = parseMarkdownLine(lines[caret.line - 1] || '');
    const previousVisibleLength = parseInlineMarkdown(previousParsed.content)
      .reduce((length, fragment) => length + fragment.text.length, 0);
    lines[caret.line - 1] = buildMarkdownLine(
      previousParsed.type,
      previousParsed.content + parsed.content,
      previousParsed.checked
    );
    lines.splice(caret.line, 1);
    markdownSource = lines.join('\n');
    renderMarkdown(false, { line: caret.line - 1, offset: previousVisibleLength });
    updateWindowTitle();
    scheduleAutoSave();
    return true;
  }

  return false;
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
      setSaveStatus('error', 'Save failed');
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
      if (currentNoteDetail) currentNoteDetail.pinned = isPinned;
      updatePinButtonStyle();
      updateLifecycleStatus();
    } catch (err) {
      console.error('Failed to set pin status:', err);
      isPinned = !isPinned;
      updatePinButtonStyle();
      updateLifecycleStatus();
    }
  });
}

async function updatePinStatus() {
  if (!noteId) return;
  try {
    const activeNotes = await window.__TAURI__.core.invoke('get_active_notes');
    const noteDetail = activeNotes.find(note => note.id === noteId);
    if (noteDetail) {
      currentNoteDetail = noteDetail;
      isPinned = noteDetail.pinned || false;
      updatePinButtonStyle();
      updateLifecycleStatus();
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

  editor.contentEditable = "false";

  editor.addEventListener('compositionstart', () => {
    isComposing = true;
  });

  editor.addEventListener('compositionend', () => {
    isComposing = false;
    syncEditorFromDom(true);
  });

  editor.addEventListener('input', () => {
    if (isRendering || isComposing) return;
    syncEditorFromDom(true);
  });

  editor.addEventListener('focus', () => {
    editor.style.backgroundColor = "#fffdf5";
  });

  editor.addEventListener('blur', () => {
    if (markdownSource.trim() === "") editor.style.backgroundColor = "transparent";
  });

  editor.addEventListener('click', (event) => {
    const taskToggle = event.target.closest('.task-toggle');
    if (!taskToggle) return;
    event.preventDefault();
    toggleTaskLine(Number(taskToggle.dataset.line));
  });

  editor.addEventListener('keydown', (event) => {
    if (event.key === 'Enter' && !isComposing) {
      event.preventDefault();
      splitCurrentLine();
      return;
    }

    if (event.key === 'Backspace' && handleBackspaceAtLineStart(event)) {
      return;
    }

    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'b') {
      event.preventDefault();
      toggleInlineFormat('**');
      return;
    }

  });

  toolbar?.addEventListener('mousedown', (event) => {
    if (event.target.closest('.toolbar-btn')) event.preventDefault();
  });

  toolbar?.addEventListener('click', (event) => {
    const button = event.target.closest('.toolbar-btn');
    if (!button) return;

    editor.focus();
    const format = button.dataset.format;
    if (format === 'bold') toggleInlineFormat('**');
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
    currentNoteDetail = noteDetail || null;
    if (noteDetail?.window) {
      await win.setPosition(new window.__TAURI__.window.Position(noteDetail.window.x, noteDetail.window.y));
      await win.setSize(new window.__TAURI__.window.Size(noteDetail.window.width, noteDetail.window.height));
    }
    updateLifecycleStatus();
    await updatePinStatus();
  } catch (err) {
    console.warn('Failed to get note position info:', err);
  }

  try {
    const savedContent = await window.__TAURI__.core.invoke('load_note', { id: noteId });
    setMarkdownSource(savedContent || "", false);
    setSaveStatus('saved', 'Saved');
    updateLifecycleStatus();
  } catch (err) {
    console.warn('Failed to load note content:', err);
    setMarkdownSource("", false);
    setSaveStatus('error', 'Load failed');
  }

  window.addEventListener('beforeunload', async () => {
    try {
      await saveCurrentNoteContent();
    } catch (err) {
      console.error('Failed to save note content:', err);
      setSaveStatus('error', 'Save failed');
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

