mod journal;
mod overlay;
mod sys;
mod tray;

use crossbeam_channel::Sender;
use ghbdtn_core::config::{Config, ConfigError, Mode};
use ghbdtn_core::keymap::Lang;
use ghbdtn_core::win::{hook::parse_hotkey, lang, CoreEvent, Engine, SilentReason, Status};
use serde::Serialize;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tray::{Look, Tray};
use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::System::SystemInformation::GetLocalTime;

static TRAY: OnceLock<Tray> = OnceLock::new();

#[derive(Serialize, Clone)]
struct AppInfo {
    exe: String,
    name: String,
}

struct Shared {
    engine: Engine,
    status: Mutex<Status>,
    count: Mutex<(u32, u32)>,
    app: Mutex<Option<AppInfo>>,
    flyout_hidden: Mutex<Option<Instant>>,
    hints: Sender<overlay::Hint>,
    journal: journal::Journal,
    ru: std::sync::atomic::AtomicBool,
}

type S = Arc<Shared>;

fn today() -> u32 {
    let t = unsafe { GetLocalTime() };
    t.wYear as u32 * 10000 + t.wMonth as u32 * 100 + t.wDay as u32
}

fn count(s: &Shared, add: bool) -> u32 {
    let mut c = s.count.lock().unwrap();
    let day = today();
    if c.0 != day {
        *c = (day, 0);
    }
    if add {
        c.1 += 1;
    }
    c.1
}

fn reason(r: SilentReason) -> &'static str {
    match r {
        SilentReason::Process => "process",
        SilentReason::Password => "password",
        SilentReason::Fullscreen => "fullscreen",
        SilentReason::Elevated => "elevated",
        SilentReason::OtherLayout => "layout",
        SilentReason::NoRuEn => "noruen",
    }
}

fn state(s: &Shared) -> (&'static str, Option<&'static str>) {
    let cfg = s.engine.config();
    if !cfg.enabled {
        return ("disabled", None);
    }
    if s.app.lock().unwrap().as_ref().is_some_and(|a| cfg.mode_for(&a.exe) == Mode::Off) {
        return ("silent", Some("process"));
    }
    match *s.status.lock().unwrap() {
        Status::Silent(r) if r != SilentReason::Process => ("silent", Some(reason(r))),
        _ => ("working", None),
    }
}

fn refresh(s: &Shared) {
    let look = match state(s).0 {
        "disabled" => Look::Off,
        "silent" => Look::Mute,
        _ => Look::On,
    };
    if let Some(t) = TRAY.get() {
        t.update(Some(look), None, None);
    }
}

#[derive(Serialize)]
struct FlyoutState {
    status: &'static str,
    reason: Option<&'static str>,
    app: Option<AppInfo>,
    count: u32,
    ru: bool,
    config: Config,
}

#[tauri::command]
fn flyout_state(s: State<S>) -> FlyoutState {
    let (status, reason) = state(&s);
    FlyoutState { status, reason, app: s.app.lock().unwrap().clone(), count: count(&s, false), ru: s.ru.load(std::sync::atomic::Ordering::Relaxed), config: s.engine.config() }
}

#[tauri::command]
fn get_config(s: State<S>) -> Config {
    s.engine.config()
}

fn err(field: &str, message: impl Into<String>) -> ConfigError {
    ConfigError { field: field.into(), message: message.into() }
}

