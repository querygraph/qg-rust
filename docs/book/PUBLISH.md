# QueryGraph Book Publishing

Use this runbook when updating, rebuilding, validating, delivering, or
publishing the QueryGraph book.

## Source Layout

- Manuscript: `docs/book/manuscript.md`
- Cover template: `docs/book/cover.md`
- EPUB metadata: `docs/book/metadata.yaml`
- Diagram/book renderer: `docs/book/build.mjs`
- Build script: `docs/book/build.sh`
- EPUB layout fixer: `docs/book/fix_epub_layout.sh`
- EPUB validator: `docs/book/check_epub_metadata.sh`
- EPUB stylesheet: `docs/book/epub.css`
- Book-local diagram assets: `docs/book/diagrams/`
- Blog-stable diagram assets: `docs/blog/assets/querygraph/diagrams/`
- Final artifacts: `docs/book/dist/`

The visible title is:

```text
Querygraph
```

The checked-in metadata stem is:

```yaml
title_stem: "querygraph"
```

## Pipeline Choice

QueryGraph deliberately uses a hybrid of the TypeSec and Grust book pipelines:

- TypeSec-style simple artifact contract: stable outputs live directly in
  `docs/book/dist/`.
- Grust-style diagram materialization: Mermaid diagrams are rendered into PNG
  assets before Pandoc builds the book.

This avoids a large generated book workspace while still producing reusable
diagram assets for blog posts and architecture notes.

## Artifact Contract

Stable deliverables:

- `docs/book/dist/querygraph.pdf`
- `docs/book/dist/querygraph.epub`
- `docs/book/dist/querygraph.mobi`
- `docs/book/dist/VERSION.md`

The Kindle/catalog title and upload filename are generated from `title_stem`,
the package version in `Cargo.toml`, and the current short git commit:

```text
querygraph (0.1.0-13ca95f)
querygraph (0.1.0-13ca95f).epub
```

The versioned EPUB path is a generated symlink:

```text
docs/book/dist/querygraph (0.1.0-13ca95f).epub -> querygraph.epub
```

`VERSION.md` must contain:

```yaml
kindle_name: querygraph (0.1.0-13ca95f)
built_at: YYYY-MM-DD
epub_file: querygraph.epub
kindle_link: querygraph (0.1.0-13ca95f).epub
```

Track the stable EPUB, PDF, MOBI, `VERSION.md`, source files, and stable diagram
assets. The versioned EPUB symlink is generated and ignored by `.gitignore`.

## Diagram Contract

Mermaid diagrams are authored inline in `docs/book/manuscript.md`.
`docs/book/build.mjs` renders each diagram to:

- `docs/book/build/diagrams/diagram-XX.mmd`
- `docs/book/build/diagrams/diagram-XX.png`

It also copies stable assets to:

- `docs/book/diagrams/diagram-XX.mmd`
- `docs/book/diagrams/diagram-XX.png`
- `docs/blog/assets/querygraph/diagrams/diagram-XX.mmd`
- `docs/blog/assets/querygraph/diagrams/diagram-XX.png`

Use `docs/book/diagrams/` for book review and repository documentation. Use
`docs/blog/assets/querygraph/diagrams/` when a blog post needs stable image
paths that do not depend on the generated build directory.

## Metadata Rules

Keep reader-facing and catalog-facing titles separate:

- Cover, navigation title, NCX title, table of contents: `Querygraph`
- OPF `dc:title` and title-sort metadata: `querygraph (<version>-<commit>)`
- Stable file: `querygraph.epub`
- Upload/delivery file: `querygraph (<version>-<commit>).epub`

Do not hard-code the version in the manuscript or cover. The build script writes
`docs/book/dist/VERSION.md` from `Cargo.toml`, `metadata.yaml`, and the current
short git commit, then renders `{{versionSubtitle}}` from that generated version
file.

## Cover Rules

The cover is `docs/book/cover.md` and has two raw blocks:

- Typst raw block for PDF.
- HTML raw block for EPUB and MOBI.

The Typst block must include:

```typst
#set page(margin: 1in, numbering: none)
```

This keeps the standalone cover unnumbered. The EPUB cover must stay simple:
centered text and margins, no flexbox.

