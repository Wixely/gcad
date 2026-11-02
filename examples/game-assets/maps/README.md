# Test texture provenance

`minify-stripes.png` and `minify-normal.png` are original 16-by-1 RGBA patterns
under GPL-3.0-or-later, identified by `origin=gcad-minify-pattern`. Stripes are
opaque white at columns 1, 5, 9 and 13 and transparent black elsewhere. Every
normal pixel is `(128,128,255,255)`. These checked-in files require no generator.

These two 2-by-2 PNGs are original deterministic test patterns created for gcad,
licensed under the repository's GPL-3.0-or-later license. They contain no downloaded
artwork. `origin=gcad-test-pattern` identifies this source; it is not a remote URL.

Pixels are listed in top-left row order as RGBA:

- `rgba.png`: (255,32,16,64), (16,255,32,128), (32,16,255,192), (128,128,128,255).
- `numeric.png`: (128,64,255,255), (64,128,255,255), (192,192,255,255), (255,128,255,255).

The colour/emissive slots treat RGB as sRGB and alpha as linear. Scalar slots read
numeric red. The normal slot declares negative Y to exercise export conversion.
These deliberately diagnostic values are not a production material.
