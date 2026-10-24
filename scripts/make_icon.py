"""Export GLYPH's transparent mark 05 to PNG/ICO. Requires Pillow, build only."""
from pathlib import Path
import io
import struct

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "icons" / "source" / "glyph-mark-05.png"
SIZES = (16, 20, 24, 32, 40, 48, 64, 96, 128, 256)


def load_mark(source: Path = SOURCE) -> Image.Image:
    with Image.open(source) as original:
        image = original.convert("RGBA")
    alpha = image.getchannel("A")
    if alpha.getextrema() != (0, 255):
        raise ValueError("The master must contain real transparency and opaque artwork")
    # Ignore almost invisible generated fringe only when measuring padding.
    bounds = alpha.point(lambda value: 255 if value > 32 else 0).getbbox()
    if bounds is None:
        raise ValueError("The icon source is empty")
    left, top, right, bottom = bounds
    pad = 4
    return image.crop((max(0, left-pad), max(0, top-pad),
                       min(image.width, right+pad), min(image.height, bottom+pad)))


def render(mark: Image.Image, size: int) -> Image.Image:
    # Menu icons use a little more of the square, preserving the same silhouette.
    fill = 0.90 if size <= 16 else 0.88 if size <= 20 else 0.86 if size <= 24 else 0.84
    scale = size * 4 * fill / max(mark.size)
    resized = mark.resize((max(1, round(mark.width * scale)),
                           max(1, round(mark.height * scale))), Image.Resampling.LANCZOS)
    canvas = Image.new("RGBA", (size * 4, size * 4), (0, 0, 0, 0))
    canvas.alpha_composite(resized, ((canvas.width-resized.width)//2,
                                   (canvas.height-resized.height)//2))
    return canvas.resize((size, size), Image.Resampling.LANCZOS)


def main() -> None:
    mark = load_mark()
    frames = [render(mark, size) for size in SIZES]
    frames[-1].save(ROOT / "icons" / "icon.png", optimize=True)
    payloads = []
    for frame in frames:
        output = io.BytesIO()
        frame.save(output, format="PNG", optimize=True)
        payloads.append(output.getvalue())
    offset = 6 + 16 * len(frames)
    entries = []
    for size, payload in zip(SIZES, payloads):
        dimension = 0 if size == 256 else size
        entries.append(struct.pack("<BBBBHHII", dimension, dimension, 0, 0,
                                   1, 32, len(payload), offset))
        offset += len(payload)
    (ROOT / "icons" / "icon.ico").write_bytes(
        struct.pack("<HHH", 0, 1, len(frames))
        + b"".join(entries) + b"".join(payloads)
    )
    print("Transparent GLYPH icon.png / icon.ico: " + ", ".join(map(str, SIZES)))


if __name__ == "__main__":
    main()
