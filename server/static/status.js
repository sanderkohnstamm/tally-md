// Header orbs: server / obsidian sync / claude / calendars — LIVE, not a
// page-load snapshot. Re-polls every 5s (the server caches the probe for 30s,
// so this is cheap), on PWA resume (visibilitychange), and on network changes;
// a 5s timeout catches a dead Pi. When the backend is
// unreachable the server orb goes red and the rest revert to neutral gray —
// their state is unknown, not necessarily broken.
(() => {
  const orbs = document.querySelectorAll('.orb');

  async function refresh() {
    try {
      const ctrl = new AbortController();
      const timer = setTimeout(() => ctrl.abort(), 5000);
      const r = await fetch('/api/status', { signal: ctrl.signal, cache: 'no-store' });
      clearTimeout(timer);
      const s = await r.json();
      s.server = r.ok;
      orbs.forEach(o => {
        o.classList.remove('ok', 'err');
        o.classList.add(s[o.dataset.k] ? 'ok' : 'err');
      });
    } catch {
      orbs.forEach(o => {
        o.classList.remove('ok', 'err');
        if (o.dataset.k === 'server') o.classList.add('err');
      });
    }
  }

  refresh();
  setInterval(refresh, 5000);
  document.addEventListener('visibilitychange', () => { if (!document.hidden) refresh(); });
  window.addEventListener('online', refresh);
  window.addEventListener('offline', refresh);
})();
