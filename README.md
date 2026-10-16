<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="brand/logo/lockup-on-dark.svg">
    <img src="brand/logo/lockup.svg" alt="Ghbdtn" height="64">
  </picture>
</p>

Ghbdtn — утилита для Windows 11. Исправляет слова, набранные не в той раскладке, и опечатки — сразу после пробела.

- `ghbdtn vbh` → `привет мир`, `сщву` → `code` — раскладка переключается сама.
- `ввожа клавиатра` → `ввода клавиатура` — опечатки по словарю и соседним словам.
- Свои замены: `чето` → `чё-то`.
- Ctrl+Backspace сразу после исправления — вернуть как было; второй откат того же слова — больше его не трогать.
- Pause — перевести последнее слово или выделение в другую раскладку, Shift+Pause — выключить и включить.
- Выключается целиком, по функциям и по программам; в полях пароля молчит.

Набранный текст не пишется на диск и никуда не отправляется.

## Установка

Установщик — на странице [Releases](https://github.com/stillmvd/ghbdtn/releases). Обновления приходят сами.

## Разработка

```powershell
pnpm install
pnpm tauri dev
pnpm check
```

Знак и иконка: `python brand/logo/generate.py`, затем `python brand/make-icons.py`.
