// Guru UI: router and views. Plain ES modules, no framework, no build step.

import { call, isDesktop, listen } from './api.js';
import { append, bar, clear, fmtHours, fmtMinutes, h, modal, pct, toast } from './dom.js';
import { onLink, renderMarkdown } from './md.js';
import { renderSimPlayer } from './sim/player.js';

const view = document.getElementById('view');
const state = { status: null, downloads: {} };

onLink((url) => (isDesktop ? call('open_url', { url }) : window.open(url, '_blank', 'noopener')));

// ------------------------------------------------------------ helpers ----

const get = (pairs, key) => (pairs || []).find((p) => p[0] === key)?.[1];
const go = (hash) => (location.hash = hash);

// Mirrors core/src/scope.rs so the level bar responds instantly.
function hoursFor(scope, level) {
  const pts = [
    [0, Math.max(0.5, scope.beginner.hours * 0.5)],
    [33, scope.beginner.hours],
    [66, scope.intermediate.hours],
    [100, scope.advanced.hours],
  ];
  const x = Math.min(100, level);
  let y = pts[3][1];
  for (let i = 0; i < 3; i++) {
    const [x0, y0] = pts[i];
    const [x1, y1] = pts[i + 1];
    if (x <= x1) {
      y = y0 + ((y1 - y0) * (x - x0)) / (x1 - x0);
      break;
    }
  }
  return Math.max(0.5, Math.round(y * 2) / 2);
}
const levelLabel = (l) => (l < 20 ? 'Quick start' : l < 50 ? 'Beginner' : l < 83 ? 'Intermediate' : 'Advanced');
const canDo = (scope, l) => (l < 50 ? scope.beginner.can_do : l < 83 ? scope.intermediate.can_do : scope.advanced.can_do);
const modulesFor = (hours) => Math.min(16, Math.max(1, Math.ceil((hours * 60) / 45)));

function page(...children) {
  clear(view).append(h('div', { class: 'page' }, ...children));
  view.scrollTop = 0;
}

function loading(text = 'Loading…') {
  page(h('div', { class: 'loading' }, h('span', { class: 'spinner' }), text));
}

function nextStep(course, data) {
  const p = data.progress;
  for (const m of course.modules) {
    if (!data.ready_modules.includes(m.id)) return null;
    if (!p.lessons_read.includes(m.id)) return { label: `Read: ${m.title}`, hash: `#/course/${course.slug}/lesson/${m.id}` };
    if (get(p.quiz_scores, m.id) === undefined) return { label: `Quiz: ${m.title}`, hash: `#/course/${course.slug}/quiz/${m.id}` };
    for (const s of m.sims) {
      if (!data.ready_sims.includes(s.id)) return null;
      if (!get(p.sims, s.id)?.completed) return { label: `Simulation: ${s.title}`, hash: `#/course/${course.slug}/sim/${s.id}` };
    }
  }
  const cap = course.capstone;
  if (cap?.id && data.ready_sims.includes(cap.id) && !get(p.sims, cap.id)?.completed) return { label: `Capstone: ${cap.title}`, hash: `#/course/${course.slug}/sim/${cap.id}` };
  return null;
}

function statusLine(card) {
  const c = card.course;
  const ev = state.downloads[c.slug];
  if (card.downloading || ev?.phase === 'building' || ev?.phase === 'planning' || ev?.phase === 'researching') {
    const done = ev?.done ?? card.modules_ready + card.sims_ready;
    const total = ev?.total || c.modules.length + card.sims_total || 1;
    return h('div', { class: 'dl-mini' }, h('span', { class: 'spinner small' }), h('span', null, ev?.phase === 'planning' ? 'Designing the course…' : `Downloading ${done}/${total}`), bar(total ? done / total : 0, 'small'));
  }
  if (c.status === 'failed') return h('div', { class: 'dl-mini warn' }, '⏸ ', c.status_detail?.startsWith('Paused') ? 'Paused' : 'Incomplete download');
  if (c.status === 'ready') return h('div', { class: 'dl-mini ok' }, '✓ Downloaded · works offline');
  return h('div', { class: 'dl-mini' }, 'Preparing…');
}

// ------------------------------------------------------------ library ----

