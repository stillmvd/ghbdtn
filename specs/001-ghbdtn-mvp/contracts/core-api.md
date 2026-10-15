# Контракт: крейт `ghbdtn-core`

Публичное API ядра для оболочки Tauri и для тестов. Чистая часть — без `cfg(windows)`.

## Чистая часть

```rust
pub enum Lang { Ru, En }

pub fn render(keys: &[Key], lang: Lang) -> String;
pub fn convert_text(text: &str, from: Lang) -> String;

pub struct Dictionaries;
impl Dictionaries {
    pub fn load() -> Dictionaries;
    pub fn known(&self, lang: Lang, word: &str) -> bool;
    pub fn freq(&self, lang: Lang, word: &str) -> u32;
}

pub fn is_code_token(word: &str, code_mode: bool) -> bool;

pub fn decide(input: &WordInput, dicts: &Dictionaries, cfg: &Config) -> Decision;
pub struct WordInput { pub keys: Vec<Key>, pub lang: Lang, pub sep: Separator, pub code_mode: bool }
pub enum Decision { None, Layout { text: String, sep: String, to: Lang }, Typo { text: String }, Replace { text: String } }

pub struct UndoBook;
impl UndoBook {
    pub fn record(&mut self, action: Action);
    pub fn take(&mut self) -> Option<Action>;
    pub fn close(&mut self);
    pub fn note_undo(&mut self, typed: &str) -> bool;
}

pub struct Config;
impl Config {
    pub fn load(path: &Path) -> Result<Config, ConfigError>;
    pub fn save(&self, path: &Path) -> io::Result<()>;
    pub fn validate(&self) -> Result<(), Vec<ConfigError>>;
}
```

Гарантии: `decide` детерминирован, без I/O; `note_undo` возвращает `true`, когда слово надо
добавить в `never` (вторая отмена).

## Win32-часть (`cfg(windows)`)

```rust
pub struct Engine;
impl Engine {
    pub fn start(cfg: Config, events: Sender<CoreEvent>) -> Engine;
    pub fn set_config(&self, cfg: Config);
    pub fn stop(self);
}

pub enum CoreEvent {
    Acted { kind: ActionKind, typed: String, result: String, caret: Option<(i32, i32)> },
    Undone,
    Status(Status),
    NeverAdded(String),
}
pub enum Status { Working, Disabled, Silent(SilentReason) }
pub enum SilentReason { Process, Password, Fullscreen, Elevated, OtherLayout, NoRuEn }
```

`CoreEvent::Acted` несёт слова только в памяти процесса — для подсказки у курсора; в лог и на диск
не попадает (конституция I).
