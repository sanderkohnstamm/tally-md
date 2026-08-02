// Chat: POST message, stream SSE deltas into the log; tool calls shown as thin notes.

const log = document.getElementById('chat-log');
const form = document.getElementById('chat-form');
const input = document.getElementById('chat-input');

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
  if (!message) return;
  input.value = '';
  bubble('user', message);
  let current = null;

  const res = await fetch('/api/chat', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ message }),
  });
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
        if (!current) current = bubble('assistant', '');
        current.textContent += ev.text;
        log.scrollTop = log.scrollHeight;
      } else if (ev.type === 'tool') {
        current = null; // next text starts a fresh bubble
        bubble('tool-note', '· ' + ev.name + ' ' + JSON.stringify(ev.args).slice(0, 80));
      }
    }
  }
});

log.scrollTop = log.scrollHeight;
