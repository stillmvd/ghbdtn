from wave1 import INK, PAPER, W, arc, glyph_svg, letter_r, moved, star, stroke


def caret_base():
    return [stroke([(0, 4), (0, 34)]), stroke([(-6, 4), (6, 4)]), stroke([(-6, 34), (6, 34)]), star(13, -2, 9)]


def caret_attached():
    return [stroke([(0, 4), (0, 34)]), stroke([(-6, 4), (5, 4)]), stroke([(-6, 34), (6, 34)]), star(11, 2, 8)]


def caret_curly():
    top = arc(-6, 8, 4, 270, 360) + arc(6, 8, 4, 180, 270)
    bottom = arc(-6, 30, 4, 0, 90) + arc(6, 30, 4, 90, 180)
    return [stroke([(0, 8), (0, 30)]), stroke(top[:len(top) // 2]), stroke(top[len(top) // 2:]),
            stroke(bottom[:len(bottom) // 2]), stroke(bottom[len(bottom) // 2:]), star(13, -1, 7.5)]


def ya_r(w=W):
    return letter_r(w) + moved(letter_r(w), mirror=True)


def ya_r_tall():
    bowl = [(0, 0), (8, 0)] + arc(8, 9.5, 9.5, -90, 90) + [(0, 19)]
    leg = [(6, 19), (13, 34)]
    right = [stroke([(0, 34), (0, 0)]), stroke(bowl), stroke(leg)]
    return right + moved([stroke(bowl), stroke(leg)], mirror=True)


def ya_r_spark():
    return ya_r() + [star(0, -10, 6.5)]


VARIANTS = [
    ("a1-base", "Как в волне 1", caret_base),
    ("a1-attached", "Искра у засечки", caret_attached),
    ("a1-curly", "Курсор с изгибом", caret_curly),
    ("c1-base", "Как в волне 1", ya_r),
    ("c1-tall", "Крупные чаши", ya_r_tall),
    ("c1-spark", "ЯR и искра", ya_r_spark),
]

NUMBER = 2
TITLE = "Волна 2 · композиция"
PROMPT = "Меняю только композицию. Отметь, что нравится, — можно по одному из каждой ветки или оба фаворита."
CHOSEN = ["A1-BASE", "C1-BASE"]
GROUPS = [("A1 · Каретка и искра", ["a1-base", "a1-attached", "a1-curly"]),
          ("C1 · ЯR на одной ножке", ["c1-base", "c1-tall", "c1-spark"])]
NOTES = {
    "a1-base": "Исходник для сравнения: искра отдельно, над правым краем засечки.",
    "a1-attached": "Искра сидит на конце верхней засечки, как вспышка на кончике курсора. Знак компактнее и плотнее в круге.",
    "a1-curly": "Засечки с изгибом, как у настоящего текстового курсора Windows. Узнаваемее, но на 24 px изгибы сливаются.",
    "c1-base": "Исходник для сравнения.",
    "c1-tall": "Чаши выше и шире, ноги короче. Знак плотнее заполняет круг и меньше похож на Ж.",
    "c1-spark": "Обе ветки вместе: ЯR и искра над общей ножкой — момент исправления. Риск — на 24 px искра станет точкой.",
}
SUMMARY = ("Моё мнение: A1 с искрой у засечки и C1 с крупными чашами — оба крепче исходников на 24 px. "
           "Изогнутый курсор узнаваем, но на мелких размерах теряет изгибы. ЯR с искрой — самый «говорящий», "
           "но перегружен: два образа в одном знаке.")

CARDS = [(title, [{"code": k.upper(), "name": next(v[1] for v in VARIANTS if v[0] == k), "note": NOTES[k],
                   "tiles": [(k.upper(), None, k, True)]}
                  for k in keys]) for title, keys in GROUPS]


def variant_svg(cid, key, in_circle):
    items = next(v[2] for v in VARIANTS if v[0] == key)()
    return f'<circle cx="32" cy="32" r="32" fill="{INK}"/>' + glyph_svg(items, 40, PAPER)
