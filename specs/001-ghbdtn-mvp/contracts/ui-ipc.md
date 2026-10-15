# Контракт: оболочка Tauri ↔ окна (флайаут, настройки, подсказка)

## Команды (`invoke`)

| Команда | Аргументы | Ответ | Ошибки |
|---|---|---|---|
| `get_config` | — | `Config` (JSON) | — |
| `set_config` | `config: Config` | `Config` сохранённый | `[{ field, message }]` — валидация, клавиши, автозапуск, файл |
| `list_processes` | — | `[{ exe, title }]` окон верхнего уровня | — |
| `remove_never` | `word: string` | `Config` | как `set_config` |
| `clear_never` | — | `Config` | как `set_config` |
| `set_autostart` | `on: bool` | `bool` | текст ошибки |
| `about` | — | `{ version, licenses: [{ name, source, license }] }` (из `core/data/LICENSES.md`) | — |
| `flyout_state` | — | `{ status, reason, app: { exe, name } \| null, count, ru, config }` | — |
| `open_settings_cmd` | — | — | — |

`status` — `"working" | "disabled" | "silent"`; `reason` — `process | password | fullscreen | elevated |
layout | noruen`. `app` — последнее окно переднего плана (кроме панели задач, проводника и своих окон),
`name` — описание файла exe. `count` — число исправлений за сегодня, только в памяти.

## События (`emit`)

| Событие | Данные | Когда |
|---|---|---|
| `status` | — | смена статуса, окна переднего плана, раскладки, после исправления; окна перечитывают `flyout_state` |
| `config-changed` | `Config` | любое сохранение и после второй отмены |
| `flyout-open` | — | перед показом флайаута |
| `hint` | `{ id, typed, result, hint, caret, word, mouse }` | действие утилиты; только окну `overlay`, не логируется |

`caret`, `word`, `mouse` — `{ x, y, w, h }` в физических пикселях от левого верхнего угла монитора или `null`.
Набранные слова в окно настроек и флайаут не передаются (FR-080, RESEARCH 6.2).

## Трей (стенд 001)

Значок «Раскладка»: плашка с текущей раскладкой RU/EN, три состояния (работает / молчит / выключено) × тема
панели задач. Левый клик — флайаут у значка: общий выключатель, «Раскладка», «Опечатки», «Замены»,
«Не трогать <приложение>», счётчик за сегодня, «Настройки». Правый клик — системное меню «Настройки / Выйти».

## Подсказка у курсора и анимация (стенд 001)

Окно `overlay` на весь монитор каретки: прозрачное, без фокуса (`WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW |
WS_EX_TRANSPARENT`, показ `SW_SHOWNOACTIVATE`), клики насквозь, скрывается через 1,7 с. Каретка —
`GetGUIThreadInfo.rcCaret`, иначе UI Automation, иначе курсор мыши. Бейдж с зачёркнутым старым словом над
словом или кареткой; подчёркивание акцентом — только если UI Automation отдала границы слова; искра у
стрелки мыши при `signals.cursor`; звук `PlaySoundW` при `signals.sound`.
