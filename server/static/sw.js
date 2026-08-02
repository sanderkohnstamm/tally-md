// Shell-only cache: static assets stale-while-revalidate (serve cached, refresh
// in the background so the installed PWA picks up new CSS/JS one launch later),
// everything else network-only. No offline data — over Tailscale we're
// effectively always online, and iOS evicts PWA storage aggressively anyway.
const CACHE = 'tally-shell-v7';
const SHELL = ['/static/style.css', '/static/themes.js', '/static/chat.js', '/static/status.js'];

self.addEventListener('install', e => {
  e.waitUntil(caches.open(CACHE).then(c => c.addAll(SHELL)));
});

self.addEventListener('activate', e => {
  e.waitUntil(
    caches.keys().then(keys =>
      Promise.all(keys.filter(k => k !== CACHE).map(k => caches.delete(k)))
    )
  );
});

self.addEventListener('fetch', e => {
  const url = new URL(e.request.url);
  if (url.pathname.startsWith('/static/')) {
    e.respondWith(
      caches.open(CACHE).then(async cache => {
        const hit = await cache.match(e.request);
        const refresh = fetch(e.request)
          .then(res => { if (res.ok) cache.put(e.request, res.clone()); return res; })
          .catch(() => hit);
        return hit || refresh;
      })
    );
  }
});
