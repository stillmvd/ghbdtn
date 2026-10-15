# Tasks: Ghbdtn MVP — исправление набора на лету

**Input**: Design documents from `specs/001-ghbdtn-mvp/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: запрошены пользователем — ядро покрывается `cargo test`: юнит-тесты в модулях
(`#[cfg(test)] mod tests` в том же файле) и корпус `src-tauri/core/tests/corpus.rs`.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: можно параллельно (другие файлы, нет незавершённых зависимостей)
- **[Story]**: US1…US7 из spec.md
- Пути — от корня репозитория; ядро — `src-tauri/core/`

## Phase 1: Setup

- [X] T001 Создать workspace `src-tauri/Cargo.toml` (members `["core"]`, пакет `ghbdtn`, Tauri 2 с фичей `tray-icon`) и крейт `src-tauri/core/Cargo.toml` (`ghbdtn-core`; deps: `spellbook = "0.4"`, `crossbeam-channel = "0.5"`, `arc-swap = "1"`, `serde` derive, `toml`, `zeroize`; `[target.'cfg(windows)'.dependencies]` `windows = "0.62"` с фичами Win32_Foundation, Win32_UI_WindowsAndMessaging, Win32_UI_Input_KeyboardAndMouse, Win32_UI_TextServices, Win32_System_Threading, Win32_Security, Win32_UI_Accessibility, Win32_System_Com, Win32_UI_Shell, Win32_Graphics_Gdi, Win32_System_DataExchange, Win32_System_RemoteDesktop, Win32_Media_Audio; `clipboard-win`), `src-tauri/core/src/lib.rs` с модулями из plan.md
- [X] T002 [P] Положить словари в `src-tauri/core/data/`: `ru.aff`, `ru.dic`, `en.aff`, `en.dic` из `wooorm/dictionaries` и `ru.freq`, `en.freq` — топ-50K строк FrequencyWords 2018 (`слово частота`); `LICENSES.md` с лицензиями и пометкой «усечено до 50K»
- [X] T003 [P] Каркас фронтенда как в Booked: `package.json` (react 19, vite 7, typescript ~5.8, @tauri-apps/api и cli 2.11.x; скрипт `check` = tsc + clippy `-D warnings` + cargo test для workspace), `index.html`, `vite.config.ts`, `tsconfig.json`, `src/main.tsx`, `src-tauri/tauri.conf.json` (окон при старте нет), `src-tauri/build.rs`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs` с пустым `run()`
- [X] T004 Проверить каркас: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `pnpm exec tsc --noEmit`, `pnpm build` — зелёные

## Phase 2: Foundational (Blocking)

- [X] T005 [P] `src-tauri/core/src/keymap.rs`: `Lang { Ru, En }`, таблицы скан-код → (обычный, Shift) для US QWERTY и ЙЦУКЕН (ряды 0x02–0x0D, 0x10–0x1B, 0x1E–0x29, 0x2B–0x35, 0x39 пробел), `char_of(scan, shift, caps, lang)` (Caps влияет только на буквы), `convert_text(text, from)` посимвольно через таблицы; тесты: `ghbdtn`→`привет`, `сщву`→`code`, `,tp`→`без`, регистр `Ghbdtn`→`Привет`, Caps
- [X] T006 [P] `src-tauri/core/src/word.rs`: `Key { scan, shift, caps }`, `Word { keys ≤ 64, lang }` (push, backspace, clear с `zeroize`, переполнение → clear), `render(lang)`, `Separator { Space, Punct(char), Enter }`, `is_letter_in(scan, lang)`; тесты
- [X] T007 [P] `src-tauri/core/src/dict.rs`: `Dictionaries::load()` — `spellbook::Dictionary` ru/en из `include_str!` + частоты в `HashMap<String,u32>`; `known(lang, w)` = Hunspell.check || freq > 0 (проверка в нижнем регистре и как есть), `freq(lang, w)`; тест: привет/ввода/claude/hello известны, ввожа/ghbdtn/сщву — нет; загрузка < 200 мс в release
- [X] T008 [P] `src-tauri/core/src/token.rs`: `is_code_token(word, code_mode)` — FR-063: цифры, `_ . / \ @ # ::`, camelCase/PascalCase (заглавная после строчной), ВСЕ ЗАГЛАВНЫЕ (≥2 букв), URL/e-mail/пути; при `code_mode` ещё FR-065: начинается с `- ~ $ %`, содержит `= : * | < >`; тесты на `getUserName`, `http://ya.ru`, `v2rayN`, `--watch`, `~/Projects`, `$HOME`, `a=b`, `NASA`, `привет`
- [X] T009 [P] `src-tauri/core/src/config.rs`: `Config` по data-model.md с serde/TOML и значениями по умолчанию (включая списки `processes.off` и `processes.code` дословно из data-model.md, `replacements = { "чето" = "чё-то" }`, звук false, автозапуск true, `hotkeys.convert = "Pause"`, `hotkeys.toggle = "Shift+Pause"`); `validate()`: «ключ замены — буквы и дефис, 1–32 символа, без пробелов; замена — 1–200 символов, одна строка; дубликаты ключей без учёта регистра запрещены; `never` — уникальные слова в нижнем регистре; имена процессов — нижний регистр, `.exe` добавляется при отсутствии»; `load(path)` (ошибка → `Err`, файл не трогать), `save(path)` атомарно (tmp + rename); `mode_for(exe) -> Off|Code|Normal`; тесты round-trip и валидации
- [X] T010 `src-tauri/core/src/decide.rs`: `WordInput`, `Decision`, `decide()` — конвейер `never` → replace → layout → typo с выключателями `features.*`; фильтр `is_code_token`; на этой фазе ветки replace/layout/typo — заглушки `None` (TODO на соответствующие задачи)
- [X] T011 [P] `src-tauri/core/src/undo.rs`: `Action`, `ActionKind`, `UndoBook { record, take, close, note_undo }` — `note_undo` считает отмены слова в памяти и возвращает `true` на второй; тесты
- [X] T012 `src-tauri/core/src/win/hook.rs`: поток с `SetWindowsHookExW(WH_KEYBOARD_LL)` + `GetMessageW`; колбэк без аллокаций: пропуск событий с `dwExtraInfo == 0x4748_4244`; чужие `LLKHF_INJECTED` → событие `Foreign`; `try_send` в bounded(1024) канал `RawKey { vk, scan, flags, time }`; атомики `HOLD`, `ARMED_UNDO`, `PENDING_WORD`, `HOTKEYS_ON` и решение «съесть» (Ctrl+Backspace при ARMED_UNDO, Pause/Shift+Pause при HOTKEYS_ON, Enter при PENDING_WORD, всё при HOLD → в канал с пометкой held); `catch_unwind`; `reinstall()`; watchdog по `GetLastInputInfo` каждые 2 с; переустановка на `WTS_SESSION_UNLOCK` (`WTSRegisterSessionNotification`) и `PBT_APMRESUMEAUTOMATIC`
- [X] T013 [P] `src-tauri/core/src/win/output.rs`: `send_fix(backspaces, text, sep)` одним `SendInput` (Backspace vk, текст `KEYEVENTF_UNICODE` с суррогатами, маркер в `dwExtraInfo`), `send_key(vk, scan, up)` для задержанных и Enter, `send_ctrl(vk)` (Ctrl+C/V); проверка возврата SendInput
- [X] T014 [P] `src-tauri/core/src/win/lang.rs`: `current_lang(hwnd) -> Option<Lang>` по `GetKeyboardLayout(GetWindowThreadProcessId)` (0x0419 ru, 0x0409 en, IME/иное → None), `hkl_for(lang)` из `GetKeyboardLayoutList`, `switch(hwnd_focus, lang)` — `PostMessageW(WM_INPUTLANGCHANGEREQUEST)` + опрос до 50 мс
- [X] T015 [P] `src-tauri/core/src/win/context.rs`: поток с `SetWinEventHook(EVENT_SYSTEM_FOREGROUND..EVENT_OBJECT_FOCUS, WINEVENT_OUTOFCONTEXT)`; `Context { hwnd, focus, pid, exe, lang, elevated, fullscreen, password, mode }` в `ArcSwap`; exe через `QueryFullProcessImageNameW` (ApplicationFrameHost → дочерний CoreWindow), elevated `TokenElevation`, fullscreen `SHQueryUserNotificationState` + окно = монитор, пароль `ES_PASSWORD` у `GetGUIThreadInfo.hwndFocus`, иначе UIA `CurrentIsPassword` (COM в этом потоке); каждое событие фокуса шлёт `Reset` в движок; `allowed(cfg, feature)` в порядке FR-061
- [X] T016 `src-tauri/core/src/win/mod.rs`: `Engine::start(cfg, events)` — потоки hook/context/engine; цикл engine: `RawKey` → Word (буквы по `keymap`, Backspace, сброс по навигации/Ctrl/Alt/Win/мыши/`Foreign`/простою 10 с), установка `PENDING_WORD`, на разделителе `decide()` → `HOLD=true` → `output` + `lang::switch` → переотправка задержанных → `HOLD=false` → `UndoBook.record` + `ARMED_UNDO=true` → `CoreEvent::Acted`; Enter съеден → после решения `send_key(Enter)`; `set_config` через `ArcSwap`; `CoreEvent`, `Status`, `SilentReason` по contracts/core-api.md
- [X] T017 `src-tauri/core/examples/live.rs`: запуск `Engine` с конфигом из `%APPDATA%\Ghbdtn\config.toml` (или по умолчанию), вывод в консоль только `ActionKind`/статусов без слов, выход по Ctrl+C
- [X] T018 `src-tauri/core/tests/corpus.rs`: каркас корпуса — топ-10K слов из `data/{ru,en}.freq`, печать долей; пороги SC-001…SC-003 как `assert!` (тест падает, пока ветки не реализованы — помечен `#[ignore]` до T020)

