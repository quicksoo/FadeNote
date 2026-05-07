const enabledInput = document.getElementById('enabled');
const timeInput = document.getElementById('time');
const recurrenceInput = document.getElementById('recurrence');
const weekdaysRow = document.getElementById('weekdays-row');
const weekdayInputs = Array.from(document.querySelectorAll('.weekday-row input'));
const statusEl = document.getElementById('status');
const saveDirInput = document.getElementById('save-dir');

function setStatus(message) {
  statusEl.textContent = message;
  if (message) setTimeout(() => { statusEl.textContent = ''; }, 2500);
}

function updateWeekdayVisibility() {
  weekdaysRow.style.display = recurrenceInput.value === 'weekly' ? 'block' : 'none';
}

function readForm() {
  return {
    enabled: enabledInput.checked,
    time: timeInput.value || '09:00',
    recurrence: recurrenceInput.value,
    weekdays: weekdayInputs.filter(input => input.checked).map(input => Number(input.value)),
    lastTriggeredKey: null
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
  updateWeekdayVisibility();
}

async function loadSettings() {
  try {
    const [settings, saveDir] = await Promise.all([
      window.__TAURI__.core.invoke('get_schedule_settings'),
      window.__TAURI__.core.invoke('get_app_data_directory')
    ]);
    writeForm(settings);
    saveDirInput.value = saveDir;
  } catch (err) {
    console.error('Failed to load settings:', err);
    saveDirInput.value = 'Unavailable';
    setStatus('Failed to load settings');
  }
}

async function saveSettings() {
  const settings = readForm();
  if (settings.recurrence === 'weekly' && settings.weekdays.length === 0) {
    setStatus('Select at least one weekday');
    return;
  }

  try {
    await window.__TAURI__.core.invoke('save_schedule_settings', { settings });
    setStatus('Saved');
  } catch (err) {
    console.error('Failed to save settings:', err);
    setStatus('Save failed');
  }
}

async function raiseNow() {
  try {
    await window.__TAURI__.core.invoke('raise_active_notes_once');
    setStatus('Raised active notes');
  } catch (err) {
    console.error('Failed to raise notes:', err);
    setStatus('Raise failed');
  }
}

recurrenceInput.addEventListener('change', updateWeekdayVisibility);
document.getElementById('save').addEventListener('click', saveSettings);
document.getElementById('raise-now').addEventListener('click', raiseNow);
document.addEventListener('DOMContentLoaded', loadSettings);