`docs/book/epub.css` owns compact code-block spacing and cover styling. Keep
code spacing fixes in CSS rather than changing manuscript formatting.

The rendered cover version subtitle is derived from `kindle_name` in
`docs/book/dist/VERSION.md`:

```text
covers querygraph (0.1.0-13ca95f)
```

## Build

From the repository root:

```sh
docs/book/build.sh
```

The build:

1. Reads the package version from `Cargo.toml`.
2. Reads the current short git commit for the version suffix.
3. Reads `title`, `title_stem`, and cover metadata from `metadata.yaml`.
4. Writes `docs/book/dist/VERSION.md`.
5. Renders cover placeholders into `docs/book/build/cover.rendered.md` from
   `docs/book/dist/VERSION.md`.
6. Renders Mermaid diagrams into `.mmd` and `.png` assets.
7. Copies stable diagram assets into `docs/book/diagrams/` and
   `docs/blog/assets/querygraph/diagrams/`.
8. Builds a standalone cover PDF.
9. Builds the body PDF with a table of contents.
10. Merges cover and body into `docs/book/dist/querygraph.pdf`.
11. Builds `docs/book/dist/querygraph.epub`.
12. Runs `fix_epub_layout.sh`.
13. Creates the versioned EPUB symlink.
14. Keeps `docs/book/dist/VERSION.md` next to the generated artifacts.
15. Runs `check_epub_metadata.sh`.
16. Converts the EPUB to `docs/book/dist/querygraph.mobi`.

Calibre is expected either on `PATH` as `ebook-convert` or at:

```sh
/Applications/calibre.app/Contents/MacOS/ebook-convert
```

## Required Validation

The build script runs the EPUB validator automatically. To rerun it manually:

```sh
docs/book/check_epub_metadata.sh \
  docs/book/dist/querygraph.epub \
  'querygraph (0.1.0-13ca95f)' \
  Querygraph
```

The validator rejects:

- missing or wrong OPF title, creator, language, or title-sort metadata;
- navigation/NCX title drift from `Querygraph`;
- cover XHTML that is not frontmatter;
- generated cover wrapper headings;
- flexbox on the EPUB cover;
- missing compact code-block CSS;
- missing stable EPUB;
- non-identical stable EPUB;
- missing or wrong versioned EPUB symlink;
- missing or incomplete `VERSION.md`.

Also inspect PDF cover/body text when cover behavior changes:

```sh
pdftotext -f 1 -l 1 docs/book/dist/querygraph.pdf -
pdftotext -f 2 -l 2 docs/book/dist/querygraph.pdf -
```

Check the versioned EPUB link:

```sh
readlink 'docs/book/dist/querygraph (0.1.0-13ca95f).epub'
```

Expected:

```text
querygraph.epub
```

Check materialized diagrams:

```sh
find docs/book/diagrams docs/blog/assets/querygraph/diagrams -maxdepth 1 -type f | sort
```

Expected: matching `diagram-XX.mmd` and `diagram-XX.png` files in both
locations.

## Delivery

For iCloud Books delivery, derive the exact upload filename from
`docs/book/dist/VERSION.md` and copy the versioned EPUB path:

```sh
cp 'docs/book/dist/querygraph (0.1.0-13ca95f).epub' "$HOME/icloud/books/"
```

This produces a regular file at the destination with the versioned filename.
Do not rely on broad `~/icloud/books` directory listings; exact-path `stat`,
`cmp`, or `cp` checks are more reliable on this Mac.

## Git Delivery

Before committing a book update, inspect:

```sh
git status --short
git diff --check
```

The normal tracked set for book changes includes:

- `docs/book/manuscript.md`
- `docs/book/metadata.yaml`
- `docs/book/cover.md`
- `docs/book/epub.css`
- `docs/book/build.mjs`
- `docs/book/build.sh`
- `docs/book/fix_epub_layout.sh`
- `docs/book/check_epub_metadata.sh`
- `docs/book/PUBLISH.md`
- `docs/book/diagrams/*`
- `docs/blog/assets/querygraph/diagrams/*`
- `docs/book/dist/querygraph.pdf`
- `docs/book/dist/querygraph.epub`
- `docs/book/dist/querygraph.mobi`
- `docs/book/dist/VERSION.md`

Do not stage generated files from `docs/book/build/`. They are ignored
intermediates.
