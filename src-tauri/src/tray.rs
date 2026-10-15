use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::SeqCst};
use std::sync::Mutex;
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Look {
    On,
    Mute,
    Off,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Face {
    look: Look,
    ru: bool,
    light: bool,
}

macro_rules! png {
    ($name:literal) => {
        include_bytes!(concat!("../icons/tray/", $name, ".png"))
    };
}

fn bytes(light: bool, look: &str, ru: bool) -> &'static [u8] {
    match (light, look, ru) {
        (false, "on", true) => png!("dark-on-ru"),
        (false, "on", false) => png!("dark-on-en"),
        (false, "mute", true) => png!("dark-mute-ru"),
        (false, "mute", false) => png!("dark-mute-en"),
        (false, "off", true) => png!("dark-off-ru"),
        (false, "off", false) => png!("dark-off-en"),
        (false, _, true) => png!("dark-blink-ru"),
        (false, _, false) => png!("dark-blink-en"),
        (true, "on", true) => png!("light-on-ru"),
        (true, "on", false) => png!("light-on-en"),
        (true, "mute", true) => png!("light-mute-ru"),
        (true, "mute", false) => png!("light-mute-en"),
        (true, "off", true) => png!("light-off-ru"),
        (true, "off", false) => png!("light-off-en"),
        (true, _, true) => png!("light-blink-ru"),
        (true, _, false) => png!("light-blink-en"),
    }
}

fn image(face: Face, blink: bool) -> Option<Image<'static>> {
    let look = if blink { "blink" } else { match face.look { Look::On => "on", Look::Mute => "mute", Look::Off => "off" } };
    Image::from_bytes(bytes(face.light, look, face.ru)).ok()
}

pub struct Tray {
    icon: TrayIcon,
    face: Mutex<Face>,
    blink: AtomicU64,
    flashing: AtomicBool,
}

impl Tray {
    pub fn build(app: &AppHandle, on_left: impl Fn(&AppHandle, tauri::Rect) + Send + Sync + 'static) -> tauri::Result<Tray> {
        let face = Face { look: Look::On, ru: true, light: crate::sys::taskbar_light() };
        let menu = Menu::with_items(app, &[
            &MenuItem::with_id(app, "settings", "Настройки", true, None::<&str>)?,
            &MenuItem::with_id(app, "quit", "Выйти", true, None::<&str>)?,
        ])?;
        let mut builder = TrayIconBuilder::with_id("main")
            .tooltip("Ghbdtn")
            .menu(&menu)
            .show_menu_on_left_click(false)
            .on_menu_event(|app, e| match e.id.as_ref() {
                "settings" => crate::open_settings(app),
                "quit" => app.exit(0),
                _ => {}
            })
            .on_tray_icon_event(move |tray, e| {
                if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, rect, .. } = e {
                    on_left(tray.app_handle(), rect);
                }
            });
        if let Some(img) = image(face, false) {
            builder = builder.icon(img);
        }
        let icon = builder.build(app)?;
        Ok(Tray { icon, face: Mutex::new(face), blink: AtomicU64::new(0), flashing: AtomicBool::new(false) })
    }

    pub fn update(&self, look: Option<Look>, ru: Option<bool>, light: Option<bool>) {
        let mut face = self.face.lock().unwrap();
        let next = Face { look: look.unwrap_or(face.look), ru: ru.unwrap_or(face.ru), light: light.unwrap_or(face.light) };
        if next == *face {
            return;
        }
        *face = next;
        if !self.flashing.load(SeqCst) {
            let _ = self.icon.set_icon(image(next, false));
        }
    }

    pub fn flash(&'static self) {
        let id = self.blink.fetch_add(1, SeqCst) + 1;
        self.flashing.store(true, SeqCst);
        let set = move |on: bool| {
            if self.blink.load(SeqCst) == id {
                let face = *self.face.lock().unwrap();
                let _ = self.icon.set_icon(image(face, on));
            }
        };
        std::thread::spawn(move || {
            for on in [true, false, true] {
                set(on);
                std::thread::sleep(Duration::from_millis(130));
            }
            let face = self.face.lock().unwrap();
            if self.blink.load(SeqCst) == id {
                self.flashing.store(false, SeqCst);
                let _ = self.icon.set_icon(image(*face, false));
            }
        });
    }
}