async function library() {
  loading();
  const cards = await call('list_courses');
  const header = h('div', { class: 'page-head' }, h('div', null, h('h1', null, 'Your courses'), h('p', { class: 'muted' }, 'Downloaded courses play offline. Lessons, quizzes, flashcards and simulations need no AI.')), h('button', { class: 'btn primary', onclick: () => go('#/new') }, '+ New course'));
  if (!cards.length) {
    page(
      h(
        'div',
        { class: 'hero' },
        h('h1', null, 'Learn anything in hours, not weeks.'),
        h('p', { class: 'lead' }, 'Tell Guru what you want to learn and how far you want to go. It builds a complete course, with reading-first lessons, quizzes, flashcards and realistic simulations of the real tools, then you study it like a downloaded course. No videos. No installs. No accounts on tools you don’t have.'),
        h('div', { class: 'row' }, h('button', { class: 'btn primary big', onclick: () => go('#/new') }, 'Create a course'), h('button', { class: 'btn big', onclick: openDemo }, 'Try the demo course (no AI needed)')),
        h('div', { class: 'hero-points' }, [
          ['Simulations instead of installs', 'Practise Grafana, Kubernetes, a hacked server or a conversation in Spanish inside Guru.'],
          ['Download once, study offline', 'AI is used to build the course. Studying needs none.'],
          ['Built on learning science', 'Retrieval, spacing, worked examples, real-world failures.'],
        ].map(([t, d]) => h('div', null, h('b', null, t), h('p', null, d))))
      )
    );
    return;
  }
  page(
    header,
    h(
      'div',
      { class: 'course-grid' },
      cards.map((card) => {
        const c = card.course;
        return h(
          'article',
          { class: 'course-card', onclick: () => go(`#/course/${c.slug}`), tabindex: 0, onkeydown: (e) => e.key === 'Enter' && go(`#/course/${c.slug}`) },
          h('div', { class: 'row spread' }, h('span', { class: 'chip' }, c.level_label || 'Course'), h('span', { class: 'muted small' }, fmtHours(c.hours || 0))),
          h('h2', null, c.title),
          h('p', { class: 'muted clamp' }, c.summary || c.goal || c.topic),
          statusLine(card),
          h('div', { class: 'mastery-line' }, h('span', { class: 'small' }, `Mastery ${pct(card.mastery)}`), bar(card.mastery)),
          h('div', { class: 'card-foot small muted' }, h('span', null, `${card.lessons_read}/${c.modules.length} lessons · ${card.sims_ready}/${card.sims_total} simulations`), card.due_cards ? h('span', { class: 'due' }, `${card.due_cards} cards due`) : null)
        );
      })
    ),
    cards.some((x) => x.course.slug === 'demo-incident-response') ? null : h('p', { class: 'muted small' }, h('a', { href: '#', onclick: (e) => (e.preventDefault(), openDemo()) }, 'Add the demo course'))
  );
}

async function openDemo() {
  const slug = await call('install_demo');
  go(`#/course/${slug}`);
}

// ---------------------------------------------------------- new course ----

const SUGGESTIONS = ['DevOps', 'Grafana & Prometheus', 'Kubernetes', 'Spring Boot', 'SQL for analysts', 'Conversational Spanish', 'Incident response', 'Excel pivot tables'];

function newCourse() {
  const topic = h('input', { class: 'input huge', placeholder: 'e.g. DevOps, Grafana, Spring Boot, Spanish for travel…', 'aria-label': 'Topic' });
  const goal = h('textarea', { class: 'input', rows: 2, placeholder: 'Optional: what do you want to be able to do? e.g. “debug production incidents at work”' });
  const btn = h('button', { class: 'btn primary big', onclick: () => sizeIt() }, 'Next: choose your level →');
  const sizeIt = async () => {
    const t = topic.value.trim();
    if (!t) {
      topic.focus();
      return;
    }
    btn.disabled = true;
    btn.textContent = 'Guru is sizing the topic…';
    try {
      const scope = await call('scope_topic', { topic: t, goal: goal.value.trim() });
      levelStep(t, goal.value.trim(), scope);
    } catch (e) {
      toast(String(e), 'bad');
      btn.disabled = false;
      btn.textContent = 'Next: choose your level →';
    }
  };
  topic.addEventListener('keydown', (e) => e.key === 'Enter' && sizeIt());
  page(
    h('div', { class: 'wizard' }, h('p', { class: 'step' }, 'Step 1 of 2'), h('h1', null, 'What do you want to learn?'), topic, h('div', { class: 'chips' }, SUGGESTIONS.map((s) => h('button', { class: 'chip big', onclick: () => ((topic.value = s), topic.focus()) }, s))), h('label', { class: 'field' }, h('span', null, 'Your goal'), goal), h('div', { class: 'row' }, btn))
  );
  topic.focus();
}

