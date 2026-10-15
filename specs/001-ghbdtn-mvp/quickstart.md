# Quickstart: проверка Ghbdtn MVP

## Предусловия

- Windows 11, раскладки «Русская» и «Английская (США)».
- Rust 1.85+, Node 24, pnpm.

## Автоматические проверки

```powershell
cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --workspace
cargo test --manifest-path src-tauri/Cargo.toml -p ghbdtn-core --release --test corpus -- --nocapture
pnpm exec tsc --noEmit
pnpm build
```

Корпус печатает доли: ложные ≤ 0,1 %, раскладка ≥ 95 %, опечатки верно ≥ 70 % / неверно ≤ 1 %.

## Живая проверка ядра (без UI)

```powershell
cargo run --manifest-path src-tauri/Cargo.toml -p ghbdtn-core --example live --release
```

Консоль пишет только факты («layout», «typo», «undo», «silent: password»), без слов.

| Где | Набрать | Ждать |
|---|---|---|
| Блокнот, EN | `ghbdtn vbh ` | `привет мир `, раскладка RU |
| Блокнот, RU | `сщву ` | `code `, раскладка EN |
| Блокнот, EN | `ghbdtn, ` | `привет, ` |
| Блокнот, RU | `ввожа клавиатра ` | `ввода клавиатура ` |
| Блокнот | `чето ` / `Чето ` | `чё-то ` / `Чё-то ` |
| после любого исправления | Ctrl+Backspace | исходное слово и раскладка |
| то же слово второй раз | Ctrl+Backspace | консоль: `never +1` |
| Блокнот, EN | `ye ` → Pause | `ну ` |
| Блокнот, выделить `Ghbdtn vbh` | Pause | `Привет мир`, буфер обмена прежний |
| любое окно | Shift+Pause, `ghbdtn ` | без изменений; Shift+Pause — снова работает |
| Windows Terminal, Claude Code | `cltkfq abrc ` | `сделай фикс ` |
| Windows Terminal, PowerShell | `git checkout -b feat/x ` | без изменений |
| поле пароля в браузере | `ghbdtn ` | без изменений, консоль: `silent: password` |
| Telegram/Slack, EN | `ghbdtn` + Enter | уходит «привет» |

## Приложение целиком

```powershell
pnpm tauri dev
```

Трей, меню, окно настроек, подсказка у курсора — сценарии US7 из `spec.md`.
