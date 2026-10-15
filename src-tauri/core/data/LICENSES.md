# Словари

| Файл | Источник | Лицензия |
|---|---|---|
| `ru.aff`, `ru.dic` | Hunspell ru_RU, © 1997–2008 Alexander I. Lebedev, через [wooorm/dictionaries](https://github.com/wooorm/dictionaries/tree/main/dictionaries/ru) | BSD-подобная (см. текст лицензии в репозитории источника) |
| `en.aff`, `en.dic` | Hunspell en_US из SCOWL, © Kevin Atkinson и др., через [wooorm/dictionaries](https://github.com/wooorm/dictionaries/tree/main/dictionaries/en) | permissive (SCOWL copyright) |
| `terms.txt` | [cspell-dicts](https://github.com/streetsidesoftware/cspell-dicts): software-terms, companies, fullstack, npm, node, python, rust, docker, k8s, aws, golang, typescript | MIT |
| `tech.txt`, `ru-it.dic` | составлены для Ghbdtn | как у проекта |
| `ru.bigram`, `ru.bigram.words` | пары соседних слов, посчитаны `examples/bigrams.rs` из [Leipzig Corpora Collection](https://wortschatz.uni-leipzig.de/en/download) rus-ru_web-public_2019_1M и rus_news_2024_1M (D. Goldhahn, T. Eckart, U. Quasthoff, LREC 2012) | CC BY |
| `ru.freq`, `en.freq` | [FrequencyWords](https://github.com/hermitdave/FrequencyWords) 2018, © Hermit Dave, на основе OpenSubtitles | CC-BY-SA-4.0 |

`ru.freq` и `en.freq` изменены: оставлены только слова из букв, взяты первые 150 000 (ru) и 100 000 (en)
(кириллица с дефисом / латиница с апострофом). Изменённые файлы распространяются под CC-BY-SA-4.0.
