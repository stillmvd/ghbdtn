pub mod clipboard;
pub mod context;
pub mod hook;
pub mod lang;
pub mod output;

use crate::config::{Config, Mode, NeverSource};
use crate::decide::{decide, WordInput};
use crate::dict::Dictionaries;
use crate::keymap::{char_of, convert_text, script_of, Lang};
use crate::layout::{short_alongside, split_tail};
use crate::undo::{Action, ActionKind, UndoBook};
use crate::word::{render, separator_kind, Key, Separator, Word};
use arc_swap::ArcSwap;
use crossbeam_channel::{bounded, Receiver, RecvTimeoutError, Sender};
use hook::{HookEvent, RawKey, ALLOWED, ARMED_UNDO, ENABLED, HOLD, PENDING};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

const IDLE_RESET: Duration = Duration::from_secs(10);
const WATCHDOG: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SilentReason {
    Process,
    Password,
    Fullscreen,
    Elevated,
    OtherLayout,
    NoRuEn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Working,
    Disabled,
    Silent(SilentReason),
}

#[derive(Clone, Debug)]
pub enum CoreEvent {
    Acted { kind: ActionKind, typed: String, result: String },
    Undone,
    Status(Status),
    Retyped { before: String, after: String, fix: Option<(String, String)> },
    NeverAdded(String),
    ConfigChanged,
    HookReinstalled,
}

pub struct Engine {
    config: Arc<ArcSwap<Config>>,
    control: Sender<HookEvent>,
    fresh: Fresh,
    words_gen: Arc<AtomicU64>,
}

type Fresh = Arc<Mutex<Option<Dictionaries>>>;

impl Engine {
    pub fn start(path: PathBuf, events: Sender<CoreEvent>) -> Engine {
        let cfg = Config::load(&path).unwrap_or_default();
        let config = Arc::new(ArcSwap::from_pointee(cfg));
        let (tx, rx) = bounded(4096);
        hook::run(tx.clone());
        context::run(tx.clone());
        let shared = config.clone();
        let fresh = Fresh::default();
        let slot = fresh.clone();
        std::thread::Builder::new()
            .name("ghbdtn-engine".into())
            .spawn(move || Worker::new(shared, path, events, slot).run(rx))
            .expect("engine thread");
        Engine { config, control: tx, fresh, words_gen: Arc::default() }
    }

    pub fn reload_words(&self, text: String) {
        let gen = self.words_gen.fetch_add(1, SeqCst) + 1;
        let (current, fresh, control) = (self.words_gen.clone(), self.fresh.clone(), self.control.clone());
        let _ = std::thread::Builder::new().name("ghbdtn-words".into()).spawn(move || {
            let mut d = Dictionaries::load();
            d.add_user_words(&text);
            let mut slot = fresh.lock().unwrap();
            if current.load(SeqCst) == gen {
                *slot = Some(d);
                let _ = control.try_send(HookEvent::Words);
            }
        });
    }

    pub fn config(&self) -> Config {
        (**self.config.load()).clone()
    }

    pub fn set_config(&self, cfg: Config) {
        self.config.store(Arc::new(cfg));
        let _ = self.control.try_send(HookEvent::Focus);
    }

    pub fn stop(&self) {
        hook::stop();
    }
}

struct LastWord {
    keys: Vec<Key>,
    lang: Lang,
    sep: String,
}

#[derive(Default)]
struct Shown {
    text: String,
    before: String,
    keys: Option<(Vec<Key>, Lang)>,
    fix: Option<(String, String)>,
    glued: bool,
}

struct Worker {
    config: Arc<ArcSwap<Config>>,
    path: PathBuf,
    events: Sender<CoreEvent>,
    dicts: Dictionaries,
    fresh: Fresh,
    extra_for: Option<Arc<Config>>,
    word: Word,
    word_lang: Option<Lang>,
    last: Option<LastWord>,
    prev: Option<LastWord>,
    tail: Option<Shown>,
    edit: Option<Shown>,
    glued: bool,
    ctx: Option<String>,
    book: UndoBook,
    caps: bool,
    backlog: VecDeque<HookEvent>,
    status: Option<Status>,
    last_key: Instant,
    last_watch: Instant,
}

