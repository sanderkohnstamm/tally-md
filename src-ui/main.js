import { EditorView, basicSetup } from 'codemirror';
import { EditorState, RangeSetBuilder } from '@codemirror/state';
import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
import { languages } from '@codemirror/language-data';
import { keymap, drawSelection, Decoration, ViewPlugin, WidgetType } from '@codemirror/view';
import { indentWithTab } from '@codemirror/commands';
import { foldService, foldAll, unfoldAll, toggleFold, foldedRanges } from '@codemirror/language';
import { palettes, applyPalette } from './themes.js';

const { invoke } = window.__TAURI__.core;

// Clickable links
const urlRegex = /https?:\/\/[^\s)>\]]+/g;
const linkMark = Decoration.mark({ class: 'cm-clickable-link' });

function findLinks(view) {
  const builder = new RangeSetBuilder();
  for (const { from, to } of view.visibleRanges) {
    for (let pos = from; pos < to; ) {
      const line = view.state.doc.lineAt(pos);
      let m;
      urlRegex.lastIndex = 0;
      while ((m = urlRegex.exec(line.text))) {
        const start = line.from + m.index;
        const end = start + m[0].length;
        builder.add(start, end, linkMark);
      }
      pos = line.to + 1;
    }
  }
  return builder.finish();
}

const clickableLinks = ViewPlugin.fromClass(class {
  constructor(view) { this.decorations = findLinks(view); }
  update(update) {
    if (update.docChanged || update.viewportChanged) {
      this.decorations = findLinks(update.view);
    }
  }
}, { decorations: v => v.decorations });

const linkClickHandler = EditorView.domEventHandlers({
  click(event, view) {
    if (!(event.ctrlKey || event.metaKey)) return false;
    const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
    if (pos == null) return false;
    const line = view.state.doc.lineAt(pos);
    const col = pos - line.from;
    urlRegex.lastIndex = 0;
    let m;
    while ((m = urlRegex.exec(line.text))) {
      if (col >= m.index && col <= m.index + m[0].length) {
        invoke('open_url', { url: m[0] });
        event.preventDefault();
        return true;
      }
    }
    return false;
  }
});

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

