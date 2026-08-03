// Live updates: poll the current page and swap #content ONLY when the
// server-rendered markup actually changed — no flicker, but edits made by
// other writers (Obsidian, Claude Code, the agent, calendar sync, the 7:00
// briefing) appear without a manual reload.
//
// Chat is excluded (it streams its own turn live and holds input state);
// settings is excluded (form state).
(() => {
  const LIVE_PATHS = ['/', '/todos'];
  if (!LIVE_PATHS.includes(location.pathname)) return;
  const content = document.getElementById('content');
  let last = null; // first successful poll establishes the baseline
  let inflight = false;

  async function tick() {
    if (document.hidden || inflight) return;
    inflight = true;
    try {
      const r = await fetch(location.pathname, { cache: 'no-store' });
      if (r.ok) {
        const doc = new DOMParser().parseFromString(await r.text(), 'text/html');
        const fresh = doc.getElementById('content');
        if (fresh) {
          const html = fresh.innerHTML;
          if (last !== null && html !== last) {
            const st = content.scrollTop;
            content.innerHTML = html;
            content.scrollTop = st;
          }
          last = html;
        }
      }
    } catch { /* offline — orbs tell that story */ }
    inflight = false;
  }

  setInterval(tick, 3000);
  document.addEventListener('visibilitychange', () => { if (!document.hidden) tick(); });
})();
