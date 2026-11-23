(() => {
  const KEY = new URLSearchParams(location.search).get('key') || '';
  const q = KEY ? '?key=' + encodeURIComponent(KEY) : '';
  const theme = new URLSearchParams(location.search).get('theme');
  if (theme === 'dark' || theme === 'light') document.documentElement.dataset.theme = theme;
  const framed = window.parent !== window;
  if (framed) document.documentElement.classList.add('framed');
  const hdr = extra => Object.assign({ 'X-Placer': '1' }, KEY ? { 'X-Panel-Key': KEY } : {}, extra || {});
  const $ = id => document.getElementById(id);
  const el = (tag, cls, text) => { const e = document.createElement(tag); if (cls) e.className = cls; if (text != null) e.textContent = text; return e; };
  async function get(path) { return (await fetch(path + q, { headers: hdr(), cache: 'no-store' })).json(); }
  let busy = false;
  async function post(path, body, msgEl) {
    if (busy) return null;
    busy = true; document.body.classList.add('busy');
    if (msgEl) { msgEl.textContent = 'working...'; msgEl.classList.remove('bad'); }
    let r = null;
    try {
      r = await (await fetch(path + q, { method: 'POST', headers: hdr({ 'Content-Type': 'application/json' }), body: JSON.stringify(body || {}) })).json();
      if (msgEl) { msgEl.textContent = r.message || r.error || ''; msgEl.classList.toggle('bad', r.ok === false); }
    } catch (e) { if (msgEl) { msgEl.textContent = 'no answer: ' + e; msgEl.classList.add('bad'); } }
    busy = false; document.body.classList.remove('busy');
    return r;
  }
  const ARM_MS = 6000;
  function label(b, main, sub, key) {
    if (b._armed && b._armKey !== key) disarm(b);
    b._key = key;
    if (b._armed) return;
    b.textContent = main;
    if (sub) b.appendChild(el('small', null, sub));
  }
  function disarm(b) {
    clearTimeout(b._armT); clearInterval(b._armI); b._armed = false; b.classList.remove('armed');
    if (b._main != null) label(b, b._main, b._sub, b._key);
  }
  function confirmTap(b, what, fn) {
    b.addEventListener('click', () => {
      if (b.disabled) return;
      if (b._armed) { disarm(b); fn(); return; }
      b._main = b.firstChild ? b.firstChild.textContent : ''; b._sub = b.querySelector('small') ? b.querySelector('small').textContent : '';
      b._armed = true; b._armKey = b._key; b.classList.add('armed');
      const t0 = Date.now(), w = typeof what === 'function' ? what() : what;
      const draw = () => { const left = Math.max(0, Math.ceil((ARM_MS - (Date.now() - t0)) / 1000)); b.textContent = 'Tap again to confirm (' + left + ')'; b.appendChild(el('small', null, w)); };
      draw();
      b._armI = setInterval(draw, 250);
      b._armT = setTimeout(() => disarm(b), ARM_MS);
    });
  }
  function frame(key, min) {
    if (!framed) return;
    const tell = () => window.parent.postMessage({ [key]: document.documentElement.scrollHeight }, '*');
    new ResizeObserver(tell).observe(document.body);
    tell();
    window.addEventListener('message', e => { if (e.data === 'heightPing') tell(); });
  }
  function back(a) { if (a) a.href = location.protocol + '//' + location.hostname + ':' + (new URLSearchParams(location.search).get('panel') || 8765) + '/' + q; }
  function store(k, v) {
    try { if (v === undefined) return localStorage.getItem('placer.' + k); localStorage.setItem('placer.' + k, v); } catch (e) { return null; }
    return v;
  }
  function loop(fn, ms) {
    const tick = async () => { if (!busy) { try { await fn(); } catch (e) { const w = $('dot'); if (w) w.className = 'dot off'; } } setTimeout(tick, ms); };
    tick();
  }
  window.UI = { KEY, q, hdr, $, el, get, post, label, confirmTap, frame, back, store, loop, isBusy: () => busy };
})();
