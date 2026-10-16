use super::output;
use crate::config::{Program, Switch};
use crate::keymap::{self, Lang, Table, SCAN_LIMIT};
use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering::SeqCst};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::core::{s, w, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HMODULE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};
use windows::Win32::System::SystemInformation::{IMAGE_FILE_MACHINE, IMAGE_FILE_MACHINE_AMD64, IMAGE_FILE_MACHINE_UNKNOWN};
use windows::Win32::System::Threading::{IsWow64Process2, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, GetKeyboardLayoutList, MapVirtualKeyExW, ToUnicodeEx, HKL, MAPVK_VSC_TO_VK, VK_LCONTROL, VK_LMENU, VK_LSHIFT,
    VK_OEM_3, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, PostMessageW, RegisterWindowMessageW,
    SetWindowsHookExW, UnhookWindowsHookEx, GUITHREADINFO, HOOKPROC, WH_GETMESSAGE, WM_INPUTLANGCHANGEREQUEST,
};

fn langid(hkl: HKL) -> u16 {
    (hkl.0 as usize & 0xFFFF) as u16
}

fn is_ime(hkl: HKL) -> bool {
    (hkl.0 as usize >> 28) & 0xF == 0xE
}

pub fn foreground_thread() -> (HWND, u32) {
    unsafe {
        let hwnd = GetForegroundWindow();
        (hwnd, GetWindowThreadProcessId(hwnd, None))
    }
}

pub fn focus_window() -> HWND {
    let (hwnd, thread) = foreground_thread();
    let mut info = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
    match unsafe { GetGUIThreadInfo(thread, &mut info) } {
        Ok(()) if !info.hwndFocus.is_invalid() => info.hwndFocus,
        _ => hwnd,
    }
}

pub fn input_thread() -> (HWND, u32) {
    let focus = focus_window();
    (focus, unsafe { GetWindowThreadProcessId(focus, None) })
}

pub fn current() -> Option<Lang> {
    current_of(input_thread().1)
}

fn current_of(thread: u32) -> Option<Lang> {
    let hkl = unsafe { GetKeyboardLayout(thread) };
    if is_ime(hkl) {
        return None;
    }
    Lang::from_langid(langid(hkl))
}

fn layouts() -> Vec<HKL> {
    unsafe {
        let n = GetKeyboardLayoutList(None);
        let mut list = vec![HKL::default(); n.max(0) as usize];
        let got = GetKeyboardLayoutList(Some(&mut list));
        list.truncate(got.max(0) as usize);
        list
    }
}

pub fn hkl_for(lang: Lang) -> Option<HKL> {
    layouts().into_iter().find(|&h| !is_ime(h) && langid(h) == lang.langid())
}

pub fn has_ru_en() -> bool {
    hkl_for(Lang::Ru).is_some() && hkl_for(Lang::En).is_some()
}

pub fn request(lang: Lang) -> Option<u32> {
    let hkl = hkl_for(lang)?;
    let (focus, thread) = input_thread();
    unsafe { PostMessageW(Some(focus), WM_INPUTLANGCHANGEREQUEST, WPARAM(0), LPARAM(hkl.0 as isize)) }.ok()?;
    Some(thread)
}

