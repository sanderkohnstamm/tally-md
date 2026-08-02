// Quick capture: POST text, prepend the returned card to the feed.
const form = document.getElementById('capture-form');
const input = document.getElementById('capture-input');
const btn = document.getElementById('capture-btn');
const feed = document.getElementById('capture-feed');

async function submit() {
  const text = input.value.trim();
  if (!text) return;
  btn.disabled = true;
  btn.textContent = 'filing…';
  try {
    const res = await fetch('/api/capture', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ text }),
    });
    if (!res.ok) throw new Error(await res.text() || res.status);
    feed.insertAdjacentHTML('afterbegin', await res.text());
    input.value = '';
  } catch (err) {
    alert('capture failed: ' + err.message);
  } finally {
    btn.disabled = false;
    btn.textContent = 'capture';
    input.focus();
  }
}

form.addEventListener('submit', e => { e.preventDefault(); submit(); });
input.addEventListener('keydown', e => {
  if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); submit(); }
});

// File upload → vault Files/ (PDFs, images, docs …)
const attachBtn = document.getElementById('attach-btn');
const attachInput = document.getElementById('attach-input');

attachBtn.addEventListener('click', () => attachInput.click());
attachInput.addEventListener('change', async () => {
  const file = attachInput.files[0];
  if (!file) return;
  attachBtn.disabled = true;
  attachBtn.textContent = '⋯';
  try {
    const body = new FormData();
    body.append('file', file);
    const res = await fetch('/api/upload', { method: 'POST', body });
    if (!res.ok) throw new Error(await res.text() || res.status);
    feed.insertAdjacentHTML('afterbegin', await res.text());
  } catch (err) {
    alert('upload failed: ' + err.message);
  } finally {
    attachBtn.disabled = false;
    attachBtn.textContent = '📎';
    attachInput.value = '';
  }
});