function levelStep(topic, goal, scope) {
  let level = 66;
  let hours = hoursFor(scope, level);
  let override = false;
  const hoursEl = h('div', { class: 'hours-big' });
  const labelEl = h('div', { class: 'level-label' });
  const canEl = h('p', { class: 'can-do' });
  const planEl = h('p', { class: 'muted' });
  const slider = h('input', { type: 'range', min: 0, max: 100, value: level, class: 'level-slider', 'aria-label': 'Target level' });
  const update = () => {
    if (!override) hours = hoursFor(scope, level);
    const mods = modulesFor(hours);
    hoursEl.textContent = fmtHours(hours);
    labelEl.textContent = levelLabel(level);
    canEl.textContent = canDo(scope, level);
    const days = Math.max(1, Math.ceil(hours));
    planEl.textContent = `${mods} module${mods > 1 ? 's' : ''} · about ${Math.round(mods * 1.5) + 1} simulations · e.g. 1 hour a day for ${days} day${days > 1 ? 's' : ''}`;
    slider.style.setProperty('--fill', `${level}%`);
  };
  slider.addEventListener('input', () => {
    level = Number(slider.value);
    override = false;
    update();
  });
  const adjust = (d) => {
    hours = Math.min(40, Math.max(0.5, hours + d));
    override = true;
    update();
  };
  const signedIn = state.status?.logged_in;
  const download = h('button', { class: 'btn primary big', onclick: () => start() }, '⬇ Download course');
  const start = async () => {
    download.disabled = true;
    download.textContent = 'Starting…';
    try {
      const slug = await call('start_download', { request: { topic, goal, level, hours, scope } });
      go(`#/course/${slug}`);
    } catch (e) {
      toast(String(e), 'bad');
      download.disabled = false;
      download.textContent = '⬇ Download course';
    }
  };
  page(
    h(
      'div',
      { class: 'wizard' },
      h('p', { class: 'step' }, h('a', { href: '#/new' }, '← Step 1'), ' · Step 2 of 2'),
      h('h1', null, scope.title || topic),
      h('p', { class: 'muted' }, 'Drag to where you want to get. Guru suggests the hours; adjust them if you like.'),
      h(
        'div',
        { class: 'level-card' },
        h('div', { class: 'level-top' }, h('div', null, labelEl, canEl), h('div', { class: 'hours-box' }, h('button', { class: 'btn round', onclick: () => adjust(-0.5), 'aria-label': 'Fewer hours' }, '−'), hoursEl, h('button', { class: 'btn round', onclick: () => adjust(0.5), 'aria-label': 'More hours' }, '+'))),
        slider,
        h('div', { class: 'zones' }, h('span', null, 'Beginner'), h('span', null, 'Intermediate'), h('span', null, 'Advanced')),
        planEl
      ),
      scope.simulated_tools?.length ? h('div', { class: 'sim-tools' }, h('b', null, 'Simulated inside Guru: '), scope.simulated_tools.map((t) => h('span', { class: 'chip' }, t)), h('span', { class: 'muted small' }, ' practise without installing or signing up')) : null,
      h('p', { class: 'muted small' }, 'Guru courses are intense: no filler, all practice. ', scope.note || ''),
      isDesktop && !signedIn ? h('div', { class: 'notice' }, 'Building a course uses AI through Codex. Sign in with ChatGPT first. ', h('button', { class: 'btn small', onclick: () => go('#/settings') }, 'Sign in')) : null,
      h('div', { class: 'row' }, download, h('span', { class: 'muted small' }, 'AI builds the whole course once (lessons, quizzes, simulations). After that, studying is offline.'))
    )
  );
  update();
}

// -------------------------------------------------------------- course ----

