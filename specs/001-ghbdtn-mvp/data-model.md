# Data Model: Ghbdtn MVP

Всё, кроме `Config`, живёт только в ОЗУ (конституция I).

## Key — нажатие в буфере

| Поле | Тип | Правило |
|---|---|---|
| `scan` | u16 | скан-код физической клавиши |
| `shift` | bool | Shift был зажат |
| `caps` | bool | Caps Lock был включён |

## Word — набираемое слово

| Поле | Тип | Правило |
|---|---|---|
| `keys` | Vec<Key> (≤ 64) | Backspace удаляет последний; переполнение — сброс |
| `lang` | Lang (Ru, En) | раскладка окна на момент первой клавиши |

Производные: `render(Lang) -> String` по таблицам раскладок. Сброс (FR-002) затирает `keys`
(`zeroize`).

## Separator — завершение слова

`Space | Punct(char) | Enter`. `Punct` — `, . ? ! ; :` в текущей раскладке. Клавиша, дающая
знак в текущей и букву в другой (`,` `.` `;` `'` `[` `]` `` ` `` на en; `ю` `б` `ж` на ru — нет,
на ru знаки отдельные), передаётся решателю как возможная буква.

## Action — сделанное действие

| Поле | Тип | Правило |
|---|---|---|
| `kind` | Layout, Typo, Replace, Manual | — |
| `typed` | String | что было на экране до действия (без разделителя) |
| `result` | String | что стало |
| `sep` | Option<String> | разделитель, напечатанный после |
| `lang_before` / `lang_after` | Lang | для отката раскладки |

Жизненный цикл: создано → `ARMED_UNDO = true` → (Ctrl+Backspace → откат, `undo_count[typed] += 1`,
при 2 → `Config.never` += typed) | (любая другая клавиша/клик/фокус → закрыто).

## Decision — выход решателя

`None | Layout { text, to: Lang } | Typo { text } | Replace { text }`. Приоритет (FR-033):
`never` → `replacements` → layout → typo. Первое сработавшее — единственное.

## Context — активное окно (ArcSwap)

| Поле | Тип |
|---|---|
| `hwnd`, `pid` | — |
| `exe` | String, нижний регистр, `notepad.exe` |
| `lang` | Option<Lang> (None — другая раскладка или IME) |
| `elevated`, `fullscreen`, `password` | bool |
| `mode` | Off, Code, Normal — из `Config` по `exe` |

`allowed(feature)` — порядок FR-061.

## Config — `%APPDATA%\Ghbdtn\config.toml`

| Поле | Тип | По умолчанию |
|---|---|---|
| `enabled` | bool | true |
| `features.layout` / `typos` / `replace` | bool | true / true / true |
| `signals.hint` / `blink` / `sound` / `cursor` | bool | true / true / false / true |
| `hotkeys.convert` | String | `"Pause"` |
| `hotkeys.toggle` | String | `"Shift+Pause"` |
| `autostart` | bool | true |
| `replacements` | map String → String | `{ "чето" = "чё-то" }` |
| `never` | list of { word, source: manual/undo } | пусто |
| `processes.off` | list of exe | mstsc, anydesk, teamviewer, virtualbox(vm), vmconnect, keepass, keepassxc, bitwarden, 1password, winword, excel, powerpnt, outlook, onenote |
| `processes.code` | list of exe | windowsterminal, openconsole, conhost, cmd, powershell, pwsh, mintty, wezterm-gui, alacritty, code, cursor, windsurf, devenv, idea64, pycharm64, webstorm64, rider64, clion64, goland64, zed, sublime_text, notepad++ |

Валидация: ключ замены — буквы и дефис, 1–32 символа, без пробелов; замена — 1–200 символов,
одна строка; дубликаты ключей без учёта регистра запрещены; `never` — уникальные слова в нижнем
регистре; имена процессов — нижний регистр, `.exe` добавляется при отсутствии. Ошибка чтения
файла — работать на значениях по умолчанию, файл не перезаписывать, сообщить в трее.

## Dictionaries

| Слой | Источник | Операции |
|---|---|---|
| Hunspell ru/en | `data/{ru,en}.{aff,dic}` | `check(word)` |
| Частоты ru/en | `data/{ru,en}.freq` (топ-50K, `слово частота`) | `freq(word) -> u32` |
| Пользовательские | `Config.replacements` ключи, `Config.never` | точное совпадение |

`known(lang, word) = hunspell.check || freq > 0`. Словари встраиваются в бинарь (`include_str!`).
