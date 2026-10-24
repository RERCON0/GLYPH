# GLYPH icon

The chosen artwork is the original lime/mint pixel glyph selected by the user.

- `icon.png`: 256 × 256 runtime window icon and README image.
- `icon.ico`: 16, 20, 24, 32, 40, 48, 64, 96, 128 and 256px frames for the EXE.
- `source/glyph-pixel.png`: exact copy of the selected attachment.
- `source/provenance.json`: artwork provenance and available original prompt.

Regenerate using `python scripts/make_icon.py` with Pillow installed. This only resizes the supplied artwork and exports formats; it preserves the selected design, colors, dark background and margins. Artwork was originally created with the built-in image generation tool. No new image generation is used during export.