**Checkpoint**: живой движок ловит слова и ничего не меняет; `cargo test` зелёный.

## Phase 3: US1 — Слово в не той раскладке (P1) 🎯 MVP

**Goal**: «ghbdtn » → «привет », раскладка переключена; Enter в чате отправляет исправленное.
**Independent Test**: quickstart, строки «Блокнот, EN/RU» и «Telegram/Slack».

- [X] T019 [US1] `src-tauri/core/src/layout.rs`: `decide_layout(keys, lang, sep, dicts, code_mode) -> Option<(text, sep, to)>` по research R3: длина < 3 — нет; typed не known в своём языке И alt known в другом; длина 3–4 — alt ещё и `freq > 0`; хвостовой знак, который в другой раскладке буква — сначала без хвоста (знак остаётся литералом), затем с хвостом как буквой (`,tp ` → `без `); регистр по Shift/Caps каждой клавиши; тесты: ghbdtn, сщву, `ghbdtn,`, `,tp`, hello/привет без изменений, `ye`, `yt`, `NASA`, `Ghbdtn`
- [X] T020 [US1] Подключить layout в `src-tauri/core/src/decide.rs`, снять `#[ignore]` с части корпуса SC-001/SC-002 в `src-tauri/core/tests/corpus.rs`, добиться порогов (≤ 0,1 % ложных, ≥ 95 % исправленных для длины ≥ 3)
- [X] T021 [US1] В `src-tauri/core/src/win/mod.rs`: после Layout — `lang::switch` в окно фокуса; следующее слово начинает Word с новой `lang`; если смена не подтвердилась за 50 мс — слово всё равно исправлено, статус без ошибки

