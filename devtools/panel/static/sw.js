var CACHE = 'modui-shell-v1';
var SHELL = ['/', '/layout.js', '/manifest.webmanifest', '/icon-192.png', '/icon-512.png'];

function isShell(url) {
  return url.origin === self.location.origin && SHELL.indexOf(url.pathname) >= 0;
}

self.addEventListener('install', function (e) {
  e.waitUntil(caches.open(CACHE).then(function (c) {
    return Promise.all(SHELL.map(function (p) { return c.add(new Request(p, { cache: 'reload' })).catch(function () {}); }));
  }));
  self.skipWaiting();
});

self.addEventListener('activate', function (e) {
  e.waitUntil(caches.keys().then(function (ks) {
    return Promise.all(ks.filter(function (k) { return k !== CACHE; }).map(function (k) { return caches.delete(k); }));
  }).then(function () { return self.clients.claim(); }));
});

self.addEventListener('fetch', function (e) {
  var req = e.request, url = new URL(req.url);
  if (req.method !== 'GET' || !isShell(url)) return;
  e.respondWith(fetch(req).then(function (r) {
    if (r && r.ok) { var copy = r.clone(); caches.open(CACHE).then(function (c) { c.put(url.pathname, copy); }); }
    return r;
  }).catch(function () { return caches.match(url.pathname); }));
});
