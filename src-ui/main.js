import { EditorView, basicSetup } from 'codemirror';
import { EditorState } from '@codemirror/state';
import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
import { languages } from '@codemirror/language-data';
import { keymap, drawSelection } from '@codemirror/view';
import { indentWithTab } from '@codemirror/commands';
import { foldService, foldAll, unfoldAll, toggleFold, foldedRanges } from '@codemirror/language';
import { palettes, applyPalette } from './themes.js';

const headingFold = foldService.of((state, lineStart, lineEnd) => {
  const line = state.doc.lineAt(lineStart);
  const match = /^(#{1,6})\s/.exec(line.text);
  if (!match) return null;
  const level = match[1].length;
  let end = lineEnd;
  for (let i = line.number + 1; i <= state.doc.lines; i++) {
    const next = state.doc.line(i);
    const nextMatch = /^(#{1,6})\s/.exec(next.text);
    if (nextMatch && nextMatch[1].length <= level) break;
    end = next.to;
  }
  return end > lineEnd ? { from: lineEnd, to: end } : null;
});

const { invoke } = window.__TAURI__.core;

const panes = ['todo', 'today', 'done'];
let views = { todo: null, today: null, done: null };
let focusedPane = 'todo';
let doneVisible = true;
let currentPalette = 0;
let statusTimeout = null;
let autoSaveTimeout = null;

function showMessage(msg) {
  const el = document.getElementById('status-message');
  el.textContent = msg;
  clearTimeout(statusTimeout);
  statusTimeout = setTimeout(() => { el.textContent = ''; }, 3000);
}

function updateFocus() {
  panes.forEach(p => {
    document.getElementById(`${p}-pane`).classList.toggle('focused', focusedPane === p);
  });
}

function markDirty(pane) {
  document.getElementById(`${pane}-dirty`).style.display = 'inline';
}

function clearDirty() {
  panes.forEach(p => {
    document.getElementById(`${p}-dirty`).style.display = 'none';
  });
}

function getActiveView() {
  return views[focusedPane];
}

function getCursorLine(view) {
  const pos = view.state.selection.main.head;
  return view.state.doc.lineAt(pos).number - 1;
}

async function save(silent) {
  const todo = views.todo.state.doc.toString();
  const today = views.today.state.doc.toString();
  const done = views.done.state.doc.toString();
  await invoke('save_files', { todo, today, done });
  clearDirty();
  if (!silent) showMessage('Saved');
}

function scheduleAutoSave() {
  clearTimeout(autoSaveTimeout);
  autoSaveTimeout = setTimeout(() => save(true), 1000);
  // Reset idle sync timer on each edit
  scheduleIdleSync();
}

async function completeItem() {
  if (focusedPane === 'todo') {
    // todo -> today
    const source = views.todo.state.doc.toString();
    const target = views.today.state.doc.toString();
    const cursorLine = getCursorLine(views.todo);
    const result = await invoke('complete_item', { source, target, cursorLine, toDone: false });

    const cursorPos = Math.min(views.todo.state.selection.main.head, result.source.length);
    views.todo.dispatch({
      changes: { from: 0, to: views.todo.state.doc.length, insert: result.source },
      selection: { anchor: cursorPos },
    });
    views.today.dispatch({
      changes: { from: 0, to: views.today.state.doc.length, insert: result.target },
    });
    await save(true);
    showMessage('todo → today');
  } else if (focusedPane === 'today') {
    // today -> done
    const source = views.today.state.doc.toString();
    const target = views.done.state.doc.toString();
    const cursorLine = getCursorLine(views.today);
    const result = await invoke('complete_item', { source, target, cursorLine, toDone: true });

    const cursorPos = Math.min(views.today.state.selection.main.head, result.source.length);
    views.today.dispatch({
      changes: { from: 0, to: views.today.state.doc.length, insert: result.source },
      selection: { anchor: cursorPos },
    });
    views.done.dispatch({
      changes: { from: 0, to: views.done.state.doc.length, insert: result.target },
    });
    await save(true);
    showMessage('today → done');
  }
}

async function sendBack() {
  if (focusedPane === 'today') {
    // today -> todo (send back)
    const source = views.today.state.doc.toString();
    const target = views.todo.state.doc.toString();
    const cursorLine = getCursorLine(views.today);
    const result = await invoke('recover_item', { source, target, cursorLine, fromDone: false });

    const cursorPos = Math.min(views.today.state.selection.main.head, result.source.length);
    views.today.dispatch({
      changes: { from: 0, to: views.today.state.doc.length, insert: result.source },
      selection: { anchor: cursorPos },
    });
    views.todo.dispatch({
      changes: { from: 0, to: views.todo.state.doc.length, insert: result.target },
    });
    await save(true);
    showMessage('today → todo');
  } else if (focusedPane === 'done') {
    // done -> todo (recover)
    const source = views.done.state.doc.toString();
    const target = views.todo.state.doc.toString();
    const cursorLine = getCursorLine(views.done);
    const result = await invoke('recover_item', { source, target, cursorLine, fromDone: true });

    const cursorPos = Math.min(views.done.state.selection.main.head, result.source.length);
    views.done.dispatch({
      changes: { from: 0, to: views.done.state.doc.length, insert: result.source },
      selection: { anchor: cursorPos },
    });
    views.todo.dispatch({
      changes: { from: 0, to: views.todo.state.doc.length, insert: result.target },
    });
    await save(true);
    showMessage('done → todo');
  }
}

function toggleWrap(view, mark) {
  const { from, to } = view.state.selection.main;
  const len = mark.length;
  const doc = view.state.doc;

  if (from >= len && to + len <= doc.length) {
    const before = doc.sliceString(from - len, from);
    const after = doc.sliceString(to, to + len);
    if (before === mark && after === mark) {
      view.dispatch({
        changes: [
          { from: from - len, to: from, insert: '' },
          { from: to, to: to + len, insert: '' },
        ],
        selection: { anchor: from - len, head: to - len },
      });
      return true;
    }
  }

  view.dispatch({
    changes: [
      { from, insert: mark },
      { from: to, insert: mark },
    ],
    selection: { anchor: from + len, head: to + len },
  });
  return true;
}

function createEditor(parent, content, pane) {
  const changeListener = EditorView.updateListener.of((update) => {
    if (update.docChanged) {
      markDirty(pane);
      scheduleAutoSave();
    }
    if (update.focusChanged && update.view.hasFocus) {
      focusedPane = pane;
      updateFocus();
    }
  });

  const customKeymap = keymap.of([
    {
      key: 'Mod-s',
      run: () => { save(); return true; },
    },
    {
      key: 'Mod-Enter',
      run: () => { completeItem(); return true; },
    },
    {
      key: 'Mod-Shift-Enter',
      run: () => { sendBack(); return true; },
    },
    {
      key: 'Mod-b',
      run: (view) => toggleWrap(view, '**'),
    },
    {
      key: 'Mod-i',
      run: (view) => toggleWrap(view, '*'),
    },
    {
      key: 'Mod-e',
      run: (view) => { toggleFold(view); return true; },
    },
    {
      key: 'Mod-Shift-e',
      run: (view) => {
        const hasFolded = foldedRanges(view.state).size > 0;
        if (hasFolded) unfoldAll(view); else foldAll(view);
        return true;
      },
    },
  ]);

  const state = EditorState.create({
    doc: content,
    extensions: [
      customKeymap,
      basicSetup,
      keymap.of([indentWithTab]),
      markdown({ base: markdownLanguage, codeLanguages: languages }),
      headingFold,
      changeListener,
      EditorView.lineWrapping,
    ],
  });

  return new EditorView({ state, parent });
}

function toggleDonePane() {
  doneVisible = !doneVisible;
  const donePane = document.getElementById('done-pane');
  const dividers = document.querySelectorAll('.divider');
  if (doneVisible) {
    donePane.style.display = '';
    dividers[1].style.display = '';
    showMessage('Done pane shown');
  } else {
    donePane.style.display = 'none';
    dividers[1].style.display = 'none';
    if (focusedPane === 'done') {
      focusedPane = 'today';
      updateFocus();
      views.today.focus();
    }
    showMessage('Done pane hidden');
  }
}

function cyclePalette() {
  currentPalette = (currentPalette + 1) % palettes.length;
  const p = palettes[currentPalette];
  applyPalette(p);
  saveThemeToSettings(currentPalette);
  showMessage(`Theme: ${p.name}`);
}

async function saveThemeToSettings(idx) {
  const s = await invoke('load_settings');
  await invoke('save_settings', {
    storageMode: s.storage_mode,
    localPath: s.local_path,
    gitRepo: s.git_repo,
    themeIndex: idx,
    dateFormat: s.date_format,
    layout: s.layout,
    paneSizes: s.pane_sizes,
    syncInterval: s.sync_interval,
    setupDone: s.setup_done,
  });
}

// --- Layout ---
function applyLayout(layout, sizes) {
  const panesEl = document.getElementById('panes');
  panesEl.style.flexDirection = layout === 'vertical' ? 'column' : 'row';

  const dividers = document.querySelectorAll('.divider');
  dividers.forEach(d => {
    if (layout === 'vertical') {
      d.style.width = '';
      d.style.height = '1px';
      d.style.cursor = 'row-resize';
    } else {
      d.style.height = '';
      d.style.width = '1px';
      d.style.cursor = 'col-resize';
    }
  });

  document.getElementById('todo-pane').style.flex = sizes[0];
  document.getElementById('today-pane').style.flex = sizes[1];
  document.getElementById('done-pane').style.flex = sizes[2];
}

function initDividerDrag() {
  const dividers = document.querySelectorAll('.divider');
  const paneEls = [
    document.getElementById('todo-pane'),
    document.getElementById('today-pane'),
    document.getElementById('done-pane'),
  ];

  dividers.forEach((divider, idx) => {
    divider.addEventListener('mousedown', (e) => {
      e.preventDefault();
      const isVertical = settingsState.layout === 'vertical';
      const panesEl = document.getElementById('panes');
      const totalSize = isVertical ? panesEl.offsetHeight : panesEl.offsetWidth;

      const startPos = isVertical ? e.clientY : e.clientX;
      const leftPane = paneEls[idx];
      const rightPane = paneEls[idx + 1];
      const startLeft = isVertical ? leftPane.offsetHeight : leftPane.offsetWidth;
      const startRight = isVertical ? rightPane.offsetHeight : rightPane.offsetWidth;

      function onMove(e) {
        const delta = (isVertical ? e.clientY : e.clientX) - startPos;
        const newLeft = Math.max(50, startLeft + delta);
        const newRight = Math.max(50, startRight - delta);
        const leftPct = (newLeft / totalSize) * 100;
        const rightPct = (newRight / totalSize) * 100;
        leftPane.style.flex = leftPct;
        rightPane.style.flex = rightPct;
        settingsState.paneSizes[idx] = leftPct;
        settingsState.paneSizes[idx + 1] = rightPct;
      }

      function onUp() {
        document.removeEventListener('mousemove', onMove);
        document.removeEventListener('mouseup', onUp);
        // Persist sizes
        savePaneSizes();
      }

      document.addEventListener('mousemove', onMove);
      document.addEventListener('mouseup', onUp);
    });
  });
}

async function savePaneSizes() {
  const s = await invoke('load_settings');
  await invoke('save_settings', {
    storageMode: s.storage_mode,
    localPath: s.local_path,
    gitRepo: s.git_repo,
    themeIndex: s.theme_index,
    dateFormat: s.date_format,
    layout: s.layout,
    paneSizes: settingsState.paneSizes,
    syncInterval: s.sync_interval,
    setupDone: s.setup_done,
  });
}

// --- Settings panel ---
let settingsState = { storageMode: 'local', localPath: '', gitRepo: '', themeIndex: 0, dateFormat: '%Y-%m-%d', layout: 'horizontal', paneSizes: [40, 30, 30], syncInterval: 5 };
let syncIdleTimeout = null;
let syncIntervalTimer = null;
let isSyncing = false;

function updateSyncStatus(msg) {
  const el = document.getElementById('sync-status');
  if (el) el.textContent = msg;
}

async function gitSync(silent) {
  if (isSyncing) return;
  if (settingsState.storageMode !== 'git') return;
  isSyncing = true;
  updateSyncStatus('syncing...');
  try {
    const result = await invoke('git_sync_full');
    updateSyncStatus('');
    if (!silent) showMessage(result);
  } catch (e) {
    updateSyncStatus('sync error');
    if (!silent) showMessage('Sync error: ' + e);
  } finally {
    isSyncing = false;
  }
}

async function gitPull(silent) {
  if (isSyncing) return;
  if (settingsState.storageMode !== 'git') return;
  isSyncing = true;
  updateSyncStatus('pulling...');
  try {
    const result = await invoke('git_pull');
    updateSyncStatus('');
    if (!silent) showMessage(result);
    // Reload files after pull
    const files = await invoke('load_files');
    views.todo.dispatch({ changes: { from: 0, to: views.todo.state.doc.length, insert: files.todo } });
    views.today.dispatch({ changes: { from: 0, to: views.today.state.doc.length, insert: files.today } });
    views.done.dispatch({ changes: { from: 0, to: views.done.state.doc.length, insert: files.done } });
  } catch (e) {
    updateSyncStatus('pull error');
    if (!silent) showMessage('Pull error: ' + e);
  } finally {
    isSyncing = false;
  }
}

async function gitPush(silent) {
  if (isSyncing) return;
  if (settingsState.storageMode !== 'git') return;
  isSyncing = true;
  updateSyncStatus('pushing...');
  try {
    const result = await invoke('git_push');
    updateSyncStatus('');
    if (!silent) showMessage(result);
  } catch (e) {
    updateSyncStatus('push error');
    if (!silent) showMessage('Push error: ' + e);
  } finally {
    isSyncing = false;
  }
}

function scheduleIdleSync() {
  if (settingsState.storageMode !== 'git' || settingsState.syncInterval === 0) return;
  clearTimeout(syncIdleTimeout);
  syncIdleTimeout = setTimeout(() => {
    gitSync(true);
  }, settingsState.syncInterval * 60 * 1000);
}

function startSyncInterval() {
  stopSyncInterval();
  if (settingsState.storageMode !== 'git' || settingsState.syncInterval === 0) return;
  syncIntervalTimer = setInterval(() => {
    gitSync(true);
  }, settingsState.syncInterval * 60 * 1000);
}

function stopSyncInterval() {
  if (syncIntervalTimer) {
    clearInterval(syncIntervalTimer);
    syncIntervalTimer = null;
  }
}

function openSettings(isFirstTime) {
  const overlay = document.getElementById('settings-overlay');
  overlay.style.display = 'flex';

  const title = document.getElementById('settings-title');
  title.textContent = isFirstTime ? 'Welcome to Tally.md' : 'Settings';

  const cancel = document.getElementById('settings-cancel');
  cancel.style.display = isFirstTime ? 'none' : '';

  const showGitFields = (mode) => {
    const isGit = mode === 'git';
    document.getElementById('group-local-path').style.display = isGit ? 'none' : '';
    document.getElementById('group-git-repo').style.display = isGit ? '' : 'none';
    document.getElementById('group-git-token').style.display = isGit ? '' : 'none';
    document.getElementById('group-sync-interval').style.display = isGit ? '' : 'none';
  };

  // Storage mode buttons
  document.querySelectorAll('[data-mode]').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.mode === settingsState.storageMode);
    btn.onclick = () => {
      settingsState.storageMode = btn.dataset.mode;
      document.querySelectorAll('[data-mode]').forEach(b => b.classList.toggle('active', b.dataset.mode === settingsState.storageMode));
      showGitFields(settingsState.storageMode);
    };
  });

  showGitFields(settingsState.storageMode);

  document.getElementById('settings-path').value = settingsState.localPath;
  document.getElementById('settings-git-repo').value = settingsState.gitRepo;

  // Git token
  const tokenInput = document.getElementById('settings-git-token');
  tokenInput.value = '';
  const tokenStatus = document.getElementById('token-status');
  invoke('git_has_token').then(has => {
    tokenStatus.textContent = has ? 'Token stored in keychain' : 'No token stored';
    if (has) tokenInput.placeholder = '••••••••  (stored)';
  });

  // Toggle token visibility
  const toggleBtn = document.getElementById('btn-toggle-token');
  toggleBtn.textContent = 'Show';
  tokenInput.type = 'password';
  toggleBtn.onclick = () => {
    const showing = tokenInput.type === 'text';
    tokenInput.type = showing ? 'password' : 'text';
    toggleBtn.textContent = showing ? 'Show' : 'Hide';
  };

  // Generate token link — auto-detect GitHub/GitLab
  const helpLink = document.getElementById('token-help-link');
  const repoUrl = document.getElementById('settings-git-repo').value || '';
  if (repoUrl.includes('gitlab')) {
    helpLink.href = 'https://gitlab.com/-/user_settings/personal_access_tokens';
  } else {
    helpLink.href = 'https://github.com/settings/tokens/new?description=Tally.md&scopes=repo';
  }

  // Sync interval buttons
  document.querySelectorAll('[data-sync]').forEach(btn => {
    btn.classList.toggle('active', parseInt(btn.dataset.sync) === settingsState.syncInterval);
    btn.onclick = () => {
      settingsState.syncInterval = parseInt(btn.dataset.sync);
      document.querySelectorAll('[data-sync]').forEach(b => b.classList.toggle('active', parseInt(b.dataset.sync) === settingsState.syncInterval));
    };
  });

  // Theme swatches
  const picker = document.getElementById('theme-picker');
  picker.innerHTML = '';
  palettes.forEach((p, i) => {
    const swatch = document.createElement('div');
    swatch.className = 'theme-swatch' + (i === settingsState.themeIndex ? ' active' : '');
    swatch.style.background = p.bg;
    swatch.style.borderColor = i === settingsState.themeIndex ? p.text : p.border;
    swatch.title = p.name;
    swatch.onclick = () => {
      settingsState.themeIndex = i;
      picker.querySelectorAll('.theme-swatch').forEach((s, j) => {
        s.classList.toggle('active', j === i);
        s.style.borderColor = j === i ? palettes[j].text : palettes[j].border;
      });
      currentPalette = i;
      applyPalette(palettes[i]);
    };
    picker.appendChild(swatch);
  });

  // Layout buttons
  document.querySelectorAll('[data-layout]').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.layout === settingsState.layout);
    btn.onclick = () => {
      settingsState.layout = btn.dataset.layout;
      document.querySelectorAll('[data-layout]').forEach(b => b.classList.toggle('active', b.dataset.layout === settingsState.layout));
      applyLayout(settingsState.layout, settingsState.paneSizes);
    };
  });

  // Date format buttons
  document.querySelectorAll('[data-fmt]').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.fmt === settingsState.dateFormat);
    btn.onclick = () => {
      settingsState.dateFormat = btn.dataset.fmt;
      document.querySelectorAll('[data-fmt]').forEach(b => b.classList.toggle('active', b.dataset.fmt === settingsState.dateFormat));
    };
  });

  // Save button
  document.getElementById('settings-save').onclick = async () => {
    settingsState.localPath = document.getElementById('settings-path').value || settingsState.localPath;
    settingsState.gitRepo = document.getElementById('settings-git-repo').value || '';

    // Store git token if provided
    const tokenVal = document.getElementById('settings-git-token').value;
    if (tokenVal && settingsState.storageMode === 'git') {
      try {
        await invoke('git_store_token', { token: tokenVal });
      } catch (e) {
        showMessage('Failed to store token: ' + e);
      }
    }

    await invoke('save_settings', {
      storageMode: settingsState.storageMode,
      localPath: settingsState.localPath,
      gitRepo: settingsState.gitRepo,
      themeIndex: settingsState.themeIndex,
      dateFormat: settingsState.dateFormat,
      layout: settingsState.layout,
      paneSizes: settingsState.paneSizes,
      syncInterval: settingsState.syncInterval,
      setupDone: true,
    });

    overlay.style.display = 'none';
    showMessage('Settings saved');

    // If git mode, do initial pull
    if (settingsState.storageMode === 'git' && settingsState.gitRepo) {
      await gitPull(false);
    }

    // Reload files from potentially new path
    const files = await invoke('load_files');
    document.querySelector('#todo-pane .pane-title').textContent = files.todo_path;
    document.querySelector('#today-pane .pane-title').textContent = files.today_path;
    document.querySelector('#done-pane .pane-title').textContent = files.done_path;
    views.todo.dispatch({ changes: { from: 0, to: views.todo.state.doc.length, insert: files.todo } });
    views.today.dispatch({ changes: { from: 0, to: views.today.state.doc.length, insert: files.today } });
    views.done.dispatch({ changes: { from: 0, to: views.done.state.doc.length, insert: files.done } });

    // Restart sync timer
    startSyncInterval();
  };

  // Cancel button
  document.getElementById('settings-cancel').onclick = () => {
    overlay.style.display = 'none';
    // Revert theme if changed
    applyPalette(palettes[currentPalette]);
  };
}