async function skipToDone() {
  if (focusedPane !== 'todo') return;
  const source = views.todo.state.doc.toString();
  const target = views.done.state.doc.toString();
  const cursorLine = getCursorLine(views.todo);
  const result = await invoke('complete_item', { source, target, cursorLine, toDone: true });

  const cursorPos = Math.min(views.todo.state.selection.main.head, result.source.length);
  views.todo.dispatch({
    changes: { from: 0, to: views.todo.state.doc.length, insert: result.source },
    selection: { anchor: cursorPos },
  });
  views.done.dispatch({
    changes: { from: 0, to: views.done.state.doc.length, insert: result.target },
  });
  await save(true);
  showMessage('todo → done');
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

  // Dynamic keymap — reads from settingsState.keybindings
  const editorActions = {
    save: () => { save(); return true; },
    moveForward: () => { completeItem(); return true; },
    sendBack: () => { sendBack(); return true; },
    skipToDone: () => { skipToDone(); return true; },
    bold: (view) => toggleWrap(view, '**'),
    italic: (view) => toggleWrap(view, '*'),
    toggleFold: (view) => { toggleFold(view); return true; },
    toggleFoldAll: (view) => {
      const hasFolded = foldedRanges(view.state).size > 0;
      if (hasFolded) unfoldAll(view); else foldAll(view);
      return true;
    },
  };

  const customKeymap = keymap.of(
    Object.entries(editorActions).map(([action, run]) => ({
      key: settingsState.keybindings[action] || DEFAULT_KEYBINDINGS[action],
      run,
    }))
  );

  const state = EditorState.create({
    doc: content,
    extensions: [
      customKeymap,
      basicSetup,
      keymap.of([indentWithTab]),
      markdown({ base: markdownLanguage, codeLanguages: languages }),
      headingFold,
      clickableLinks,
      linkClickHandler,
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

async function saveSettingsToBackend() {
  await invoke('save_settings', {
    storageMode: settingsState.storageMode,
    localPath: settingsState.localPath,
    gitRepo: settingsState.gitRepo,
    gitRepoName: settingsState.gitRepoName,
    themeIndex: settingsState.themeIndex,
    dateFormat: settingsState.dateFormat,
    layout: settingsState.layout,
    paneSizes: settingsState.paneSizes,
    syncInterval: settingsState.syncInterval,
    setupDone: settingsState.setupDone ?? true,
    keybindings: settingsState.keybindings,
  });
}

async function saveThemeToSettings(idx) {
  settingsState.themeIndex = idx;
  await saveSettingsToBackend();
}

// --- Layout ---
function applyLayout(layout, sizes) {
  const panesEl = document.getElementById('panes');
  const todoPaneEl = document.getElementById('todo-pane');
  const todayPaneEl = document.getElementById('today-pane');
  const donePaneEl = document.getElementById('done-pane');
  const dividers = document.querySelectorAll('#panes > .divider');

  // Remove split wrapper if it exists
  const existingWrapper = document.getElementById('right-stack');
  if (existingWrapper) {
    // Move panes back to #panes
    const wrapperDivider = existingWrapper.querySelector('.divider');
    panesEl.insertBefore(todayPaneEl, existingWrapper);
    if (wrapperDivider) panesEl.insertBefore(wrapperDivider, existingWrapper);
    panesEl.insertBefore(donePaneEl, existingWrapper);
    existingWrapper.remove();
  }

  if (layout === 'split') {
    panesEl.style.flexDirection = 'row';

    // Wrap today + divider + done in a vertical stack
    const wrapper = document.createElement('div');
    wrapper.id = 'right-stack';
    wrapper.style.display = 'flex';
    wrapper.style.flexDirection = 'column';
    wrapper.style.flex = sizes[1] + sizes[2];
    wrapper.style.overflow = 'hidden';

    // The second divider becomes a horizontal one inside the stack
    const stackDivider = dividers[1] || document.createElement('div');
    stackDivider.className = 'divider divider-row';

    wrapper.appendChild(todayPaneEl);
    wrapper.appendChild(stackDivider);
    wrapper.appendChild(donePaneEl);
    panesEl.appendChild(wrapper);

    // First divider is vertical (col-resize)
    if (dividers[0]) {
      dividers[0].className = 'divider divider-col';
    }

    todoPaneEl.style.flex = sizes[0];
    todayPaneEl.style.flex = sizes[1];
    donePaneEl.style.flex = sizes[2];
  } else {
    panesEl.style.flexDirection = layout === 'vertical' ? 'column' : 'row';
    const cls = layout === 'vertical' ? 'divider divider-row' : 'divider divider-col';

    dividers.forEach(d => {
      d.className = cls;
    });

    todoPaneEl.style.flex = sizes[0];
    todayPaneEl.style.flex = sizes[1];
    donePaneEl.style.flex = sizes[2];
  }
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
      const isSplit = settingsState.layout === 'split';
      // In split mode: divider 0 is horizontal (col-resize), divider 1 is vertical (row-resize inside stack)
      const useVertical = settingsState.layout === 'vertical' || (isSplit && idx === 1);
      const container = (isSplit && idx === 1) ? document.getElementById('right-stack') : document.getElementById('panes');
      const totalSize = useVertical ? container.offsetHeight : container.offsetWidth;

      const startPos = useVertical ? e.clientY : e.clientX;
      const leftPane = (isSplit && idx === 0) ? paneEls[0] : paneEls[idx];
      const rightPane = (isSplit && idx === 0) ? document.getElementById('right-stack') : paneEls[idx + 1];
      const startLeft = useVertical ? leftPane.offsetHeight : leftPane.offsetWidth;
      const startRight = useVertical ? rightPane.offsetHeight : rightPane.offsetWidth;

      function onMove(e) {
        const delta = (useVertical ? e.clientY : e.clientX) - startPos;
        const newLeft = Math.max(50, startLeft + delta);
        const newRight = Math.max(50, startRight - delta);
        const leftPct = (newLeft / totalSize) * 100;
        const rightPct = (newRight / totalSize) * 100;
        leftPane.style.flex = leftPct;
        rightPane.style.flex = rightPct;
        if (isSplit && idx === 0) {
          settingsState.paneSizes[0] = leftPct;
          // Keep today/done ratio, scale to new total
          settingsState.paneSizes[1] = rightPct / 2;
          settingsState.paneSizes[2] = rightPct / 2;
        } else {
          settingsState.paneSizes[idx] = leftPct;
          settingsState.paneSizes[idx + 1] = rightPct;
        }
      }

      function onUp() {
        document.removeEventListener('mousemove', onMove);
        document.removeEventListener('mouseup', onUp);
        savePaneSizes();
      }

      document.addEventListener('mousemove', onMove);
      document.addEventListener('mouseup', onUp);
    });
  });
}

