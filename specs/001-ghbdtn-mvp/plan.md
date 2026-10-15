# Implementation Plan: Ghbdtn MVP — исправление набора на лету

**Branch**: `001-ghbdtn-mvp` | **Date**: 2026-10-15 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/001-ghbdtn-mvp/spec.md`

## Summary

Фоновая утилита в трее: ловит нажатия низкоуровневым хуком, держит текущее слово как скан-коды,
на разделителе считает его вид в ru и en и решает — замена из словаря, смена раскладки (двойное
подтверждение Hunspell + частоты) или уверенная опечатка (правка dist 1 с весами соседних клавиш).
Исправляет через `SendInput` (Backspace + Unicode), переключает раскладку окна, даёт откат
Ctrl+Backspace. Исключения по процессам, паролям, полному экрану, elevated; IDE и терминалы — в
«режиме кода». Ядро — крейт `ghbdtn-core` (чистая логика + тонкий Win32-слой), оболочка — Tauri 2
(трей, окно настроек по требованию, подсказка у курсора). Детали решений — [research.md](research.md).

## Technical Context

**Language/Version**: Rust 1.96 (edition 2021), TypeScript 5.8

**Primary Dependencies**: `windows` 0.62, `spellbook` 0.4, `crossbeam-channel` 0.5, `arc-swap` 1,
`serde` + `toml`, `zeroize`, `clipboard-win`; Tauri 2 (+ single-instance, autostart, updater,
tauri-plugin-ship), React 19, Vite 7

**Storage**: `%APPDATA%\Ghbdtn\config.toml`; словари встроены в бинарь

**Testing**: `cargo test` (юнит + корпус `tests/corpus.rs`), `tsc --noEmit`, живая проверка по
[quickstart.md](quickstart.md)

**Target Platform**: Windows 11 x64

**Project Type**: desktop-app (Tauri) + library crate

**Performance Goals**: колбэк хука < 50 мкс без аллокаций; решение по слову < 2 мс; исправление на
экране ≤ 150 мс после разделителя (SC-004)

**Constraints**: RAM в покое ≤ 60 МБ без окна настроек; ни байта набранного текста на диск и в сеть;
работа без прав администратора

**Scale/Scope**: 1 пользователь; ~50K + Hunspell слов на язык; 2 раскладки

## Constitution Check

| Принцип | Как выполнен | Статус |
|---|---|---|
| I. Приватность | слово — `Vec<Key>` в памяти с `zeroize`; `Action` живёт до закрытия окна отмены; лог — `ActionKind` без строк; сети нет (кроме updater tauri-ship по явной проверке) | ✅ |
| II. Мгновенный хук | колбэк: атомики + `try_send` в bounded-канал, `catch_unwind`; watchdog и переустановка | ✅ |
| III. Не ломать ввод | двойное подтверждение, фильтр кода во всех приложениях, dist 1 с отрывом, корпус как гейт | ✅ |
| IV. Отменяемость | `UndoBook` на каждое действие, включая ручную конвертацию и замены | ✅ |
| V. Исключения | `Context::allowed` в порядке FR-061 перед каждым действием; списки по умолчанию в `Config` | ✅ |

Повторная проверка после Phase 1: нарушений нет, Complexity Tracking пуст.

## Project Structure

### Documentation (this feature)

```text
specs/001-ghbdtn-mvp/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── core-api.md
│   └── ui-ipc.md
└── tasks.md
```

### Source Code (repository root)

```text
package.json, index.html, vite.config.ts, tsconfig.json
src/                         окно настроек и подсказка (React), после стенда
src-tauri/
├── Cargo.toml               workspace: members = ["core"]
├── tauri.conf.json
├── src/
│   ├── main.rs, lib.rs      Tauri: плагины, запуск Engine, команды
│   ├── tray.rs              меню и значок
│   └── hint.rs              окно подсказки у курсора
└── core/                    ghbdtn-core
    ├── Cargo.toml
    ├── data/                ru.aff ru.dic en.aff en.dic ru.freq en.freq LICENSES.md
    ├── src/
    │   ├── lib.rs
    │   ├── keymap.rs        таблицы скан-код → символ ru/en, convert_text
    │   ├── word.rs          Key, Word, Separator
    │   ├── dict.rs          Dictionaries (spellbook + частоты)
    │   ├── token.rs         is_code_token
    │   ├── layout.rs        решение о смене раскладки
    │   ├── typo.rs          правки dist 1, веса соседства
    │   ├── replace.rs       замены с переносом регистра
    │   ├── decide.rs        конвейер never → replace → layout → typo
    │   ├── undo.rs          UndoBook
    │   ├── config.rs        Config, defaults, validate, load/save
    │   └── win/             cfg(windows)
    │       ├── mod.rs       Engine: потоки, события
    │       ├── hook.rs      WH_KEYBOARD_LL, флаги, watchdog
    │       ├── output.rs    SendInput, маркер, задержанные клавиши
    │       ├── lang.rs      текущая раскладка, смена, HKL ru/en
    │       ├── context.rs   WinEvent, процесс, elevated, fullscreen, IME, пароль
    │       └── clipboard.rs выделение через буфер обмена
    ├── examples/live.rs     живая проверка без UI
    └── tests/corpus.rs      SC-001…SC-003
```

**Structure Decision**: workspace как в Booked (`src-tauri/core`). Логика решений в чистых модулях
и покрыта тестами без Win32; `win/` — тонкий слой, проверяется живьём по quickstart.

## Порядок работ

1. Ядро без Win32: keymap → word → dict → token → layout → replace → typo → decide → undo → config;
   корпус как гейт качества.
2. Win32-слой: hook → output → lang → context → clipboard → Engine; `examples/live.rs`.
3. Живая проверка пользователем по quickstart.
4. Стенд UI (трей, окно настроек, подсказка, анимация курсора) → выбор пользователя → оболочка Tauri.
5. Интеграция tauri-ship.

## Complexity Tracking

Нарушений конституции нет.
