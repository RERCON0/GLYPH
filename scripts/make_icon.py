"""Rebuild GLYPH's original geometric G mark. Requires Pillow, build only."""
from pathlib import Path

from PIL import Image, ImageDraw

root = Path(__file__).resolve().parents[1]
scale = 4
image = Image.new("RGBA", (256 * scale, 256 * scale), (11, 11, 14, 255))
draw = ImageDraw.Draw(image)
accent = (112, 214, 221, 255)
# An open, angular G: a glyph rather than another padlock icon.
points = [(180, 70), (80, 70), (60, 90), (60, 176), (80, 196), (186, 196), (186, 132), (130, 132)]
draw.line([(x * scale, y * scale) for x, y in points], fill=accent, width=14 * scale)
draw.rectangle((190 * scale, 60 * scale, 204 * scale, 74 * scale), fill=accent)
image = image.resize((256, 256), Image.Resampling.LANCZOS)
image.save(root / "icons" / "icon.png")
image.save(root / "icons" / "icon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
