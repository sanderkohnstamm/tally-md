// Chat: POST message, stream SSE deltas into the log; tool calls shown as thin
// notes. Assistant bubbles get light markdown (bold/code/em) once complete.

const log = document.getElementById('chat-log');
const form = document.getElementById('chat-form');
const input = document.getElementById('chat-input');
const sendBtn = form.querySelector('button');

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

form.addEventListener('submit', async e => {
  e.preventDefault();
  const message = input.value.trim();
  if (!message || input.disabled) return;
  document.getElementById('chat-hint')?.remove();
  input.value = '';
  input.disabled = true;
  sendBtn.disabled = true;
  sendBtn.textContent = '…';
  bubble('user', message);
  let current = null;
  const turnBubbles = [];

  try {
    const res = await fetch('/api/chat', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ message }),
    });
    if (!res.ok) throw new Error(await res.text() || 'HTTP ' + res.status);
    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let buf = '';

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      buf += decoder.decode(value, { stream: true });
      const lines = buf.split('\n\n');
      buf = lines.pop();
      for (const line of lines) {
        if (!line.startsWith('data: ')) continue;
        const ev = JSON.parse(line.slice(6));
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
        }
      }
    }
    turnBubbles.forEach(b => { b.innerHTML = mdLite(b.textContent); });
  } catch (err) {
    bubble('tool-note', '✕ ' + err.message);
  } finally {
    input.disabled = false;
    sendBtn.disabled = false;
    sendBtn.textContent = '→';
    log.scrollTop = log.scrollHeight;
    input.focus();
  }
});

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
