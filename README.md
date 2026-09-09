# Booklet PDF

A cross-platform app (Rust + GTK4) that turns an ordinary PDF into one ready
for printing a book: **2 pages on a single landscape sheet**, in the right
order for folding and stitching.

- `booklet-gui` – graphical app with a live imposition preview that renders the
  actual page content
- `booklet` – the same functionality from the command line
- `booklet-core` – the imposition library (no GTK, usable on its own)

Page content is never re-encoded: each source page is wrapped in a Form
XObject, so fonts, vectors and images keep their original quality.

Everything is pure Rust — no C libraries, no external binaries. Previews are
rasterized by [hayro](https://github.com/LaurenzV/hayro), the imposition is
done with [lopdf](https://github.com/J-F-Liu/lopdf).

The interface speaks **English, Slovak and Czech**, picked up from your locale
and switchable at any time from the header bar.

## Folding modes

### Booklet – saddle stitch

The whole document is one signature. The first sheet carries the last and the
first page on its outside, so once you stack the sheets, fold them in the
middle and staple through the fold, you get a booklet with a front and a back
cover.

Example for 8 pages (a *sheet* is one piece of paper, *front*/*back* are its
two sides):

| sheet | front (left \| right) | back (left \| right) |
|-------|-----------------------|----------------------|
| 1     | 8 \| 1                | 2 \| 7               |
| 2     | 6 \| 3                | 4 \| 5               |

### Signatures – folded in sections

The document is split into signatures of N sheets (N × 4 pages). Each
signature is folded and stitched on its own, and the signatures are then sewn
or glued one after another. This is how real books are made — a 200-page
saddle-stitched booklet would have an unusable bulge at the fold.

One thing to watch out for: with the default 4 sheets per signature (16 pages),
a document of 16 pages or fewer gives **exactly the same result as a plain
booklet**, because it all fits into a single signature. The app says so in the
summary. The split only kicks in from page 17:

```
$ booklet book.pdf --mode signatures -n 4 --dry-run
40 source pages → 10 sheets of paper (20 output pages), 0 blank slots.
3 signatures, sheets: 4+4+2
  signature 1: pages 1–16    signature 2: pages 17–32    signature 3: pages 33–40
```

In the preview each signature is separated by a heading, and every sheet says
which sheet of its signature it is — that is what you need when nesting the
sheets.

### 2 pages per sheet – no folding

The order is left alone (1|2, 3|4, …). For saving paper while reading, not for
binding.

## Other settings

| Setting | What it is for |
|---|---|
| **Size / orientation** | A2–A6, Letter, Legal, Tabloid, or *match the source* (the sheet is exactly twice the page, so nothing is scaled down) |
| **Binding left / right** | right for right-to-left scripts and manga |
| **Pages** | a range such as `1-8,11`; a reversed range (`8-1`) reverses the order |
| **Margin** | empty margin around the whole sheet |
| **Fold gutter** | extra space at the fold (for binding or punched holes) |
| **Mark the fold** | ticks at the sheet edges (they are not drawn over the page content — for finished booklets), a dashed line across the whole sheet, or nothing |
| **Crop marks** | marks at the sheet edges where the page edges are |
| **Paper flip** | must match the duplex setting in your printer driver — if the backs come out upside down, switch it |
| **Order** | interleaved (duplex printer) or all fronts first and then the backs (manual duplex) |
| **Creep** | shifts the content of outer sheets towards the fold so the margins come out even after trimming |
| **Scale pages to fit** | turn it off if you want scale 1:1 |
| **Page thumbnails** | draws the real page content in the preview; the page number moves to a corner badge |

If the page count is not a multiple of 4, blank slots are added at the end —
that is, on the back cover, never before the first page.

The fold is marked with short ticks at the top and bottom edge of the sheet by
default: you can see where to fold and nothing is drawn over the page content.
If you need a stronger guide, switch to the dashed line across the sheet — but
that line stays printed in the finished booklet.

## How to print it

1. Open the output PDF in a viewer and print it **without any further
   scaling** ("actual size" / "100 %", not "fit to page").
2. Turn on double-sided printing. For a landscape sheet, **flip on the short
   edge** usually works; if the backs come out upside down, switch the
   *Paper flip* option in the app.
3. Without a duplex printer choose *Manual duplex – all fronts first*, print
   the fronts, put the stack back in and print the backs. If your printer
   returns the stack flipped, use the *backs in reverse* variant.
4. Stack the sheets on top of each other (do not fold them one by one!), fold
   through the middle and stitch or staple in the fold.

It pays to try it on 4 pages before printing the whole thing.

## Building and running

You need Rust 1.92+ and the GTK 4 development packages.

```bash
# Debian / Ubuntu
sudo apt install libgtk-4-dev build-essential
# Fedora
sudo dnf install gtk4-devel gcc
# macOS
brew install gtk4 pkg-config
# Windows: gvsbuild or MSYS2 (mingw-w64-x86_64-gtk4)

cargo build --release
./target/release/booklet-gui           # graphical app
./target/release/booklet-gui book.pdf  # or straight with a file
```

`booklet-core` does not depend on GTK, so `cargo build -p booklet-core` and
`cargo test` work without GTK installed.

## Language

The interface language is detected from `BOOKLET_LANG`, `LC_ALL`,
`LC_MESSAGES`, `LANG` and `LANGUAGE`, in that order, and falls back to
English. You can change it live from the drop-down in the header bar, and the
CLI takes `--lang en|sk|cs`.

```bash
BOOKLET_LANG=sk booklet-gui   # start in Slovak
booklet book.pdf --lang cs    # Czech messages from the CLI
```

The CLI `--help` text is English only.

## Command line

```bash
# booklet on landscape A4
booklet book.pdf -o book-print.pdf

# signatures of 4 sheets (16 pages), 8 mm for the binding, crop marks
booklet book.pdf --mode signatures -n 4 --gutter 8 --crop

# no marks at all
booklet book.pdf --fold none

# just show what ends up where
booklet book.pdf --mode signatures -n 2 --dry-run
```

`booklet --help` lists every option.

## Tests

```bash
cargo test
```

`booklet-core` has unit tests for the page order, the geometry and the
translations, plus end-to-end tests that generate a PDF, impose it and check
back which source page ended up in which slot. `booklet-gui` has smoke tests
for the preview drawing (including rotated slots, every mark variant and every
language).