async function coursePage(slug) {
  loading();
  const data = await call('get_course', { slug });
  const card = data.card;
  const c = card.course;
  const p = data.progress;
  const ev = state.downloads[slug];
  const downloading = card.downloading || ['planning', 'researching', 'building'].includes(ev?.phase);
  const next = nextStep(c, data);

  const dlPanel = () => {
    if (c.status === 'ready' && !downloading) return null;
    const done = ev?.done ?? card.modules_ready + card.sims_ready;
    const total = ev?.total || c.modules.length + card.sims_total;
    const failed = c.status === 'failed' && !downloading;
    return h(
      'div',
      { class: `download-panel ${failed ? 'warn' : ''}` },
      h('div', { class: 'row spread' }, h('b', null, failed ? (c.status_detail?.startsWith('Paused') ? 'Download paused' : 'Download incomplete') : ev?.phase === 'planning' || !c.modules.length ? 'Designing your course…' : `Downloading your course · ${done}/${total}`), downloading ? h('button', { class: 'btn small', onclick: async () => { await call('pause_download', { slug }); toast('Pausing after the current items…'); } }, 'Pause') : h('button', { class: 'btn primary small', onclick: async () => { await call('resume_download', { slug }); state.downloads[slug] = { phase: 'building', done, total }; coursePage(slug); } }, 'Resume')),
      bar(total ? done / total : 0),
      h('p', { class: 'small' }, ev?.current || (failed ? c.status_detail : 'Starting…')),
      ev?.activity ? h('p', { class: 'small muted' }, ev.activity) : null,
      h('p', { class: 'small muted' }, 'You can start module 1 as soon as it is ready; the rest keeps downloading.')
    );
  };

  const moduleCard = (m, i) => {
    const ready = data.ready_modules.includes(m.id);
    const read = p.lessons_read.includes(m.id);
    const quiz = get(p.quiz_scores, m.id);
    return h(
      'article',
      { class: `module ${ready ? '' : 'pending'}` },
      h('div', { class: 'module-num' }, String(i + 1)),
      h(
        'div',
        { class: 'module-body' },
        h('div', { class: 'row spread' }, h('h3', null, m.title), h('span', { class: 'muted small' }, `${m.minutes} min`)),
        h('p', { class: 'muted' }, m.summary),
        ready
          ? h(
              'div',
              { class: 'steps' },
              h('button', { class: `step-btn ${read ? 'done' : ''}`, onclick: () => go(`#/course/${slug}/lesson/${m.id}`) }, read ? '✓ ' : '', 'Lesson'),
              h('button', { class: `step-btn ${quiz !== undefined ? 'done' : ''}`, onclick: () => go(`#/course/${slug}/quiz/${m.id}`) }, quiz !== undefined ? `✓ Quiz ${pct(quiz)}` : 'Quiz'),
              m.sims.map((s) => {
                const r = get(p.sims, s.id);
                const ok = data.ready_sims.includes(s.id);
                return h('button', { class: `step-btn sim ${r?.completed ? 'done' : ''}`, disabled: !ok, onclick: () => go(`#/course/${slug}/sim/${s.id}`), title: s.premise }, r?.completed ? '✓ ' : '▶ ', s.title, ok ? '' : ' (building…)');
              })
            )
          : h('p', { class: 'small muted' }, h('span', { class: 'spinner small' }), ' Being built…')
      )
    );
  };

  const cap = c.capstone;
  page(
    h('a', { class: 'back', href: '#/' }, '← All courses'),
    h(
      'div',
      { class: 'course-head' },
      h(
        'div',
        null,
        h('div', { class: 'row' }, h('span', { class: 'chip' }, c.level_label), h('span', { class: 'chip' }, fmtHours(c.hours || 0)), c.status === 'ready' ? h('span', { class: 'chip ok' }, 'Offline') : null),
        h('h1', null, c.title),
        c.summary ? h('p', { class: 'lead' }, c.summary) : null,
        c.target_performance ? h('div', { class: 'target' }, h('b', null, 'By the end you can: '), c.target_performance) : null
      ),
      h(
        'div',
        { class: 'course-side' },
        h('div', { class: 'mastery-big' }, h('span', null, pct(card.mastery)), h('small', null, 'mastery')),
        next ? h('button', { class: 'btn primary', onclick: () => go(next.hash) }, 'Continue → ', h('small', null, next.label)) : null,
        h('button', { class: 'btn', onclick: () => go(`#/course/${slug}/review`), disabled: !card.due_cards }, `Flashcards${card.due_cards ? ` · ${card.due_cards} due` : ''}`),
        h('p', { class: 'muted small' }, `Studied ${fmtMinutes(p.seconds_studied || 0)}`)
      )
    ),
    dlPanel(),
    h('h2', null, 'Modules'),
    h('div', { class: 'modules' }, c.modules.map(moduleCard)),
    cap?.id
      ? h(
          'article',
          { class: 'module capstone' },
          h('div', { class: 'module-num' }, '★'),
          h('div', { class: 'module-body' }, h('h3', null, `Capstone · ${cap.title}`), h('p', { class: 'muted' }, cap.premise), data.ready_sims.includes(cap.id) ? h('button', { class: `step-btn sim ${get(p.sims, cap.id)?.completed ? 'done' : ''}`, onclick: () => go(`#/course/${slug}/sim/${cap.id}`) }, get(p.sims, cap.id)?.completed ? '✓ Completed' : '▶ Start capstone') : h('p', { class: 'small muted' }, 'Being built…'))
        )
      : null,
    c.skills.length
      ? h(
          'section',
          { class: 'skills' },
          h('h2', null, 'Skills'),
          c.skills.map((s) => h('div', { class: 'skill-row' }, h('span', null, s.name), bar(get(p.mastery, s.id) || 0), h('span', { class: 'muted small' }, pct(get(p.mastery, s.id) || 0))))
        )
      : null,
    c.out_of_scope?.length ? h('p', { class: 'muted small' }, h('b', null, 'Deliberately later: '), c.out_of_scope.join(' · ')) : null,
    h('div', { class: 'row danger-zone' }, h('button', { class: 'btn ghost small', onclick: () => confirmDelete(c) }, 'Delete course'))
  );
}

