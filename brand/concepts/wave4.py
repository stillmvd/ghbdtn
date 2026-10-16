from wave1 import glyph_svg
from wave3 import WEIGHTS, caret, ya_r

BOLD = WEIGHTS["bold"]
SIGNS = {"a1": ("Каретка и искра", lambda: caret(BOLD)), "c1": ("ЯR на одной ножке", lambda: ya_r(BOLD))}
SCHEMES = {
    "lime": ("лайм", "#c6ec5a", "#1a2400"),
    "night": ("ночь", "#161618", "#c6ec5a"),
    "moss": ("мох", "#4f7a00", "#ffffff"),
}

NUMBER = 4
TITLE = "Волна 4 · цвет"
PROMPT = ("Оба знака жирной линией в палитре Лайма. Отметь знак и цвет, которые станут иконкой, — "
          "можно один, можно пару на сравнение.")
CHOSEN = ["A1 лайм"]
NOTES = {
    "a1": "Лайм-круг ярче всего на панели задач и в Пуске; в ночном круге искра светится. Мох спокойнее, лучше в светлой теме.",
    "c1": "ЯR в лайме читается как значок раскладки, в ночном круге — как монограмма. Мох — самый сдержанный вариант.",
}
SUMMARY = ("Моё мнение: иконка приложения — лайм-круг, он узнаётся среди серых значков Windows и совпадает с акцентом "
           "интерфейса. Ночь хороша для README и аватара GitHub. Из двух знаков я бы выбрал ЯR: каретка с искрой "
           "у AI-редакторов уже встречается, а ЯR — только у Ghbdtn.")

CARDS = [("Знак и цвет", [{"code": sign.upper(), "name": name, "note": NOTES[sign],
                            "tiles": [(f"{sign.upper()} {label}", label, f"{sign}-{scheme}", True)
                                      for scheme, (label, *_rest) in SCHEMES.items()]}
                           for sign, (name, _) in SIGNS.items()])]


def variant_svg(cid, key, in_circle):
    sign, scheme = key.split("-")
    _, bg, ink = SCHEMES[scheme]
    return f'<circle cx="32" cy="32" r="32" fill="{bg}"/>' + glyph_svg(SIGNS[sign][1](), 40, ink)
