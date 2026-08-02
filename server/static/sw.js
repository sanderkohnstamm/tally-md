// Shell-only cache: static assets cache-first, everything else network-only.
// No offline data — over Tailscale we're effectively always online, and iOS
// evicts PWA storage aggressively anyway.
const CACHE = 'tally-shell-v2';
const SHELL = ['/static/style.css', '/static/themes.js', '/static/chat.js', '/static/capture.js', '/static/status.js'];

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
    e.respondWith(caches.match(e.request).then(hit => hit || fetch(e.request)));
  }
});
