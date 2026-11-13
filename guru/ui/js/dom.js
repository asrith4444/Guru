// Tiny DOM helpers. Content is always inserted as text nodes, never as HTML,
// so AI-generated course material cannot inject markup or scripts.

export function h(tag, attrs, ...children) {
  const el = document.createElement(tag);
  if (attrs) {
    for (const [k, v] of Object.entries(attrs)) {
      if (v === null || v === undefined || v === false) continue;
      if (k === 'class') el.className = v;
      else if (k === 'style' && typeof v === 'object') for (const [sk, sv] of Object.entries(v)) sk.startsWith('--') ? el.style.setProperty(sk, sv) : (el.style[sk] = sv);
      else if (k.startsWith('on') && typeof v === 'function') el.addEventListener(k.slice(2).toLowerCase(), v);
      else if (k === 'dataset') Object.assign(el.dataset, v);
      else if (k === 'value') el.value = v;
      else if (k === 'checked') el.checked = !!v;
      else el.setAttribute(k, v === true ? '' : String(v));
    }
  }
  append(el, children);
  return el;
}

export function append(el, ...children) {
  for (const c of children.flat(Infinity)) {
    if (c === null || c === undefined || c === false) continue;
    el.append(c instanceof Node ? c : document.createTextNode(String(c)));
  }
  return el;
}

export function clear(el) {
  while (el.firstChild) el.removeChild(el.firstChild);
  return el;
}

const SVG_NS = 'http://www.w3.org/2000/svg';
export function s(tag, attrs, ...children) {
  const el = document.createElementNS(SVG_NS, tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v === null || v === undefined) continue;
    if (k.startsWith('on') && typeof v === 'function') el.addEventListener(k.slice(2).toLowerCase(), v);
    else el.setAttribute(k, String(v));
  }
  for (const c of children.flat(Infinity)) {
    if (c === null || c === undefined) continue;
    el.append(c instanceof Node ? c : document.createTextNode(String(c)));
  }
  return el;
}

let toastHost;
export function toast(message, kind = 'info') {
  if (!toastHost) {
    toastHost = h('div', { class: 'toasts', role: 'status', 'aria-live': 'polite' });
    document.body.append(toastHost);
  }
  const t = h('div', { class: `toast ${kind}` }, message);
  while (toastHost.children.length >= 3) toastHost.firstChild.remove();
  toastHost.append(t);
  setTimeout(() => t.classList.add('out'), 3600);
  setTimeout(() => t.remove(), 4000);
}

export function modal(title, body, actions = []) {
  // A dialog needs full attention: never cover its buttons with notifications.
  toastHost?.replaceChildren();
  const close = () => back.remove();
  const back = h(
    'div',
    { class: 'modal-back', onclick: (e) => e.target === back && close() },
    h(
      'div',
      { class: 'modal', role: 'dialog', 'aria-modal': 'true', 'aria-label': title },
      h('h2', null, title),
      h('div', { class: 'modal-body' }, body),
      h(
        'div',
        { class: 'modal-actions' },
        actions.map((a) =>
          h('button', { class: a.primary ? 'btn primary' : 'btn', onclick: () => { if (a.onClick?.() !== false) close(); } }, a.label)
        )
      )
    )
  );
  document.body.append(back);
  back.querySelector('.modal-actions button:last-child')?.focus();
  return close;
}

export function fmtMinutes(seconds) {
  const m = Math.round(seconds / 60);
  return m >= 60 ? `${Math.floor(m / 60)} h ${m % 60} min` : `${m} min`;
}

export function fmtHours(h) {
  return Number.isInteger(h) ? `${h} h` : `${h.toFixed(1)} h`;
}

export function pct(x) {
  return `${Math.round((x || 0) * 100)}%`;
}

export function bar(value, cls = '') {
  return h('div', { class: `bar ${cls}` }, h('span', { style: { width: pct(Math.max(0, Math.min(1, value))) } }));
}