async function savePaneSizes() {
  await saveSettingsToBackend();
}

// --- Settings panel ---
const DEFAULT_KEYBINDINGS = {
  save: 'Mod-s',
  moveForward: 'Mod-Enter',
  sendBack: 'Mod-Shift-Enter',
  skipToDone: 'Mod-Shift-d',
  bold: 'Mod-b',
  italic: 'Mod-i',
  toggleFold: 'Mod-e',
  toggleFoldAll: 'Mod-Shift-e',
  cyclePane: 'Mod-\\',
  cyclePaneBack: 'Mod-Shift-\\',
  toggleDonePane: 'Mod-Shift-b',
  sync: 'Mod-Shift-s',
  cycleTheme: 'Mod-k',
  openSettings: 'Mod-,',
};

const ACTION_LABELS = {
  save: 'Save',
  moveForward: 'Move forward',
  sendBack: 'Send back',
  skipToDone: 'Skip to done',
  bold: 'Bold',
  italic: 'Italic',
  toggleFold: 'Toggle fold',
  toggleFoldAll: 'Toggle fold all',
  cyclePane: 'Next pane',
  cyclePaneBack: 'Previous pane',
  toggleDonePane: 'Toggle done pane',
  sync: 'Git sync',
  cycleTheme: 'Cycle theme',
  openSettings: 'Settings',
};

let settingsState = { storageMode: 'local', localPath: '', gitRepo: '', gitRepoName: 'tally-md-log', themeIndex: 0, dateFormat: '%Y-%m-%d', layout: 'horizontal', paneSizes: [40, 30, 30], syncInterval: 5, keybindings: { ...DEFAULT_KEYBINDINGS } };
let syncIdleTimeout = null;
let syncIntervalTimer = null;
let isSyncing = false;