function cyclePane(direction) {
  const visible = panes.filter(p => {
    if (p === 'done' && !doneVisible) return false;
    return true;
  });
  const idx = visible.indexOf(focusedPane);
  const next = (idx + direction + visible.length) % visible.length;
  focusedPane = visible[next];
  updateFocus();
  views[focusedPane].focus();
}

// Keyboard shortcuts
document.addEventListener('keydown', (e) => {
  if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === '\\') {
    e.preventDefault();
    cyclePane(-1);
  } else if ((e.ctrlKey || e.metaKey) && e.key === '\\') {
    e.preventDefault();
    cyclePane(1);
  }
  if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'S') {
    e.preventDefault();
    gitSync(false);
  } else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'B') {
    e.preventDefault();
    toggleDonePane();
  }
  if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
    e.preventDefault();
    cyclePalette();
  }
  if ((e.ctrlKey || e.metaKey) && e.key === ',') {
    e.preventDefault();
    openSettings(false);
  }
  if (e.key === 'Escape') {
    const overlay = document.getElementById('settings-overlay');
    if (overlay.style.display !== 'none') {
      overlay.style.display = 'none';
      applyPalette(palettes[currentPalette]);
    }
  }
});

function setHelpText() {
  const mod = navigator.platform.includes('Mac') ? '⌘' : 'Ctrl';
  const shift = navigator.platform.includes('Mac') ? '⇧' : 'Shift';
  document.getElementById('status-help').textContent =
    `${mod}+Enter: move → · ${mod}+${shift}+Enter: send ← · ${mod}+\\: pane · ${mod}+S: save · ${mod}+${shift}+S: sync · ${mod}+K: theme · ${mod}+,: settings`;
}

