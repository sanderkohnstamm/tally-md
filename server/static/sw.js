// Shell cache, stale-while-revalidate: serve cached, refresh in the background.
// skipWaiting + clients.claim so a new worker takes over immediately — without
// them the very first (cache-first) worker kept serving stale CSS forever.
// Asset URLs carry ?v= in the templates, which also busts any pre-claim cache.
const CACHE = 'tally-shell-v10';
const SHELL = [
  '/static/style.css?v=10',
  '/static/themes.js?v=10',
  '/static/chat.js?v=10',
  '/static/status.js?v=10',
];

self.addEventListener('install', e => {
  self.skipWaiting();
  e.waitUntil(caches.open(CACHE).then(c => c.addAll(SHELL)));
});

self.addEventListener('activate', e => {
  e.waitUntil(
    caches.keys()
      .then(keys => Promise.all(keys.filter(k => k !== CACHE).map(k => caches.delete(k))))
      .then(() => self.clients.claim())
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