function confirmDelete(c) {
  modal('Delete this course?', h('p', null, `“${c.title}” and your progress will be removed from this computer.`), [
    { label: 'Cancel' },
    {
      label: 'Delete',
      primary: true,
      onClick: () => {
        call('delete_course', { slug: c.slug }).then(() => go('#/'), (e) => toast(String(e), 'bad'));
      },
    },
  ]);
}

// -------------------------------------------------------------- lesson ----

async function lessonPage(slug, id) {
  loading();
  const [data, m] = await Promise.all([call('get_course', { slug }), call('get_module', { slug, id })]);
  const c = data.card.course;
  const info = c.modules.find((x) => x.id === id);
  const opened = Date.now();
  const finish = async () => {
    await call('mark_read', { slug, module: id, seconds: Math.round((Date.now() - opened) / 1000) });
    go(`#/course/${slug}/quiz/${id}`);
  };
  page(
    h('a', { class: 'back', href: `#/course/${slug}` }, `← ${c.title}`),
    h(
      'article',
      { class: 'lesson' },
      h('p', { class: 'step' }, `Module ${c.modules.indexOf(info) + 1} · ${info?.minutes || ''} min`),
      h('h1', null, m.title),
      info?.objectives?.length ? h('div', { class: 'objectives-box' }, h('b', null, 'After this module you can'), h('ul', null, info.objectives.map((o) => h('li', null, o)))) : null,
      renderMarkdown(m.lesson),
      m.key_points?.length ? h('div', { class: 'key-points' }, h('h3', null, 'Key points'), h('ul', null, m.key_points.map((k) => h('li', null, k)))) : null,
      h('div', { class: 'lesson-end' }, h('button', { class: 'btn primary big', onclick: finish }, 'Done reading → Quiz'), askButton(slug, `Lesson: ${m.title}`))
    )
  );
}

function askButton(slug, context) {
  if (!isDesktop) return null;
  return h(
    'button',
    {
      class: 'btn ghost ai',
      onclick: () => {
        const q = h('textarea', { class: 'input', rows: 3, placeholder: 'Ask anything about this…' });
        const answer = h('div', { class: 'ai-answer' });
        modal('Ask Guru', h('div', null, h('p', { class: 'muted small' }, 'Uses AI (one Codex request).'), q, answer), [
          { label: 'Close' },
          {
            label: 'Ask',
            primary: true,
            onClick: () => {
              if (!q.value.trim()) return false;
              clear(answer).append(h('span', { class: 'spinner small' }), ' Thinking…');
              call('ask_guru', { slug, context, question: q.value.trim() }).then(
                (t) => clear(answer).append(renderMarkdown(t)),
                (e) => clear(answer).append(String(e))
              );
              return false;
            },
          },
        ]);
      },
    },
    '✦ Ask Guru (AI)'
  );
}

// ---------------------------------------------------------------- quiz ----

