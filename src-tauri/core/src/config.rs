use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Features {
    pub layout: bool,
    pub typos: bool,
    pub replace: bool,
    pub early: bool,
}

impl Default for Features {
    fn default() -> Self {
        Features { layout: true, typos: true, replace: true, early: true }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Signals {
    pub hint: bool,
    pub blink: bool,
    pub sound: bool,
    pub cursor: bool,
}

impl Default for Signals {
    fn default() -> Self {
        Signals { hint: true, blink: true, sound: false, cursor: true }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hotkeys {
    pub convert: String,
    pub toggle: String,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Hotkeys { convert: "Pause".into(), toggle: "Shift+Pause".into() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NeverSource {
    Manual,
    Undo,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NeverWord {
    pub word: String,
    pub source: NeverSource,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Processes {
    pub off: Vec<String>,
    pub code: Vec<String>,
}

const OFF: &[&str] = &[
    "mstsc", "anydesk", "teamviewer", "virtualbox", "virtualboxvm", "vmconnect", "vmware", "vmware-vmx",
    "keepass", "keepassxc", "bitwarden", "1password", "winword", "excel", "powerpnt", "outlook", "onenote",
];

const CODE: &[&str] = &[
    "windowsterminal", "openconsole", "conhost", "cmd", "powershell", "pwsh", "mintty", "wezterm-gui",
    "alacritty", "code", "cursor", "windsurf", "devenv", "idea64", "pycharm64", "webstorm64", "rider64",
    "clion64", "goland64", "zed", "sublime_text", "notepad++",
];

fn exes(list: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = list.iter().map(|s| format!("{s}.exe")).collect();
    v.sort();
    v
}

impl Default for Processes {
    fn default() -> Self {
        Processes { off: exes(OFF), code: exes(CODE) }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Insert {
    #[default]
    Unicode,
    Paste,
    Keys,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Switch {
    #[default]
    Message,
    Hotkey,
    Thread,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Program {
    pub exe: String,
    pub insert: Insert,
    pub switch: Switch,
    pub delay_ms: u32,
    pub retries: u8,
}

impl Default for Program {
    fn default() -> Self {
        Program { exe: String::new(), insert: Insert::Unicode, switch: Switch::Message, delay_ms: 0, retries: 2 }
    }
}

const PASTE: &[&str] = &["telegram", "whatsapp", "whatsapp.root", "viber", "icq", "lingvo"];
const HOTKEY: &[&str] = &[
    "devenv", "excel", "powerpnt", "outlook", "onenote", "zoom", "skype", "lync", "thunderbird", "afterfx",
    "adobe premiere pro",
];
const KEYS: &[&str] = &["cmd", "powershell", "pwsh", "conhost"];
const WORD_DELAY_MS: u32 = 30;
pub const MAX_DELAY_MS: u32 = 500;
pub const MAX_RETRIES: u8 = 5;

fn default_programs() -> Vec<Program> {
    let make = |names: &[&str], insert, switch| {
        names.iter().map(move |n| Program { exe: format!("{n}.exe"), insert, switch, ..Program::default() }).collect::<Vec<_>>()
    };
    let mut v = make(PASTE, Insert::Paste, Switch::Message);
    v.extend(make(HOTKEY, Insert::Unicode, Switch::Hotkey));
    v.extend(make(KEYS, Insert::Keys, Switch::Message));
    v.push(Program { exe: "winword.exe".into(), switch: Switch::Hotkey, delay_ms: WORD_DELAY_MS, ..Program::default() });
    v.sort_by(|a, b| a.exe.cmp(&b.exe));
    v
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Off,
    Code,
    Normal,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub autostart: bool,
    pub journal: bool,
    pub learn: bool,
    pub features: Features,
    pub signals: Signals,
    pub hotkeys: Hotkeys,
    pub replacements: BTreeMap<String, String>,
    pub never: Vec<NeverWord>,
    pub processes: Processes,
    pub programs: Vec<Program>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            enabled: true,
            autostart: true,
            journal: false,
            learn: true,
            features: Features::default(),
            signals: Signals::default(),
            hotkeys: Hotkeys::default(),
            replacements: BTreeMap::from([("чето".to_string(), "чё-то".to_string())]),
            never: Vec::new(),
            processes: Processes::default(),
            programs: default_programs(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConfigError {
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

fn err(field: impl Into<String>, message: impl Into<String>) -> ConfigError {
    ConfigError { field: field.into(), message: message.into() }
}

pub fn normalize_exe(name: &str) -> String {
    let n = name.trim().to_lowercase();
    if n.ends_with(".exe") { n } else { format!("{n}.exe") }
}

impl Config {
    pub fn default_path() -> PathBuf {
        let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
        base.join("Ghbdtn").join("config.toml")
    }

    pub fn words_path() -> PathBuf {
        Config::default_path().with_file_name("words.txt")
    }

    pub fn load(path: &Path) -> Result<Config, ConfigError> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
            Err(e) => return Err(err("file", e.to_string())),
        };
        let mut cfg: Config = toml::from_str(&text).map_err(|e| err("file", e.to_string()))?;
        cfg.normalize();
        cfg.validate().map_err(|mut v| v.remove(0))?;
        Ok(cfg)
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(tmp, path)
    }

    pub fn normalize(&mut self) {
        for list in [&mut self.processes.off, &mut self.processes.code] {
            *list = list.iter().map(|p| normalize_exe(p)).collect();
            list.sort();
            list.dedup();
        }
        for n in &mut self.never {
            n.word = n.word.trim().to_lowercase();
        }
        let mut seen = std::collections::HashSet::new();
        self.never.retain(|n| !n.word.is_empty() && seen.insert(n.word.clone()));
        for p in &mut self.programs {
            p.exe = normalize_exe(&p.exe);
            p.delay_ms = p.delay_ms.min(MAX_DELAY_MS);
            p.retries = p.retries.clamp(1, MAX_RETRIES);
        }
        let mut seen = std::collections::HashSet::new();
        self.programs.retain(|p| p.exe != ".exe" && seen.insert(p.exe.clone()));
        self.programs.sort_by(|a, b| a.exe.cmp(&b.exe));
    }

    pub fn validate(&self) -> Result<(), Vec<ConfigError>> {
        let mut errors = Vec::new();
        let mut keys = std::collections::HashSet::new();
        for (k, v) in &self.replacements {
            let field = format!("replacements.{k}");
            let len = k.chars().count();
            if !(1..=32).contains(&len) || !k.chars().all(|c| c.is_alphabetic() || c == '-') {
                errors.push(err(&field, "ключ — буквы и дефис, 1–32 символа, без пробелов"));
            }
            if !keys.insert(k.to_lowercase()) {
                errors.push(err(&field, "такой ключ уже есть"));
            }
            let vlen = v.chars().count();
            if !(1..=200).contains(&vlen) || v.contains(['\n', '\r']) {
                errors.push(err(&field, "замена — 1–200 символов, одна строка"));
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn mode_for(&self, exe: &str) -> Mode {
        let exe = normalize_exe(exe);
        if self.processes.off.contains(&exe) {
            Mode::Off
        } else if self.processes.code.contains(&exe) {
            Mode::Code
        } else {
            Mode::Normal
        }
    }

    pub fn program_for(&self, exe: &str) -> Program {
        let exe = normalize_exe(exe);
        self.programs.iter().find(|p| p.exe == exe).cloned().unwrap_or_default()
    }

    pub fn is_never(&self, word: &str) -> bool {
        let w = word.to_lowercase();
        self.never.iter().any(|n| n.word == w)
    }

    pub fn never_starts(&self, prefix: &str) -> bool {
        let p = prefix.to_lowercase();
        self.never.iter().any(|n| n.word.starts_with(&p))
    }

    pub fn add_never(&mut self, word: &str, source: NeverSource) {
        if !self.is_never(word) {
            self.never.push(NeverWord { word: word.to_lowercase(), source });
        }
    }

    pub fn replacement(&self, word: &str) -> Option<&str> {
        let w = word.to_lowercase();
        self.replacements.iter().find(|(k, _)| k.to_lowercase() == w).map(|(_, v)| v.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let dir = std::env::temp_dir().join(format!("ghbdtn-test-{}", std::process::id()));
        let path = dir.join("config.toml");
        let mut cfg = Config::default();
        cfg.add_never("Ghbdtn", NeverSource::Undo);
        cfg.save(&path).unwrap();
        let back = Config::load(&path).unwrap();
        assert_eq!(cfg, back);
        assert!(back.is_never("GHBDTN"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_file_is_default() {
        let cfg = Config::load(Path::new("Z:/nope/config.toml")).unwrap();
        assert_eq!(cfg, Config::default());
        assert!(!cfg.signals.sound);
        assert!(cfg.autostart);
    }

    #[test]
    fn validation() {
        let mut cfg = Config::default();
        cfg.replacements.insert("два слова".into(), "x".into());
        cfg.replacements.insert("Чето".into(), "y".into());
        cfg.replacements.insert("ok".into(), "a\nb".into());
        let errs = cfg.validate().unwrap_err();
        assert_eq!(errs.len(), 3);
    }

    #[test]
    fn modes() {
        let mut cfg = Config::default();
        assert_eq!(cfg.mode_for("WindowsTerminal.exe"), Mode::Code);
        assert_eq!(cfg.mode_for("KeePass.exe"), Mode::Off);
        assert_eq!(cfg.mode_for("notepad.exe"), Mode::Normal);
        cfg.processes.off.push("Notepad".into());
        cfg.normalize();
        assert_eq!(cfg.mode_for("notepad.exe"), Mode::Off);
        assert_eq!(cfg.replacement("Чето"), Some("чё-то"));
    }

    #[test]
    fn programs() {
        let mut cfg = Config::default();
        assert_eq!(cfg.program_for("Telegram.exe").insert, Insert::Paste);
        assert_eq!(cfg.program_for("devenv").switch, Switch::Hotkey);
        let word = cfg.program_for("WINWORD.EXE");
        assert_eq!((word.switch, word.delay_ms), (Switch::Hotkey, WORD_DELAY_MS));
        assert_eq!(cfg.program_for("cmd.exe").insert, Insert::Keys);
        assert_eq!(cfg.program_for("notepad.exe"), Program::default());
        cfg.programs.push(Program { exe: "Notepad".into(), insert: Insert::Keys, delay_ms: 9000, retries: 0, ..Program::default() });
        cfg.programs.push(Program { exe: "notepad.exe".into(), ..Program::default() });
        cfg.normalize();
        let n = cfg.program_for("notepad.exe");
        assert_eq!((n.insert, n.delay_ms, n.retries), (Insert::Keys, MAX_DELAY_MS, 1));
        assert_eq!(cfg.programs.iter().filter(|p| p.exe == "notepad.exe").count(), 1);
    }

    #[test]
    fn old_config_gets_default_programs() {
        let cfg: Config = toml::from_str("enabled = true").unwrap();
        assert_eq!(cfg.programs, default_programs());
        let text = toml::to_string_pretty(&Config::default()).unwrap();
        assert!(text.contains("insert = \"paste\""));
    }
}
