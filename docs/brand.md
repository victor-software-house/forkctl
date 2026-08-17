# Mark

The mark is the topology forkctl works in: one upstream, forks branching off
it, and the one we maintain in rust. Same palette and 32-unit grid as `verctl`
and `qctl`; each tool's topology differs — a rising history there, a fork graph
here, a queue split by a rule in `qctl`.

| File | Use |
|:--|:--|
| [`mark.svg`](mark.svg) | Square mark, cream field. Avatar, favicon, docs. |
| [`mark-dark.svg`](mark-dark.svg) | Same geometry, ink field. |
| [`banner.svg`](banner.svg) | README header, 1200×240. |
| [`banner-dark.svg`](banner-dark.svg) | Same, ink field. |

Pair the two fields with `<picture>` and `prefers-color-scheme`, as the README
header does. Never recolour a single file at the call site.

## Palette

| | Hex | Role |
|:--|:--|:--|
| Cream | `#f3efe6` | Field, or figure on ink |
| Ink | `#161616` | Figure, or field |
| Rust | `#c45c2a` | The fork we maintain. One accent, never two |

Banner-only tints: `#6f675c` (muted on cream), `#8d857a` (muted on ink),
`#ddd6c8` / `#2f2f2f` (hairline).

## Construction

A 32-unit square, corner radius 6, mirrored about `y 16`, the figure clearing
the field by 4 units left and right and 5 top and bottom. Upstream is a 7×8
block at `x 4`. Every connector is 3 units thick: a stem out to the bus, the
bus itself at `x 14.5` running `y 7.5` to `24.5`, and two branches at `x 17.5`.
The bus ends flush with the outer edge of each branch and the branches sit on
the fork centres (`y 9` and `y 23`) — junctions that miss by even half a unit
read as a mistake at 96px. The forks are 6.5×8 at `x 21.5`, the lower one rust.
The gaps between parts are load bearing: closed up, the graph renders as one
blob at 16px, which is the size it gets judged at.

All the ink parts are subpaths of **one** `<path>`, and the same for rust. As
separate `<rect>`s they each antialias against the field, so a shared edge on a
fractional coordinate composites to roughly 75% coverage and shows as a grey
hairline through the figure — measured at the bus edge, `srgb(77,76,74)` where
it should be `srgb(22,22,22)`. One path is rasterised in one pass and the seam
cannot occur. Anything added to this mark goes in the same path.

## Banner text

Set in [Geist Mono](https://github.com/vercel/geist-font) (OFL) and converted
to outlines, so nothing depends on a font at render time: wordmark Black 60px
with −3 tracking, tagline Medium 17px, chip Regular 16px. To change the
wording, reshape with `fonttools` + `uharfbuzz` at those sizes rather than
adding a `<text>` element.