async function quizPage(slug, id) {
  loading();
  const [data, m] = await Promise.all([call('get_course', { slug }), call('get_module', { slug, id })]);
  const c = data.card.course;
  const info = c.modules.find((x) => x.id === id);
  const qs = m.quiz || [];
  let i = 0;
  const results = [];
  const box = h('div', { class: 'quiz-box' });

  const show = () => {
    clear(box);
    if (i >= qs.length) return done();
    const q = qs[i];
    const feedback = h('div', { class: 'feedback' });
    let read;
    let control;
    if (q.kind === 'text') {
      control = h('input', { class: 'input', placeholder: 'Type your answer', onkeydown: (e) => e.key === 'Enter' && check() });
      read = () => control.value.trim();
    } else {
      const multi = q.kind === 'multi';
      control = h('div', { class: 'options' }, q.options.map((o, k) => h('label', { class: 'option' }, h('input', { type: multi ? 'checkbox' : 'radio', name: 'q', value: k }), h('span', null, o))));
      read = () => [...control.querySelectorAll('input:checked')].map((x) => Number(x.value));
    }
    const checkBtn = h('button', { class: 'btn primary', onclick: () => check() }, 'Check');
    const check = () => {
      const v = read();
      let correct;
      if (q.kind === 'text') correct = !!v && (q.accept || []).some((p) => { try { return new RegExp(p, 'i').test(v); } catch { return false; } });
      else correct = [...new Set(v)].sort().join() === [...new Set(q.correct)].sort().join();
      results.push({ skill: q.skill_id, correct });
      clear(feedback).append(h('b', null, correct ? '✓ Correct. ' : '✗ Not quite. '), q.kind === 'text' && !correct ? h('span', null, 'Accepted answer pattern: ', h('code', null, (q.accept || [])[0] || '')) : null, h('p', null, q.explanation));
      feedback.className = `feedback ${correct ? 'ok' : 'bad'}`;
      if (q.kind !== 'text') control.querySelectorAll('label').forEach((l, k) => { if (q.correct.includes(k)) l.classList.add('right'); });
      control.querySelectorAll('input').forEach((x) => (x.disabled = true));
      checkBtn.replaceWith(h('button', { class: 'btn primary', onclick: () => { i++; show(); } }, i + 1 < qs.length ? 'Next →' : 'See results'));
    };
    append(box, [h('p', { class: 'step' }, `Question ${i + 1} of ${qs.length}`), h('h2', null, q.prompt), control, h('div', { class: 'row' }, checkBtn), feedback]);
    box.querySelector('input')?.focus();
  };

  const done = async () => {
    const score = results.length ? results.filter((r) => r.correct).length / results.length : 0;
    const per = {};
    for (const r of results) (per[r.skill] ||= []).push(r.correct ? 1 : 0);
    const perSkill = Object.entries(per).map(([s, v]) => [s, v.reduce((a, b) => a + b, 0) / v.length]);
    await call('record_quiz', { slug, module: id, perSkill, score });
    const sim = info?.sims.find((s) => data.ready_sims.includes(s.id));
    append(box, [
      h('div', { class: 'score' }, pct(score)),
      h('p', { class: 'lead center' }, score >= 0.8 ? 'Solid. Now apply it.' : 'Good effort. The simulation will make it stick, and the missed ideas come back as flashcards.'),
      h('div', { class: 'row center' }, sim ? h('button', { class: 'btn primary big', onclick: () => go(`#/course/${slug}/sim/${sim.id}`) }, `▶ Simulation: ${sim.title}`) : null, h('button', { class: 'btn', onclick: () => go(`#/course/${slug}`) }, 'Back to course'), h('button', { class: 'btn ghost', onclick: () => { i = 0; results.length = 0; show(); } }, 'Retry quiz')),
    ]);
  };

  page(h('a', { class: 'back', href: `#/course/${slug}` }, `← ${c.title}`), h('h1', null, `Quiz · ${m.title}`), qs.length ? box : h('p', null, 'This module has no quiz.'));
  if (qs.length) show();
}

// -------------------------------------------------------------- review ----

