import math
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent
W = 4.6


def arc(cx, cy, r, a0, a1, step=2):
    n = max(2, int(abs(a1 - a0) / step) + 1)
    return [(cx + r * math.cos(math.radians(a0 + (a1 - a0) * i / (n - 1))),
             cy + r * math.sin(math.radians(a0 + (a1 - a0) * i / (n - 1)))) for i in range(n)]


def stroke(pts, w=W, cap="round", join="round", closed=False, fill=False):
    return {"pts": pts, "w": w, "cap": cap, "join": join, "closed": closed, "fill": fill}


def moved(items, dx=0.0, dy=0.0, k=1.0, mirror=False):
    out = []
    for it in items:
        pts = [((-x if mirror else x) * k + dx, y * k + dy) for x, y in it["pts"]]
        out.append({**it, "pts": pts, "w": it["w"] * (1 if it["fill"] else k)})
    return out


def star(cx, cy, r, inner=0.28, rays=4, turn=-90):
    pts = []
    for i in range(rays * 2):
        rr = r if i % 2 == 0 else r * inner
        a = math.radians(turn + i * 180 / rays)
        pts.append((cx + rr * math.cos(a), cy + rr * math.sin(a)))
    return stroke(pts, 0.8, join="round", closed=True, fill=True)


def letter_r(w=W):
    return [stroke([(0, 30), (0, 0), (9, 0)] + arc(9, 8, 8, -90, 90) + [(0, 16)], w),
            stroke([(7, 16), (16, 30)], w)]


def letter_g(w=W):
    cx, cy, r = 14, 15, 15
    pts = arc(cx, cy, r, -38, -322)
    ex, ey = pts[-1]
    return [stroke(pts + [(ex, cy + 1), (cx + 2, cy + 1)], w, join="miter")]


def letter_p(w=W, width=20):
    return [stroke([(0, 30), (0, 0), (width, 0), (width, 30)], w, join="miter")]


def caret_spark():
    return [stroke([(0, 4), (0, 34)]), stroke([(-6, 4), (6, 4)]), stroke([(-6, 34), (6, 34)]),
            star(13, -2, 9)]


def cursor_spark():
    arrow = [(1, 1), (1, 17.5), (5.3, 13.4), (8.2, 20), (10.9, 18.8), (8.1, 12.4), (13.8, 12.4)]
    return [stroke([(x * 1.8, y * 1.8 + 6) for x, y in arrow], 0.8, closed=True, fill=True),
            star(27, 2, 8)]


def g_p():
    return letter_g() + moved(letter_p(), dx=34)


def keycap():
    s, r = 46, 10
    box = (arc(r, r, r, 180, 270) + arc(s - r, r, r, 270, 360) + arc(s - r, s - r, r, 0, 90) + arc(r, s - r, r, 90, 180))
    return ([stroke(box, 3.6, closed=True)] + moved(letter_g(), dx=8, dy=8, k=0.42)
            + moved(letter_p(), dx=22, dy=21, k=0.6))


def ya_r():
    return letter_r() + moved(letter_r(), mirror=True)


def r_ya():
    return letter_r() + moved(letter_r(), dx=38, mirror=True)


VARIANTS = [
    ("a1-caret", "Каретка и искра", caret_spark),
    ("a2-cursor", "Курсор и искра", cursor_spark),
    ("b1-gp", "G и П", g_p),
    ("b2-key", "Клавиша G/П", keycap),
    ("c1-yar", "ЯR на одной ножке", ya_r),
    ("c2-rya", "R и Я", r_ya),
]

NUMBER = 1
TITLE = "Волна 1 · направления"
PROMPT = ("Отметь, что вести во вторую волну, — можно несколько. Каждый знак показан без подложки "
          "(заголовок окна, README) и в круге (иконка приложения): отмечай нужную версию.")
CHOSEN = ["A1 в круге", "C1 в круге"]
GROUPS = [("A · Каретка и искра", ["a1-caret", "a2-cursor"]),
          ("B · Одна клавиша G/П", ["b1-gp", "b2-key"]),
          ("C · R и Я — зеркало раскладок", ["c1-yar", "c2-rya"])]
TILES = {key: [("без подложки", False), ("в круге", True)] for key, *_ in VARIANTS}
NOTES = {
    "a1-caret": "Текстовый курсор, у которого вспыхивает искра, — как на стенде 001 («Курсор C»). Про момент исправления, без букв.",
    "a2-cursor": "Стрелка мыши с искрой. Узнаётся сразу, но про мышь, а Ghbdtn — про клавиатуру. Взял для сравнения.",
    "b1-gp": "G и П на одной клавише: «Ghbdtn» — это «привет», набранный в английской раскладке. Смысл продукта в двух буквах.",
    "b2-key": "Клавиша с двумя надписями, как на русской клавиатуре. На 24 px буквы мелкие — проверь ряд размеров.",
    "c1-yar": "Я и R — зеркальные буквы двух алфавитов на общей ножке. Симметричный знак, похож на бабочку или Ж — смотри, не мешает ли.",
    "c2-rya": "R и Я рядом, спинками наружу. Проще, читается как пара раскладок.",
}
SUMMARY = ("Моё мнение: сильнее всего C1 и B1 — оба про суть «две раскладки», и только у Ghbdtn. "
           "A1 красив и связан с курсором из стенда 001, но каретка с искрой бывает у AI-редакторов. "
           "A2 отбрасываю — он про мышь. B2 на 24 px, скорее всего, мелкий.")

CARDS = [(title, [{"code": k.split("-")[0].upper(), "name": next(v[1] for v in VARIANTS if v[0] == k),
                   "note": NOTES[k],
                   "tiles": [(k.split("-")[0].upper() + (f" {label}" if label else ""), label, k, circ)
                             for label, circ in TILES[k]]}
                  for k in keys]) for title, keys in GROUPS]

INK = "#161618"
PAPER = "#f4f4f6"


def bbox(items):
    xs, ys = [], []
    for it in items:
        h = it["w"] / 2
        for x, y in it["pts"]:
            xs += [x - h, x + h]
            ys += [y - h, y + h]
    return min(xs), min(ys), max(xs), max(ys)


def path_d(it):
    d = "M" + " L".join(f"{x:.2f} {y:.2f}" for x, y in it["pts"])
    return d + (" Z" if it["closed"] else "")


def glyph_svg(items, size, color):
    x0, y0, x1, y1 = bbox(items)
    s = size / max(x1 - x0, y1 - y0)
    tx = 32 - s * (x0 + x1) / 2
    ty = 32 - s * (y0 + y1) / 2
    body = "".join(
        f'<path d="{path_d(it)}" fill="{color if it["fill"] else "none"}" stroke="{color}" stroke-width="{it["w"]}" '
        f'stroke-linecap="{it["cap"]}" stroke-linejoin="{it["join"]}" stroke-miterlimit="12"/>' for it in items)
    return f'<g transform="translate({tx:.3f} {ty:.3f}) scale({s:.4f})">{body}</g>'


def variant_svg(cid, key, in_circle):
    fn = next(v[2] for v in VARIANTS if v[0] == key)
    items = fn()
    if in_circle:
        return f'<circle cx="32" cy="32" r="32" fill="{INK}"/>' + glyph_svg(items, 40, PAPER)
    return glyph_svg(items, 58, INK)
