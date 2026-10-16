from wave1 import glyph_svg
from wave3 import WEIGHTS, caret
from wave4 import SCHEMES

_, BG, INK = SCHEMES["lime"]
SHARES = [66, 72, 78, 84]

NUMBER = 5
TITLE = "Волна 5 · размер знака в круге"
PROMPT = ("A1 жирной линией в лайме. Меняю только долю каретки по высоте круга, сейчас она 62 %. "
          "Выбери одну — с ней пойдёт раскатка иконок.")
CHOSEN = ["A1 72 %"]
NOTES = {
    66: "Чуть крупнее текущего: много воздуха, круг читается первым, знак вторым.",
    72: "Баланс: искра не прижата к краю, на 24 px каретка уже заметна.",
    78: "Знак главнее круга, искра ближе к краю — на 16–24 px ещё держится.",
    84: "Максимум: круг становится подложкой, искра у самого края и на 16 px сливается с ним."
}
SUMMARY = ("Моё мнение: 72 %. Значки Windows 11 занимают около 80 % клетки, но у нас фон — сам круг, "
           "и при 78 % и выше искра выходит к краю. 72 % держит каретку крупной на панели задач и оставляет "
           "искре воздух.")

CARDS = [("Доля знака", [{"code": "A1", "name": "Каретка и искра · лайм", "radio": True,
                          "note": " ".join(f"{p} %: {NOTES[p]}" for p in SHARES),
                          "tiles": [(f"A1 {p} %", f"{p} %", str(p), True) for p in SHARES]}])]

CSS = """
.compare{display:grid;gap:10px;grid-template-columns:repeat(auto-fit,minmax(min(100%,420px),1fr))}
.compare .build-tile{justify-items:stretch}
.compare-row{display:grid;grid-template-columns:repeat(4,1fr);gap:8px;justify-items:center}
[data-label="Волна 5"] .grid.wide{grid-template-columns:1fr}
"""


def mark(key):
    share = int(key)
    return f'<circle cx="32" cy="32" r="32" fill="{BG}"/>' + glyph_svg(caret(WEIGHTS["bold"]), 64 * share / 100, INK)


def variant_svg(cid, key, in_circle):
    return mark(key)


def preview():
    def column(p, px):
        return (f'<span class="size"><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="{px}" '
                f'height="{px}" aria-hidden="true">{mark(str(p))}</svg>{p} %</span>')

    tiles = "".join(
        f'<div class="build-tile {theme}">'
        + "".join(f'<div class="compare-row">{"".join(column(p, px) for p in SHARES)}</div>' for px in (48, 32, 24, 16))
        + '</div>' for theme in ("light", "dark"))
    return f'<div class="build"><h3>Рядом на светлой и тёмной панели · 48 / 32 / 24 / 16 px</h3><div class="compare">{tiles}</div></div>'
