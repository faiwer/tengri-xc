# Roboto subset

`Roboto-Regular.subset.ttf` is the font the flight preview image draws its
attribution label with (`src/flight/image/attribution.rs`). It's embedded in
the binary — the prod container is `debian:bookworm-slim` with no fonts
installed, and tiny-skia has no text support of its own.

## Licence

SIL Open Font License 1.1, see `OFL.txt`. Roboto declares no Reserved Font
Name, so the subset keeps the name. Obligations: ship the licence and
copyright notice (this directory does), keep it under OFL, don't sell the font
by itself.

## Regenerating

Upstream is the `google/fonts` variable build, instanced to Regular and cut
down to Latin — 488 KB to 18 KB.

```sh
python3 -m venv .fontvenv && .fontvenv/bin/pip install fonttools
curl -fL -o Roboto-VF.ttf \
  'https://raw.githubusercontent.com/google/fonts/main/ofl/roboto/Roboto%5Bwdth,wght%5D.ttf'
.fontvenv/bin/fonttools varLib.instancer Roboto-VF.ttf wght=400 wdth=100 -o Roboto-Static.ttf
.fontvenv/bin/pyftsubset Roboto-Static.ttf \
  --output-file=Roboto-Regular.subset.ttf \
  --unicodes=U+0020-007E,U+00A0-00FF,U+2013,U+2014,U+2018,U+2019,U+201C,U+201D,U+2026,U+2122 \
  --layout-features='' --no-hinting --desubroutinize --name-IDs='*' --notdef-outline
```

The range covers printable ASCII, Latin-1 Supplement (accented provider names,
`©`, `®`, `°`) and a few typographic marks. A character outside it renders as
the `.notdef` box, which `--notdef-outline` keeps visible so the gap is
obvious. Hinting and layout tables are dropped: `ab_glyph` ignores hinting and
reads only the legacy `kern` table, which Roboto doesn't use.