function updateStorageIndicator() {
  const el = document.getElementById('storage-mode');
  if (settingsState.storageMode === 'git') {
    el.textContent = 'git: ' + (settingsState.gitRepoName || 'repo');
  } else {
    el.textContent = 'local';
  }
}

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
    updateSyncStatus(silent ? '' : 'sync error');
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
  } catch (e) {
    // Don't persist error in status bar for silent pulls
    updateSyncStatus(silent ? '' : 'pull error');
    if (!silent) showMessage('Pull error: ' + e);
  }
  // Always reload files — even if pull errored, local files may be fine
  try {
    if (views.todo) {
      const files = await invoke('load_files');
      views.todo.dispatch({ changes: { from: 0, to: views.todo.state.doc.length, insert: files.todo } });
      views.today.dispatch({ changes: { from: 0, to: views.today.state.doc.length, insert: files.today } });
      views.done.dispatch({ changes: { from: 0, to: views.done.state.doc.length, insert: files.done } });
    }
  } catch (_) {}
  isSyncing = false;
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
  const savedPalette = currentPalette;

  const title = document.getElementById('settings-title');
  title.textContent = isFirstTime ? 'Welcome to Tally.md' : 'Settings';

  const cancel = document.getElementById('settings-cancel');
  cancel.style.display = isFirstTime ? 'none' : '';

  const showGitFields = (mode) => {
    const isGit = mode === 'git';
    document.getElementById('group-local-path').style.display = isGit ? 'none' : '';
    document.getElementById('group-git-repo').style.display = isGit ? '' : 'none';
    document.getElementById('group-git-url').style.display = isGit ? '' : 'none';
    document.getElementById('group-git-token').style.display = isGit ? '' : 'none';
    document.getElementById('group-git-init').style.display = isGit ? '' : 'none';
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
  const repoNameInput = document.getElementById('settings-git-repo-name');
  repoNameInput.value = settingsState.gitRepoName || 'tally-md-log';
  document.getElementById('settings-git-repo').value = settingsState.gitRepo;

  // Auto-fill git URL when repo name changes (if URL is empty or was auto-generated)
  const gitRepoInput = document.getElementById('settings-git-repo');
  repoNameInput.oninput = () => {
    const name = repoNameInput.value || 'tally-md-log';
    gitRepoInput.placeholder = `https://github.com/user/${name}.git`;
  };
  repoNameInput.oninput();

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

  // Generate token link — auto-detect GitHub/GitLab, open via IPC
  const helpLink = document.getElementById('token-help-link');
  function updateTokenLink() {
    const repoUrl = gitRepoInput.value || settingsState.gitRepo || '';
    if (repoUrl.includes('gitlab')) {
      helpLink.dataset.url = 'https://gitlab.com/-/user_settings/personal_access_tokens';
    } else {
      helpLink.dataset.url = 'https://github.com/settings/tokens/new?description=Tally.md&scopes=repo';
    }
  }
  updateTokenLink();
  gitRepoInput.oninput = updateTokenLink;
  helpLink.onclick = (e) => {
    e.preventDefault();
    if (helpLink.dataset.url) invoke('open_url', { url: helpLink.dataset.url });
  };

  // Init repo button
  const initBtn = document.getElementById('btn-init-repo');
  const initStatus = document.getElementById('init-status');
  initBtn.onclick = async () => {
    const repoUrl = document.getElementById('settings-git-repo').value || settingsState.gitRepo;
    const repoName = document.getElementById('settings-git-repo-name').value || settingsState.gitRepoName || 'tally-md-log';
    // Store token first if provided
    const tokenVal = document.getElementById('settings-git-token').value;
    if (tokenVal) {
      try {
        await invoke('git_store_token', { token: tokenVal });
      } catch (e) {
        initStatus.textContent = 'Token error: ' + e;
        initBtn.disabled = false;
        return;
      }
    }
    if (!repoUrl) {
      initStatus.textContent = 'Enter a repo URL first';
      return;
    }
    initBtn.disabled = true;
    initStatus.textContent = 'Initializing...';
    try {
      const result = await invoke('git_init_repo', { repoUrl, repoName });
      initStatus.textContent = result;
    } catch (e) {
      initStatus.textContent = 'Error: ' + e;
    } finally {
      initBtn.disabled = false;
    }
  };

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

  // Date format buttons — show today's date in each format
  const now = new Date();
  const d = now.getDate();
  const m = now.getMonth() + 1;
  const y = now.getFullYear();
  const dd = String(d).padStart(2, '0');
  const mm = String(m).padStart(2, '0');
  const monthName = now.toLocaleString('en', { month: 'long' });
  const fmtPreview = {
    '%Y-%m-%d': `${y}-${mm}-${dd}`,
    '%d-%m-%Y': `${dd}-${mm}-${y}`,
    '%d/%m/%Y': `${dd}/${mm}/${y}`,
    '%m/%d/%Y': `${mm}/${dd}/${y}`,
    '%B %d, %Y': `${monthName} ${dd}, ${y}`,
    '%d %B %Y': `${dd} ${monthName} ${y}`,
  };
  document.querySelectorAll('[data-fmt]').forEach(btn => {
    btn.textContent = fmtPreview[btn.dataset.fmt] || btn.dataset.fmt;
    btn.classList.toggle('active', btn.dataset.fmt === settingsState.dateFormat);
    btn.onclick = async () => {
      settingsState.dateFormat = btn.dataset.fmt;
      document.querySelectorAll('[data-fmt]').forEach(b => b.classList.toggle('active', b.dataset.fmt === settingsState.dateFormat));
      // Reformat done.md headers live
      if (views.done) {
        const reformatted = await invoke('reformat_dates', {
          doneContent: views.done.state.doc.toString(),
          newFormat: btn.dataset.fmt,
        });
        views.done.dispatch({
          changes: { from: 0, to: views.done.state.doc.length, insert: reformatted },
        });
      }
    };
  });

  // Save button
  document.getElementById('settings-save').onclick = async () => {
    settingsState.localPath = document.getElementById('settings-path').value || settingsState.localPath;
    settingsState.gitRepoName = document.getElementById('settings-git-repo-name').value || 'tally-md-log';
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

    settingsState.setupDone = true;
    await saveSettingsToBackend();

    overlay.style.display = 'none';
    updateStorageIndicator();
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

  // Keyboard shortcuts button
  document.getElementById('btn-open-shortcuts').onclick = () => {
    openShortcuts();
  };

  // Cancel button
  document.getElementById('settings-cancel').onclick = () => {
    overlay.style.display = 'none';
    // Revert theme if changed during preview
    currentPalette = savedPalette;
    settingsState.themeIndex = savedPalette;
    applyPalette(palettes[savedPalette]);
  };
}

function openShortcuts() {
  const overlay = document.getElementById('shortcuts-overlay');
  overlay.style.display = 'flex';
  const list = document.getElementById('shortcuts-list');
  list.innerHTML = '';

  for (const [action, label] of Object.entries(ACTION_LABELS)) {
    const row = document.createElement('div');
    row.className = 'shortcut-row';

    const labelEl = document.createElement('span');
    labelEl.className = 'shortcut-label';
    labelEl.textContent = label;

    const keyEl = document.createElement('span');
    keyEl.className = 'shortcut-key';
    keyEl.textContent = formatKeyForDisplay(settingsState.keybindings[action] || DEFAULT_KEYBINDINGS[action]);
    keyEl.dataset.action = action;

    keyEl.onclick = () => {
      // Already recording another? Cancel it
      list.querySelectorAll('.recording').forEach(el => {
        el.classList.remove('recording');
        el.textContent = formatKeyForDisplay(settingsState.keybindings[el.dataset.action]);
      });

      keyEl.classList.add('recording');
      keyEl.textContent = 'Press keys...';

      function onKey(e) {
        e.preventDefault();
        e.stopPropagation();
        if (e.key === 'Escape') {
          keyEl.classList.remove('recording');
          keyEl.textContent = formatKeyForDisplay(settingsState.keybindings[action]);
          document.removeEventListener('keydown', onKey, true);
          return;
        }
        // Ignore bare modifier keys
        if (['Control', 'Meta', 'Alt', 'Shift'].includes(e.key)) return;

        // Build key string in CodeMirror format
        const parts = [];
        if (e.ctrlKey || e.metaKey) parts.push('Mod');
        if (e.shiftKey) parts.push('Shift');
        if (e.altKey) parts.push('Alt');

        let key = e.key;
        if (key === ' ') key = 'Space';
        else if (key.length === 1) key = key.toLowerCase();
        else if (key === 'Enter') key = 'Enter';
        parts.push(key);

        const keyStr = parts.join('-');
        settingsState.keybindings[action] = keyStr;
        keyEl.classList.remove('recording');
        keyEl.textContent = formatKeyForDisplay(keyStr);
        document.removeEventListener('keydown', onKey, true);
      }

      document.addEventListener('keydown', onKey, true);
    };

    row.appendChild(labelEl);
    row.appendChild(keyEl);
    list.appendChild(row);
  }

  // Reset to defaults
  document.getElementById('btn-reset-shortcuts').onclick = () => {
    settingsState.keybindings = { ...DEFAULT_KEYBINDINGS };
    list.querySelectorAll('.shortcut-key').forEach(el => {
      el.textContent = formatKeyForDisplay(DEFAULT_KEYBINDINGS[el.dataset.action]);
    });
  };

  // Done button
  document.getElementById('shortcuts-done').onclick = async () => {
    overlay.style.display = 'none';
    rebuildEditors();
    setHelpText();
    await saveSettingsToBackend();
  };
}

function rebuildEditors() {
  // Recreate editors to pick up new keybindings
  const contents = {};
  const cursors = {};
  for (const p of panes) {
    contents[p] = views[p].state.doc.toString();
    cursors[p] = views[p].state.selection.main.head;
    views[p].destroy();
  }
  views.todo = createEditor(document.getElementById('todo-editor'), contents.todo, 'todo');
  views.today = createEditor(document.getElementById('today-editor'), contents.today, 'today');
  views.done = createEditor(document.getElementById('done-editor'), contents.done, 'done');
  // Restore cursor positions
  for (const p of panes) {
    const pos = Math.min(cursors[p], views[p].state.doc.length);
    views[p].dispatch({ selection: { anchor: pos } });
  }
  views[focusedPane].focus();
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

// Convert "Mod-Shift-s" style key string to match against a KeyboardEvent
function matchesKey(keyStr, e) {
  if (!keyStr) return false;
  const parts = keyStr.split('-');
  const key = parts.pop();
  const needMod = parts.includes('Mod');
  const needShift = parts.includes('Shift');
  const hasMod = e.ctrlKey || e.metaKey;
  if (needMod !== hasMod) return false;
  if (needShift !== e.shiftKey) return false;
  // Compare key case-insensitively, handle special names
  const keyLower = key.toLowerCase();
  const eventKey = e.key === '\\' ? '\\' : e.key.toLowerCase();
  if (keyLower === 'enter') return e.key === 'Enter';
  return eventKey === keyLower;
}

function getKey(action) {
  return settingsState.keybindings[action] || DEFAULT_KEYBINDINGS[action];
}

// Global keyboard shortcuts
const globalActions = {
  cyclePane: () => cyclePane(1),
  cyclePaneBack: () => cyclePane(-1),
  sync: () => gitSync(false),
  toggleDonePane: () => toggleDonePane(),
  cycleTheme: () => cyclePalette(),
  openSettings: () => openSettings(false),
};

document.addEventListener('keydown', (e) => {
  // Check shift variants first (cyclePaneBack before cyclePane)
  const orderedActions = ['cyclePaneBack', 'sync', 'toggleDonePane', 'cyclePane', 'cycleTheme', 'openSettings'];
  for (const action of orderedActions) {
    if (matchesKey(getKey(action), e)) {
      e.preventDefault();
      globalActions[action]();
      return;
    }
  }
  if (e.key === 'Escape') {
    const overlay = document.getElementById('settings-overlay');
    if (overlay.style.display !== 'none') {
      overlay.style.display = 'none';
      applyPalette(palettes[currentPalette]);
    }
  }
});

function formatKeyForDisplay(keyStr) {
  if (!keyStr) return '';
  const mac = navigator.platform.includes('Mac');
  return keyStr
    .replace('Mod', mac ? '⌘' : 'Ctrl')
    .replace('Shift', mac ? '⇧' : 'Shift')
    .replace('Enter', '↵')
    .replace(/-/g, '+');
}

function setHelpText() {
  const kb = settingsState.keybindings;
  const items = [
    [kb.moveForward, 'move →'],
    [kb.sendBack, 'send ←'],
    [kb.skipToDone, 'skip→done'],
    [kb.cyclePane, 'pane'],
    [kb.save, 'save'],
    [kb.sync, 'sync'],
    [kb.openSettings, 'settings'],
  ];
  document.getElementById('status-help').textContent =
    items.map(([k, label]) => `${formatKeyForDisplay(k)}: ${label}`).join(' · ');
}

async function init() {
  // Load settings first
  const settings = await invoke('load_settings');
  settingsState = {
    storageMode: settings.storage_mode,
    localPath: settings.local_path,
    gitRepo: settings.git_repo,
    gitRepoName: settings.git_repo_name || 'tally-md-log',
    themeIndex: settings.theme_index,
    dateFormat: settings.date_format,
    layout: settings.layout || 'horizontal',
    paneSizes: settings.pane_sizes || [40, 30, 30],
    syncInterval: settings.sync_interval ?? 5,
    keybindings: { ...DEFAULT_KEYBINDINGS, ...settings.keybindings },
    setupDone: settings.setup_done,
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

  updateStorageIndicator();
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