fn is_nav(vk: VIRTUAL_KEY) -> bool {
    matches!(
        vk,
        VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN | VK_HOME | VK_END | VK_PRIOR | VK_NEXT | VK_TAB | VK_ESCAPE | VK_DELETE
            | VK_INSERT
    )
}

fn is_modifier(vk: VIRTUAL_KEY) -> bool {
    matches!(
        vk,
        VK_SHIFT | VK_LSHIFT | VK_RSHIFT | VK_CONTROL | VK_LCONTROL | VK_RCONTROL | VK_MENU | VK_LMENU | VK_RMENU
            | VK_LWIN | VK_RWIN
    )
}

impl Worker {
    fn new(config: Arc<ArcSwap<Config>>, path: PathBuf, events: Sender<CoreEvent>, fresh: Fresh) -> Worker {
        Worker {
            config,
            path: path.clone(),
            events,
            dicts: {
                let mut d = Dictionaries::load();
                if let Ok(text) = std::fs::read_to_string(path.with_file_name("words.txt")) {
                    d.add_user_words(&text);
                }
                d
            },
            fresh,
            extra_for: None,
            word: Word::default(),
            word_lang: None,
            last: None,
            prev: None,
            tail: None,
            edit: None,
            glued: false,
            ctx: None,
            book: UndoBook::default(),
            caps: unsafe { GetKeyState(VK_CAPITAL.0 as i32) } & 1 != 0,
            backlog: VecDeque::new(),
            status: None,
            last_key: Instant::now(),
            last_watch: Instant::now(),
        }
    }

    fn cfg(&mut self) -> Arc<Config> {
        let cfg = self.config.load_full();
        if !self.extra_for.as_ref().is_some_and(|c| Arc::ptr_eq(c, &cfg)) {
            self.dicts.set_extra(cfg.replacements.keys().cloned());
            ENABLED.store(cfg.enabled, SeqCst);
            hook::HK_CONVERT.store(hook::parse_hotkey(&cfg.hotkeys.convert).unwrap_or(0), SeqCst);
            hook::HK_TOGGLE.store(hook::parse_hotkey(&cfg.hotkeys.toggle).unwrap_or(0), SeqCst);
            self.extra_for = Some(cfg.clone());
        }
        cfg
    }

    fn emit(&self, e: CoreEvent) {
        let _ = self.events.try_send(e);
    }

    fn save(&mut self, cfg: Config) {
        let _ = cfg.save(&self.path);
        self.config.store(Arc::new(cfg));
        self.emit(CoreEvent::ConfigChanged);
    }

    fn evaluate(&mut self) -> (Status, Mode) {
        let cfg = self.cfg();
        let ctx = context::current();
        let mode = cfg.mode_for(&ctx.exe);
        let lang = lang::current();
        hook::set_lang(lang);
        let status = if !cfg.enabled {
            Status::Disabled
        } else if !lang::has_ru_en() {
            Status::Silent(SilentReason::NoRuEn)
        } else if mode == Mode::Off {
            Status::Silent(SilentReason::Process)
        } else if ctx.password {
            Status::Silent(SilentReason::Password)
        } else if ctx.fullscreen {
            Status::Silent(SilentReason::Fullscreen)
        } else if ctx.elevated {
            Status::Silent(SilentReason::Elevated)
        } else if lang.is_none() {
            Status::Silent(SilentReason::OtherLayout)
        } else {
            Status::Working
        };
        ALLOWED.store(status == Status::Working, SeqCst);
        if self.status != Some(status) {
            self.status = Some(status);
            self.emit(CoreEvent::Status(status));
        }
        (status, mode)
    }

    fn reset(&mut self) {
        self.word.clear();
        self.word_lang = None;
        self.last = None;
        self.prev = None;
        self.tail = None;
        self.edit = None;
        self.glued = false;
        self.ctx = None;
        self.book.close();
        ARMED_UNDO.store(false, SeqCst);
        PENDING.store(false, SeqCst);
    }

