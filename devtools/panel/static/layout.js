(function (root) {
  'use strict';
  var KEY = 'modui.order';

  function normalize(saved, keys) {
    var out = [], seen = {};
    (Array.isArray(saved) ? saved : []).forEach(function (k) {
      if (keys.indexOf(k) >= 0 && !seen[k]) { out.push(k); seen[k] = 1; }
    });
    keys.forEach(function (k) { if (!seen[k]) { out.push(k); seen[k] = 1; } });
    return out;
  }

  function place(saved, domKeys) {
    var order = normalize(saved, domKeys).filter(function (k) { return (saved || []).indexOf(k) >= 0 && domKeys.indexOf(k) >= 0; });
    domKeys.forEach(function (k, n) {
      if (order.indexOf(k) >= 0) return;
      var prev = n > 0 ? order.indexOf(domKeys[n - 1]) : -1;
      order.splice(prev + 1, 0, k);
    });
    return order;
  }

  function moveTo(order, key, index) {
    var at = order.indexOf(key);
    if (at < 0) return order.slice();
    var rest = order.filter(function (k) { return k !== key; });
    index = Math.max(0, Math.min(rest.length, index));
    rest.splice(index, 0, key);
    return rest;
  }

  function targetIndex(mids, y) {
    var n = 0;
    mids.forEach(function (m) { if (m < y) n++; });
    return n;
  }

  function load() {
    try { var v = localStorage.getItem(KEY); return v ? JSON.parse(v) : []; } catch (e) { return []; }
  }
  function save(order) { try { localStorage.setItem(KEY, JSON.stringify(order)); } catch (e) {} }
  function forget() { try { localStorage.removeItem(KEY); } catch (e) {} }

  var api = { normalize: normalize, place: place, moveTo: moveTo, targetIndex: targetIndex, KEY: KEY, load: load, save: save, forget: forget };

  if (typeof document === 'undefined') { root.ModLayout = api; if (typeof module !== 'undefined') module.exports = api; return; }

  var box = null, order = [];

  function cards() { return Array.prototype.slice.call(box.querySelectorAll('[data-card]')); }
  function keys() { return cards().map(function (c) { return c.dataset.card; }); }
  function apply() {
    var by = {};
    cards().forEach(function (c) { by[c.dataset.card] = c; });
    order = place(order, keys());
    order.forEach(function (k, n) { if (by[k]) by[k].style.order = String(n + 1); });
  }

  function grip(card) {
    var sum = card.querySelector(':scope > summary');
    if (!sum || sum.querySelector('.grip')) return;
    var g = document.createElement('span');
    g.className = 'grip'; g.textContent = '⠇'; g.tabIndex = 0; g.title = 'Drag to move this card';
    g.setAttribute('role', 'button'); g.setAttribute('aria-label', 'Move card');
    g.addEventListener('click', function (e) { e.preventDefault(); e.stopPropagation(); });
    g.addEventListener('keydown', function (e) {
      if (e.key !== 'ArrowUp' && e.key !== 'ArrowDown') return;
      e.preventDefault();
      var k = card.dataset.card, at = order.indexOf(k);
      order = moveTo(order, k, at + (e.key === 'ArrowUp' ? -1 : 1));
      apply(); save(order); g.focus();
    });
    g.addEventListener('pointerdown', function (e) {
      if (e.button !== undefined && e.button > 0) return;
      e.preventDefault(); e.stopPropagation();
      var k = card.dataset.card, id = e.pointerId;
      try { g.setPointerCapture(id); } catch (x) {}
      card.classList.add('dragging');
      var move = function (ev) {
        var y = ev.clientY, others = cards().filter(function (c) { return c !== card; });
        others.sort(function (a, b) { return order.indexOf(a.dataset.card) - order.indexOf(b.dataset.card); });
        var mids = others.map(function (c) { var r = c.getBoundingClientRect(); return r.top + r.height / 2; });
        var next = moveTo(order, k, targetIndex(mids, y));
        if (next.join('|') !== order.join('|')) { order = next; apply(); }
        if (y < 70) window.scrollBy(0, -14); else if (y > window.innerHeight - 70) window.scrollBy(0, 14);
      };
      var done = function () {
        g.removeEventListener('pointermove', move); g.removeEventListener('pointerup', done); g.removeEventListener('pointercancel', done);
        card.classList.remove('dragging'); save(order);
      };
      g.addEventListener('pointermove', move); g.addEventListener('pointerup', done); g.addEventListener('pointercancel', done);
    });
    sum.appendChild(g);
  }

  function refresh() {
    if (!box) return;
    cards().forEach(grip);
    apply();
  }
  function init(el) { box = el; order = load(); refresh(); }
  function reset() { forget(); order = []; if (box) { cards().forEach(function (c) { c.style.order = ''; }); order = keys(); } }

  api.init = init; api.refresh = refresh; api.reset = reset;
  root.ModLayout = api;
})(typeof window !== 'undefined' ? window : globalThis);