async function init() {
  // Load settings first
  const settings = await invoke('load_settings');
  settingsState = {
    storageMode: settings.storage_mode,
    localPath: settings.local_path,
    gitRepo: settings.git_repo,
    themeIndex: settings.theme_index,
    dateFormat: settings.date_format,
    layout: settings.layout || 'horizontal',
    paneSizes: settings.pane_sizes || [40, 30, 30],
    syncInterval: settings.sync_interval ?? 5,
  };
  currentPalette = settings.theme_index;

  // Pull from git on startup
  if (settings.setup_done && settingsState.storageMode === 'git' && settingsState.gitRepo) {
    await gitPull(true);
  }

  const files = await invoke('load_files');

  document.querySelector('#todo-pane .pane-title').textContent = files.todo_path;
  document.querySelector('#today-pane .pane-title').textContent = files.today_path;
  document.querySelector('#done-pane .pane-title').textContent = files.done_path;

  views.todo = createEditor(
    document.getElementById('todo-editor'),
    files.todo,
    'todo'
  );

  views.today = createEditor(
    document.getElementById('today-editor'),
    files.today,
    'today'
  );

  views.done = createEditor(
    document.getElementById('done-editor'),
    files.done,
    'done'
  );

  applyPalette(palettes[currentPalette]);
  applyLayout(settingsState.layout, settingsState.paneSizes);
  initDividerDrag();
  setHelpText();
  focusedPane = 'todo';
  updateFocus();
  views.todo.focus();

  if (!settings.setup_done) {
    openSettings(true);
  } else {
    showMessage('Ready');
  }

  // Start auto-sync interval
  startSyncInterval();

  // Push on app close
  window.addEventListener('beforeunload', () => {
    if (settingsState.storageMode === 'git' && settingsState.gitRepo) {
      // Fire-and-forget push — beforeunload can't await
      invoke('git_push').catch(() => {});
    }
  });
}

init();
