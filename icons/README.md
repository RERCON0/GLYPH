# GLYPH icon

The selected artwork is mark 05 from the middle column of the SNATCH exploration
sheet, adapted for GLYPH. The white mark has a slim charcoal contour and real
transparency, including the zigzag channel. There is no background tile.

- `icon.png`: 256 x 256 runtime window icon and README image.
- `icon.ico`: 16, 20, 24, 32, 40, 48, 64, 96, 128 and 256px EXE frames.
- `source/glyph-mark-05.png`: transparent master produced with built-in image_gen.
- `source/snatch-concept-05-reference.png`: original selection sheet.
- `source/mark-05-provenance.json`: generation mode, reference and exact edit prompt.

Regenerate with `python scripts/make_icon.py` and Pillow installed. Export crops
transparent margins, preserves the generated alpha and contours, and applies
optical sizing for small frames. It does not generate or redraw the artwork.
The earlier pixel master and its provenance remain in `source/` for reference.