## Phase 4: US2 — Отмена (P1)

**Goal**: Ctrl+Backspace сразу после действия откатывает слово и раскладку; вторая отмена → «не трогать».
**Independent Test**: quickstart, строки «после любого исправления» и «то же слово второй раз».

- [X] T022 [US2] В `src-tauri/core/src/win/mod.rs`: обработка съеденного Ctrl+Backspace — `UndoBook.take()` → Backspace × (len(result)+len(sep)) + typed + sep → `lang::switch(lang_before)` → `ARMED_UNDO=false` → `note_undo(typed)`; при `true` — добавить в `Config.never` (source `undo`), сохранить конфиг, `CoreEvent::NeverAdded`; любая другая клавиша/клик/фокус → `close()` и `ARMED_UNDO=false`
- [X] T023 [US2] Тест в `src-tauri/core/src/undo.rs` и `src-tauri/core/src/decide.rs`: слово из `never` не меняется ни одной веткой

## Phase 5: US5 — Выключатели и исключения (P1)

**Goal**: общий выключатель Shift+Pause, выключатели функций, исключения, режим кода в IDE/терминалах.
**Independent Test**: quickstart, строки «Shift+Pause», «Windows Terminal», «поле пароля».

- [X] T024 [US5] В `src-tauri/core/src/win/mod.rs`: Shift+Pause переключает `Config.enabled`, сохраняет конфиг, `CoreEvent::Status`; при выключенном — буфер не копится
- [X] T025 [US5] В `src-tauri/core/src/win/mod.rs`: перед накоплением слова и перед действием — `Context::allowed` (FR-061); `Off`-процесс, пароль, fullscreen, elevated, чужая раскладка → не копить клавиши, `Status::Silent(reason)`; `Code`-процесс → `WordInput.code_mode = true`; при старте и смене списка раскладок — нет ru или en в `GetKeyboardLayoutList` → `Status::Silent(NoRuEn)`, автосмена и ручная конвертация не работают
- [X] T026 [P] [US5] Тесты «трудного набора» в `src-tauri/core/tests/corpus.rs`: команды, флаги, пути, ники, аббревиатуры, camelCase в режиме кода и без — ноль изменений

