"""Export GLYPH's selected pixel artwork to PNG/ICO. Requires Pillow, build only."""
from pathlib import Path
import io
import struct

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "icons" / "source" / "glyph-pixel.png"
SIZES = (16, 20, 24, 32, 40, 48, 64, 96, 128, 256)


def main() -> None:
    with Image.open(SOURCE) as source:
        image = source.convert("RGBA")
    if image.width != image.height:
        raise ValueError("The selected icon source must be square")
    # Preserve the selected artwork, colors, dark background and original margins.
    frames = [
        image.resize((size, size), Image.Resampling.LANCZOS)
        for size in SIZES
    ]
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
        entries.append(
            struct.pack("<BBBBHHII", dimension, dimension, 0, 0, 1, 32,
                        len(payload), offset)
        )
        offset += len(payload)
    (ROOT / "icons" / "icon.ico").write_bytes(
        struct.pack("<HHH", 0, 1, len(frames))
        + b"".join(entries)
        + b"".join(payloads)
    )
    print("GLYPH icon.png / icon.ico: " + ", ".join(map(str, SIZES)))


if __name__ == "__main__":
    main()
