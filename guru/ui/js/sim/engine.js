// Guru simulation engine: plays an AI-authored simulation spec fully offline.
// Pure logic, no DOM, so it can be tested with `node --test`.
//
// Spec reference: core/src/sim.rs. Built-in behaviour must stay in sync with
// `builtin_flags` there (the validator relies on it).

const DEFAULT_TIME = 'Nov 12 03:12';

export function re(pattern, flags = 'i') {
  try {
    return new RegExp(pattern, flags);
  } catch {
    return null;
  }
}

function basename(cmd) {
  const first = (cmd || '').trim().split(/\s+/)[0] || '';
  return first.split('/').pop();
}

// Split a command line into words, honouring single and double quotes.
export function tokenize(line) {
  const out = [];
  let cur = '';
  let quote = null;
  let had = false;
  for (const ch of line) {
    if (quote) {
      if (ch === quote) quote = null;
      else cur += ch;
    } else if (ch === '"' || ch === "'") {
      quote = ch;
      had = true;
    } else if (/\s/.test(ch)) {
      if (cur || had) out.push(cur);
      cur = '';
      had = false;
    } else {
      cur += ch;
    }
  }
  if (cur || had) out.push(cur);
  return out;
}

// Split on unquoted pipes.
export function splitPipes(line) {
  const parts = [];
  let cur = '';
  let quote = null;
  for (let i = 0; i < line.length; i++) {
    const ch = line[i];
    if (quote) {
      if (ch === quote) quote = null;
      cur += ch;
    } else if (ch === '"' || ch === "'") {
      quote = ch;
      cur += ch;
    } else if (ch === '|' && line[i + 1] !== '|') {
      parts.push(cur.trim());
      cur = '';
    } else {
      cur += ch;
    }
  }
  parts.push(cur.trim());
  return parts.filter((p) => p.length);
}

function normalize(path) {
  const parts = [];
  for (const p of path.split('/')) {
    if (!p || p === '.') continue;
    if (p === '..') parts.pop();
    else parts.push(p);
  }
  return '/' + parts.join('/');
}

class Host {
  constructor(spec) {
    this.spec = spec;
    this.files = new Map();
    for (const f of spec.files || []) this.files.set(normalize(f.path), f.content || '');
    this.processes = (spec.processes || []).map((p) => ({ ...p }));
    this.services = new Map((spec.services || []).map((s) => [s.name, { ...s, enabled: true }]));
    this.user = spec.user || 'root';
    this.home = this.user === 'root' ? '/root' : `/home/${this.user}`;
    this.cwd = normalize(spec.cwd || this.home);
    this.history = [];
    this.rules = (spec.commands || []).map((r) => ({ ...r, rx: re(r.pattern) }));
  }

  get prompt() {
    const dir = this.cwd === this.home ? '~' : this.cwd;
    return `${this.user}@${this.spec.hostname}:${dir}${this.user === 'root' ? '#' : '$'}`;
  }

  resolve(path) {
    if (!path || path === '~') return this.home;
    if (path.startsWith('~/')) return normalize(this.home + path.slice(1));
    return normalize(path.startsWith('/') ? path : `${this.cwd}/${path}`);
  }

  isDir(path) {
    if (path === '/' || path === this.home || path === this.cwd) return true;
    const prefix = path.endsWith('/') ? path : path + '/';
    for (const f of this.files.keys()) if (f.startsWith(prefix)) return true;
    return false;
  }

  list(dir) {
    const prefix = dir === '/' ? '/' : dir + '/';
    const names = new Map();
    for (const f of this.files.keys()) {
      if (!f.startsWith(prefix)) continue;
      const rest = f.slice(prefix.length);
      const name = rest.split('/')[0];
      names.set(name, rest.includes('/') ? 'dir' : 'file');
    }
    return [...names.entries()].sort((a, b) => a[0].localeCompare(b[0]));
  }
}