pub fn wait(lang: Lang, thread: u32) -> bool {
    let start = Instant::now();
    loop {
        if current_of(thread) == Some(lang) {
            return true;
        }
        if start.elapsed() >= Duration::from_millis(50) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

pub fn switch(lang: Lang) -> bool {
    request(lang).is_some_and(|thread| wait(lang, thread))
}

pub fn system_table(lang: Lang) -> Option<Table> {
    let hkl = hkl_for(lang)?;
    let mut table: Table = [[None; 2]; SCAN_LIMIT];
    for scan in keymap::scans() {
        let vk = unsafe { MapVirtualKeyExW(scan as u32, MAPVK_VSC_TO_VK, Some(hkl)) };
        if vk == 0 {
            continue;
        }
        for shift in [false, true] {
            let mut state = [0u8; 256];
            if shift {
                state[VK_SHIFT.0 as usize] = 0x80;
            }
            let mut buf = [0u16; 4];
            if unsafe { ToUnicodeEx(vk, scan as u32, &state, &mut buf, NO_STATE_CHANGE, Some(hkl)) } == 1 {
                table[scan as usize][shift as usize] = char::from_u32(buf[0] as u32).filter(|c| !c.is_control());
            }
        }
    }
    Some(table)
}

const NO_STATE_CHANGE: u32 = 4;

pub fn current_hkl() -> HKL {
    unsafe { GetKeyboardLayout(input_thread().1) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    AltShift,
    CtrlShift,
    Grave,
}

fn toggle_from(value: &str) -> Option<Toggle> {
    match value.trim() {
        "1" => Some(Toggle::AltShift),
        "2" => Some(Toggle::CtrlShift),
        "4" => Some(Toggle::Grave),
        _ => None,
    }
}

fn registry_string(name: PCWSTR) -> Option<String> {
    let mut buf = [0u16; 16];
    let mut size = std::mem::size_of_val(&buf) as u32;
    let status = unsafe {
        RegGetValueW(HKEY_CURRENT_USER, w!("Keyboard Layout\\Toggle"), name, RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr().cast()), Some(&mut size))
    };
    status.is_ok().then(|| String::from_utf16_lossy(&buf[..(size as usize / 2).saturating_sub(1)]))
}

pub fn toggle() -> Option<Toggle> {
    let value = registry_string(w!("Language Hotkey")).or_else(|| registry_string(w!("Hotkey"))).unwrap_or_else(|| "1".into());
    toggle_from(&value)
}

fn press(toggle: Toggle) {
    match toggle {
        Toggle::AltShift => output::press_keys(&[VK_LMENU, VK_LSHIFT]),
        Toggle::CtrlShift => output::press_keys(&[VK_LCONTROL, VK_LSHIFT]),
        Toggle::Grave => output::press_keys(&[VK_OEM_3]),
    };
}

fn poll(thread: u32, until: impl Fn(HKL) -> bool, limit: Duration) -> bool {
    let start = Instant::now();
    loop {
        if until(unsafe { GetKeyboardLayout(thread) }) {
            return true;
        }
        if start.elapsed() >= limit {
            return false;
        }
        std::thread::sleep(HOTKEY_POLL);
    }
}

const HOTKEY_POLL: Duration = Duration::from_millis(3);
const HOTKEY_LIMIT: Duration = Duration::from_millis(250);

fn switch_hotkey(lang: Lang, toggle: Toggle) -> bool {
    let thread = input_thread().1;
    for _ in 1..layouts().len().max(2) {
        if current_of(thread) == Some(lang) {
            return true;
        }
        let before = unsafe { GetKeyboardLayout(thread) };
        press(toggle);
        if !poll(thread, |h| h != before, HOTKEY_LIMIT) {
            HOTKEY_MISSES.fetch_add(1, SeqCst);
            return false;
        }
        HOTKEY_MISSES.store(0, SeqCst);
        HOTKEY_SWITCHES.fetch_add(1, SeqCst);
    }
    current_of(thread) == Some(lang)
}

static HOTKEY_MISSES: AtomicU8 = AtomicU8::new(0);
pub static HOTKEY_SWITCHES: AtomicU64 = AtomicU64::new(0);
const HOTKEY_DEAD_AFTER: u8 = 2;

pub fn hotkey_dead() -> bool {
    HOTKEY_MISSES.load(SeqCst) >= HOTKEY_DEAD_AFTER
}

fn hotkey_for(program: &Program, toggle: impl FnOnce() -> Option<Toggle>, dead: bool) -> Option<Toggle> {
    (program.switch == Switch::Hotkey && !dead).then(toggle).flatten()
}

const DLL_NAME: &str = "ghbdtn_layout.dll";
const THREAD_LIMIT: Duration = Duration::from_millis(250);

struct LayoutDll {
    module: HMODULE,
    hook: HOOKPROC,
    message: u32,
}

unsafe impl Send for LayoutDll {}
unsafe impl Sync for LayoutDll {}

fn layout_dll() -> Option<&'static LayoutDll> {
    static DLL: OnceLock<Option<LayoutDll>> = OnceLock::new();
    DLL.get_or_init(|| {
        let exe = std::env::current_exe().ok()?;
        let dir = exe.parent()?;
        let dir = if dir.ends_with("examples") { dir.parent()? } else { dir };
        let path = Some(dir.join(DLL_NAME)).filter(|p| p.is_file())?;
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
        unsafe {
            let module = LoadLibraryW(PCWSTR(wide.as_ptr())).ok()?;
            let hook = GetProcAddress(module, s!("layout_hook"))?;
            Some(LayoutDll { module, hook: Some(std::mem::transmute::<unsafe extern "system" fn() -> isize, unsafe extern "system" fn(i32, WPARAM, LPARAM) -> LRESULT>(hook)), message: RegisterWindowMessageW(w!("GhbdtnActivateLayout")) })
        }
    })
    .as_ref()
}

pub fn layout_dll_ready() -> bool {
    layout_dll().is_some()
}

fn native_x64(hwnd: HWND) -> bool {
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    let Ok(process) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }) else { return false };
    let (mut machine, mut native) = (IMAGE_FILE_MACHINE::default(), IMAGE_FILE_MACHINE::default());
    let ok = unsafe { IsWow64Process2(process, &mut machine, Some(&mut native)) }.is_ok();
    let _ = unsafe { CloseHandle(process) };
    ok && machine == IMAGE_FILE_MACHINE_UNKNOWN && native == IMAGE_FILE_MACHINE_AMD64 && cfg!(target_arch = "x86_64")
}