async function reviewPage(slug) {
  loading();
  const [data, cards] = await Promise.all([call('get_course', { slug }), call('due_cards', { slug, limit: 30 })]);
  const c = data.card.course;
  const queue = [...cards];
  let reviewed = 0;
  const box = h('div', { class: 'flashcard-box' });
  const show = () => {
    clear(box);
    const card = queue.shift();
    if (!card) {
      append(box, [h('div', { class: 'score' }, '✓'), h('p', { class: 'lead center' }, reviewed ? `${reviewed} cards reviewed. They come back just before you would forget them.` : 'Nothing due. Come back tomorrow.'), h('div', { class: 'row center' }, h('button', { class: 'btn primary', onclick: () => go(`#/course/${slug}`) }, 'Back to course'))]);
      return;
    }
    const back = h('div', { class: 'card-back', hidden: true }, card.back);
    const grades = h(
      'div',
      { class: 'grades', hidden: true },
      [
        [1, 'Forgot'],
        [2, 'Hard'],
        [3, 'Good'],
        [4, 'Easy'],
      ].map(([g, label]) => h('button', { class: `btn grade g${g}`, onclick: () => grade(g) }, h('kbd', null, String(g)), ' ', label))
    );
    const reveal = h('button', { class: 'btn primary', onclick: () => flip() }, 'Show answer');
    const flip = () => {
      back.hidden = false;
      grades.hidden = false;
      reveal.remove();
    };
    const grade = async (g) => {
      document.removeEventListener('keydown', keys);
      const again = await call('grade_card', { slug, id: card.id, grade: g });
      reviewed++;
      if (again) queue.push(card);
      show();
    };
    const keys = (e) => {
      if (!back.hidden && ['1', '2', '3', '4'].includes(e.key)) grade(Number(e.key));
      else if (back.hidden && (e.key === ' ' || e.key === 'Enter')) {
        e.preventDefault();
        flip();
      }
    };
    document.addEventListener('keydown', keys, { once: false });
    append(box, [h('p', { class: 'step' }, `${queue.length + 1} left`), h('div', { class: 'flashcard' }, h('div', { class: 'card-front' }, card.front), back), h('p', { class: 'muted small center' }, 'Recall it before you reveal: the effort is what makes it stick.'), h('div', { class: 'row center' }, reveal), grades]);
  };
  page(h('a', { class: 'back', href: `#/course/${slug}` }, `← ${c.title}`), h('h1', null, 'Flashcards'), box);
  show();
}

// ----------------------------------------------------------------- sim ----

async function simPage(slug, id) {
  loading('Loading simulation…');
  const [data, spec] = await Promise.all([call('get_course', { slug }), call('get_sim', { slug, id })]);
  document.body.classList.add('immersive');
  clear(view);
  const root = h('div', { class: 'sim-root' });
  view.append(root);
  renderSimPlayer(root, spec, {
    aiAvailable: isDesktop && !!state.status?.logged_in,
    onAskAI: (hostname, history, command) => call('ai_command', { slug, simId: id, hostname, history, command }),
    onExit: async (result) => {
      await call('record_sim', { slug, simId: id, skills: spec.skill_ids || [], result });
      document.body.classList.remove('immersive');
      go(`#/course/${slug}`);
    },
  });
  void data;
}

// ------------------------------------------------------------ settings ----

async function settingsPage() {
  loading();
  const st = (state.status = await call('app_status'));
  const cfg = { ...st.config, parallel_jobs_text: st.config.parallel_jobs ? String(st.config.parallel_jobs) : '' };
  const m = st.machine;
  const b = st.budget;
  const loginOut = h('div', { class: 'small' });
  const signIn = async () => {
    clear(loginOut).append(h('span', { class: 'spinner small' }), ' Opening your browser to sign in with ChatGPT…');
    await call('start_login');
  };
  const text = (key, label, placeholder) => h('label', { class: 'field' }, h('span', null, label), h('input', { class: 'input', value: cfg[key] ?? '', placeholder, oninput: (e) => (cfg[key] = e.target.value.trim() || null) }));
  const toggle = (key, label) => h('label', { class: 'toggle' }, h('input', { type: 'checkbox', checked: cfg[key], onchange: (e) => (cfg[key] = e.target.checked) }), h('span', null, label));
  page(
    h('h1', null, 'Settings'),
    h(
      'section',
      { class: 'panel' },
      h('h2', null, 'AI account'),
      st.codex_version ? h('p', null, '✓ Codex CLI ', h('code', null, st.codex_version)) : h('p', { class: 'warn' }, 'Codex CLI not found. Install it (npm i -g @openai/codex) or set its path below.'),
      h('p', null, st.logged_in ? `✓ ${st.login_message}` : st.login_message || 'Not signed in.'),
      !st.logged_in && st.codex_version ? h('button', { class: 'btn primary', onclick: signIn }, 'Sign in with ChatGPT') : null,
      loginOut,
      h('p', { class: 'muted small' }, 'Guru uses AI only to build courses and when you press an “Ask AI” button. Studying a downloaded course needs none.')
    ),
    h(
      'section',
      { class: 'panel' },
      h('h2', null, 'This computer'),
      h('p', null, `${m.cpus} CPU threads · ${m.total_ram_mb} MB RAM (${m.avail_ram_mb} MB free) · ${m.free_disk_mb ?? '?'} MB disk free`),
      h('p', { class: 'muted small' }, `Course building runs ${cfg.parallel_jobs || (m.avail_ram_mb >= 8000 ? 3 : m.avail_ram_mb >= 3000 ? 2 : 1)} Codex worker(s) in parallel, sized to free memory. Simulations run inside Guru, so practising needs no extra RAM, containers or installs. Resource budget for anything heavier: ≤${b.max_ram_mb} MB RAM, ≤${b.max_disk_mb} MB disk.`)
    ),
    h(
      'section',
      { class: 'panel' },
      h('h2', null, 'Course building'),
      h('div', { class: 'form-grid' }, text('model', 'Model', 'Codex default'), text('reasoning_effort', 'Reasoning effort', 'Codex default (e.g. medium, high)'), text('parallel_jobs_text', 'Parallel builders', 'auto'), text('codex_bin', 'Codex path', 'codex (from PATH)')),
      toggle('web_search', 'Let Codex search the web for current versions and real-world issues'),
      toggle('stackexchange', 'Collect real problems from Stack Overflow / Stack Exchange'),
      h(
        'div',
        { class: 'row' },
        h(
          'button',
          {
            class: 'btn primary',
            onclick: async () => {
              const n = parseInt(cfg.parallel_jobs_text ?? cfg.parallel_jobs ?? '', 10);
              const out = { ...cfg, parallel_jobs: Number.isFinite(n) ? Math.min(6, Math.max(1, n)) : null };
              delete out.parallel_jobs_text;
              await call('save_settings', { config: out });
              toast('Settings saved', 'ok');
            },
          },
          'Save'
        )
      ),
      h('p', { class: 'muted small' }, `Courses are stored in ${st.courses_dir}`)
    )
  );
}