    fn back(&mut self) {
        if !self.word.is_empty() {
            self.word.backspace();
            if self.word.is_empty() {
                self.word_lang = None;
            }
            if let Some(e) = self.edit.as_mut() {
                e.text.pop();
            }
            return;
        }
        self.last = None;
        self.prev = None;
        self.ctx = None;
        if self.edit.is_none() {
            self.edit = self.tail.take();
        }
        let Some(e) = self.edit.as_mut() else {
            self.glued = true;
            return;
        };
        if e.text.pop().is_none() {
            self.edit = None;
            self.glued = true;
            return;
        }
        if let Some((keys, lang)) = &e.keys {
            if render(keys, *lang) == e.text {
                for k in keys {
                    self.word.push(*k);
                }
                self.word_lang = Some(*lang);
                self.glued = e.glued;
            }
        }
    }

    fn run(mut self, rx: Receiver<HookEvent>) {
        self.evaluate();
        loop {
            let ev = match self.backlog.pop_front() {
                Some(ev) => ev,
                None => match rx.recv_timeout(Duration::from_millis(500)) {
                    Ok(ev) => ev,
                    Err(RecvTimeoutError::Timeout) => {
                        self.tick();
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => return,
                },
            };
            match ev {
                HookEvent::Foreign | HookEvent::Click => self.reset(),
                HookEvent::Words => self.take_words(),
                HookEvent::Focus => {
                    self.reset();
                    self.evaluate();
                }
                HookEvent::Key(k) => {
                    self.last_key = Instant::now();
                    if !k.up {
                        self.key(k, &rx);
                    }
                }
            }
            if HOLD.load(SeqCst) && self.backlog.is_empty() {
                self.release(&rx);
            }
        }
    }

    fn take_words(&mut self) {
        if let Some(d) = self.fresh.lock().unwrap().take() {
            self.dicts = d;
            self.extra_for = None;
        }
    }

    fn tick(&mut self) {
        self.take_words();
        if !self.word.is_empty() && self.last_key.elapsed() > IDLE_RESET {
            self.reset();
        }
        if self.last_watch.elapsed() >= WATCHDOG {
            self.last_watch = Instant::now();
            let mut info = windows::Win32::UI::Input::KeyboardAndMouse::LASTINPUTINFO {
                cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                dwTime: 0,
            };
            if unsafe { GetLastInputInfo(&mut info) }.as_bool() {
                let hook_tick = hook::LAST_TICK.load(SeqCst);
                let now = unsafe { GetTickCount() };
                if info.dwTime.wrapping_sub(hook_tick) as i32 > 2000 && now.wrapping_sub(info.dwTime) < 2000 {
                    hook::reinstall();
                    self.emit(CoreEvent::HookReinstalled);
                }
            }
        }
    }

    fn release(&mut self, rx: &Receiver<HookEvent>) {
        let mut replay: Vec<RawKey> = Vec::new();
        let drain = |this: &mut Worker, replay: &mut Vec<RawKey>| {
            while let Ok(ev) = rx.try_recv() {
                let ev = match ev {
                    HookEvent::Key(mut k) if k.held => {
                        replay.push(k);
                        k.held = false;
                        HookEvent::Key(k)
                    }
                    other => other,
                };
                this.backlog.push_back(ev);
            }
        };
        drain(self, &mut replay);
        hook::REPLAYED.fetch_add(replay.len() as u64, SeqCst);
        output::replay(&replay);
        replay.clear();
        HOLD.store(false, SeqCst);
        drain(self, &mut replay);
        hook::REPLAYED.fetch_add(replay.len() as u64, SeqCst);
        output::replay(&replay);
    }

    fn key(&mut self, k: RawKey, rx: &Receiver<HookEvent>) {
        let vk = VIRTUAL_KEY(k.vk);
        let cfg = self.cfg();
        if k.eaten {
            if hook::HK_TOGGLE.load(SeqCst) == hook_code(&k) {
                let mut c = (*cfg).clone();
                c.enabled = !c.enabled;
                self.save(c);
                self.reset();
                self.evaluate();
            } else if vk == VK_BACK {
                self.undo(&k);
            } else if vk == VK_RETURN {
                self.finish(Separator::Enter, true, k.at);
            } else {
                self.convert();
            }
            self.release(rx);
            return;
        }
        if vk == VK_CAPITAL {
            self.caps = !self.caps;
            return;
        }
        if is_modifier(vk) {
            return;
        }
        self.book.close();
        ARMED_UNDO.store(false, SeqCst);
        if k.ctrl || k.alt || k.win || is_nav(vk) {
            self.reset();
            return;
        }
        if vk == VK_BACK {
            self.back();
            return;
        }
        if vk == VK_RETURN {
            self.finish(Separator::Enter, false, k.at);
            return;
        }
        let (status, _) = if self.word.is_empty() { self.evaluate() } else { (self.status.unwrap_or(Status::Working), Mode::Normal) };
        if status != Status::Working {
            self.reset();
            return;
        }
        let lang = match self.word_lang {
            Some(l) => l,
            None => match lang::current() {
                Some(l) => l,
                None => return self.reset(),
            },
        };
        let key = Key::new(k.scan, k.shift, self.caps);
        if let Some(sep) = separator_kind(key, lang) {
            self.finish(sep, false, k.at);
            return;
        }
        if char_of(k.scan, k.shift, self.caps, lang).is_some() {
            if self.word.is_empty() {
                self.prev = self.last.take();
                self.word_lang = Some(lang);
                self.tail = None;
                if self.edit.as_ref().is_some_and(|e| e.text.chars().last().is_some_and(char::is_alphanumeric)) {
                    self.glued = true;
                }
            }
            self.word.push(key);
            if self.word.is_empty() {
                return self.reset();
            }
            if let (Some(e), Some(c)) = (self.edit.as_mut(), key.char_in(lang)) {
                e.text.push(c);
            }
        } else {
            self.reset();
        }
    }

    fn finish(&mut self, sep: Separator, enter_eaten: bool, started: Instant) {
        let glued = std::mem::take(&mut self.glued);
        let edit = self.edit.take();
        let tail = self.tail.take();
        let Some(lang) = self.word_lang.take().filter(|_| !self.word.is_empty()) else {
            self.last = None;
            if sep != Separator::Space {
                self.ctx = None;
            }
            if enter_eaten {
                output::enter();
            }
            if sep != Separator::Enter {
                self.tail = lang::current().map(|l| {
                    let mut t = edit.or(tail).unwrap_or_default();
                    t.text += &sep.text(l);
                    t
                });
            }
            return;
        };
        if let Some(e) = edit.filter(|e| !e.text.is_empty() && e.text != e.before) {
            self.emit(CoreEvent::Retyped { before: e.before, after: e.text, fix: e.fix });
        }
        let keys = self.word.take();
        let (status, mode) = self.evaluate();
        let cfg = self.cfg();
        let fix = (status == Status::Working && !keys.is_empty() && !glued && (sep != Separator::Enter || enter_eaten))
            .then(|| decide(&WordInput { keys: &keys, lang, code_mode: mode == Mode::Code, prev: self.ctx.as_deref() }, &self.dicts, &cfg))
            .flatten();
        let word_like = |t: &str| t.chars().all(|c| c.is_alphabetic() || c == '-') || t.chars().all(|c| c.is_ascii_digit());
        let plain = |t: &str| (sep == Separator::Space && !glued && word_like(t)).then(|| t.to_string());
        self.ctx = match &fix {
            Some(f) => plain(f.text.rsplit(' ').next().unwrap_or_default()),
            None => plain(&render(&keys, lang)),
        };
        let sep_typed = sep.text(lang);
        match fix {
            Some(fix) => {
                hook::hold();
                let to = fix.to.unwrap_or(lang);
                let sep_out = sep.text(to);
                let (prev_typed, prev_out) = match self.prev.take().as_ref() {
                    Some(prev) if fix.to.is_some() && prev.lang == lang && prev.sep == " " => {
                        short_alongside(&prev.keys, lang)
                            .map(|alt| (render(&prev.keys, lang) + " ", alt + " "))
                            .unwrap_or_default()
                    }
                    _ => Default::default(),
                };
                let typed_all = prev_typed + &fix.typed;
                let result_all = prev_out + &fix.text;
                output::rewrite(&(typed_all.clone() + &sep_typed), &(result_all.clone() + &sep_out));
                let us = started.elapsed().as_micros() as u64;
                hook::FIXES.fetch_add(1, SeqCst);
                hook::FIX_US_TOTAL.fetch_add(us, SeqCst);
                hook::FIX_US_MAX.fetch_max(us, SeqCst);
                if fix.to.is_some() {
                    lang::switch(to);
                    hook::set_lang(Some(to));
                }
                if sep == Separator::Enter {
                    output::enter();
                } else {
                    self.book.record(Action {
                        kind: fix.kind,
                        word: fix.typed.clone(),
                        typed: typed_all,
                        result: result_all,
                        sep_before: sep_typed,
                        sep_after: sep_out.clone(),
                        lang_before: lang,
                        lang_after: to,
                    });
                    ARMED_UNDO.store(true, SeqCst);
                }
                self.emit(CoreEvent::Acted { kind: fix.kind, typed: fix.typed.clone(), result: fix.text.clone() });
                self.last = None;
                if sep != Separator::Enter {
                    self.tail = Some(Shown { text: fix.text.clone() + &sep_out, before: fix.text.clone(), keys: None, fix: Some((fix.typed.clone(), fix.text.clone())), glued: false });
                }
            }
            None => {
                if enter_eaten {
                    output::enter();
                }
                if sep != Separator::Enter {
                    let shown = render(&keys, lang);
                    self.tail = Some(Shown { text: shown.clone() + &sep_typed, before: shown, keys: Some((keys.clone(), lang)), fix: None, glued });
                }
                self.last = (sep != Separator::Enter).then_some(LastWord { keys, lang, sep: sep_typed });
            }
        }
    }

    fn undo(&mut self, k: &RawKey) {
        self.tail = None;
        self.edit = None;
        self.ctx = None;
        ARMED_UNDO.store(false, SeqCst);
        let Some(action) = self.book.take() else {
            if !k.shift && !k.alt {
                output::chord(VK_CONTROL, VK_BACK);
            }
            return;
        };
        output::rewrite(&(action.result.clone() + &action.sep_after), &(action.typed.clone() + &action.sep_before));
        if action.lang_before != action.lang_after {
            lang::switch(action.lang_before);
            hook::set_lang(Some(action.lang_before));
        }
        self.emit(CoreEvent::Undone);
        let (core, _) = split_tail(&action.word, action.lang_before);
        let core = core.to_string();
        if self.book.note_undo(&core) {
            let mut cfg = (*self.cfg()).clone();
            cfg.add_never(&core, NeverSource::Undo);
            self.save(cfg);
            self.emit(CoreEvent::NeverAdded(core));
        }
    }

    fn convert(&mut self) {
        self.tail = None;
        self.edit = None;
        self.glued = false;
        self.ctx = None;
        let (keys, lang, sep) = if !self.word.is_empty() {
            let Some(lang) = self.word_lang.take() else { return };
            (self.word.take(), lang, String::new())
        } else if let Some(last) = self.last.take() {
            (last.keys.clone(), last.lang, last.sep.clone())
        } else {
            return self.convert_selection();
        };
        let to = lang.other();
        let typed = render(&keys, lang);
        let result = render(&keys, to);
        let sep_out = convert_text(&sep, lang);
        output::rewrite(&(typed.clone() + &sep), &(result.clone() + &sep_out));
        lang::switch(to);
        hook::set_lang(Some(to));
        self.book.record(Action {
            kind: ActionKind::Manual,
            word: typed.clone(),
            typed: typed.clone(),
            result: result.clone(),
            sep_before: sep,
            sep_after: sep_out,
            lang_before: lang,
            lang_after: to,
        });
        ARMED_UNDO.store(true, SeqCst);
        self.emit(CoreEvent::Acted { kind: ActionKind::Manual, typed, result });
    }

    fn convert_selection(&mut self) {
        let Some((text, saved)) = clipboard::copy_selection() else { return };
        let Some(from) = script_of(&text) else { return };
        let result = convert_text(&text, from);
        clipboard::paste(&result, saved);
        lang::switch(from.other());
        hook::set_lang(Some(from.other()));
        self.emit(CoreEvent::Acted { kind: ActionKind::Manual, typed: text, result });
    }
}

fn hook_code(k: &RawKey) -> u32 {
    let mut code = k.vk as u32;
    if k.shift {
        code |= hook::MOD_SHIFT;
    }
    if k.ctrl {
        code |= hook::MOD_CTRL;
    }
    if k.alt {
        code |= hook::MOD_ALT;
    }
    code
}