fn switch_thread(lang: Lang) -> bool {
    let (Some(dll), Some(hkl)) = (layout_dll(), hkl_for(lang)) else { return false };
    let (focus, thread) = input_thread();
    if thread == 0 || !native_x64(focus) {
        return false;
    }
    let Ok(hook) = (unsafe { SetWindowsHookExW(WH_GETMESSAGE, dll.hook, Some(dll.module.into()), thread) }) else { return false };
    let posted = unsafe { PostMessageW(Some(focus), dll.message, WPARAM(0), LPARAM(hkl.0 as isize)) }.is_ok();
    let ok = posted && poll(thread, |h| !is_ime(h) && langid(h) == lang.langid(), THREAD_LIMIT);
    let _ = unsafe { UnhookWindowsHookEx(hook) };
    if ok {
        THREAD_SWITCHES.fetch_add(1, SeqCst);
    }
    ok
}

pub static THREAD_SWITCHES: AtomicU64 = AtomicU64::new(0);
static THREAD_MISSES: Mutex<Vec<(String, u8)>> = Mutex::new(Vec::new());
const THREAD_DEAD_AFTER: u8 = 2;

fn thread_misses() -> std::sync::MutexGuard<'static, Vec<(String, u8)>> {
    THREAD_MISSES.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn thread_dead(exe: &str) -> bool {
    thread_misses().iter().any(|(e, n)| e == exe && *n >= THREAD_DEAD_AFTER)
}

fn switch_thread_for(lang: Lang, exe: &str) -> bool {
    if thread_dead(exe) {
        return false;
    }
    let ok = switch_thread(lang);
    let mut misses = thread_misses();
    let n = misses.iter().find(|(e, _)| e == exe).map_or(0, |m| m.1);
    misses.retain(|(e, _)| e != exe);
    if !ok {
        misses.push((exe.to_string(), n + 1));
    }
    ok
}

pub fn switch_with(lang: Lang, program: &Program) -> bool {
    if current() == Some(lang) {
        return true;
    }
    let ok = (program.switch == Switch::Thread && switch_thread_for(lang, &program.exe))
        || hotkey_for(program, toggle, hotkey_dead()).is_some_and(|t| switch_hotkey(lang, t))
        || current() == Some(lang)
        || (0..program.retries.max(1)).any(|_| switch(lang));
    if ok && program.delay_ms > 0 {
        std::thread::sleep(Duration::from_millis(program.delay_ms as u64));
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_values() {
        assert_eq!(toggle_from("1"), Some(Toggle::AltShift));
        assert_eq!(toggle_from("2"), Some(Toggle::CtrlShift));
        assert_eq!(toggle_from("4"), Some(Toggle::Grave));
        assert_eq!(toggle_from("3"), None);
        assert_eq!(toggle_from(""), None);
    }

    #[test]
    fn standard_layouts_match_builtin_map() {
        if !has_ru_en() {
            return;
        }
        for lang in [Lang::Ru, Lang::En] {
            let system = system_table(lang).expect("table");
            assert!(keymap::accept_table(lang, &system), "{lang:?}");
            let builtin = keymap::builtin_table(lang);
            let diff: Vec<_> = keymap::scans().filter(|&s| system[s as usize] != builtin[s as usize]).map(|s| (s, system[s as usize], builtin[s as usize])).collect();
            assert!(diff.is_empty(), "{lang:?} {diff:?}");
        }
    }

    #[test]
    fn method_chain() {
        let hotkey = Program { switch: Switch::Hotkey, ..Program::default() };
        let alt = || Some(Toggle::AltShift);
        assert_eq!(hotkey_for(&hotkey, alt, false), Some(Toggle::AltShift));
        assert_eq!(hotkey_for(&hotkey, alt, true), None);
        assert_eq!(hotkey_for(&hotkey, || None, false), None);
        assert_eq!(hotkey_for(&Program::default(), alt, false), None);
    }
}
