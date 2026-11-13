// Backend bridge. Inside the desktop app this calls the Rust commands in
// app/src/main.rs. Opened in a plain browser (for UI development and tests),
// it falls back to an in-memory mock that serves the built-in demo course.

const tauri = window.__TAURI__;
export const isDesktop = !!tauri?.core;

export async function call(cmd, args = {}) {
  if (isDesktop) return tauri.core.invoke(cmd, args);
  return mock(cmd, args);
}

export async function listen(event, fn) {
  if (isDesktop) return tauri.event.listen(event, (e) => fn(e.payload));
  (mockListeners[event] ||= new Set()).add(fn);
  return () => mockListeners[event].delete(fn);
}

// ---------------------------------------------------------------- mock ----

const mockListeners = {};
const emit = (event, payload) => (mockListeners[event] || []).forEach((fn) => fn(payload));
const courses = new Map(); // slug -> { course, modules, sims, progress, deck }
let demoCache;

async function loadDemo() {
  if (demoCache) return demoCache;
  const get = (p) => fetch(`demo/${p}`).then((r) => r.json());
  const course = await get('course.json');
  const modules = {};
  for (const m of course.modules) modules[m.id] = await get(`modules/${m.id}.json`);
  const sims = {};
  for (const m of course.modules) for (const s of m.sims) sims[s.id] = await get(`sims/${s.id}.json`);
  demoCache = { course, modules, sims };
  return demoCache;
}

const clone = (x) => JSON.parse(JSON.stringify(x));
const emptyProgress = () => ({ lessons_read: [], quiz_scores: [], sims: [], mastery: [], seconds_studied: 0, last_module: '' });
const upsert = (arr, k, v) => {
  const e = arr.find((x) => x[0] === k);
  if (e) e[1] = v;
  else arr.push([k, v]);
};
const get = (arr, k) => arr.find((x) => x[0] === k)?.[1];

function evidence(p, skill, score) {
  const old = get(p.mastery, skill);
  upsert(p.mastery, skill, old === undefined ? score : 0.4 * old + 0.6 * score);
}

function deckFrom(modules) {
  return Object.values(modules).flatMap((m) => m.cards.map((c, i) => ({ id: `${m.id}c${i + 1}`, ...c, ease: 2.5, interval_days: 0, reps: 0, lapses: 0, due_day: 0 })));
}

function cardFor(entry, downloading = false) {
  const { course, modules, sims, progress, deck } = entry;
  const total = course.skills.reduce((s, k) => s + Math.max(1, k.weight), 0) || 1;
  const mastery = course.skills.reduce((s, k) => s + Math.max(1, k.weight) * (get(progress.mastery, k.id) || 0), 0) / total;
  const allSims = course.modules.flatMap((m) => m.sims).concat(course.capstone?.id ? [course.capstone] : []);
  return {
    course,
    mastery,
    modules_ready: Object.keys(modules).length,
    sims_ready: Object.keys(sims).length,
    sims_total: allSims.length,
    lessons_read: progress.lessons_read.length,
    due_cards: deck.filter((c) => c.due_day <= 0).length,
    downloading,
    size_mb: 0.2,
  };
}

function offlineScope(topic) {
  const broad = /devops|engineer|cloud|security|architecture|language|spanish|french|german|japanese|data/i.test(topic);
  const f = broad ? 2 : topic.split(/\s+/).length > 3 ? 1.5 : 1;
  const r = (x) => Math.round(x * 2) / 2;
  return {
    title: topic,
    breadth: broad ? 7 : 4,
    beginner: { hours: r(1 * f), can_do: 'Understand the core ideas and complete common tasks with guidance.' },
    intermediate: { hours: r(2.5 * f), can_do: 'Work independently on typical real-world tasks and fix the common failures.' },
    advanced: { hours: r(6 * f), can_do: 'Handle complex, unusual situations, optimise, and teach others.' },
    simulated_tools: broad ? ['Linux terminal', 'Grafana', 'CI/CD pipeline'] : [topic],
    note: 'Browser preview: estimated offline.',
    estimated_offline: true,
  };
}

const downloading = new Set();
const slugOf = (topic) => (topic.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'course').slice(0, 40);