fn save(app: &AppHandle, s: &Shared, mut cfg: Config) -> Result<Config, Vec<ConfigError>> {
    cfg.normalize();
    let mut errors = cfg.validate().err().unwrap_or_default();
    for (field, key) in [("hotkeys.convert", &cfg.hotkeys.convert), ("hotkeys.toggle", &cfg.hotkeys.toggle)] {
        if parse_hotkey(key).is_none() {
            errors.push(err(field, "такую клавишу не поймать — возьми Pause, F1–F24, Insert или букву с Ctrl/Alt/Shift"));
        }
    }
    if cfg.hotkeys.convert.eq_ignore_ascii_case(&cfg.hotkeys.toggle) {
        errors.push(err("hotkeys.toggle", "совпадает с клавишей перевода"));
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let prev = s.engine.config();
    if prev.autostart != cfg.autostart {
        sys::set_autostart(cfg.autostart).map_err(|e| vec![err("autostart", e)])?;
    }
    if let Err(e) = cfg.save(&Config::default_path()) {
        if prev.autostart != cfg.autostart {
            let _ = sys::set_autostart(prev.autostart);
        }
        return Err(vec![err("file", e.to_string())]);
    }
    s.engine.set_config(cfg.clone());
    refresh(s);
    let _ = app.emit("config-changed", &cfg);
    Ok(cfg)
}

#[tauri::command]
fn set_config(app: AppHandle, s: State<S>, config: Config) -> Result<Config, Vec<ConfigError>> {
    save(&app, &s, config)
}

#[tauri::command]
fn remove_never(app: AppHandle, s: State<S>, word: String) -> Result<Config, Vec<ConfigError>> {
    let mut cfg = s.engine.config();
    cfg.never.retain(|n| n.word != word);
    save(&app, &s, cfg)
}

#[tauri::command]
fn clear_never(app: AppHandle, s: State<S>) -> Result<Config, Vec<ConfigError>> {
    let mut cfg = s.engine.config();
    cfg.never.clear();
    save(&app, &s, cfg)
}

#[tauri::command]
fn set_autostart(app: AppHandle, s: State<S>, on: bool) -> Result<bool, String> {
    let mut cfg = s.engine.config();
    cfg.autostart = on;
    save(&app, &s, cfg).map(|c| c.autostart).map_err(|e| e.into_iter().map(|e| e.message).collect::<Vec<_>>().join("; "))
}

#[tauri::command]
fn list_processes() -> Vec<sys::Proc> {
    sys::list_processes()
}

#[derive(Serialize)]
struct License {
    name: String,
    source: String,
    license: String,
}

#[derive(Serialize)]
struct About {
    version: String,
    licenses: Vec<License>,
}

fn plain(cell: &str) -> String {
    let mut out = String::new();
    let mut rest = cell.trim().replace('`', "");
    while let Some(open) = rest.find('[') {
        let Some(mid) = rest[open..].find("](").map(|m| open + m) else { break };
        let Some(close) = rest[mid..].find(')').map(|c| mid + c) else { break };
        out.push_str(&rest[..open]);
        out.push_str(&rest[open + 1..mid]);
        rest = rest[close + 1..].to_string();
    }
    out.push_str(&rest);
    out.trim().to_string()
}

#[tauri::command]
fn about(app: AppHandle) -> About {
    let licenses = include_str!("../core/data/LICENSES.md")
        .lines()
        .filter(|l| l.starts_with("| `"))
        .filter_map(|l| {
            let cells: Vec<&str> = l.trim_matches('|').split('|').collect();
            (cells.len() >= 3).then(|| License { name: plain(cells[0]), source: plain(cells[1]), license: plain(cells[2]) })
        })
        .collect();
    About { version: app.package_info().version.to_string(), licenses }
}

#[tauri::command]
async fn open_settings_cmd(app: AppHandle) {
    open_settings(&app);
}

pub fn open_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("flyout") {
        let _ = w.hide();
    }
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("index.html".into()))
        .title("Ghbdtn")
        .inner_size(880.0, 620.0)
        .min_inner_size(720.0, 520.0)
        .decorations(false)
        .center()
        .build();
}

fn work_area(x: i32, y: i32) -> Option<(i32, i32, i32, i32)> {
    unsafe {
        let m = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        GetMonitorInfoW(m, &mut info).as_bool().then_some((info.rcWork.left, info.rcWork.top, info.rcWork.right, info.rcWork.bottom))
    }
}

fn toggle_flyout(app: &AppHandle, rect: tauri::Rect) {
    let Some(w) = app.get_webview_window("flyout") else { return };
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
        return;
    }
    let s = app.state::<S>();
    if s.flyout_hidden.lock().unwrap().is_some_and(|t| t.elapsed() < Duration::from_millis(350)) {
        return;
    }
    let scale = w.scale_factor().unwrap_or(1.0);
    let pos = rect.position.to_physical::<i32>(scale);
    let Ok(size) = w.outer_size() else { return };
    let Some((left, top, right, bottom)) = work_area(pos.x, pos.y) else { return };
    let edge = (4.0 * scale) as i32;
    let x = (right + edge - size.width as i32).max(left);
    let y = if pos.y < top { top - edge } else { bottom + edge - size.height as i32 };
    let _ = w.set_position(PhysicalPosition { x, y });
    let _ = app.emit_to("flyout", "flyout-open", ());
    let _ = w.show();
    let _ = w.set_focus();
}

