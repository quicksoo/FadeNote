const enabledInput = document.getElementById('enabled');
const timeInput = document.getElementById('time');
const recurrenceInput = document.getElementById('recurrence');
const weekdaysRow = document.getElementById('weekdays-row');
const weekdayInputs = Array.from(document.querySelectorAll('.weekday-row input'));
const themeInput = document.getElementById('theme');
const languageInput = document.getElementById('language');
const statusEl = document.getElementById('status');
const saveDirInput = document.getElementById('save-dir');
let currentSettings = null;

function tr(key, values) {
  return window.FadeNoteI18n?.t(key, values) || key;
}

function setStatus(message) {
  statusEl.textContent = message;
  if (message) setTimeout(() => { statusEl.textContent = ''; }, 2500);
}

function updateWeekdayVisibility() {
  weekdaysRow.style.display = recurrenceInput.value === 'weekly' ? 'block' : 'none';
}

function readForm() {
  const selectedTheme = themeInput.value || 'paper';
  return {
    enabled: enabledInput.checked,
    time: timeInput.value || '09:00',
    recurrence: recurrenceInput.value,
    weekdays: weekdayInputs.filter(input => input.checked).map(input => Number(input.value)),
    theme: window.FadeNoteTheme?.normalizeTheme(selectedTheme) || selectedTheme,
    language: window.FadeNoteI18n?.normalizeLanguage(languageInput.value) || languageInput.value || 'system',
    lastTriggeredKey: currentSettings?.lastTriggeredKey || null
  };
}

function writeForm(settings) {
  enabledInput.checked = Boolean(settings.enabled);
  timeInput.value = settings.time || '09:00';
  recurrenceInput.value = settings.recurrence || 'daily';
  const weekdays = settings.weekdays?.length ? settings.weekdays : [1, 2, 3, 4, 5];
  weekdayInputs.forEach(input => {
    input.checked = weekdays.includes(Number(input.value));
  });
  const theme = window.FadeNoteTheme?.applyTheme(settings.theme) || 'paper';
  themeInput.value = theme;
  languageInput.value = window.FadeNoteI18n?.applyLanguage(settings.language) || settings.language || 'system';
  updateWeekdayVisibility();
}

async function saveTheme({ silent = false } = {}) {
  const nextTheme = window.FadeNoteTheme?.normalizeTheme(themeInput.value) || themeInput.value;
  const settings = {
    ...(currentSettings || readForm()),
    theme: nextTheme
  };

  try {
    await window.__TAURI__.core.invoke('save_schedule_settings', { settings });
    currentSettings = settings;
    window.FadeNoteTheme?.applyTheme(nextTheme);
    await window.__TAURI__.event.emit('fadenote://theme-changed', nextTheme);
    if (!silent) setStatus(tr('settings.themeSaved'));
  } catch (err) {
    console.error('Failed to save theme:', err);
    setStatus(tr('settings.themeSaveFailed'));
  }
}

async function saveLanguage() {
  const nextLanguage = window.FadeNoteI18n?.normalizeLanguage(languageInput.value) || languageInput.value || 'system';
  const settings = {
    ...(currentSettings || readForm()),
    language: nextLanguage
  };

  try {
    await window.__TAURI__.core.invoke('save_schedule_settings', { settings });
    currentSettings = settings;
    window.FadeNoteI18n?.applyLanguage(nextLanguage);
    await window.__TAURI__.event.emit('fadenote://language-changed', nextLanguage);
  } catch (err) {
    console.error('Failed to save language:', err);
    setStatus(tr('settings.languageSaveFailed'));
  }
}

async function loadSettings() {
  try {
    const [settings, saveDir] = await Promise.all([
      window.__TAURI__.core.invoke('get_schedule_settings'),
      window.__TAURI__.core.invoke('get_app_data_directory')
    ]);
    currentSettings = settings;
    writeForm(settings);
    window.FadeNoteI18n?.applyStaticText();
    saveDirInput.value = saveDir;
  } catch (err) {
    console.error('Failed to load settings:', err);
    saveDirInput.value = tr('settings.unavailable');
    setStatus(tr('settings.loadFailed'));
  }
}

async function saveSettings() {
  const settings = readForm();
  if (settings.recurrence === 'weekly' && settings.weekdays.length === 0) {
    setStatus(tr('settings.selectWeekday'));
    return;
  }

  try {
    await window.__TAURI__.core.invoke('save_schedule_settings', { settings });
    currentSettings = settings;
    window.FadeNoteTheme?.applyTheme(settings.theme);
    await window.__TAURI__.event.emit('fadenote://theme-changed', settings.theme);
    setStatus(tr('settings.saved'));
  } catch (err) {
    console.error('Failed to save settings:', err);
    setStatus(tr('settings.saveFailed'));
  }
}

async function raiseNow() {
  try {
    await window.__TAURI__.core.invoke('raise_active_notes_once');
    setStatus(tr('settings.raised'));
  } catch (err) {
    console.error('Failed to raise notes:', err);
    setStatus(tr('settings.raiseFailed'));
  }
}

recurrenceInput.addEventListener('change', updateWeekdayVisibility);
themeInput.addEventListener('change', () => {
  window.FadeNoteTheme?.applyTheme(themeInput.value);
  saveTheme({ silent: true });
});
languageInput.addEventListener('change', () => {
  window.FadeNoteI18n?.applyLanguage(languageInput.value);
  saveLanguage();
});
document.getElementById('save').addEventListener('click', saveSettings);
document.getElementById('raise-now').addEventListener('click', raiseNow);
document.addEventListener('DOMContentLoaded', loadSettings);
