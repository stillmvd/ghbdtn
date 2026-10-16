use super::hook::{RawKey, MARK};
use std::sync::atomic::{AtomicU64, Ordering::SeqCst};
use windows::Win32::UI::Input::KeyboardAndMouse::*;

pub static SENT: AtomicU64 = AtomicU64::new(0);

fn key(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: MARK },
        },
    }
}

fn tap(out: &mut Vec<INPUT>, vk: VIRTUAL_KEY) {
    out.push(key(vk, 0, KEYBD_EVENT_FLAGS(0)));
    out.push(key(vk, 0, KEYEVENTF_KEYUP));
}

fn send(inputs: &[INPUT]) -> bool {
    SENT.fetch_add(inputs.len() as u64, SeqCst);
    inputs.is_empty() || unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) } as usize == inputs.len()
}

fn held_modifiers() -> Vec<VIRTUAL_KEY> {
    [VK_LCONTROL, VK_RCONTROL, VK_LMENU, VK_RMENU, VK_LWIN, VK_RWIN, VK_LSHIFT, VK_RSHIFT]
        .into_iter()
        .filter(|vk| unsafe { GetAsyncKeyState(vk.0 as i32) } < 0)
        .collect()
}

fn with_clean_modifiers(body: Vec<INPUT>) -> bool {
    let held = held_modifiers();
    let mut all: Vec<INPUT> = held.iter().map(|&vk| key(vk, 0, KEYEVENTF_KEYUP)).collect();
    all.extend(body);
    all.extend(held.iter().map(|&vk| key(vk, 0, KEYBD_EVENT_FLAGS(0))));
    send(&all)
}

pub fn replace(backspaces: usize, text: &str) -> bool {
    let mut out = Vec::with_capacity(backspaces * 2 + text.len() * 4);
    for _ in 0..backspaces {
        tap(&mut out, VK_BACK);
    }
    for unit in text.encode_utf16() {
        out.push(key(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE));
        out.push(key(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
    }
    with_clean_modifiers(out)
}

pub fn rewrite(old: &str, new: &str) -> bool {
    let same = old.chars().zip(new.chars()).take_while(|(a, b)| a == b).count();
    let tail: String = new.chars().skip(same).collect();
    replace(old.chars().count() - same, &tail)
}

pub fn enter() -> bool {
    let mut out = Vec::new();
    tap(&mut out, VK_RETURN);
    send(&out)
}

pub fn chord(modifier: VIRTUAL_KEY, vk: VIRTUAL_KEY) -> bool {
    let mut out = vec![key(modifier, 0, KEYBD_EVENT_FLAGS(0))];
    tap(&mut out, vk);
    out.push(key(modifier, 0, KEYEVENTF_KEYUP));
    with_clean_modifiers(out)
}

pub fn replay(keys: &[RawKey]) -> bool {
    let out: Vec<INPUT> = keys
        .iter()
        .map(|k| {
            let mut flags = KEYBD_EVENT_FLAGS(0);
            if k.up {
                flags |= KEYEVENTF_KEYUP;
            }
            if k.extended {
                flags |= KEYEVENTF_EXTENDEDKEY;
            }
            key(VIRTUAL_KEY(k.vk), k.scan, flags)
        })
        .collect();
    send(&out)
}