export class SimEngine {
  constructor(spec) {
    this.spec = spec;
    this.flags = new Set();
    this.nodeStatus = {};
    for (const n of spec.world?.nodes || []) this.nodeStatus[n.id] = n.status;
    // Workbench items share the status map so effects can change them too.
    for (const it of this.benchItems()) this.nodeStatus[it.id] = it.status;
    this.hosts = {};
    for (const h of spec.hosts || []) this.hosts[h.node_id] = new Host(h);
    this.answers = {};
    // Objectives stay achieved once reached, even if later actions change the
    // world again (e.g. a fault is re-introduced while testing).
    this.achieved = new Set();
    this.revealed = new Set();
    this.hintsUsed = 0;
    this.started = Date.now();
    this.listeners = new Set();
    this.chatUsed = {};
  }

  on(fn) {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }

  emit(kind, data) {
    for (const fn of this.listeners) fn(kind, data);
  }

  setFlag(f) {
    if (!this.flags.has(f)) {
      this.flags.add(f);
      this.updateAchieved();
      this.emit('flag', f);
    }
  }

  clearFlag(f) {
    if (this.flags.delete(f)) this.emit('flag', f);
  }

  updateAchieved() {
    for (const o of this.spec.objectives || []) {
      if (o.kind === 'state' && (o.flags || []).every((f) => this.flags.has(f))) this.achieved.add(o.id);
    }
  }

  benchItems() {
    const out = [];
    for (const a of this.spec.apps || []) for (const sc of a.screens || []) for (const w of sc.widgets || []) if (w.type === 'workbench') out.push(...(w.items || []));
    return out;
  }

  missing(requires) {
    return (requires || []).filter((f) => !this.flags.has(f));
  }

  applyEffects(e) {
    if (!e) return [];
    for (const f of e.clear_flags || []) this.clearFlag(f);
    for (const f of e.set_flags || []) this.setFlag(f);
    for (const ns of e.node_status || []) {
      this.nodeStatus[ns.node_id] = ns.status;
      this.emit('node', ns);
    }
    const msgs = e.message ? [e.message] : [];
    for (const m of msgs) this.emit('toast', m);
    return msgs;
  }

  // ---------- objectives ----------

  // Achieved by the learner (revealed objectives do not count).
  objectiveDone(o) {
    if (o.kind === 'state') return this.achieved.has(o.id) || (o.flags || []).every((f) => this.flags.has(f));
    return !!this.answers[o.id]?.correct;
  }

  // Done or revealed: the simulation can always be finished.
  objectiveClosed(o) {
    return this.objectiveDone(o) || this.revealed.has(o.id);
  }

  // "Show me": no dead ends. Returns the explanation; no credit is given.
  reveal(id) {
    const o = (this.spec.objectives || []).find((x) => x.id === id);
    if (!o) return '';
    if (!this.objectiveDone(o)) this.revealed.add(id);
    this.emit('objective', id);
    return o.explanation || '';
  }

  progress() {
    const objs = this.spec.objectives || [];
    const done = objs.filter((o) => this.objectiveDone(o)).length;
    const closed = objs.filter((o) => this.objectiveClosed(o)).length;
    return { done, closed, total: objs.length, complete: objs.length > 0 && closed === objs.length };
  }

  // value: array of option indices, or a string for free text.
  answer(id, value) {
    const o = (this.spec.objectives || []).find((x) => x.id === id);
    if (!o) return { correct: false };
    let correct;
    if (o.options?.length) {
      const got = [...new Set(value)].sort().join(',');
      const want = [...new Set(o.correct)].sort().join(',');
      correct = got === want;
    } else {
      const v = String(value || '').trim();
      correct = v.length > 0 && (o.accept || []).some((p) => re(p)?.test(v));
    }
    const prev = this.answers[id];
    this.answers[id] = { correct: correct || !!prev?.correct, attempts: (prev?.attempts || 0) + 1 };
    this.emit('objective', id);
    return { correct, explanation: correct ? o.explanation : '' };
  }

  useHint() {
    const hints = this.spec.hints || [];
    if (this.hintsUsed >= hints.length) return null;
    return hints[this.hintsUsed++];
  }

  result() {
    const p = this.progress();
    return {
      completed: p.complete,
      objectives_done: p.done,
      objectives_total: p.total,
      hints_used: this.hintsUsed,
      revealed: this.revealed.size,
      seconds: Math.round((Date.now() - this.started) / 1000),
    };
  }

