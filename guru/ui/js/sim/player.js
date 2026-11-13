// The simulation player: renders a simulation spec as a playable world.
//
//   ┌ top bar: title · timer · objectives progress · hint · exit ───────────────┐
//   │ side: briefing, objectives (answer / do), hints │ world map (click nodes) │
//   │                                                 │ tabs: terminals · logs · │
//   │                                                 │ simulated tools          │
//   └──────────────────────────────────────────────────────────────────────────┘

import { lineChart } from '../charts.js';
import { append, clear, fmtMinutes, h, modal, s, toast } from '../dom.js';
import { renderMarkdown } from '../md.js';
import { SimEngine } from './engine.js';

const KIND_GLYPH = {
  server: 'SRV', database: 'DB', load_balancer: 'LB', container: 'CTR', service: 'SVC', client: 'USR',
  cloud: 'NET', queue: 'MQ', firewall: 'FW', app: 'APP', person: 'P',
};

export function renderSimPlayer(root, spec, { onExit, onAskAI, aiAvailable }) {
  const engine = new SimEngine(spec);
  let finished = false;
  const tabs = [];
  let active = null;

  // ---------- top bar ----------
  const timer = h('span', { class: 'sim-timer' }, '0 min');
  const progressLabel = h('span', { class: 'sim-progress-label' });
  const progressBar = h('div', { class: 'bar small' }, h('span'));
  const hintBtn = h('button', { class: 'btn ghost', onclick: () => revealHint() }, 'Hint');
  const top = h(
    'header',
    { class: 'sim-top' },
    h('button', { class: 'btn ghost', onclick: () => exit(false), title: 'Leave the simulation' }, '← Exit'),
    h('div', { class: 'sim-title' }, h('span', { class: `chip kind-${spec.kind}` }, spec.kind || 'simulation'), h('h1', null, spec.title), h('span', { class: 'muted' }, spec.tool)),
    h('div', { class: 'sim-stats' }, timer, h('div', { class: 'sim-progress' }, progressLabel, progressBar), hintBtn)
  );
  const tick = setInterval(() => (timer.textContent = fmtMinutes((Date.now() - engine.started) / 1000)), 15000);

  // ---------- side panel ----------
  const objectivesEl = h('div', { class: 'objectives' });
  const hintsEl = h('div', { class: 'hints' });
  const side = h(
    'aside',
    { class: 'sim-side' },
    h('details', { class: 'briefing', open: true }, h('summary', null, 'Briefing'), renderMarkdown(spec.briefing)),
    h('h3', null, 'Objectives'),
    objectivesEl,
    hintsEl
  );

  // ---------- world map ----------
  const worldEl = h('div', { class: 'world' });
  const tabBar = h('div', { class: 'tabs', role: 'tablist' });
  const panelHost = h('div', { class: 'tab-panels' });
  const main = h('section', { class: 'sim-main' }, worldEl, tabBar, panelHost);

  append(clear(root), h('div', { class: 'sim-player' }, top, h('div', { class: 'sim-body' }, side, main)));

  function addTab(id, label, build, nodeId) {
    const panel = h('div', { class: 'tab-panel', hidden: true, role: 'tabpanel' });
    const btn = h('button', { class: 'tab', role: 'tab', onclick: () => select(id) }, label);
    const tab = { id, btn, panel, build, built: false, nodeId };
    tabs.push(tab);
    tabBar.append(btn);
    panelHost.append(panel);
    return tab;
  }
  function select(id) {
    for (const t of tabs) {
      const on = t.id === id;
      t.btn.classList.toggle('active', on);
      t.btn.setAttribute('aria-selected', on);
      t.panel.hidden = !on;
      if (on && !t.built) {
        t.build(t.panel);
        t.built = true;
      }
      if (on) {
        active = t;
        t.panel.querySelector('input.term-input')?.focus();
      }
    }
    renderWorld();
  }

  for (const host of spec.hosts || []) addTab(`term:${host.node_id}`, `Terminal · ${host.hostname}`, (p) => buildTerminal(p, host), host.node_id);
  if ((spec.logs || []).length) addTab('logs', 'Logs', (p) => buildLogs(p));
  for (const app of spec.apps || []) addTab(`app:${app.id}`, app.name, (p) => buildApp(p, app), app.node_id);

  function renderWorld() {
    const nodes = spec.world?.nodes || [];
    if (!nodes.length) {
      worldEl.hidden = true;
      return;
    }
    const W = 1000;
    const H = 260;
    const pos = (n) => ({ x: 70 + (n.x / 100) * (W - 140), y: 34 + (n.y / 100) * (H - 68) });
    const byId = Object.fromEntries(nodes.map((n) => [n.id, n]));
    const links = (spec.world.links || []).filter((l) => byId[l.from] && byId[l.to]).map((l) => {
      const a = pos(byId[l.from]);
      const b = pos(byId[l.to]);
      return s('g', { class: 'link' }, s('line', { x1: a.x, y1: a.y, x2: b.x, y2: b.y }), l.label ? s('text', { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 - 6, 'text-anchor': 'middle' }, l.label) : null);
    });
    const nodeEls = nodes.map((n) => {
      const p = pos(n);
      const status = engine.nodeStatus[n.id] || n.status;
      const tab = tabs.find((t) => t.nodeId === n.id);
      const selected = active && active.nodeId === n.id;
      return s(
        'g',
        { class: `node status-${status}${tab ? ' clickable' : ''}${selected ? ' selected' : ''}`, transform: `translate(${p.x},${p.y})`, onclick: () => tab && select(tab.id), tabindex: tab ? 0 : null, onkeydown: (e) => e.key === 'Enter' && tab && select(tab.id) },
        s('title', null, `${n.label} — ${status}${n.detail ? '\n' + n.detail : ''}`),
        s('rect', { x: -62, y: -24, width: 124, height: 48, rx: 10 }),
        s('rect', { class: 'status-bar', x: -62, y: -24, width: 6, height: 48, rx: 3 }),
        s('text', { class: 'glyph', x: -48, y: 4 }, KIND_GLYPH[n.kind] || '•'),
        s('text', { class: 'label', x: -14, y: -3 }, n.label.length > 14 ? n.label.slice(0, 13) + '…' : n.label),
        s('text', { class: 'sub', x: -14, y: 13 }, status)
      );
    });
    clear(worldEl).append(s('svg', { viewBox: `0 0 ${W} ${H}`, class: 'world-svg', role: 'img', 'aria-label': 'System map' }, links, nodeEls));
  }

  // ---------- objectives ----------
  function renderObjectives() {
    clear(objectivesEl);
    for (const o of spec.objectives || []) {
      const done = engine.objectiveDone(o);
      const revealed = !done && engine.revealed.has(o.id);
      const item = h(
        'div',
        { class: `objective ${done ? 'done' : ''} ${revealed ? 'revealed' : ''}` },
        h('div', { class: 'obj-head' }, h('span', { class: 'check', 'aria-hidden': 'true' }, done ? '✓' : revealed ? '👁' : o.kind === 'state' ? '○' : '?'), h('span', null, o.text))
      );
      if (o.kind === 'answer' && !done && !revealed) item.append(answerForm(o));
      if ((done || revealed) && o.explanation) item.append(h('p', { class: 'explain' }, revealed ? h('b', null, 'Shown: ') : null, o.explanation));
      if (o.kind === 'state' && !done && !revealed) item.append(h('p', { class: 'muted small' }, 'Do it in the terminal or tools.'));
      // No dead ends: after the hints, or a few wrong answers, Guru can show the way.
      const stuck = engine.hintsUsed >= (spec.hints || []).length || (engine.answers[o.id]?.attempts || 0) >= 2;
      if (!done && !revealed && stuck)
        item.append(
          h(
            'button',
            {
              class: 'btn tiny ghost',
              onclick: () => {
                engine.reveal(o.id);
              },
              title: 'Shows the answer and how to do it. No credit for this objective.',
            },
            'Show me how'
          )
        );
      objectivesEl.append(item);
    }
    const p = engine.progress();
    progressLabel.textContent = `${p.done}/${p.total} objectives${p.closed > p.done ? ` (+${p.closed - p.done} shown)` : ''}`;
    progressBar.firstChild.style.width = `${p.total ? (p.closed / p.total) * 100 : 0}%`;
    hintBtn.disabled = engine.hintsUsed >= (spec.hints || []).length;
    if (p.complete && !finished) {
      finished = true;
      setTimeout(showDebrief, 400);
    }
  }

  function answerForm(o) {
    const feedback = h('p', { class: 'feedback' });
    let control;
    let read;
    if (o.options?.length) {
      const multi = (o.correct || []).length > 1;
      const name = `obj-${o.id}`;
      control = h('div', { class: 'options' }, o.options.map((opt, i) => h('label', { class: 'option' }, h('input', { type: multi ? 'checkbox' : 'radio', name, value: i }), h('span', null, opt))));
      read = () => [...control.querySelectorAll('input:checked')].map((x) => Number(x.value));
    } else {
      control = h('input', { type: 'text', class: 'input', placeholder: 'Your answer', onkeydown: (e) => e.key === 'Enter' && submit() });
      read = () => control.value;
    }
    const submit = () => {
      const r = engine.answer(o.id, read());
      if (!r.correct) {
        if ((engine.answers[o.id]?.attempts || 0) >= 2) return renderObjectives();
        feedback.textContent = 'Not quite. Look for more evidence.';
        feedback.className = 'feedback bad';
      }
    };
    return h('div', { class: 'answer' }, h('p', { class: 'question' }, o.question), control, h('button', { class: 'btn small', onclick: submit }, 'Check'), feedback);
  }

  function revealHint() {
    const hint = engine.useHint();
    if (!hint) return;
    hintsEl.append(h('div', { class: 'hint' }, h('b', null, `Hint ${engine.hintsUsed}`), ' ', hint));
    renderObjectives();
  }

  // ---------- terminal ----------
  function buildTerminal(panel, host) {
    const nodeId = host.node_id;
    const out = h('div', { class: 'term-out' });
    const promptEl = h('span', { class: 'term-prompt' });
    const input = h('input', { class: 'term-input', spellcheck: 'false', autocomplete: 'off', 'aria-label': `Command for ${host.hostname}` });
    const term = h('div', { class: 'term', onclick: () => input.focus() }, out, h('div', { class: 'term-line' }, promptEl, input));
    panel.append(term);
    const setPrompt = () => (promptEl.textContent = engine.hosts[nodeId].prompt + ' ');
    const print = (text, cls) => {
      if (text === undefined || text === null || text === '') return;
      out.append(h('pre', { class: cls || null }, text));
      term.scrollTop = term.scrollHeight;
    };
    if (host.motd) print(host.motd, 'dim');
    print("Type 'help' for the commands this simulation understands.", 'dim');
    setPrompt();
    let hist = [];
    let hi = 0;
    input.addEventListener('keydown', async (e) => {
      if (e.key === 'Enter') {
        const line = input.value;
        input.value = '';
        print(engine.hosts[nodeId].prompt + ' ' + line, 'cmd');
        if (line.trim()) hist.push(line);
        hi = hist.length;
        const r = engine.exec(nodeId, line);
        if (r.clear) clear(out);
        print(r.output);
        if (r.notFound && aiAvailable) {
          const btn = h('button', { class: 'btn tiny ai' }, '✦ Ask AI to simulate this command');
          btn.onclick = async () => {
            btn.disabled = true;
            btn.textContent = 'Simulating…';
            try {
              const text = await onAskAI(host.hostname, engine.hosts[nodeId].history.slice(-10).join('\n'), line);
              btn.replaceWith(h('pre', { class: 'ai-out' }, text));
            } catch (err) {
              btn.textContent = String(err);
            }
          };
          out.append(btn);
        }
        setPrompt();
        term.scrollTop = term.scrollHeight;
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        if (hi > 0) input.value = hist[--hi];
      } else if (e.key === 'ArrowDown') {
        e.preventDefault();
        hi = Math.min(hist.length, hi + 1);
        input.value = hist[hi] || '';
      } else if (e.key === 'Tab') {
        e.preventDefault();
        input.value = engine.complete(nodeId, input.value);
      } else if (e.key === 'l' && e.ctrlKey) {
        e.preventDefault();
        clear(out);
      }
    });
  }

  // ---------- logs ----------
  function logView(streamIds, { compact = false } = {}) {
    const streams = (spec.logs || []).filter((l) => streamIds.includes(l.id));
    const sel = h('select', { class: 'input', 'aria-label': 'Log stream' }, streams.map((l) => h('option', { value: l.id }, l.name)));
    const search = h('input', { class: 'input', placeholder: 'Search (text or /regex/)', 'aria-label': 'Search logs' });
    const levels = ['debug', 'info', 'warn', 'error', 'critical'];
    const on = new Set(levels);
    const chips = h('div', { class: 'level-chips' }, levels.map((lv) => h('button', { class: `chip lv-${lv} on`, onclick: (e) => { on.has(lv) ? on.delete(lv) : on.add(lv); e.target.classList.toggle('on'); draw(); } }, lv)));
    const count = h('span', { class: 'muted small' });
    const body = h('div', { class: 'log-lines' });
    const draw = () => {
      const stream = engine.viewStream(sel.value);
      if (!stream) return;
      const q = search.value.trim();
      let rx = null;
      if (q) {
        const m = q.match(/^\/(.+)\/([a-z]*)$/);
        try {
          rx = m ? new RegExp(m[1], m[2] || 'i') : new RegExp(q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'i');
        } catch {
          rx = null;
        }
      }
      const rows = stream.lines.filter((l) => on.has(l.level) && (!rx || rx.test(l.message) || rx.test(l.ts)));
      count.textContent = `${rows.length} of ${stream.lines.length} lines`;
      clear(body).append(...rows.map((l) => h('div', { class: `log-row lv-${l.level}` }, h('span', { class: 'ts' }, l.ts), h('span', { class: 'lv' }, l.level), h('span', { class: 'msg' }, highlight(l.message, rx)))));
    };
    sel.onchange = draw;
    search.oninput = draw;
    draw();
    return h('div', { class: `logs ${compact ? 'compact' : ''}` }, h('div', { class: 'log-tools' }, streams.length > 1 ? sel : h('b', null, streams[0]?.name || ''), search, chips, count), body);
  }
  function buildLogs(panel) {
    panel.append(logView((spec.logs || []).map((l) => l.id)));
  }

  // ---------- simulated tools ----------
  function buildApp(panel, app) {
    const content = h('div', { class: 'app-content' });
    const nav = h('nav', { class: 'app-nav' });
    const showScreen = (sc) => {
      for (const b of nav.children) b.classList.toggle('active', b.dataset.id === sc.id);
      clear(content).append(h('h2', { class: 'app-screen-title' }, sc.title), h('div', { class: 'widgets' }, sc.widgets.map((w) => widget(w))));
    };
    for (const sc of app.screens || []) nav.append(h('button', { dataset: { id: sc.id }, onclick: () => showScreen(sc) }, sc.title));
    panel.append(
      h('div', { class: 'app', style: { '--app-accent': app.accent || '#7c83ff' } }, h('div', { class: 'app-bar' }, h('span', { class: 'app-dot' }), h('b', null, app.name), h('span', { class: 'muted small' }, 'simulated in Guru · no install, no account')), h('div', { class: 'app-body' }, nav, content))
    );
    if (app.screens?.length) showScreen(app.screens[0]);
  }

  function widget(w) {
    const card = (cls, ...kids) => h('div', { class: `widget ${cls}` }, w.title ? h('div', { class: 'widget-title' }, w.title) : null, ...kids);
    switch (w.type) {
      case 'markdown':
        return card('w-md', renderMarkdown(w.text));
      case 'stat':
        return card(`w-stat st-${w.status || 'ok'}`, h('div', { class: 'stat-value' }, w.value, h('small', null, w.unit ? ' ' + w.unit : '')));
      case 'timeseries':
        return card('w-wide', lineChart(w));
      case 'table':
        return card('w-wide', table(w.columns, w.rows));
      case 'logs':
        return card('w-wide', logView([w.stream_id], { compact: true }));
      case 'query':
        return queryWidget(w, card);
      case 'form':
        return formWidget(w, card);
      case 'actions':
        return card('w-actions', h('div', { class: 'row' }, (w.buttons || []).map((b) => h('button', { class: 'btn', onclick: () => { const r = engine.pressButton(b); if (r.ok && r.message) toast(r.message, 'ok'); } }, b.label))));
      case 'editor':
        return editorWidget(w, card);
      case 'chat':
        return chatWidget(w, card);
      case 'workbench':
        return workbenchWidget(w, card);
      default:
        return card('', h('p', { class: 'muted' }, `Unsupported widget: ${w.type}`));
    }
  }

  // Physical work: pick an instrument, click a component, read it or act on it.
  function workbenchWidget(w, card) {
    const tools = ['Look', ...(w.tools || [])];
    let tool = tools[1] || 'Look';
    let selected = (w.items || [])[0];
    const toolbar = h('div', { class: 'bench-tools', role: 'radiogroup', 'aria-label': 'Instrument' });
    const drawTools = () =>
      clear(toolbar).append(
        h('span', { class: 'muted small' }, 'Instrument:'),
        ...tools.map((t) => h('button', { class: `chip ${t === tool ? 'on' : ''}`, role: 'radio', 'aria-checked': t === tool, onclick: () => { tool = t; drawTools(); drawPanel(); } }, t === 'Look' ? '👁 Look' : t))
      );
    const canvas = h('div', { class: 'bench-canvas' });
    const panel = h('div', { class: 'bench-panel' });
    const notebook = h('ol', { class: 'bench-notes' });
    const W = 1000;
    const H = 380;
    const pos = (it) => ({ x: 80 + (it.x / 100) * (W - 160), y: 40 + (it.y / 100) * (H - 80) });
    const byId = Object.fromEntries((w.items || []).map((i) => [i.id, i]));
    const drawCanvas = () => {
      const links = (w.links || []).filter((l) => byId[l.from] && byId[l.to]).map((l) => {
        const a = pos(byId[l.from]);
        const b = pos(byId[l.to]);
        return s('g', { class: 'bench-link' }, s('line', { x1: a.x, y1: a.y, x2: b.x, y2: b.y }), l.label ? s('text', { x: (a.x + b.x) / 2 + 6, y: (a.y + b.y) / 2 - 4 }, l.label) : null);
      });
      const items = (w.items || []).map((it) => {
        const p = pos(it);
        const st = engine.nodeStatus[it.id] || it.status;
        return s(
          'g',
          { class: `bench-item st-${st} ${selected === it ? 'selected' : ''}`, 'data-item': it.id, role: 'button', 'aria-label': `${it.label}, ${st}`, transform: `translate(${p.x},${p.y})`, tabindex: 0, onclick: () => { selected = it; draw(); }, onkeydown: (e) => e.key === 'Enter' && ((selected = it), draw()) },
          s('title', null, `${it.label} (${it.kind}): ${st}`),
          s('rect', { x: -78, y: -26, width: 156, height: 52, rx: 10 }),
          s('circle', { class: 'lamp', cx: -62, cy: 0, r: 6 }),
          s('text', { class: 'label', x: -48, y: -3 }, it.label.length > 19 ? it.label.slice(0, 18) + '…' : it.label),
          s('text', { class: 'sub', x: -48, y: 14 }, `${it.kind.length > 16 ? it.kind.slice(0, 15) + '…' : it.kind} · ${st}`)
        );
      });
      clear(canvas).append(s('svg', { viewBox: `0 0 ${W} ${H}`, class: 'bench-svg', role: 'img', 'aria-label': w.title || 'Workbench' }, links, items));
    };
    const note = (who, text, cls = '') => {
      notebook.prepend(h('li', { class: cls }, h('b', null, who), ' ', text));
    };
    const drawPanel = () => {
      clear(panel);
      if (!selected) return;
      const st = engine.nodeStatus[selected.id] || selected.status;
      panel.append(
        h('div', { class: 'row spread' }, h('h3', null, selected.label), h('span', { class: `chip st-${st}` }, st)),
        h('p', { class: 'muted small' }, selected.kind, ' · ', selected.detail),
        h('div', { class: 'row' }, h('button', { class: 'btn primary small', onclick: () => { const v = engine.inspect(selected, tool); note(`${selected.label} · ${tool}:`, v); draw(); } }, tool === 'Look' ? 'Look closely' : `Measure with ${tool}`)),
        (selected.actions || []).length ? h('div', { class: 'bench-actions' }, (selected.actions || []).map((a) => {
          const missing = engine.missing(a.requires);
          return h('button', { class: `btn small ${missing.length ? 'guarded' : ''}`, title: missing.length ? 'Not safe or not possible yet' : '', onclick: () => {
            const r = engine.pressButton(a);
            note(`${selected.label} · ${a.label}:`, r.message, r.ok ? 'ok' : 'blocked');
            draw();
          } }, missing.length ? '⚠ ' : '', a.label);
        })) : null
      );
    };
    const draw = () => {
      drawCanvas();
      drawPanel();
    };
    drawTools();
    draw();
    engine.on((kind) => kind === 'node' && drawCanvas());
    return card('w-wide w-bench', w.text ? h('p', { class: 'muted small' }, w.text) : null, toolbar, canvas, h('div', { class: 'bench-under' }, panel, h('div', { class: 'bench-log' }, h('b', { class: 'small' }, 'Notebook (latest first)'), notebook)));
  }

  function queryWidget(w, card) {
    const editor = h('textarea', { class: 'input mono', rows: 3, placeholder: w.placeholder || '', spellcheck: 'false' });
    const result = h('div', { class: 'query-result' });
    const run = () => {
      const r = engine.query(w, editor.value);
      clear(result);
      if (r.result === 'error') result.append(h('div', { class: 'q-error' }, r.text));
      else if (r.result === 'text') result.append(h('div', { class: 'q-text' }, r.text));
      else if (r.result === 'table') result.append(table(r.columns, r.rows), r.text ? h('p', { class: 'muted small' }, r.text) : null);
      else if (r.result === 'timeseries') result.append(lineChart({ labels: r.labels, series: r.series }), r.text ? h('p', { class: 'muted small' }, r.text) : null);
    };
    editor.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        run();
      }
    });
    return card('w-wide w-query', h('div', { class: 'query-head' }, h('span', { class: 'chip' }, w.language || 'query'), h('button', { class: 'btn primary small', onclick: run }, 'Run query'), h('span', { class: 'muted small' }, 'Ctrl+Enter')), editor, result);
  }

  function formWidget(w, card) {
    const inputs = {};
    const fields = (w.fields || []).map((f) => {
      let el;
      if (f.kind === 'select') el = h('select', { class: 'input' }, (f.options || []).map((o) => h('option', { value: o, selected: o === f.value ? true : null }, o)));
      else if (f.kind === 'toggle') el = h('input', { type: 'checkbox', checked: /^(true|on|yes|1)$/i.test(f.value) });
      else if (f.kind === 'textarea') el = h('textarea', { class: 'input mono', rows: 3, value: f.value });
      else el = h('input', { class: 'input', type: f.kind === 'number' ? 'text' : 'text', inputmode: f.kind === 'number' ? 'decimal' : null, value: f.value });
      inputs[f.id] = el;
      return h('label', { class: 'field' }, h('span', null, f.label), el);
    });
    const msg = h('div', { class: 'form-msg' });
    const submit = () => {
      const values = {};
      for (const [id, el] of Object.entries(inputs)) values[id] = el.type === 'checkbox' ? String(el.checked) : el.value;
      const r = engine.submitForm(w, values);
      msg.textContent = r.message;
      msg.className = `form-msg ${r.ok ? 'ok' : 'bad'}`;
    };
    return card('w-wide w-form', h('div', { class: 'form-grid' }, fields), h('div', { class: 'row' }, h('button', { class: 'btn primary', onclick: submit }, w.submit_label || 'Save')), msg);
  }

  function editorWidget(w, card) {
    const area = h('textarea', { class: 'input mono editor', rows: Math.min(18, Math.max(6, (w.text || '').split('\n').length + 1)), value: w.text, spellcheck: 'false' });
    const results = h('ul', { class: 'checks' });
    const check = () => {
      const r = engine.checkEditor(w, area.value);
      clear(results).append(...r.results.map((x) => h('li', { class: x.passed ? 'ok' : 'bad' }, x.passed ? '✓ ' : '✗ ', x.message)));
    };
    return card('w-wide w-editor', h('div', { class: 'query-head' }, h('span', { class: 'chip' }, w.path || w.language || 'file'), h('button', { class: 'btn primary small', onclick: check }, 'Save & check')), area, results);
  }

  function chatWidget(w, card) {
    const log = h('div', { class: 'chat-log' });
    const say = (who, text, cls = '') => {
      log.append(h('div', { class: `msg ${who} ${cls}` }, text));
      log.scrollTop = log.scrollHeight;
    };
    if (w.text) say('them', w.text);
    const input = h('input', { class: 'input', placeholder: 'Your reply…' });
    const send = () => {
      const text = input.value.trim();
      if (!text) return;
      input.value = '';
      say('me', text);
      const r = engine.chat(w, text);
      if (r.feedback) say('coach', r.feedback, 'coach');
      setTimeout(() => say('them', r.reply), 350);
    };
    input.addEventListener('keydown', (e) => e.key === 'Enter' && send());
    return card('w-wide w-chat', w.persona ? h('p', { class: 'muted small' }, w.persona) : null, log, h('div', { class: 'row' }, input, h('button', { class: 'btn primary', onclick: send }, 'Send')));
  }

  // ---------- completion ----------
  function showDebrief() {
    const r = engine.result();
    modal(
      r.revealed ? 'Scenario complete' : 'Scenario solved',
      h(
        'div',
        null,
        h('p', { class: 'lead' }, `${r.objectives_done} of ${r.objectives_total} objectives solved on your own in ${fmtMinutes(r.seconds)}${r.hints_used ? `, ${r.hints_used} hint(s)` : ', no hints'}${r.revealed ? `; ${r.revealed} shown by Guru. Replay it later to earn those yourself.` : '.'}`),
        renderMarkdown(spec.debrief)
      ),
      [{ label: 'Keep exploring' }, { label: 'Finish', primary: true, onClick: () => exit(true) }]
    );
  }

  function exit(done) {
    const r = engine.result();
    if (!done && !r.completed && r.objectives_done < r.objectives_total) {
      modal('Leave the simulation?', h('p', null, `You have completed ${r.objectives_done} of ${r.objectives_total} objectives. Your best attempt is kept.`), [
        { label: 'Stay' },
        { label: 'Leave', primary: true, onClick: () => finish(r) },
      ]);
      return;
    }
    finish(r);
  }
  function finish(r) {
    clearInterval(tick);
    onExit(r);
  }

  engine.on((kind, data) => {
    if (kind === 'toast') toast(data, 'ok');
    if (kind === 'blocked' && data) toast(data, 'bad');
    if (kind === 'flag' || kind === 'objective') renderObjectives();
    if (kind === 'node') renderWorld();
  });

  renderObjectives();
  // Tool simulations open on the tool; incidents on the first terminal.
  const first = (['tool', 'workbench'].includes(spec.kind) && tabs.find((t) => t.id.startsWith('app:'))) || tabs.find((t) => t.id.startsWith('term:')) || tabs[0];
  if (first) select(first.id);
  else renderWorld();
  return engine;
}

function table(columns = [], rows = []) {
  return h('div', { class: 'table-wrap' }, h('table', null, h('thead', null, h('tr', null, columns.map((c) => h('th', null, c)))), h('tbody', null, rows.map((r) => h('tr', null, r.map((c) => h('td', null, c)))))));
}

function highlight(text, rx) {
  if (!rx) return text;
  const g = new RegExp(rx.source, rx.flags.includes('g') ? rx.flags : rx.flags + 'g');
  const out = [];
  let last = 0;
  let m;
  while ((m = g.exec(text)) && m[0].length) {
    out.push(text.slice(last, m.index), h('mark', null, m[0]));
    last = m.index + m[0].length;
  }
  out.push(text.slice(last));
  return out;
}