## Phase 6: US3 — Опечатки (P2)

**Goal**: «ввожа клавиатра » → «ввода клавиатура » только при одном явном кандидате.
**Independent Test**: quickstart, строка «ввожа текса».

- [X] T027 [US3] `src-tauri/core/src/typo.rs`: карта соседства клавиш по физическим позициям скан-кодов; генератор правок dist 1 (удаление, вставка, замена, перестановка соседних) по алфавиту языка; вес по research R4; `best_fix(word, lang, dicts) -> Option<String>` (длина ≥ 4, кандидат known и freq > 0, вес ≤ 0,6, отрыв score ≥ ln 20); перенос регистра первой буквы и ВСЕ ЗАГЛАВНЫЕ; тесты: ввожа→ввода, текса→текста, известное слово не трогать, два равных кандидата — нет
- [X] T028 [US3] Подключить typo в `src-tauri/core/src/decide.rs`, снять `#[ignore]` с SC-003 в `src-tauri/core/tests/corpus.rs` (верно ≥ 70 %, неверно ≤ 1 %), подобрать пороги

## Phase 7: US4 — Словарь замен (P2)

**Goal**: «чето » → «чё-то », «Чето » → «Чё-то ».
**Independent Test**: quickstart, строка «чето / Чето».

- [X] T029 [US4] `src-tauri/core/src/replace.rs`: поиск ключа без учёта регистра; перенос регистра (первая заглавная → первая заглавная замены; ВСЕ ЗАГЛАВНЫЕ → замена заглавными); подключить в `src-tauri/core/src/decide.rs` перед layout; тесты
- [X] T030 [US4] Ключи замен считаются known в `src-tauri/core/src/dict.rs` (не исправляются как опечатки и раскладка), тест в `src-tauri/core/src/decide.rs`

## Phase 8: US6 — Ручная конвертация (P2)

**Goal**: Pause перепечатывает последнее слово или выделение в другой раскладке.
**Independent Test**: quickstart, строки «ye → Pause» и «выделить Ghbdtn vbh».

- [X] T031 [US6] В `src-tauri/core/src/win/mod.rs`: «последнее слово» живёт после разделителя до следующего нажатия не-буквы/сброса; Pause при нём — Backspace × (len+sep) + `render(other)` + sep, `lang::switch`, `Action { kind: Manual }`, окно отмены
- [X] T032 [US6] `src-tauri/core/src/win/clipboard.rs`: без последнего слова — сохранить буфер обмена (текст), запомнить `GetClipboardSequenceNumber`, `send_ctrl(C)`, ждать смены до 150 мс, `convert_text` по раскладке окна, записать, `send_ctrl(V)`, через 300 мс восстановить прежний буфер; нет смены — ничего не делать

## Phase 9: US7 — Трей и сигналы (P3)

