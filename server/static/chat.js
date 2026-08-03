// Chat: POST starts a server-side turn that runs to completion even if this
// page goes away; we attach to its SSE event stream to render, and re-attach
// (resuming from the last seen event) after backgrounding or network blips.

const log = document.getElementById('chat-log');
const form = document.getElementById('chat-form');
const input = document.getElementById('chat-input');
const sendBtn = document.getElementById('chat-send');

let currentTurn = null;   // turn id still rendering, null when idle
let eventIndex = 0;       // events consumed so far (resume point)
let streaming = false;
let current = null;       // assistant bubble receiving deltas
let turnBubbles = [];

// Mirrors the server-side mdlite filter used for history.
function mdLite(text) {
  const esc = text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  return esc
    .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/(^|[^\w*])\*([^*\n]+)\*(?![\w*])/g, '$1<em>$2</em>');
}

function bubble(cls, text) {
  const div = document.createElement('div');
  div.className = 'msg ' + cls;
  div.textContent = text;
  log.appendChild(div);
  log.scrollTop = log.scrollHeight;
  return div;
}

function setBusy(b) {
  input.disabled = b;
  sendBtn.disabled = b;
  sendBtn.textContent = b ? '…' : '→';
}

function finishTurn() {
  turnBubbles.forEach(b => { b.innerHTML = mdLite(b.textContent); });
  currentTurn = null;
  current = null;
  eventIndex = 0;
  turnBubbles = [];
  setBusy(false);
  log.scrollTop = log.scrollHeight;
}

function handleEvent(ev) {
  eventIndex++;
  if (ev.type === 'text_delta') {
    if (!current) { current = bubble('assistant', ''); turnBubbles.push(current); }
    current.textContent += ev.text;
    log.scrollTop = log.scrollHeight;
  } else if (ev.type === 'tool') {
    current = null; // next text starts a fresh bubble
    if (ev.name !== 'ToolSearch') { // internal plumbing, not interesting
      bubble('tool-note', '· ' + ev.name.replace(/^mcp__tally__/, '') + ' '
        + JSON.stringify(ev.args).slice(0, 80));
    }
  } else if (ev.type === 'error') {
    bubble('tool-note', '✕ ' + ev.message);
    finishTurn();
  } else if (ev.type === 'done') {
    finishTurn();
  }
}

async function attachStream(turn) {
  if (streaming) return;
  streaming = true;
  setBusy(true);
  try {
    const res = await fetch(`/api/chat/stream?turn=${turn}&from=${eventIndex}`);
    if (!res.ok) throw new Error('HTTP ' + res.status);
    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let buf = '';
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      buf += decoder.decode(value, { stream: true });
      const parts = buf.split('\n\n');
      buf = parts.pop();
      for (const part of parts) {
        if (part.startsWith('data: ')) handleEvent(JSON.parse(part.slice(6)));
      }
    }
  } catch {
    // connection dropped — the turn keeps running server-side; retry below
  }
  streaming = false;
  if (currentTurn === turn) setTimeout(() => { if (currentTurn === turn) attachStream(turn); }, 2000);
}

form.addEventListener('submit', async e => {
  e.preventDefault();
  const message = input.value.trim();
  if (!message || currentTurn) return;
  document.getElementById('chat-hint')?.remove();
  try {
    const res = await fetch('/api/chat', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ message }),
    });
    const out = await res.json();
    if (out.error === 'busy') { bubble('tool-note', '· still thinking — one moment'); return; }
    if (out.error || !out.turn) throw new Error(out.error || 'no turn id');
    input.value = '';
    bubble('user', message);
    currentTurn = out.turn;
    eventIndex = 0;
    attachStream(currentTurn);
  } catch (err) {
    bubble('tool-note', '✕ ' + err.message);
  }
});

// Re-attach fast when the PWA comes back to the foreground mid-turn.
document.addEventListener('visibilitychange', () => {
  if (!document.hidden && currentTurn && !streaming) attachStream(currentTurn);
});

// A turn was already running when this page loaded — pick it up.
if (log.dataset.turn) {
  currentTurn = parseInt(log.dataset.turn, 10);
  attachStream(currentTurn);
}

// File upload → vault Files/. The 📎 is a <label> for the file input —
// native activation, since iOS ignores JS .click() on hidden file inputs.
const attachBtn = document.getElementById('attach-btn');
const attachInput = document.getElementById('attach-input');

attachInput.addEventListener('change', async () => {
  const file = attachInput.files[0];
  if (!file) return;
  attachBtn.classList.add('busy');
  attachBtn.textContent = '⋯';
  try {
    const body = new FormData();
    body.append('file', file);
    const res = await fetch('/api/upload', { method: 'POST', body });
    if (!res.ok) throw new Error(await res.text() || 'HTTP ' + res.status);
    const info = await res.json();
    bubble('tool-note', '📎 saved to ' + info.rel + ' — ask about it here');
  } catch (err) {
    bubble('tool-note', '✕ upload failed: ' + err.message);
  } finally {
    attachBtn.classList.remove('busy');
    attachBtn.textContent = '📎';
    attachInput.value = '';
  }
});

log.scrollTop = log.scrollHeight;