async function mock(cmd, a) {
  await new Promise((r) => setTimeout(r, 30));
  switch (cmd) {
    case 'app_status':
      return {
        codex_version: null,
        codex_bin: 'codex',
        logged_in: false,
        login_message: 'Browser preview: run the desktop app to sign in with ChatGPT.',
        machine: { os: navigator.platform, arch: '', cpus: navigator.hardwareConcurrency || 4, total_ram_mb: 16384, avail_ram_mb: 9000, free_disk_mb: 120000, tools: [] },
        budget: { tier: 'large', max_ram_mb: 4500, max_disk_mb: 10240, max_cpus: 4, containers: false, local_cluster: false },
        config: { codex_bin: null, model: null, reasoning_effort: null, web_search: true, stackexchange: true, stackexchange_key: null, parallel_jobs: null },
        courses_dir: '~/.guru/courses',
      };
    case 'save_settings':
    case 'start_login':
    case 'open_url':
      return null;
    case 'install_demo': {
      const d = await loadDemo();
      const slug = d.course.slug;
      if (!courses.has(slug)) courses.set(slug, { course: clone(d.course), modules: clone(d.modules), sims: clone(d.sims), progress: emptyProgress(), deck: deckFrom(d.modules) });
      return slug;
    }
    case 'list_courses':
      return [...courses.entries()].map(([slug, e]) => cardFor(e, downloading.has(slug)));
    case 'get_course': {
      const e = courses.get(a.slug);
      if (!e) throw 'course not found';
      return { card: cardFor(e, downloading.has(a.slug)), progress: e.progress, ready_modules: Object.keys(e.modules), ready_sims: Object.keys(e.sims) };
    }
    case 'get_module':
      return courses.get(a.slug).modules[a.id];
    case 'get_sim':
      return clone(courses.get(a.slug).sims[a.id]);
    case 'delete_course':
      courses.delete(a.slug);
      return null;
    case 'scope_topic':
      await new Promise((r) => setTimeout(r, 500));
      return offlineScope(a.topic);
    case 'start_download': {
      // Like the real app: return at once, progress arrives as events.
      mockDownload(a.request);
      return slugOf(a.request.topic);
    }
    case 'resume_download':
    case 'pause_download':
      return null;
    case 'mark_read': {
      const p = courses.get(a.slug).progress;
      if (!p.lessons_read.includes(a.module)) p.lessons_read.push(a.module);
      p.last_module = a.module;
      p.seconds_studied += a.seconds;
      return null;
    }
    case 'record_quiz': {
      const p = courses.get(a.slug).progress;
      upsert(p.quiz_scores, a.module, Math.max(get(p.quiz_scores, a.module) || 0, a.score));
      for (const [s, v] of a.perSkill) evidence(p, s, v);
      return p.mastery;
    }
    case 'record_sim': {
      const p = courses.get(a.slug).progress;
      const r = a.result;
      const ratio = r.objectives_total ? r.objectives_done / r.objectives_total : 0;
      for (const s of a.skills) evidence(p, s, Math.max(0, Math.min(1, ratio - 0.1 * r.hints_used)));
      const old = get(p.sims, a.simId);
      if (!old || old.objectives_done <= r.objectives_done) upsert(p.sims, a.simId, r);
      p.seconds_studied += r.seconds;
      return null;
    }
    case 'due_cards':
      return courses.get(a.slug).deck.filter((c) => c.due_day <= 0).slice(0, a.limit);
    case 'grade_card': {
      const c = courses.get(a.slug).deck.find((x) => x.id === a.id);
      if (a.grade === 1) return true;
      c.due_day = a.grade === 4 ? 3 : 1;
      return false;
    }
    case 'ask_guru':
      return 'Browser preview: in the desktop app this question goes to Codex (your ChatGPT sign-in).';
    case 'ai_command':
      return `(browser preview) the desktop app would ask Codex to improvise: ${a.command}`;
    default:
      throw `unknown command ${cmd}`;
  }
}

async function mockDownload(req) {
  const d = await loadDemo();
  const slug = slugOf(req.topic);
  const course = clone(d.course);
  Object.assign(course, { slug, title: req.topic, topic: req.topic, goal: req.goal, level: req.level, hours: req.hours, status: 'downloading', generator: 'browser preview' });
  const entry = { course, modules: {}, sims: {}, progress: emptyProgress(), deck: [] };
  courses.set(slug, entry);
  downloading.add(slug);
  const items = course.modules.flatMap((m) => [['module', m], ...m.sims.map((s) => ['sim', s])]);
  const total = items.length;
  emit('download', { slug, phase: 'planning', done: 0, total: 0, current: 'Designing the course', activity: 'Breaking the skill down…', message: '' });
  let done = 0;
  for (const [kind, item] of items) {
    await new Promise((r) => setTimeout(r, 700));
    emit('download', { slug, phase: 'building', done, total, current: kind === 'module' ? `Lesson & quiz: ${item.title}` : `Simulation: ${item.title}`, activity: 'Researching: real-world failures', message: '' });
    await new Promise((r) => setTimeout(r, 700));
    if (kind === 'module') {
      entry.modules[item.id] = clone(d.modules[item.id]);
      entry.deck.push(...deckFrom({ [item.id]: d.modules[item.id] }));
    } else entry.sims[item.id] = clone(d.sims[item.id]);
    done++;
  }
  course.status = 'ready';
  downloading.delete(slug);
  emit('download', { slug, phase: 'ready', done: total, total, current: '', activity: '', message: 'Your course is ready. No AI needed from here.' });
  return slug;
}
