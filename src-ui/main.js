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
    showMessage(result.message);
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
    showMessage(result.message);
  } else if (focusedPane === 'done') {
    // done -> todo (recover)
    const source = views.done.state.doc.toString();
    const target = views.todo.state.doc.toString();
    const cursorLine = getCursorLine(views.done);
    const result = await invoke('recover_item', { source, target, cursorLine, fromDone: true });

    const cursorPos = Math.min(views.done.state.selection.main.head, result.source.length);
    views.todo.dispatch({
      changes: { from: 0, to: views.todo.state.doc.length, insert: result.target },
    });
    views.done.dispatch({
      changes: { from: 0, to: views.done.state.doc.length, insert: result.source },
      selection: { anchor: cursorPos },
    });
    await save(true);
    showMessage(result.message);
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
  showMessage(`Theme: ${p.name}`);
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
  if ((e.ctrlKey || e.metaKey) && e.key === '\\') {
    e.preventDefault();
    cyclePane(1);
  }
  if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'B') {
    e.preventDefault();
    toggleDonePane();
  }
  if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
    e.preventDefault();
    cyclePalette();
  }
});

function setHelpText() {
  const mod = navigator.platform.includes('Mac') ? '⌘' : 'Ctrl';
  const shift = navigator.platform.includes('Mac') ? '⇧' : 'Shift';
  document.getElementById('status-help').textContent =
    `${mod}+Enter: move item → · ${mod}+\\: switch pane · ${mod}+${shift}+B: toggle done · ${mod}+S: save · ${mod}+B: bold · ${mod}+I: italic · ${mod}+K: theme · ${mod}+E: fold`;
}

async function init() {
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
  setHelpText();
  focusedPane = 'todo';
  updateFocus();
  views.todo.focus();
  showMessage('Ready');
}

init();
