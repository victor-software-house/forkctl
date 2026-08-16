# Mark

The mark is what forkctl carries: two patches stacked on the upstream base,
the top patch in rust. Same palette and 32-unit grid as `verctl` and `qctl`;
each tool's topology differs — a rising history there, a carried stack here,
an ordered queue in `qctl`.

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
| Rust | `#c45c2a` | The top patch. One accent, never two |

Banner-only tints: `#6f675c` (muted on cream), `#8d857a` (muted on ink),
`#ddd6c8` / `#2f2f2f` (hairline).

## Construction

A 32-unit square, corner radius 6. Upstream is a 3.5-unit bar at `y 25`
spanning `x 3` to `29`. Two 8.5-unit patches stack above it on the same centre
line at `y 14` and `y 3.5`, the top one rust. The 2 and 2.5-unit gaps are load
bearing: closed up, the stack renders as one blob at 16px, which is the size
it gets judged at.

## Banner text

Set in [Geist Mono](https://github.com/vercel/geist-font) (OFL) and converted
to outlines, so nothing depends on a font at render time: wordmark Black 60px
with −3 tracking, tagline Medium 17px, chip Regular 16px. To change the
wording, reshape with `fonttools` + `uharfbuzz` at those sizes rather than
adding a `<text>` element.
