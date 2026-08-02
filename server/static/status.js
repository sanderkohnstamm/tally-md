// Header orbs: server / obsidian sync / claude / calendars.
// Gray while loading, green ok, red not — server orb goes red if the fetch fails.
(async () => {
  const orbs = document.querySelectorAll('.orb');
  try {
    const r = await fetch('/api/status');
    const s = await r.json();
    s.server = r.ok;
    orbs.forEach(o => o.classList.add(s[o.dataset.k] ? 'ok' : 'err'));
  } catch {
    orbs.forEach(o => o.classList.add('err'));
  }
})();