// -------------------------------------------------------------- router ----

const routes = [
  [/^#?\/?$/, () => library()],
  [/^#\/new$/, () => newCourse()],
  [/^#\/settings$/, () => settingsPage()],
  [/^#\/course\/([\w-]+)$/, (m) => coursePage(m[1])],
  [/^#\/course\/([\w-]+)\/lesson\/([\w-]+)$/, (m) => lessonPage(m[1], m[2])],
  [/^#\/course\/([\w-]+)\/quiz\/([\w-]+)$/, (m) => quizPage(m[1], m[2])],
  [/^#\/course\/([\w-]+)\/review$/, (m) => reviewPage(m[1])],
  [/^#\/course\/([\w-]+)\/sim\/([\w-]+)$/, (m) => simPage(m[1], m[2])],
];

async function route() {
  document.body.classList.remove('immersive');
  const hash = location.hash || '#/';
  for (const a of document.querySelectorAll('.nav a')) a.classList.toggle('active', a.getAttribute('href') === hash || (a.getAttribute('href') === '#/' && hash.startsWith('#/course')));
  for (const [rx, fn] of routes) {
    const m = hash.match(rx);
    if (m) {
      try {
        await fn(m);
      } catch (e) {
        page(h('div', { class: 'notice warn' }, h('b', null, 'Something went wrong. '), String(e?.message || e)), h('a', { href: '#/' }, '← Back to your courses'));
      }
      return;
    }
  }
  go('#/');
}

async function refreshStatus() {
  try {
    state.status = await call('app_status');
  } catch {
    return;
  }
  const el = document.getElementById('account');
  clear(el).append(state.status.logged_in ? h('span', { class: 'dot ok' }) : h('span', { class: 'dot' }), state.status.logged_in ? 'AI ready' : isDesktop ? 'AI: sign in' : 'Browser preview');
}

let rerender;
listen('download', (ev) => {
  state.downloads[ev.slug] = ev;
  const hash = location.hash || '#/';
  if (ev.phase === 'ready') toast(`${ev.message}`, 'ok');
  if (ev.phase === 'failed') toast(`Download stopped: ${ev.message.slice(0, 160)}`, 'bad');
  const relevant = hash === '#/' || hash === '' || hash === `#/course/${ev.slug}`;
  if (!relevant) return;
  clearTimeout(rerender);
  rerender = setTimeout(route, ev.phase === 'building' && ev.activity ? 1200 : 150);
});
listen('login-url', (url) => {
  toast('Finish signing in in your browser.', 'ok');
  const box = document.querySelector('.panel .small');
  if (box) clear(box).append('If no browser opened, visit: ', h('a', { href: '#', onclick: (e) => (e.preventDefault(), call('open_url', { url })) }, url));
});
listen('login-done', async (ok) => {
  toast(ok ? 'Signed in with ChatGPT.' : 'Sign-in did not complete.', ok ? 'ok' : 'bad');
  await refreshStatus();
  if (location.hash === '#/settings') settingsPage();
});

window.addEventListener('hashchange', route);
refreshStatus();
route();