fn watch(app: AppHandle, s: S) {
    let mut last = String::new();
    let mut tick = 0u32;
    loop {
        std::thread::sleep(Duration::from_millis(300));
        tick = tick.wrapping_add(1);
        let mut changed = false;
        if let Some((exe, path)) = sys::foreground_app() {
            if exe != last {
                *s.app.lock().unwrap() = Some(AppInfo { name: sys::friendly_name(&path), exe: exe.clone() });
                last = exe;
                changed = true;
            }
        }
        let ru = if sys::own_foreground() { None } else { lang::current().map(|l| l == Lang::Ru) };
        if let Some(r) = ru {
            if s.ru.swap(r, std::sync::atomic::Ordering::Relaxed) != r {
                changed = true;
            }
        }
        let light = tick.is_multiple_of(10).then(sys::taskbar_light);
        if let Some(t) = TRAY.get() {
            t.update(None, ru, light);
        }
        refresh(&s);
        if changed {
            let _ = app.emit("status", ());
        }
    }
}

fn pump(app: AppHandle, s: S, rx: crossbeam_channel::Receiver<CoreEvent>) {
    for ev in rx {
        let cfg = s.engine.config();
        if cfg.journal {
            let app = s.app.lock().unwrap().as_ref().map(|a| a.exe.clone());
            let app = app.as_deref();
            match &ev {
                CoreEvent::Acted { kind, typed, result } => {
                    let kind = format!("{kind:?}").to_lowercase();
                    s.journal.write(app, journal::Entry::Acted { kind: &kind, typed, result });
                }
                CoreEvent::Undone => s.journal.write(app, journal::Entry::Undo),
                CoreEvent::Retyped { before, after, fix } => s.journal.write(
                    app,
                    journal::Entry::Retyped { before, after, fix: fix.as_ref().map(|(a, b)| (a.as_str(), b.as_str())) },
                ),
                CoreEvent::NeverAdded(word) => s.journal.write(app, journal::Entry::Never { word }),
                CoreEvent::Status(Status::Silent(r)) => s.journal.write(app, journal::Entry::Silent { reason: reason(*r) }),
                _ => {}
            }
        }
        match ev {
            CoreEvent::Acted { typed, result, .. } => {
                count(&s, true);
                if cfg.signals.blink {
                    if let Some(t) = TRAY.get() {
                        t.flash();
                    }
                }
                let _ = s.hints.send(overlay::Hint { typed, result, signals: cfg.signals.clone() });
                let _ = app.emit("status", ());
            }
            CoreEvent::Status(st) => {
                *s.status.lock().unwrap() = st;
                refresh(&s);
                let _ = app.emit("status", ());
            }
            CoreEvent::NeverAdded(_) | CoreEvent::ConfigChanged => {
                let _ = app.emit("config-changed", s.engine.config());
            }
            _ => {}
        }
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| open_settings(app)))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_ship::init(|app| {
            if let Some(s) = app.try_state::<S>() {
                s.engine.stop();
            }
        }))
        .invoke_handler(tauri::generate_handler![
            flyout_state,
            get_config,
            set_config,
            remove_never,
            clear_never,
            set_autostart,
            list_processes,
            about,
            open_settings_cmd
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let (tx, rx) = crossbeam_channel::unbounded();
            let engine = Engine::start(Config::default_path(), tx);
            let _ = sys::set_autostart(engine.config().autostart);
            overlay::create(&handle)?;
            let shared: S = Arc::new(Shared {
                engine,
                status: Mutex::new(Status::Working),
                count: Mutex::new((today(), 0)),
                app: Mutex::new(None),
                flyout_hidden: Mutex::new(None),
                hints: overlay::start(handle.clone()),
                journal: journal::Journal::new(),
                ru: std::sync::atomic::AtomicBool::new(true),
            });
            app.manage(shared.clone());

            let flyout = WebviewWindowBuilder::new(app, "flyout", WebviewUrl::App("index.html".into()))
                .title("Ghbdtn")
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .resizable(false)
                .skip_taskbar(true)
                .always_on_top(true)
                .visible(false)
                .focused(false)
                .inner_size(392.0, 420.0)
                .build()?;
            let fly = flyout.clone();
            let hidden = shared.clone();
            flyout.on_window_event(move |e| {
                if let WindowEvent::Focused(false) = e {
                    let _ = fly.hide();
                    *hidden.flyout_hidden.lock().unwrap() = Some(Instant::now());
                }
            });

            let tray = Tray::build(&handle, toggle_flyout)?;
            let _ = TRAY.set(tray);
            refresh(&shared);

            let (a, s) = (handle.clone(), shared.clone());
            std::thread::Builder::new().name("ghbdtn-events".into()).spawn(move || pump(a, s, rx))?;
            let (a, s) = (handle.clone(), shared.clone());
            std::thread::Builder::new().name("ghbdtn-watch".into()).spawn(move || watch(a, s))?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("ghbdtn")
        .run(|_, event| {
            if let tauri::RunEvent::ExitRequested { api, code: None, .. } = event {
                api.prevent_exit();
            }
        });
}
