use serde::Serialize;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use windows::Win32::System::SystemInformation::GetLocalTime;

const KEEP_DAYS: i64 = 30;
const MAX_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Serialize)]
#[serde(tag = "ev", rename_all = "lowercase")]
pub enum Entry<'a> {
    Acted { kind: &'a str, typed: &'a str, result: &'a str },
    Undo,
    Retyped { before: &'a str, after: &'a str, fix: Option<(&'a str, &'a str)> },
    Never { word: &'a str },
    Silent { reason: &'a str },
}

#[derive(Serialize)]
struct Line<'a> {
    t: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    app: Option<&'a str>,
    #[serde(flatten)]
    entry: Entry<'a>,
}

pub struct Journal {
    dir: PathBuf,
    day: Mutex<String>,
}

fn days(date: &str) -> Option<i64> {
    let mut it = date.splitn(3, '-').map(|p| p.parse::<i64>().ok());
    let (y, m, d) = (it.next()??, it.next()??, it.next()??);
    let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    Some(365 * y + y / 4 - y / 100 + y / 400 + (153 * m + 2) / 5 + d)
}

impl Journal {
    pub fn new() -> Journal {
        let dir = ghbdtn_core::config::Config::default_path().with_file_name("log");
        Journal { dir, day: Mutex::new(String::new()) }
    }

    pub fn write(&self, app: Option<&str>, entry: Entry) {
        let now = unsafe { GetLocalTime() };
        let date = format!("{:04}-{:02}-{:02}", now.wYear, now.wMonth, now.wDay);
        let line = Line { t: format!("{:02}:{:02}:{:02}", now.wHour, now.wMinute, now.wSecond), app, entry };
        let Ok(text) = serde_json::to_string(&line) else { return };
        let mut day = self.day.lock().unwrap();
        if *day != date {
            let _ = std::fs::create_dir_all(&self.dir);
            self.prune(&date);
            *day = date.clone();
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(self.dir.join(format!("{date}.jsonl"))) {
            let _ = writeln!(f, "{text}");
        }
    }

    fn prune(&self, today: &str) {
        let Ok(read) = std::fs::read_dir(&self.dir) else { return };
        let today = days(today).unwrap_or(0);
        let mut files: Vec<(String, PathBuf, u64)> = read
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().strip_suffix(".jsonl")?.to_string();
                let size = e.metadata().ok()?.len();
                Some((name, e.path(), size))
            })
            .collect();
        files.sort();
        files.retain(|(name, path, _)| {
            let old = days(name).is_some_and(|d| today - d > KEEP_DAYS);
            if old {
                let _ = std::fs::remove_file(path);
            }
            !old
        });
        let mut total: u64 = files.iter().map(|f| f.2).sum();
        for (_, path, size) in &files {
            if total <= MAX_BYTES {
                break;
            }
            if std::fs::remove_file(path).is_ok() {
                total -= size;
            }
        }
    }
}
