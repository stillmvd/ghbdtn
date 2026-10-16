import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
LOGO = ROOT / "brand/logo"
ICONS = ROOT / "src-tauri/icons"
INKSCAPE = Path(r"C:\Program Files\Inkscape\bin\inkscape.com")

SMALL_MAX = 32
ICO_SIZES = [24, 30, 32, 36, 40, 48, 60, 64, 72, 96, 128, 256]


def render(size, out):
    src = LOGO / ("mark-small.svg" if size <= SMALL_MAX else "mark.svg")
    subprocess.run([str(INKSCAPE), str(src),
                    f"--actions=export-filename:{out};export-width:{size};export-height:{size};export-do"],
                   check=True, capture_output=True, timeout=180)


def main():
    if not INKSCAPE.exists():
        sys.exit(f"Inkscape не найден: {INKSCAPE}")
    with tempfile.TemporaryDirectory() as tmp:
        frames = []
        for size in ICO_SIZES:
            path = Path(tmp) / f"{size}.png"
            render(size, path)
            frames.append(Image.open(path).convert("RGBA"))
        frames[-1].save(ICONS / "icon.ico", format="ICO", sizes=[(s, s) for s in ICO_SIZES], append_images=frames[:-1])
        for image in frames:
            image.close()
    print(f"ICO: {len(ICO_SIZES)} -> {ICONS / 'icon.ico'}")


if __name__ == "__main__":
    main()
