// Run with: node --test ui/js/sim/
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

import { SimEngine, splitPipes, tokenize } from './engine.js';

const demo = (name) => JSON.parse(readFileSync(new URL(`../../demo/sims/${name}.json`, import.meta.url)));

test('tokenizer and pipes respect quotes', () => {
  assert.deepEqual(tokenize(`grep -i "failed password" /var/log/auth.log`), ['grep', '-i', 'failed password', '/var/log/auth.log']);
  assert.deepEqual(tokenize(`echo ''`), ['echo', '']);
  assert.deepEqual(splitPipes(`awk '{print $1}' f | sort | uniq -c`), [`awk '{print $1}' f`, 'sort', 'uniq -c']);
  assert.deepEqual(splitPipes(`grep "a|b" f`), [`grep "a|b" f`]);
});

test('the infected server can be investigated and contained end to end', () => {
  const e = new SimEngine(demo('infected-web-01'));
  const run = (cmd) => e.exec('web01', cmd);
  const toasts = [];
  e.on((kind, data) => kind === 'toast' && toasts.push(data));

  assert.match(run('top').output, /4242 root\s+389\.7/);
  assert.match(run('ps aux').output, /\/tmp\/\.x\/kworkerds/);
  assert.match(run('ls -l /proc/4242/exe').output, /-> \/tmp\/\.x\/kworkerds/);
  assert.match(run('sudo ss -tnp').output, /192\.0\.2\.77:3333/);

  const accepted = run('grep Accepted /var/log/auth.log').output;
  assert.match(accepted, /Accepted password for deploy from 203\.0\.113\.45/);
  assert.ok(e.flags.has('read:/var/log/auth.log'));
  const failures = Number(run('cat /var/log/auth.log | grep -c "Failed password"').output);
  assert.equal(failures, 46);
  const top = run(`grep Failed /var/log/auth.log | awk '{print $13}' | sort | uniq -c | sort -rn | head -1`).output;
  assert.match(top, /46 203\.0\.113\.45/);

  assert.match(run('ls /etc/cron.d').output, /sysupdate/);
  assert.match(run('ls -a /tmp').output, /\.x\//);
  run('cd /tmp/.x');
  assert.equal(run('pwd').output, '/tmp/.x');
  assert.match(e.hosts.web01.prompt, /root@web-01:\/tmp\/\.x#/);
  assert.match(run('cat x.sh').output, /cron\.d\/sysupdate/);
  assert.match(run('foo --bar').output, /command not found/);
  assert.equal(run('foo').notFound, true);

  assert.equal(e.answer('o1', [1]).correct, true);
  assert.equal(e.answer('o2', 'it was 10.0.0.1').correct, false);
  assert.equal(e.answer('o2', ' 203.0.113.45 ').correct, true);
  assert.equal(e.answer('o3', [1]).correct, true);

  run('kill -9 4242');
  assert.ok(e.flags.has('killed:4242') && e.flags.has('killed:kworkerds'));
  assert.doesNotMatch(run('ps aux').output, /kworkerds/);
  assert.match(run('kill 4242').output, /No such process/);

  run('rm /etc/cron.d/sysupdate');
  assert.match(run('rm /tmp/.x').output, /Is a directory/);
  run('cd /');
  run('rm -rf /tmp/.x');
  assert.ok(e.flags.has('removed:/tmp/.x') && e.flags.has('removed:/tmp/.x/kworkerds'));
  assert.equal(e.progress().complete, false);

  run('sudo iptables -I INPUT -s 203.0.113.45 -j DROP');
  assert.ok(toasts.some((t) => /dropped/.test(t)));
  const p = e.progress();
  assert.deepEqual([p.done, p.total, p.complete], [6, 6, true]);
  assert.equal(e.result().completed, true);
});

test('systemctl, journalctl, redirection and completion', () => {
  const e = new SimEngine(demo('infected-web-01'));
  const run = (cmd) => e.exec('web01', cmd);
  assert.match(run('systemctl status nginx').output, /active \(running\)/);
  run('systemctl stop nginx');
  assert.ok(e.flags.has('stopped:nginx'));
  assert.match(run('systemctl status nginx').output, /inactive/);
  assert.match(run('systemctl status nope').output, /could not be found/);
  assert.match(run('journalctl -u syslog -n 3').output, /logrotate/);
  run('echo "PasswordAuthentication no" >> /etc/ssh/sshd_config');
  assert.match(run('tail -n 1 /etc/ssh/sshd_config').output, /^PasswordAuthentication no$/);
  assert.equal(e.complete('web01', 'cat /etc/ssh/ss'), 'cat /etc/ssh/sshd_config ');
  assert.equal(e.complete('web01', 'journ'), 'journalctl ');
  assert.match(run('find / -name "*.sh"').output, /\/tmp\/\.x\/x\.sh/);
});

test('the simulated Grafana: data source, queries, alert rule', () => {
  const spec = demo('grafana-alerting');
  const e = new SimEngine(spec);
  const widget = (screen, type) => spec.apps[0].screens.find((s) => s.id === screen).widgets.find((w) => w.type === type);

  const ds = widget('datasources', 'form');
  assert.equal(e.submitForm(ds, { type: 'Prometheus', url: 'http://localhost:9090' }).ok, false);
  assert.match(e.submitForm(ds, { type: 'Prometheus', url: 'http://localhost:9090' }).message, /connection refused/);
  assert.equal(e.submitForm(ds, { type: 'Prometheus', url: 'http://prometheus:9090' }).ok, true);

  const q = widget('explore', 'query');
  assert.equal(e.query(q, 'rate(http_requests_total)').result, 'error');
  assert.equal(e.query(q, 'sum(rate(http_requests_total[5m]))').result, 'timeseries');
  assert.equal(e.query(q, 'sum by (route) (rate(http_requests_total[1m]))').result, 'timeseries');
  e.query(q, 'sum(rate(http_requests_total{status=~"5.."}[5m])) / sum(rate(http_requests_total[5m]))');
  assert.equal(e.query(q, 'nonsense').result, 'error');
  assert.equal(e.answer('g4', [2]).correct, true);

  const alert = widget('alerting', 'form');
  const base = { name: 'High 5xx', query: 'sum(rate(http_requests_total{status=~"5.."}[5m])) / sum(rate(http_requests_total[5m]))', op: 'IS ABOVE', threshold: '0.05' };
  assert.match(e.submitForm(alert, { ...base, for: '0s' }).message, /pending period/);
  assert.equal(e.submitForm(alert, { ...base, for: '5m' }).ok, true);
  assert.equal(e.nodeStatus.api, 'degraded');
  assert.deepEqual(e.progress(), { done: 5, total: 5, complete: true });
});

test('editor checks and chat turns', () => {
  const e = new SimEngine({ world: { nodes: [] }, objectives: [] });
  const editor = {
    checks: [
      { must_match: true, pattern: '^PasswordAuthentication no$', message: 'disable passwords' },
      { must_match: false, pattern: '^PermitRootLogin yes', message: 'no root login' },
    ],
    effects: { set_flags: ['fixed:ssh'], node_status: [], message: 'ok' },
  };
  assert.equal(e.checkEditor(editor, 'PermitRootLogin yes\nPasswordAuthentication no').passed, false);
  assert.equal(e.checkEditor(editor, 'PermitRootLogin no\nPasswordAuthentication no').passed, true);
  assert.ok(e.flags.has('fixed:ssh'));

  const chat = {
    id: 'c',
    fallback: '¿Perdón?',
    turns: [
      { expect: 'hola|buenos', reply: '¡Hola! ¿Qué desea?', feedback: '', effects: { set_flags: ['greeted'], node_status: [], message: '' } },
      { expect: 'quisiera|quiero', reply: 'Claro.', feedback: 'Quisiera is more polite than quiero.', effects: { set_flags: [], node_status: [], message: '' } },
    ],
  };
  assert.equal(e.chat(chat, 'Hola, buenas tardes').reply, '¡Hola! ¿Qué desea?');
  assert.ok(e.flags.has('greeted'));
  assert.equal(e.chat(chat, 'quiero un café').feedback, 'Quisiera is more polite than quiero.');
  assert.equal(e.chat(chat, 'xyz').reply, '¿Perdón?');
});
