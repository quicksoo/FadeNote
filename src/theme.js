(function () {
  const THEMES = new Set(['paper', 'yellow', 'blue', 'dusk']);
  const DEFAULT_THEME = 'paper';

  function normalizeTheme(theme) {
    return THEMES.has(theme) ? theme : DEFAULT_THEME;
  }

  function applyTheme(theme) {
    const normalizedTheme = normalizeTheme(theme);
    document.documentElement.dataset.theme = normalizedTheme;
    return normalizedTheme;
  }

  async function loadTheme() {
    try {
      const settings = await window.__TAURI__.core.invoke('get_schedule_settings');
      return applyTheme(settings.theme);
    } catch (err) {
      console.warn('Failed to load theme:', err);
      return applyTheme(DEFAULT_THEME);
    }
  }

  window.FadeNoteTheme = {
    applyTheme,
    loadTheme,
    normalizeTheme,
    themes: Array.from(THEMES)
  };

  document.addEventListener('DOMContentLoaded', () => {
    loadTheme();
    window.__TAURI__?.event?.listen?.('fadenote://theme-changed', (event) => {
      applyTheme(event.payload);
    }).catch((err) => {
      console.warn('Failed to listen for theme changes:', err);
    });
  });
})();
