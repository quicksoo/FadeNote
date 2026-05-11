(() => {
  const DEFAULT_LANGUAGE = 'en';
  const SYSTEM_LANGUAGE = 'system';
  const SUPPORTED_LANGUAGES = new Set([SYSTEM_LANGUAGE, 'en', 'zh-CN']);

  const messages = {
    en: {
      'common.cancel': 'Cancel',
      'common.delete': 'Delete',
      'common.save': 'Save',
      'note.close': 'Close',
      'note.delete': 'Delete',
      'note.alwaysOnTop': 'Always on Top',
      'note.pin': 'Pin',
      'note.newButton': 'New note',
      'note.newButtonTitle': 'New note (Ctrl+N)',
      'note.bold': 'Bold (Ctrl+B)',
      'note.list': 'List',
      'note.task': 'Task',
      'note.status': 'Note status',
      'note.saved': 'Saved',
      'note.saving': 'Saving...',
      'note.saveFailed': 'Save failed',
      'note.loadFailed': 'Load failed',
      'note.placeholder': 'Write something...',
      'note.newTitle': 'New Note',
      'note.archivePending': 'Archive date pending',
      'note.deleteTitle': 'Delete this note?',
      'note.deleteMessage': 'This cannot be undone.',

      'settings.documentTitle': 'FadeNote Settings',
      'settings.title': 'Settings',
      'settings.themeTitle': 'Theme',
      'settings.themeDesc': 'Choose a quiet global paper tone.',
      'settings.themeLabel': 'Theme',
      'settings.theme.paper': 'Paper',
      'settings.theme.yellow': 'Cream',
      'settings.theme.blue': 'Blue',
      'settings.theme.dusk': 'Dusk',
      'settings.languageTitle': 'Language',
      'settings.languageDesc': 'Follow the system language, or choose one manually.',
      'settings.languageLabel': 'Language',
      'settings.language.system': 'System',
      'settings.language.zhCN': '简体中文',
      'settings.language.en': 'English',
      'settings.scheduleTitle': 'Scheduled raise',
      'settings.scheduleDesc': 'Bring active notes to the front at a set time.',
      'settings.enableSchedule': 'Enable scheduled raise',
      'settings.time': 'Time',
      'settings.repeat': 'Repeat',
      'settings.daily': 'Daily',
      'settings.weekly': 'Weekly',
      'settings.weekdays': 'Weekdays',
      'settings.mon': 'Mon',
      'settings.tue': 'Tue',
      'settings.wed': 'Wed',
      'settings.thu': 'Thu',
      'settings.fri': 'Fri',
      'settings.sat': 'Sat',
      'settings.sun': 'Sun',
      'settings.raiseNow': 'Raise now',
      'settings.fileDir': 'File save directory',
      'settings.fileDirDesc': 'Notes are saved as local Markdown files here.',
      'settings.loading': 'Loading...',
      'settings.saved': 'Saved',
      'settings.saveFailed': 'Save failed',
      'settings.themeSaved': 'Theme saved',
      'settings.themeSaveFailed': 'Theme save failed',
      'settings.languageSaveFailed': 'Language save failed',
      'settings.loadFailed': 'Failed to load settings',
      'settings.selectWeekday': 'Select at least one weekday',
      'settings.raised': 'Raised active notes',
      'settings.raiseFailed': 'Raise failed',
      'settings.unavailable': 'Unavailable',

      'archive.documentTitle': 'Archived Notes',
      'archive.title': 'Archived Notes',
      'archive.empty': 'No archived notes',
      'archive.restore': 'Restore',
      'archive.delete': 'Delete',
      'archive.archived': 'Archived: {time}',
      'archive.unknownTime': 'Unknown time',
      'archive.placeholder': '(Archived note)',
      'archive.deleteTitle': 'Delete this note?',
      'archive.deleteMessage': 'This cannot be undone.',
      'archive.loadFailed': 'Load failed: {message}'
    },
    'zh-CN': {
      'common.cancel': '取消',
      'common.delete': '删除',
      'common.save': '保存',
      'note.close': '关闭',
      'note.delete': '删除',
      'note.alwaysOnTop': '置顶',
      'note.pin': '固定',
      'note.newButton': '新建便签',
      'note.newButtonTitle': '新建便签 (Ctrl+N)',
      'note.bold': '加粗 (Ctrl+B)',
      'note.list': '列表',
      'note.task': '任务',
      'note.status': '便签状态',
      'note.saved': '已保存',
      'note.saving': '保存中...',
      'note.saveFailed': '保存失败',
      'note.loadFailed': '加载失败',
      'note.placeholder': '写点什么...',
      'note.newTitle': '新便签',
      'note.archivePending': '归档时间待定',
      'note.deleteTitle': '删除这个便签？',
      'note.deleteMessage': '此操作无法撤销。',

      'settings.documentTitle': 'FadeNote 设置',
      'settings.title': '设置',
      'settings.themeTitle': '主题',
      'settings.themeDesc': '选择一个安静的全局纸张色调。',
      'settings.themeLabel': '主题',
      'settings.theme.paper': '纸张',
      'settings.theme.yellow': '奶油',
      'settings.theme.blue': '蓝色',
      'settings.theme.dusk': '暮色',
      'settings.languageTitle': '语言',
      'settings.languageDesc': '跟随系统语言，或手动选择。',
      'settings.languageLabel': '语言',
      'settings.language.system': '跟随系统',
      'settings.language.zhCN': '简体中文',
      'settings.language.en': 'English',
      'settings.scheduleTitle': '定时唤起',
      'settings.scheduleDesc': '在指定时间把活跃便签带到前台。',
      'settings.enableSchedule': '启用定时唤起',
      'settings.time': '时间',
      'settings.repeat': '重复',
      'settings.daily': '每天',
      'settings.weekly': '每周',
      'settings.weekdays': '星期',
      'settings.mon': '周一',
      'settings.tue': '周二',
      'settings.wed': '周三',
      'settings.thu': '周四',
      'settings.fri': '周五',
      'settings.sat': '周六',
      'settings.sun': '周日',
      'settings.raiseNow': '立即唤起',
      'settings.fileDir': '文件保存目录',
      'settings.fileDirDesc': '便签会以本地 Markdown 文件保存在这里。',
      'settings.loading': '加载中...',
      'settings.saved': '已保存',
      'settings.saveFailed': '保存失败',
      'settings.themeSaved': '主题已保存',
      'settings.themeSaveFailed': '主题保存失败',
      'settings.languageSaveFailed': '语言保存失败',
      'settings.loadFailed': '设置加载失败',
      'settings.selectWeekday': '请至少选择一个星期',
      'settings.raised': '已唤起活跃便签',
      'settings.raiseFailed': '唤起失败',
      'settings.unavailable': '不可用',

      'archive.documentTitle': '归档便签',
      'archive.title': '归档便签',
      'archive.empty': '没有归档便签',
      'archive.restore': '恢复',
      'archive.delete': '删除',
      'archive.archived': '归档：{time}',
      'archive.unknownTime': '未知时间',
      'archive.placeholder': '（归档便签）',
      'archive.deleteTitle': '删除这个便签？',
      'archive.deleteMessage': '此操作无法撤销。',
      'archive.loadFailed': '加载失败：{message}'
    }
  };

  let preference = SYSTEM_LANGUAGE;
  let currentLanguage = resolveLanguage(preference);

  function normalizeLanguage(language) {
    return SUPPORTED_LANGUAGES.has(language) ? language : SYSTEM_LANGUAGE;
  }

  function resolveLanguage(languagePreference) {
    const normalized = normalizeLanguage(languagePreference);
    if (normalized !== SYSTEM_LANGUAGE) return normalized;

    const browserLanguages = navigator.languages?.length ? navigator.languages : [navigator.language];
    const preferred = browserLanguages.find(Boolean) || DEFAULT_LANGUAGE;
    return preferred.toLowerCase().startsWith('zh') ? 'zh-CN' : DEFAULT_LANGUAGE;
  }

  function t(key, values = {}) {
    const template = messages[currentLanguage]?.[key] || messages[DEFAULT_LANGUAGE]?.[key] || key;
    return template.replace(/\{(\w+)\}/g, (_, name) => values[name] ?? '');
  }

  function applyStaticText() {
    document.querySelectorAll('[data-i18n]').forEach((element) => {
      element.textContent = t(element.dataset.i18n);
    });

    document.querySelectorAll('[data-i18n-title]').forEach((element) => {
      element.setAttribute('title', t(element.dataset.i18nTitle));
    });

    document.querySelectorAll('[data-i18n-aria-label]').forEach((element) => {
      element.setAttribute('aria-label', t(element.dataset.i18nAriaLabel));
    });

    document.querySelectorAll('[data-i18n-placeholder]').forEach((element) => {
      element.setAttribute('placeholder', t(element.dataset.i18nPlaceholder));
    });

    document.querySelectorAll('[data-i18n-value]').forEach((element) => {
      element.value = t(element.dataset.i18nValue);
    });

    const documentTitleKey = document.documentElement.dataset.i18nDocumentTitle;
    if (documentTitleKey) document.title = t(documentTitleKey);

    const editor = document.querySelector('.markdown-editor');
    if (editor) editor.dataset.placeholder = t('note.placeholder');

    const saveStatus = document.getElementById('note-save-status');
    if (saveStatus) {
      const statusKey = saveStatus.classList.contains('saving')
        ? 'note.saving'
        : saveStatus.classList.contains('error')
          ? 'note.saveFailed'
          : 'note.saved';
      const statusLabel = t(statusKey);
      saveStatus.title = statusLabel;
      saveStatus.setAttribute('aria-label', statusLabel);
    }
  }

  function applyLanguage(languagePreference) {
    preference = normalizeLanguage(languagePreference);
    currentLanguage = resolveLanguage(preference);
    document.documentElement.lang = currentLanguage;
    document.documentElement.dataset.language = preference;
    applyStaticText();
    return preference;
  }

  async function loadLanguage() {
    try {
      const settings = await window.__TAURI__?.core?.invoke('get_schedule_settings');
      return applyLanguage(settings?.language || SYSTEM_LANGUAGE);
    } catch (err) {
      console.warn('Failed to load language setting:', err);
      return applyLanguage(SYSTEM_LANGUAGE);
    }
  }

  window.FadeNoteI18n = {
    applyLanguage,
    applyStaticText,
    loadLanguage,
    normalizeLanguage,
    resolveLanguage,
    t
  };

  document.addEventListener('DOMContentLoaded', () => {
    loadLanguage();
    window.__TAURI__?.event?.listen('fadenote://language-changed', (event) => {
      applyLanguage(event.payload);
    });
  });
})();