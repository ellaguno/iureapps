"""Dibuja el icono del lanzador (assets/iureapps.png) con el estilo de la suite.

Fondo azul marino, la cúpula azul de Iurefficient y un mosaico de 2×2 (las apps).
Uso: python3 scripts/icono.py assets/iureapps.png
"""
import sys
from PIL import Image, ImageDraw

S = 1024  # se dibuja grande y se reduce para suavizar bordes
NAVY = (17, 40, 76, 255)
BLUE = (6, 80, 236, 255)
WHITE = (246, 246, 248, 255)
ACCENTS = [(59, 130, 246, 255), (20, 184, 166, 255), (239, 68, 68, 255), (245, 158, 11, 255)]

img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
d = ImageDraw.Draw(img)
m = int(S * 0.06)
d.rounded_rectangle([m, m, S - m, S - m], radius=int(S * 0.2), fill=NAVY)
# Cúpula: semicírculo inferior de un círculo centrado arriba.
cx, cy, r = S // 2, int(S * 0.16), int(S * 0.19)
d.pieslice([cx - r, cy - r, cx + r, cy + r], 0, 180, fill=BLUE)
# Mosaico 2×2: blancos con una franja de color de cada app.
size, gap = int(S * 0.2), int(S * 0.05)
x0 = (S - 2 * size - gap) // 2
y0 = int(S * 0.43)
for k, accent in enumerate(ACCENTS):
    x = x0 + (k % 2) * (size + gap)
    y = y0 + (k // 2) * (size + gap)
    d.rounded_rectangle([x, y, x + size, y + size], radius=int(size * 0.18), fill=WHITE)
    d.rounded_rectangle([x, y, x + size, y + int(size * 0.28)], radius=int(size * 0.18), fill=accent)
    d.rectangle([x, y + int(size * 0.14), x + size, y + int(size * 0.28)], fill=accent)

out = sys.argv[1] if len(sys.argv) > 1 else "assets/iureapps.png"
img.resize((256, 256), Image.LANCZOS).save(out, optimize=True)
print(out)