  // ---------- logs ----------

  viewStream(id) {
    this.setFlag(`viewed:${id}`);
    return (this.spec.logs || []).find((l) => l.id === id);
  }

  // ---------- terminal ----------

  exec(nodeId, rawLine) {
    const host = this.hosts[nodeId];
    if (!host) return { output: 'No terminal on this machine.' };
    const line = rawLine.trim();
    if (!line) return { output: '' };
    host.history.push(line);
    const messages = [];

    // Whole-line scripted rules first (they may include pipes).
    const whole = this.matchRule(host, line);
    if (whole) return { output: whole.output, messages: this.applyEffects(whole.effects) };

    // Output redirection: cmd > file / cmd >> file.
    let redirect = null;
    const m = line.match(/^(.*?)\s(>>?)\s*(\S+)\s*$/);
    let body = line;
    if (m && !/['"]/.test(m[3])) {
      body = m[1];
      redirect = { append: m[2] === '>>', path: host.resolve(m[3]) };
    }

    const stages = splitPipes(body);
    let text = null;
    let notFound = false;
    let clear = false;
    for (let i = 0; i < stages.length; i++) {
      const stage = stages[i];
      const rule = this.matchRule(host, stage);
      if (rule) {
        messages.push(...this.applyEffects(rule.effects));
        text = rule.output;
        continue;
      }
      const r = this.builtin(host, stage, text, i > 0);
      if (r.notFound) notFound = true;
      if (r.clear) clear = true;
      messages.push(...(r.messages || []));
      text = r.output;
      if (r.notFound) break;
    }
    if (redirect && !notFound) {
      const prev = redirect.append ? host.files.get(redirect.path) || '' : '';
      const sep = prev && !prev.endsWith('\n') ? '\n' : '';
      host.files.set(redirect.path, prev + sep + (text ?? '') + '\n');
      this.setFlag(`wrote:${redirect.path}`);
      text = '';
    }
    return { output: text ?? '', messages, notFound, clear };
  }

  matchRule(host, line) {
    const plain = line.trim();
    for (const r of host.rules) if (r.rx && r.rx.test(plain)) return r;
    return null;
  }

  readFile(host, path) {
    const p = host.resolve(path);
    if (host.files.has(p)) {
      this.setFlag(`read:${p}`);
      return { ok: true, text: host.files.get(p), path: p };
    }
    if (host.isDir(p)) return { ok: false, err: `${path}: Is a directory` };
    return { ok: false, err: `${path}: No such file or directory` };
  }

  builtin(host, stage, stdin, piped) {
    let words = tokenize(stage);
    if (words[0] === 'sudo') words = words.slice(1);
    const [cmd, ...args] = words;
    const flags = args.filter((a) => /^-/.test(a));
    const operands = args.filter((a) => !/^-/.test(a));
    const has = (ch) => flags.some((f) => !f.startsWith('--') && f.includes(ch));
    const out = (output, extra = {}) => ({ output, ...extra });
    const inputOrFiles = (files, name) => {
      if (!files.length) return { text: stdin ?? '' };
      const parts = [];
      for (const f of files) {
        const r = this.readFile(host, f);
        if (!r.ok) return { error: `${name}: ${r.err}` };
        parts.push(r.text);
      }
      return { text: parts.join('\n') };
    };
    const lines = (t) => (t === '' ? [] : t.replace(/\n$/, '').split('\n'));
    const numArg = (def) => {
      const i = args.indexOf('-n');
      if (i >= 0 && args[i + 1]) return parseInt(args[i + 1], 10) || def;
      const short = args.find((a) => /^-\d+$/.test(a));
      return short ? parseInt(short.slice(1), 10) : def;
    };

    switch (cmd) {
      case 'help':
        return out(
          'Simulated shell. Try: ls, cd, cat, grep, head, tail, find, wc, sort, uniq, awk, cut, ps, top, kill,\n' +
            'pkill, systemctl, journalctl, rm, echo, whoami, hostname, uname, history, clear.\n' +
            'Tools of this scenario (e.g. docker, kubectl, netstat, iptables…) also work when the scenario supports them.'
        );
      case 'clear':
        return out('', { clear: true });
      case 'pwd':
        return out(host.cwd);
      case 'whoami':
        return out(host.user);
      case 'hostname':
        return out(host.spec.hostname);
      case 'id':
        return out(host.user === 'root' ? 'uid=0(root) gid=0(root) groups=0(root)' : `uid=1001(${host.user}) gid=1001(${host.user}) groups=1001(${host.user})`);
      case 'uname':
        return out(has('a') ? `Linux ${host.spec.hostname} 5.15.0-105-generic #115-Ubuntu SMP x86_64 GNU/Linux` : 'Linux');
      case 'date':
        return out('Tue Nov 12 03:26:11 UTC 2024');
      case 'echo':
        return out(args.join(' '));
      case 'history':
        return out(host.history.map((h, i) => `${String(i + 1).padStart(5)}  ${h}`).join('\n'));
      case 'exit':
      case 'logout':
        return out('(This terminal stays open in the simulation.)');
      case 'which': {
        const known = ['ls', 'cat', 'grep', 'ps', 'kill', 'systemctl', 'journalctl', 'rm', 'top', 'awk', 'sort'];
        return out(operands.map((o) => (known.includes(o) || this.ruleMentions(host, o) ? `/usr/bin/${o}` : '')).filter(Boolean).join('\n'));
      }
      case 'cd': {
        const target = host.resolve(operands[0] || '~');
        if (!host.isDir(target)) return out(`bash: cd: ${operands[0]}: No such file or directory`);
        host.cwd = target;
        return out('');
      }
      case 'ls': {
        const targets = operands.length ? operands : ['.'];
        const all = has('a');
        const long = has('l');
        const blocks = [];
        for (const t of targets) {
          const p = host.resolve(t);
          if (host.files.has(p)) {
            blocks.push(long ? this.longEntry(p.split('/').pop(), 'file', host.files.get(p)) : t);
            continue;
          }
          if (!host.isDir(p)) {
            blocks.push(`ls: cannot access '${t}': No such file or directory`);
            continue;
          }
          let entries = host.list(p).filter(([n]) => all || !n.startsWith('.'));
          const header = targets.length > 1 ? `${t}:\n` : '';
          if (long) {
            const body = entries.map(([n, kind]) => this.longEntry(n, kind, host.files.get(`${p === '/' ? '' : p}/${n}`) || ''));
            blocks.push(header + [`total ${entries.length * 4}`, ...(all ? ['drwxr-xr-x 2 root root 4096 ' + DEFAULT_TIME + ' .', 'drwxr-xr-x 3 root root 4096 ' + DEFAULT_TIME + ' ..'] : []), ...body].join('\n'));
          } else {
            blocks.push(header + entries.map(([n, kind]) => (kind === 'dir' ? n + '/' : n)).join('  '));
          }
        }
        return out(blocks.join('\n\n'));
      }
      case 'cat': {
        if (!operands.length) return out(stdin ?? '');
        const r = inputOrFiles(operands, 'cat');
        return out(r.error ?? r.text.replace(/\n$/, ''));
      }
      case 'head':
      case 'tail': {
        const n = numArg(10);
        const files = operands.filter((o) => String(n) !== o);
        const r = inputOrFiles(files, cmd);
        if (r.error) return out(r.error);
        const ls = lines(r.text);
        const sel = cmd === 'head' ? ls.slice(0, n) : ls.slice(Math.max(0, ls.length - n));
        const note = cmd === 'tail' && has('f') ? '\n(follow mode is not available in the simulation)' : '';
        return out(sel.join('\n') + note);
      }
      case 'grep':
      case 'egrep': {
        if (!operands.length) return out('Usage: grep [OPTION]... PATTERNS [FILE]...');
        const [pattern, ...files] = operands;
        const rx = re(pattern, has('i') ? 'i' : '') || re(pattern.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), has('i') ? 'i' : '');
        const invert = has('v');
        const recursive = has('r') || has('R');
        let targets = files;
        if (recursive) {
          const roots = files.length ? files : ['.'];
          targets = [];
          for (const root of roots) {
            const p = host.resolve(root);
            for (const f of host.files.keys()) if (f === p || f.startsWith(p === '/' ? '/' : p + '/')) targets.push(f);
          }
        }
        const results = [];
        let count = 0;
        const multi = targets.length > 1 || recursive;
        const sources = targets.length ? targets : [null];
        for (const f of sources) {
          let text;
          if (f === null) text = stdin ?? '';
          else {
            const r = this.readFile(host, f);
            if (!r.ok) {
              results.push(`grep: ${r.err}`);
              continue;
            }
            text = r.text;
          }
          let fileCount = 0;
          lines(text).forEach((l, i) => {
            if (rx.test(l) !== invert) {
              fileCount++;
              if (!has('c') && !has('l')) results.push(`${multi && f ? f + ':' : ''}${has('n') ? i + 1 + ':' : ''}${l}`);
            }
          });
          if (has('l') && fileCount && f) results.push(f);
          if (has('c')) results.push(`${multi && f ? f + ':' : ''}${fileCount}`);
          count += fileCount;
        }
        return out(results.join('\n'), { status: count ? 0 : 1 });
      }
      case 'wc': {
        const r = inputOrFiles(operands, 'wc');
        if (r.error) return out(r.error);
        const ls = lines(r.text);
        if (has('l')) return out(String(ls.length));
        if (has('w')) return out(String(r.text.split(/\s+/).filter(Boolean).length));
        if (has('c')) return out(String(r.text.length));
        return out(`${ls.length} ${r.text.split(/\s+/).filter(Boolean).length} ${r.text.length}`);
      }
      case 'sort': {
        const r = inputOrFiles(operands, 'sort');
        if (r.error) return out(r.error);
        let ls = lines(r.text);
        if (has('n')) ls.sort((a, b) => parseFloat(a) - parseFloat(b));
        else ls.sort();
        if (has('r')) ls.reverse();
        if (has('u')) ls = [...new Set(ls)];
        return out(ls.join('\n'));
      }
      case 'uniq': {
        const r = inputOrFiles(operands, 'uniq');
        if (r.error) return out(r.error);
        const res = [];
        for (const l of lines(r.text)) {
          const last = res[res.length - 1];
          if (last && last.l === l) last.n++;
          else res.push({ l, n: 1 });
        }
        return out(res.map((x) => (has('c') ? `${String(x.n).padStart(7)} ${x.l}` : x.l)).join('\n'));
      }
      case 'cut': {
        const dIdx = args.findIndex((a) => a.startsWith('-d'));
        const fIdx = args.findIndex((a) => a.startsWith('-f'));
        const delim = dIdx >= 0 ? (args[dIdx].length > 2 ? args[dIdx].slice(2) : args[dIdx + 1]) : '\t';
        const fieldsSpec = fIdx >= 0 ? (args[fIdx].length > 2 ? args[fIdx].slice(2) : args[fIdx + 1]) : '1';
        const wanted = fieldsSpec.split(',').map((x) => parseInt(x, 10) - 1);
        const used = new Set([dIdx >= 0 && args[dIdx].length === 2 ? args[dIdx + 1] : null, fIdx >= 0 && args[fIdx].length === 2 ? args[fIdx + 1] : null]);
        const files = operands.filter((o) => !used.has(o));
        const r = inputOrFiles(files, 'cut');
        if (r.error) return out(r.error);
        return out(lines(r.text).map((l) => wanted.map((i) => l.split(delim)[i] ?? '').join(delim)).join('\n'));
      }
      case 'awk': {
        // Supports the log-analysis classic: awk [-F sep] '{print $1, $3}' [file]
        let sep = null;
        const rest = [];
        for (let i = 0; i < args.length; i++) {
          if (args[i] === '-F') sep = args[++i];
          else if (args[i].startsWith('-F')) sep = args[i].slice(2);
          else rest.push(args[i]);
        }
        const prog = rest[0] || '';
        const pm = prog.match(/^\s*\{\s*print\s+(.+?)\s*;?\s*\}\s*$/);
        if (!pm) return out("awk: this simulation supports '{print $N, $M}' programs");
        const r = inputOrFiles(rest.slice(1), 'awk');
        if (r.error) return out(r.error);
        const exprs = pm[1].split(',').map((x) => x.trim());
        return out(
          lines(r.text)
            .map((l) => {
              const fieldsArr = sep ? l.split(sep) : l.trim().split(/\s+/);
              return exprs
                .map((e) => (e === '$0' ? l : e === 'NF' ? fieldsArr.length : /^\$\d+$/.test(e) ? fieldsArr[parseInt(e.slice(1), 10) - 1] ?? '' : e.replace(/^"|"$/g, '')))
                .join(' ');
            })
            .join('\n')
        );
      }
      case 'find': {
        const root = host.resolve(operands[0] && !operands[0].startsWith('-') ? operands[0] : '.');
        const ni = args.indexOf('-name');
        const iname = args.indexOf('-iname');
        const pat = ni >= 0 ? args[ni + 1] : iname >= 0 ? args[iname + 1] : null;
        const rx = pat ? re('^' + pat.replace(/[.+^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '.*').replace(/\?/g, '.') + '$', iname >= 0 ? 'i' : '') : null;
        const found = new Set();
        for (const f of host.files.keys()) {
          if (!(f === root || f.startsWith(root === '/' ? '/' : root + '/'))) continue;
          const segs = f.slice(root.length).split('/').filter(Boolean);
          let acc = root === '/' ? '' : root;
          for (const s of segs) {
            acc += '/' + s;
            if (!rx || rx.test(s)) found.add(acc);
          }
        }
        return out([...found].sort().join('\n'));
      }
      case 'ps': {
        const rows = host.processes.map((p) => `${p.user.padEnd(9)} ${String(p.pid).padStart(6)} ${p.cpu.toFixed(1).padStart(5)} ${p.mem.toFixed(1).padStart(5)} ${DEFAULT_TIME.slice(7)}   ${p.command}`);
        return out(['USER         PID  %CPU  %MEM START   COMMAND', ...rows].join('\n'));
      }
      case 'top':
      case 'htop': {
        const sorted = [...host.processes].sort((a, b) => b.cpu - a.cpu);
        const total = Math.min(100, sorted.reduce((s, p) => s + p.cpu, 0) / 4).toFixed(1);
        return out(
          [
            `top - 03:26:11 up 10 days,  5:41,  1 user,  load average: ${(total / 25).toFixed(2)}, 3.98, 3.20`,
            `Tasks: ${sorted.length} total,   1 running`,
            `%Cpu(s): ${total} us,  1.2 sy,  0.0 ni, ${(100 - total).toFixed(1)} id`,
            '',
            '    PID USER      %CPU  %MEM COMMAND',
            ...sorted.map((p) => `${String(p.pid).padStart(7)} ${p.user.padEnd(9)} ${p.cpu.toFixed(1).padStart(5)} ${p.mem.toFixed(1).padStart(5)} ${p.command}`),
            '',
            '(snapshot; press Enter for a new prompt)',
          ].join('\n')
        );
      }
      case 'kill': {
        const pids = operands.map((x) => parseInt(x, 10)).filter((x) => !Number.isNaN(x));
        if (!pids.length) return out('kill: usage: kill [-s sigspec | -n signum | -sigspec] pid ...');
        const res = [];
        const messages = [];
        for (const pid of pids) {
          const idx = host.processes.findIndex((p) => p.pid === pid);
          if (idx < 0) {
            res.push(`bash: kill: (${pid}) - No such process`);
            continue;
          }
          const [p] = host.processes.splice(idx, 1);
          this.setFlag(`killed:${p.pid}`);
          this.setFlag(`killed:${basename(p.command)}`);
          messages.push(`Process ${basename(p.command)} (${p.pid}) terminated.`);
        }
        for (const m of messages) this.emit('toast', m);
        return out(res.join('\n'), { messages });
      }
      case 'pkill':
      case 'killall': {
        const name = operands[0];
        if (!name) return out(`${cmd}: no process name specified`);
        const rx = re(name.replace(/[.+^${}()|[\]\\]/g, '\\$&'), '');
        const hit = host.processes.filter((p) => (cmd === 'killall' ? basename(p.command) === name : rx.test(p.command)));
        if (!hit.length) return out(cmd === 'killall' ? `${name}: no process found` : '');
        for (const p of hit) {
          host.processes = host.processes.filter((x) => x !== p);
          this.setFlag(`killed:${p.pid}`);
          this.setFlag(`killed:${basename(p.command)}`);
        }
        const msg = `Terminated ${hit.length} process(es).`;
        this.emit('toast', msg);
        return out('', { messages: [msg] });
      }
      case 'systemctl': {
        const [verb, name] = operands;
        const svcName = (name || '').replace(/\.service$/, '');
        if (!verb || verb === 'list-units' || verb === '--type=service') {
          return out(['UNIT                LOAD   ACTIVE   SUB', ...[...host.services.values()].map((s) => `${(s.name + '.service').padEnd(20)}loaded ${s.status.padEnd(8)} ${s.status === 'active' ? 'running' : 'dead'}`)].join('\n'));
        }
        const s = host.services.get(svcName);
        if (!s) return out(`Unit ${svcName}.service could not be found.`);
        const msgOut = (status) => {
          s.status = status;
          return '';
        };
        switch (verb) {
          case 'status':
            return out(`● ${s.name}.service - ${s.detail}\n     Loaded: loaded (/lib/systemd/system/${s.name}.service; ${s.enabled ? 'enabled' : 'disabled'})\n     Active: ${s.status === 'active' ? 'active (running)' : s.status === 'failed' ? 'failed (Result: exit-code)' : 'inactive (dead)'} since Tue 2024-11-12 03:00:01 UTC`);
          case 'stop':
            this.setFlag(`stopped:${s.name}`);
            return out(msgOut('inactive'));
          case 'start':
            this.setFlag(`started:${s.name}`);
            return out(msgOut('active'));
          case 'restart':
            this.setFlag(`restarted:${s.name}`);
            return out(msgOut('active'));
          case 'disable':
            s.enabled = false;
            this.setFlag(`disabled:${s.name}`);
            return out(`Removed /etc/systemd/system/multi-user.target.wants/${s.name}.service.`);
          case 'enable':
            s.enabled = true;
            this.setFlag(`enabled:${s.name}`);
            return out(`Created symlink /etc/systemd/system/multi-user.target.wants/${s.name}.service.`);
          default:
            return out(`Unknown command verb ${verb}.`);
        }
      }
      case 'journalctl': {
        const ui = args.indexOf('-u');
        const unit = ui >= 0 ? (args[ui + 1] || '').replace(/\.service$/, '') : null;
        const streams = (this.spec.logs || []).filter((l) => l.node_id === host.spec.node_id && (!unit || l.id.includes(unit) || l.name.toLowerCase().includes(unit.toLowerCase())));
        const all = streams.flatMap((s) => s.lines.map((x) => `${x.ts} ${host.spec.hostname} ${x.message}`));
        if (!all.length) return out('-- No entries --');
        streams.forEach((s) => this.setFlag(`viewed:${s.id}`));
        return out(all.slice(-numArg(200)).join('\n'));
      }
      case 'rm': {
        if (!operands.length) return out('rm: missing operand');
        const recursive = has('r') || has('R');
        const res = [];
        for (const o of operands) {
          const p = host.resolve(o);
          if (host.files.has(p)) {
            host.files.delete(p);
            this.setFlag(`removed:${p}`);
          } else if (host.isDir(p)) {
            if (!recursive) {
              res.push(`rm: cannot remove '${o}': Is a directory`);
              continue;
            }
            for (const f of [...host.files.keys()]) {
              if (f.startsWith(p + '/')) {
                host.files.delete(f);
                this.setFlag(`removed:${f}`);
              }
            }
            this.setFlag(`removed:${p}`);
          } else if (!has('f')) {
            res.push(`rm: cannot remove '${o}': No such file or directory`);
          }
        }
        return out(res.join('\n'));
      }
      case 'touch': {
        for (const o of operands) {
          const p = host.resolve(o);
          if (!host.files.has(p)) host.files.set(p, '');
        }
        return out('');
      }
      default:
        return out(`bash: ${cmd}: command not found`, { notFound: true });
    }
  }

  ruleMentions(host, name) {
    return host.rules.some((r) => r.pattern.includes(name));
  }

  longEntry(name, kind, content) {
    const size = kind === 'dir' ? 4096 : new TextEncoder().encode(content || '').length;
    const mode = kind === 'dir' ? 'drwxr-xr-x' : '-rw-r--r--';
    return `${mode} 1 root root ${String(size).padStart(6)} ${DEFAULT_TIME} ${name}`;
  }

  // Tab completion of paths and command names.
  complete(nodeId, line) {
    const host = this.hosts[nodeId];
    if (!host) return line;
    const words = line.split(/\s+/);
    const last = words[words.length - 1] || '';
    if (words.length === 1) {
      const cmds = ['ls', 'cat', 'cd', 'grep', 'head', 'tail', 'ps', 'top', 'kill', 'pkill', 'systemctl', 'journalctl', 'find', 'awk', 'sort', 'uniq', 'history', 'clear'];
      const hit = cmds.filter((c) => c.startsWith(last));
      return hit.length === 1 ? hit[0] + ' ' : line;
    }
    const slash = last.lastIndexOf('/');
    const dirPart = slash >= 0 ? last.slice(0, slash + 1) : '';
    const base = slash >= 0 ? last.slice(slash + 1) : last;
    const dir = host.resolve(dirPart || '.');
    const hits = host.list(dir).filter(([n]) => n.startsWith(base));
    if (hits.length !== 1) return line;
    const [n, kind] = hits[0];
    words[words.length - 1] = dirPart + n + (kind === 'dir' ? '/' : ' ');
    return words.join(' ');
  }

  // ---------- simulated tools ----------

  query(widget, text) {
    const q = text.trim();
    for (const r of widget.responses || []) {
      const rx = re(r.pattern);
      if (rx && rx.test(q)) {
        this.applyEffects(r.effects);
        return r;
      }
    }
    return { result: 'error', text: widget.fallback || 'No results.' };
  }

  submitForm(widget, values) {
    for (const rule of widget.rules || []) {
      const ok = (rule.conditions || []).every((c) => re(c.pattern)?.test(String(values[c.field] ?? '').trim()));
      if (ok) {
        this.applyEffects(rule.effects);
        return { ok: rule.ok, message: rule.message };
      }
    }
    return { ok: false, message: widget.fallback || 'Not accepted.' };
  }

  // Buttons and workbench actions: refused (and explained) until their
  // requirements are met, which is how simulations teach safe order of work.
  pressButton(button) {
    const missing = this.missing(button.requires);
    if (missing.length) {
      this.emit('blocked', button.blocked_message);
      return { ok: false, message: button.blocked_message || 'Not possible yet.' };
    }
    this.applyEffects(button.effects);
    return { ok: true, message: button.message };
  }

  // Workbench: measure or look at an item with a tool. The first reading for
  // that tool whose requirements are met wins, so readings follow the state.
  inspect(item, tool) {
    const reading = (item.readings || []).find((r) => r.tool === tool && this.missing(r.requires).length === 0);
    this.setFlag(`inspected:${item.id}`);
    if (!reading) return tool === 'Look' ? `${item.label}: nothing unusual to see.` : `No meaningful reading with the ${tool} here.`;
    this.applyEffects(reading.effects);
    return reading.value;
  }

  checkEditor(widget, content) {
    const results = (widget.checks || []).map((c) => {
      const hit = re(c.pattern, 'im')?.test(content) ?? false;
      return { passed: c.must_match ? hit : !hit, message: c.message };
    });
    const passed = results.every((r) => r.passed);
    if (passed) this.applyEffects(widget.effects);
    return { passed, results };
  }

  chat(widget, message) {
    const used = (this.chatUsed[widget.id] ||= new Set());
    const turns = widget.turns || [];
    let idx = turns.findIndex((t, i) => !used.has(i) && re(t.expect)?.test(message));
    if (idx < 0) idx = turns.findIndex((t) => re(t.expect)?.test(message));
    if (idx < 0) return { reply: widget.fallback || '…', feedback: '' };
    used.add(idx);
    const t = turns[idx];
    this.applyEffects(t.effects);
    return { reply: t.reply, feedback: t.feedback };
  }
}
