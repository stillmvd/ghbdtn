from wave1 import INK, PAPER, glyph_svg, letter_r, moved, star, stroke

WEIGHTS = {"thin": 3.8, "mid": 4.6, "bold": 5.6}


def caret(w):
    return [stroke([(0, 4), (0, 34)], w), stroke([(-6, 4), (6, 4)], w), stroke([(-6, 34), (6, 34)], w),
            star(13, -2, 9)]


def ya_r(w):
    return letter_r(w) + moved(letter_r(w), mirror=True)


VARIANTS = [(f"{kind}-{name}", f"{label} · {w}", (lambda f, w: lambda: f(w))(fn, w))
            for kind, fn, label in (("a1", caret, "Каретка"), ("c1", ya_r, "ЯR"))
            for name, w in WEIGHTS.items()]

NUMBER = 3
TITLE = "Волна 3 · толщина линии"
PROMPT = "Композиция найдена, меняю только толщину. Выбери вес для каждого знака, который ведём дальше."
CHOSEN = ["A1 жирная", "C1 жирная"]
GROUPS = [("A1 · Каретка и искра", [f"a1-{n}" for n in WEIGHTS]),
          ("C1 · ЯR на одной ножке", [f"c1-{n}" for n in WEIGHTS])]
NAMES = {"thin": "Тонкая", "mid": "Как сейчас", "bold": "Жирная"}
NOTES = {
    "a1-thin": "Лёгкая, ближе к настоящему курсору. Искра заметнее, но на 24 px палочка тоньше пикселя на краях.",
    "a1-mid": "Текущая толщина: баланс палочки и искры.",
    "a1-bold": "Плотнее на панели задач, курсор становится похож на букву I.",
    "c1-thin": "Изящнее, просветы между чашами шире — на 24 px буквы не слипаются.",
    "c1-mid": "Текущая толщина.",
    "c1-bold": "Знак весомее, но чаши и ножка на 24 px начинают сливаться в пятно.",
}
SUMMARY = ("Моё мнение: каретке подходит средняя — тонкая теряется на 24 px, жирная превращает курсор в букву I. "
           "ЯR — тонкая или средняя: у неё много линий в центре, жирная на мелких размерах слипается.")

CARDS = [("Толщина", [{"code": keys[0].split("-")[0].upper(), "name": title.split(" · ")[1], "radio": True,
                       "note": " ".join(f"{NAMES[k.split('-')[1]]}: {NOTES[k]}" for k in keys),
                       "tiles": [(f"{k.split('-')[0].upper()} {NAMES[k.split('-')[1]].lower()}", NAMES[k.split("-")[1]].lower(),
                                  k, True, k.endswith("mid")) for k in keys]}
                      for title, keys in GROUPS])]


def variant_svg(cid, key, in_circle):
    items = next(v[2] for v in VARIANTS if v[0] == key)()
    return f'<circle cx="32" cy="32" r="32" fill="{INK}"/>' + glyph_svg(items, 40, PAPER)
