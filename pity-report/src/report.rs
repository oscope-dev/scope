use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Display;
use std::fs::File;
use std::io::Write;
use std::ops::Add;
use sysinfo::{PidExt, Process, ProcessExt, System, SystemExt};
use tokio::sync::{RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use chrono::{DateTime, Utc};
use ptree::TreeItem;
use tracing::info;


#[derive(Debug, Clone)]
pub(crate) struct ProcessDetails {
    pid: u32,
    command: String,
    args: Vec<String>,
    env: Vec<String>,
    parent_pid: u32,
    start_time: SystemTime,
    end_time: SystemTime,
}

#[derive(Debug, Default)]
pub(crate) struct DataCapture {
    start_time: DateTime<Utc>,
    process_details: RwLock<BTreeMap<u32, ProcessDetails>>,
    kernel_version: String,
    stdout: RwLock<Vec<(DateTime<Utc>, String)>>,
    stderr: RwLock<Vec<(DateTime<Utc>, String)>>,
}

#[derive(Clone)]
struct ProcessTree {
    text: String,
    children: Vec<ProcessTree>
}

impl ProcessTree {
    fn new(process: &ProcessDetails) -> Self {
        Self {
            text: format!("({}) - {} {}", process.pid, process.command, process.args.join(" ")),
            children: Default::default()
        }
    }

    fn make_root() -> Self {
        Self {
            text: format!("pity"),
            children: Default::default(),
        }
    }

    fn add_child(&mut self, child: ProcessTree) {
        self.children.push(child);
    }
}

impl TreeItem for ProcessTree {
    type Child = Self;
    fn write_self<W: std::io::Write>(&self, f: &mut W, style: &ptree::Style) -> std::io::Result<()> {
        write!(f, "{}", style.paint(&self.text))
    }
    fn children(&self) -> Cow<[Self::Child]> {
        Cow::from(self.children.clone())
    }
}

struct ReportBuilder {
    processes: BTreeMap<u32, ProcessDetails>,
    processes_by_parent_pid: BTreeMap<u32, Vec<u32>>,
    output: Vec<(DateTime<Utc>, String)>,
}

impl ReportBuilder {
    fn new(process_vec: Vec<ProcessDetails>) -> Self {
        let mut process_map = BTreeMap::new();
        let mut process_by_parent: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for process in process_vec {
            process_by_parent.entry(process.parent_pid).or_default().push(process.pid);
            process_map.insert(process.pid, process);
        }

        Self {
            processes: process_map,
            processes_by_parent_pid: process_by_parent,
            output: Default::default(),
        }
    }

    fn build_root_process_tree(&self) -> ProcessTree {
        let mut root = ProcessTree::make_root();
        for child_pid in self.processes_by_parent_pid.get(&std::process::id()).cloned().unwrap_or_default() {
            root.add_child(self.build_process_tree(child_pid));
        }

        root
    }
    fn build_process_tree(&self, process_pid: u32) -> ProcessTree {
        let mut target = ProcessTree::new(self.processes.get(&process_pid).unwrap());
        for child_pid in self.processes_by_parent_pid.get(&process_pid).cloned().unwrap_or_default() {
            target.add_child(self.build_process_tree(child_pid));
        }

        target
    }

    fn add_log_line(&mut self, ts: &DateTime<Utc>, line: &str) {
        self.output.push((ts.clone(), line.to_string()));
        self.output.sort_by(|l, r| l.0.cmp(&r.0) );
    }

    fn write_report_to_file(&self) -> anyhow::Result<()>{
        let id = nanoid::nanoid!(10, &nanoid::alphabet::SAFE);
        let text = self.to_string();

        let file_path = format!("/tmp/pity/pity-report-{}.txt", id);
        let mut file = File::create(&file_path)?;
        file.write_all(&text.into_bytes())?;

        info!(target:"user", "Report created at {}", file_path);

        Ok(())
    }
}

impl Display for ReportBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "=== Log Output\n\n")?;
        for (ts, line) in &self.output {
            write!(f, "{} {}\n", ts.to_rfc3339(), line)?;
        }

        write!(f, "\n\n=== Process Tree\n")?;
        let mut buff = Vec::new();
        if let Ok(_) = ptree::write_tree(&self.build_root_process_tree(), &mut buff) {
            write!(f, "{}", String::from_utf8(buff).unwrap())?;
        }

        write!(f, "\n")?;
        for (pid, process) in &self.processes {
            write!(f, "{} - {}\n", pid, process.command)?;
            write!(f, " - ARGS:\n")?;
            for arg in &process.args {
                write!(f, "   - {}\n", arg)?;
            }
            write!(f, " - ENV:\n")?;
            for env in &process.env {
                write!(f, "   - {}\n", env)?;
            }
        }

        write!(f, "(done)\n")
    }
}

impl DataCapture {
    pub fn new(system: &System) -> Self {
        Self {
            kernel_version: system.os_version().unwrap_or_else(|| "unknown".to_string()),
            ..Default::default()
        }
    }

    pub async fn make_report(&self) -> anyhow::Result<()> {
        let children = self.filter_to_children().await;
        let mut builder = ReportBuilder::new(children);
        for (ts, line) in self.stderr.read().await.iter() {
            builder.add_log_line(ts, &format!("ERR: {}", line));
        }
        for (ts, line) in self.stdout.read().await.iter() {
            builder.add_log_line(ts, &format!("OUT: {}", line));
        }

        builder.write_report_to_file()?;
        Ok(())
    }

    pub async fn add_stdout(&self, line: &str) {
        let mut stdout = self.stdout.write().await;
        stdout.push((Utc::now(), line.to_string()))
    }

    pub async fn add_stderr(&self, line: &str) {
        let mut stderr = self.stderr.write().await;
        stderr.push((Utc::now(), line.to_string()))
    }

    async fn filter_to_children(&self) -> Vec<ProcessDetails> {
        let mut process_details = Vec::new();
        let mut tracked_pids = BTreeSet::new();
        tracked_pids.insert(std::process::id());

        let process_map = self.process_details.read().await;
        let mut process_list: Vec<_> = process_map.values().cloned().collect();
        process_list.sort_by(|l, r|l.pid.cmp(&r.pid));

        for process in process_list {
            if tracked_pids.contains(&process.parent_pid) {
                tracked_pids.insert(process.pid);
                process_details.push(process);
            }
        }

        process_details
    }

    pub async fn handle_process(&self, process: &Process) {
        let mut details = self.process_details.write().await;

        details.entry(process.pid().as_u32())
            .and_modify(|d| (*d).end_time = SystemTime::now() )
            .or_insert_with(|| {

                ProcessDetails {
                    command: process.exe().display().to_string(),
                    pid: process.pid().as_u32(),
                    parent_pid: process.parent().map(|x| x.as_u32()).unwrap_or(0),
                    env: process.environ().iter().cloned().collect(),
                    args: process.cmd().iter().cloned().collect(),
                    start_time: UNIX_EPOCH.add(Duration::from_secs(process.start_time())),
                    end_time: SystemTime::now(),
                }
            });

    }
}
