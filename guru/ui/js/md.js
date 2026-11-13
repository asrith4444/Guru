// A small, safe Markdown renderer for lessons, briefings and debriefs.
// Builds DOM nodes directly (no innerHTML), so generated text can never inject
// HTML. Supports: headings, paragraphs, lists (incl. numbered), blockquotes,
// fenced code, tables, horizontal rules, **bold**, *italic*, `code`, [links](url).

import { h } from './dom.js';

let linkHandler = (url) => window.open(url, '_blank', 'noopener');
export function onLink(fn) {
  linkHandler = fn;
}

function inline(text) {
  const out = [];
  const rx = /(`[^`]+`)|(\*\*[^*]+\*\*)|(\*[^*\s][^*]*\*)|(\[[^\]]+\]\((https?:\/\/[^)\s]+)\))|(https?:\/\/[^\s)]+)/g;
  let last = 0;
  let m;
  while ((m = rx.exec(text))) {
    if (m.index > last) out.push(text.slice(last, m.index));
    const t = m[0];
    if (m[1]) out.push(h('code', null, t.slice(1, -1)));
    else if (m[2]) out.push(h('strong', null, inline(t.slice(2, -2))));
    else if (m[3]) out.push(h('em', null, inline(t.slice(1, -1))));
    else if (m[4]) {
      const label = t.slice(1, t.indexOf(']'));
      const url = m[5];
      out.push(h('a', { href: '#', title: url, onclick: (e) => (e.preventDefault(), linkHandler(url)) }, label));
    } else if (m[6]) {
      out.push(h('a', { href: '#', title: m[6], onclick: (e) => (e.preventDefault(), linkHandler(m[6])) }, m[6]));
    }
    last = m.index + t.length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

function splitRow(line) {
  return line.trim().replace(/^\|/, '').replace(/\|$/, '').split('|').map((c) => c.trim());
}

export function renderMarkdown(src) {
  const root = h('div', { class: 'md' });
  const lines = (src || '').replace(/\r/g, '').split('\n');
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (!line.trim()) {
      i++;
      continue;
    }
    const fence = line.match(/^```\s*([\w+-]*)/);
    if (fence) {
      const code = [];
      i++;
      while (i < lines.length && !/^```/.test(lines[i])) code.push(lines[i++]);
      i++;
      root.append(h('pre', { class: 'code', 'data-lang': fence[1] || null }, h('code', null, code.join('\n'))));
      continue;
    }
    const head = line.match(/^(#{1,4})\s+(.*)$/);
    if (head) {
      root.append(h(`h${Math.min(head[1].length + 1, 5)}`, null, inline(head[2])));
      i++;
      continue;
    }
    if (/^(-{3,}|\*{3,})\s*$/.test(line)) {
      root.append(h('hr'));
      i++;
      continue;
    }
    if (/^\s*\|.*\|\s*$/.test(line) && i + 1 < lines.length && /^\s*\|?[\s:-]+\|[\s|:-]*$/.test(lines[i + 1])) {
      const header = splitRow(line);
      i += 2;
      const rows = [];
      while (i < lines.length && /^\s*\|.*\|\s*$/.test(lines[i])) rows.push(splitRow(lines[i++]));
      root.append(
        h('div', { class: 'table-wrap' }, h('table', null, h('thead', null, h('tr', null, header.map((c) => h('th', null, inline(c))))), h('tbody', null, rows.map((r) => h('tr', null, r.map((c) => h('td', null, inline(c))))))))
      );
      continue;
    }
    if (/^>\s?/.test(line)) {
      const quote = [];
      while (i < lines.length && /^>\s?/.test(lines[i])) quote.push(lines[i++].replace(/^>\s?/, ''));
      const text = quote.join(' ');
      const pause = /^pause and predict/i.test(text);
      root.append(h('blockquote', { class: pause ? 'pause' : null }, inline(text)));
      continue;
    }
    const list = line.match(/^(\s*)([-*]|\d+[.)])\s+/);
    if (list) {
      const ordered = /\d/.test(list[2]);
      const el = h(ordered ? 'ol' : 'ul');
      while (i < lines.length) {
        const m = lines[i].match(/^(\s*)([-*]|\d+[.)])\s+(.*)$/);
        if (!m) {
          const cont = lines[i];
          if (cont.trim() && /^\s{2,}/.test(cont) && el.lastChild) {
            const t = cont.trim();
            if (t.startsWith('>')) {
              const q = t.replace(/^>\s?/, '');
              el.lastChild.append(h('blockquote', { class: /^pause and predict/i.test(q) ? 'pause' : null }, inline(q)));
            } else el.lastChild.append(' ', ...inline(t));
            i++;
            continue;
          }
          break;
        }
        el.append(h('li', null, inline(m[3])));
        i++;
      }
      root.append(el);
      continue;
    }
    const para = [];
    while (i < lines.length && lines[i].trim() && !/^(```|#{1,4}\s|>|\s*([-*]|\d+[.)])\s|\s*\|)/.test(lines[i])) para.push(lines[i++]);
    if (!para.length) para.push(lines[i++]);
    root.append(h('p', null, inline(para.join(' '))));
  }
  return root;
}