**Goal**: значок со статусом, меню, окно настроек, подсказка у курсора, мигание, звук, анимация.
**Independent Test**: spec.md US7.

- [x] T033 [US7] Стенд-артефакт вариантов: значок трея (3 состояния), меню, окно настроек (выключатели, замены, «не трогать», исключения, клавиши, автозапуск, «О программе»), подсказка у курсора, анимация курсора — стиль `~/.claude/brand/BRAND.md`; ждать выбор пользователя
- [x] T034 [US7] `src-tauri/src/lib.rs`: single-instance (второй запуск открывает настройки), autostart (при старте синхронизировать запись HKCU Run с `Config.autostart`, по умолчанию true), запуск `Engine`, проброс `CoreEvent` в трей/подсказку, команды из contracts/ui-ipc.md
- [x] T035 [US7] `src-tauri/src/tray.rs`: значок трёх состояний, меню по contracts/ui-ipc.md, левый клик — общий выключатель, мигание при `Acted`
- [x] T036 [US7] `src-tauri/src/hint.rs` + `src/hint/`: прозрачное окно без фокуса у каретки (`GetGUIThreadInfo.rcCaret`, иначе курсор мыши), «было → стало» ~1,5 с, анимация по выбору со стенда; звук `PlaySoundW` при `signals.sound`
- [x] T037 [US7] `src/settings/`: окно настроек по выбранному стенду — `get_config`/`set_config` с показом ошибок валидации у полей, список процессов, «не трогать» с удалением и очисткой, «О программе» с атрибуцией из `src-tauri/core/data/LICENSES.md`

## Phase 10: Polish

- [x] T038 [P] Подключить tauri-ship по `C:\Users\stillmvd\Projects\Tauri Ship\README.md`: зависимости, плагины, `tauri.conf.json` (version из package.json, NSIS-шаблон, passive), `<UpdateToast lang="ru" />`; `pnpm ship doctor`
- [x] T039 [P] Проверка приватности (SC-008): поиск набранных слов в `%APPDATA%\Ghbdtn` и логах после сессии; `grep` по коду — нет записи `typed`/`result` в лог/файл. Выполнено 2026-10-15: в коде запись на диск только конфиг и журнал (по выключателю), сеть — только updater; 37 исходных опечаток из журнала не найдены в 382 файлах `%APPDATA%Ghbdtn` и `%LOCALAPPDATA%m.stillmvd.ghbdtn` (кроме журнала); буфер при конвертации выделения помечается исключённым из истории и облака
- [ ] T040 Прогон quickstart.md целиком, обновление `HANDOFF.md`
- [ ] T041 Скорость и живучесть (SC-004, SC-005, SC-007): в `src-tauri/core/tests/corpus.rs` — среднее время `decide()` < 2 мс в release; в `src-tauri/core/examples/live.rs` — счётчики: время от разделителя до конца `SendInput`, число событий хука и переотправленных задержанных клавиш, число переустановок хука; час живой работы — потерь клавиш 0, переустановок без сна 0. Счётчики сделаны 2026-10-15 (`live.rs` раз в минуту, проверка в конце selftest); decide avg в корпусе 1,3 мс; час живой работы — не прогнан

## Dependencies & Execution Order

- Phase 1 → Phase 2 → US1 → (US2, US5 параллельно) → (US3, US4, US6 параллельно) → US7 → Polish.
- US2 требует хотя бы одного действия — после US1. US3/US4 — только `decide.rs` и свои модули.
- US7 начинается со стенда T033 и ждёт выбор пользователя; ядро к этому моменту проверено живьём.

## Parallel Example: Phase 2

```text
T005 keymap.rs | T006 word.rs | T007 dict.rs | T008 token.rs | T009 config.rs | T011 undo.rs
T013 output.rs | T014 lang.rs | T015 context.rs
```

## Implementation Strategy

1. MVP = Phase 1–2 + US1 + US2 + US5 → живая проверка пользователем (раскладка, отмена, исключения).
2. Затем US3, US4, US6 → повторная живая проверка.
3. Стенд UI → US7 → tauri-ship.
4. После каждой фазы: clippy `-D warnings`, `cargo test`, `tsc`, build, `HANDOFF.md`.
