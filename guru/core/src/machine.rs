//! Machine awareness. Guru measures the learner's computer once per session and
//! turns it into a hard *lab budget* that every generated environment must fit
//! inside, so practice never makes the machine swap, freeze or fill its disk.

use std::path::Path;
use std::process::Command;

use serde::Deserialize;
use serde::Serialize;

/// Tools whose presence changes how a lab should be designed.
const TOOLS: &[&str] = &[
    "git",
    "curl",
    "docker",
    "podman",
    "kubectl",
    "kind",
    "k3d",
    "minikube",
    "java",
    "mvn",
    "gradle",
    "node",
    "npm",
    "python3",
    "pip3",
    "go",
    "rustc",
    "cargo",
    "dotnet",
    "gcc",
    "make",
    "psql",
    "mysql",
    "redis-server",
    "terraform",
    "ansible",
    "nginx",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MachineProfile {
    pub os: String,
    pub arch: String,
    pub cpus: u32,
    pub total_ram_mb: u64,
    pub avail_ram_mb: u64,
    pub free_disk_mb: Option<u64>,
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Under ~1.5 GB to spare: native processes only, simulations over services.
    Small,
    /// Up to ~4 GB: single lightweight containers/services, no clusters.
    Medium,
    /// More: multi-service compose stacks, a single-node local cluster.
    Large,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LabBudget {
    pub tier: Tier,
    pub max_ram_mb: u64,
    pub max_disk_mb: u64,
    pub max_cpus: u32,
    pub containers: bool,
    pub local_cluster: bool,
}

impl MachineProfile {
    pub fn detect(data_dir: &Path) -> Self {
        let (total_ram_mb, avail_ram_mb) = detect_ram().unwrap_or((4096, 2048));
        Self {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            cpus: std::thread::available_parallelism()
                .map(|n| n.get() as u32)
                .unwrap_or(2),
            total_ram_mb,
            avail_ram_mb,
            free_disk_mb: detect_free_disk_mb(data_dir),
            tools: TOOLS
                .iter()
                .filter(|t| which(t).is_some())
                .map(|t| t.to_string())
                .collect(),
        }
    }

    pub fn has(&self, tool: &str) -> bool {
        self.tools.iter().any(|t| t == tool)
    }

    pub fn budget(&self) -> LabBudget {
        // Leave the OS, the editor/browser the learner reads in, and Codex itself
        // comfortably alive: never plan to use more than half of what is free or
        // 40% of the machine, whichever is smaller.
        let max_ram_mb = ((self.avail_ram_mb as f64) * 0.5)
            .min((self.total_ram_mb as f64) * 0.4)
            .max(256.0) as u64;
        let tier = if max_ram_mb < 1536 {
            Tier::Small
        } else if max_ram_mb < 4096 {
            Tier::Medium
        } else {
            Tier::Large
        };
        let max_disk_mb = self
            .free_disk_mb
            .map(|d| (d / 5).min(10 * 1024))
            .unwrap_or(2048);
        let runtime = self.has("docker") || self.has("podman");
        LabBudget {
            tier,
            max_ram_mb,
            max_disk_mb,
            max_cpus: (self.cpus / 2).max(1),
            containers: runtime && tier != Tier::Small && max_disk_mb >= 2048,
            local_cluster: runtime
                && tier == Tier::Large
                && (self.has("kind") || self.has("k3d") || self.has("minikube")),
        }
    }

    /// The constraint block every environment-building prompt carries.
    pub fn constraints_prompt(&self) -> String {
        let b = self.budget();
        let mut s = String::new();
        s.push_str(&format!(
            "Learner machine: {} {} | {} CPU threads | RAM total {} MB, available {} MB | free disk {}\n",
            self.os,
            self.arch,
            self.cpus,
            self.total_ram_mb,
            self.avail_ram_mb,
            self.free_disk_mb
                .map(|d| format!("{d} MB"))
                .unwrap_or_else(|| "unknown".into())
        ));
        s.push_str(&format!("Installed tools: {}\n", self.tools.join(", ")));
        s.push_str(&format!(
            "HARD LAB BUDGET (tier {:?}): at most {} MB RAM, {} MB disk, {} CPU threads in total for everything the lab runs at once.\n",
            b.tier, b.max_ram_mb, b.max_disk_mb, b.max_cpus
        ));
        s.push_str(if b.containers {
            "Containers are allowed (always pass explicit --memory/--cpus limits; prefer alpine/slim/distroless images; reuse one image across tasks).\n"
        } else {
            "Do NOT use containers: run things natively or simulate them with lightweight stand-ins.\n"
        });
        s.push_str(if b.local_cluster {
            "A single-node local Kubernetes cluster (kind/k3d/minikube) is allowed, one node only, with resource limits on every workload.\n"
        } else {
            "Do NOT create Kubernetes clusters; teach cluster concepts with manifests validated offline, kubectl --dry-run=client, or simulated output.\n"
        });
        s.push_str(match b.tier {
            Tier::Small => "Prefer interpreted/native tools already installed; cap JVMs at -Xmx256m, Node at --max-old-space-size=256; avoid IDE-scale tooling.\n",
            Tier::Medium => "Cap JVMs at -Xmx512m and Node at --max-old-space-size=512; at most two services running at the same time.\n",
            Tier::Large => "Cap every JVM/Node/DB process explicitly; never exceed the budget even though the machine is large.\n",
        });
        s.push_str("Everything started must be stopped by the teardown script; nothing may keep running after the session.\n");
        s
    }
}

pub fn which(tool: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT".into())
            .split(';')
            .map(|e| e.to_lowercase())
            .chain(std::iter::once(String::new()))
            .collect()
    } else {
        vec![String::new()]
    };
    for dir in std::env::split_paths(&path) {
        for ext in &exts {
            let candidate = dir.join(format!("{tool}{ext}"));
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}

fn detect_ram() -> Option<(u64, u64)> {
    if cfg!(target_os = "linux") {
        let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
        let (mut total, mut avail) = parse_meminfo(&meminfo)?;
        // Inside containers / WSL / systemd slices the cgroup limit is the real ceiling.
        if let Some(limit) = cgroup_limit_mb() {
            total = total.min(limit);
            avail = avail.min(limit);
        }
        return Some((total, avail));
    }
    if cfg!(target_os = "macos") {
        let total = run("sysctl", &["-n", "hw.memsize"])?
            .trim()
            .parse::<u64>()
            .ok()?
            / 1_048_576;
        let vm = run("vm_stat", &[])?;
        let avail = parse_vm_stat(&vm).unwrap_or(total / 2);
        return Some((total, avail));
    }
    if cfg!(windows) {
        let out = run(
            "powershell",
            &[
                "-NoProfile",
                "-Command",
                "$o=Get-CimInstance Win32_OperatingSystem; \"$($o.TotalVisibleMemorySize) $($o.FreePhysicalMemory)\"",
            ],
        )?;
        let mut it = out.split_whitespace().filter_map(|v| v.parse::<u64>().ok());
        return Some((it.next()? / 1024, it.next()? / 1024));
    }
    None
}

pub fn parse_meminfo(text: &str) -> Option<(u64, u64)> {
    let field = |name: &str| {
        text.lines()
            .find(|l| l.starts_with(name))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<u64>().ok())
    };
    let total = field("MemTotal:")? / 1024;
    let avail = field("MemAvailable:")
        .or_else(|| field("MemFree:"))
        .map(|kb| kb / 1024)
        .unwrap_or(total / 2);
    Some((total, avail))
}

fn cgroup_limit_mb() -> Option<u64> {
    let raw = std::fs::read_to_string("/sys/fs/cgroup/memory.max")
        .or_else(|_| std::fs::read_to_string("/sys/fs/cgroup/memory/memory.limit_in_bytes"))
        .ok()?;
    let bytes: u64 = raw.trim().parse().ok()?;
    // "max" fails to parse; absurdly large values mean "unlimited".
    (bytes < (1 << 50)).then_some(bytes / 1_048_576)
}

fn parse_vm_stat(text: &str) -> Option<u64> {
    let page: u64 = text
        .lines()
        .next()
        .and_then(|l| l.split("page size of ").nth(1))
        .and_then(|r| r.split_whitespace().next())
        .and_then(|v| v.parse().ok())
        .unwrap_or(4096);
    let pages = |name: &str| -> u64 {
        text.lines()
            .find(|l| l.starts_with(name))
            .and_then(|l| l.split(':').nth(1))
            .and_then(|v| v.trim().trim_end_matches('.').parse().ok())
            .unwrap_or(0)
    };
    let free = pages("Pages free") + pages("Pages inactive") + pages("Pages speculative");
    (free > 0).then_some(free * page / 1_048_576)
}

fn detect_free_disk_mb(dir: &Path) -> Option<u64> {
    if cfg!(windows) {
        return None;
    }
    let mut probe = dir.to_path_buf();
    while !probe.exists() {
        probe = probe.parent()?.to_path_buf();
    }
    let out = run("df", &["-Pk", probe.to_str()?])?;
    parse_df(&out)
}

pub fn parse_df(text: &str) -> Option<u64> {
    let line = text.lines().nth(1)?;
    let avail_kb: u64 = line.split_whitespace().nth(3)?.parse().ok()?;
    Some(avail_kb / 1024)
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(total: u64, avail: u64, tools: &[&str]) -> MachineProfile {
        MachineProfile {
            os: "linux".into(),
            arch: "x86_64".into(),
            cpus: 8,
            total_ram_mb: total,
            avail_ram_mb: avail,
            free_disk_mb: Some(50_000),
            tools: tools.iter().map(|t| t.to_string()).collect(),
        }
    }

    #[test]
    fn small_laptop_gets_no_containers() {
        let b = profile(4096, 1800, &["docker"]).budget();
        assert_eq!(b.tier, Tier::Small);
        assert_eq!(b.max_ram_mb, 900);
        assert!(!b.containers);
        assert!(!b.local_cluster);
    }

    #[test]
    fn mid_machine_gets_containers_but_no_cluster() {
        let b = profile(16_384, 6000, &["docker", "kind"]).budget();
        assert_eq!(b.tier, Tier::Medium);
        assert_eq!(b.max_ram_mb, 3000);
        assert!(b.containers);
        assert!(!b.local_cluster);
    }

    #[test]
    fn big_machine_may_run_a_single_node_cluster() {
        let b = profile(32_768, 24_000, &["podman", "k3d"]).budget();
        assert_eq!(b.tier, Tier::Large);
        assert!(b.containers && b.local_cluster);
        assert_eq!(b.max_cpus, 4);
        assert_eq!(b.max_disk_mb, 10_000);
    }

    #[test]
    fn no_runtime_means_no_containers_even_when_large() {
        let b = profile(65_536, 60_000, &[]).budget();
        assert!(!b.containers);
        assert!(profile(65_536, 60_000, &[])
            .constraints_prompt()
            .contains("Do NOT use containers"));
    }

    #[test]
    fn parses_meminfo_and_df() {
        let mi = "MemTotal:       16303428 kB\nMemFree:         1000000 kB\nMemAvailable:    8151714 kB\n";
        assert_eq!(parse_meminfo(mi), Some((15921, 7960)));
        let df = "Filesystem 1024-blocks Used Available Capacity Mounted on\n/dev/sda1 100000000 50000000 51200000 50% /\n";
        assert_eq!(parse_df(df), Some(50_000));
    }

    #[test]
    fn parses_vm_stat() {
        let vm = "Mach Virtual Memory Statistics: (page size of 16384 bytes)\nPages free:        100000.\nPages inactive:     28000.\n";
        assert_eq!(parse_vm_stat(vm), Some(2000));
    }
}
