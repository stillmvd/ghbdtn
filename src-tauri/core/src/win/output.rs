use super::hook::{RawKey, MARK};
use std::sync::atomic::{AtomicU64, Ordering::SeqCst};
use windows::Win32::UI::Input::KeyboardAndMouse::*;

pub static SENT: AtomicU64 = AtomicU64::new(0);

const MODIFIERS: [VIRTUAL_KEY; 8] = [VK_LCONTROL, VK_RCONTROL, VK_LMENU, VK_RMENU, VK_LWIN, VK_RWIN, VK_LSHIFT, VK_RSHIFT];

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
    MODIFIERS.into_iter().filter(|vk| unsafe { GetAsyncKeyState(vk.0 as i32) } < 0).collect()
}

#[derive(Default)]
pub struct Batch {
    out: Vec<INPUT>,
    mods: Option<Vec<VIRTUAL_KEY>>,
}

impl Batch {
    fn clean(&mut self, body: Vec<INPUT>) {
        let held = self.mods.get_or_insert_with(held_modifiers).clone();
        self.out.extend(held.iter().map(|&vk| key(vk, 0, KEYEVENTF_KEYUP)));
        self.out.extend(body);
        self.out.extend(held.iter().map(|&vk| key(vk, 0, KEYBD_EVENT_FLAGS(0))));
    }

    pub fn replace(&mut self, backspaces: usize, text: &str) {
        let mut out = Vec::with_capacity(backspaces * 2 + text.len() * 4);
        for _ in 0..backspaces {
            tap(&mut out, VK_BACK);
        }
        for unit in text.encode_utf16() {
            out.push(key(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE));
            out.push(key(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
        }
        self.clean(out);
    }

    pub fn rewrite(&mut self, old: &str, new: &str) {
        let same = old.chars().zip(new.chars()).take_while(|(a, b)| a == b).count();
        let tail: String = new.chars().skip(same).collect();
        self.replace(old.chars().count() - same, &tail);
    }

    pub fn enter(&mut self) {
        tap(&mut self.out, VK_RETURN);
    }

    pub fn chord(&mut self, modifier: VIRTUAL_KEY, vk: VIRTUAL_KEY) {
        let mut out = vec![key(modifier, 0, KEYBD_EVENT_FLAGS(0))];
        tap(&mut out, vk);
        out.push(key(modifier, 0, KEYEVENTF_KEYUP));
        self.clean(out);
    }

    pub fn replay(&mut self, k: &RawKey) {
        let vk = VIRTUAL_KEY(k.vk);
        if MODIFIERS.contains(&vk) {
            let mods = self.mods.get_or_insert_with(held_modifiers);
            mods.retain(|&m| m != vk);
            if !k.up {
                mods.push(vk);
            }
        }
        let mut flags = KEYBD_EVENT_FLAGS(0);
        if k.up {
            flags |= KEYEVENTF_KEYUP;
        }
        if k.extended {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }
        self.out.push(key(vk, k.scan, flags));
    }

    pub fn is_empty(&self) -> bool {
        self.out.is_empty()
    }

    pub fn len(&self) -> usize {
        self.out.len()
    }

    pub fn send(&mut self) -> bool {
        self.mods = None;
        let ok = send(&self.out);
        self.out.clear();
        ok
    }

    #[cfg(test)]
    fn events(&self) -> Vec<(u16, u16, bool)> {
        self.out
            .iter()
            .map(|i| unsafe { (i.Anonymous.ki.wVk.0, i.Anonymous.ki.wScan, i.Anonymous.ki.dwFlags.contains(KEYEVENTF_KEYUP)) })
            .collect()
    }
}

pub fn chord(modifier: VIRTUAL_KEY, vk: VIRTUAL_KEY) -> bool {
    let mut b = Batch::default();
    b.chord(modifier, vk);
    b.send()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(vk: VIRTUAL_KEY, scan: u16, up: bool) -> RawKey {
        RawKey {
            vk: vk.0,
            scan,
            up,
            extended: false,
            shift: false,
            ctrl: false,
            alt: false,
            win: false,
            eaten: false,
            held: true,
            counted: true,
            at: std::time::Instant::now(),
        }
    }

    #[test]
    fn batch_keeps_order() {
        let mut b = Batch { out: Vec::new(), mods: Some(Vec::new()) };
        b.replay(&raw(VK_SPACE, 0x39, false));
        b.replay(&raw(VK_SPACE, 0x39, true));
        b.rewrite("ghbdtn ", "привет ");
        b.replay(&raw(VK_V, 0x2F, false));
        b.replay(&raw(VK_V, 0x2F, true));
        let ev = b.events();
        assert_eq!(ev[..2], [(VK_SPACE.0, 0x39, false), (VK_SPACE.0, 0x39, true)]);
        assert_eq!(ev[2..16].iter().filter(|e| e.0 == VK_BACK.0).count(), 14);
        let typed: String = ev[16..ev.len() - 2].iter().filter(|e| !e.2).map(|e| char::from_u32(e.1 as u32).unwrap()).collect();
        assert_eq!(typed, "привет ");
        assert_eq!(ev[ev.len() - 2..], [(VK_V.0, 0x2F, false), (VK_V.0, 0x2F, true)]);
    }

    #[test]
    fn batch_releases_replayed_shift_around_rewrite() {
        let mut b = Batch { out: Vec::new(), mods: Some(Vec::new()) };
        b.replay(&raw(VK_LSHIFT, 0x2A, false));
        b.rewrite("a", "b");
        b.replay(&raw(VK_LSHIFT, 0x2A, true));
        b.rewrite("c", "d");
        let ev = b.events();
        assert_eq!(ev.len(), 12);
        assert_eq!(ev[1], (VK_LSHIFT.0, 0, true));
        assert_eq!(ev[6], (VK_LSHIFT.0, 0, false));
        assert_eq!(ev[7], (VK_LSHIFT.0, 0x2A, true));
        assert!(ev[8..].iter().all(|e| e.0 != VK_LSHIFT.0));
    }
}
