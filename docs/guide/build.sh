#!/usr/bin/env bash
# Build the stack guide (the second QueryGraph book): EPUB + PDF with
# versioned delivery links, mirroring docs/book/build.sh minus the mermaid
# diagram stage. The dedicated book remains docs/book.
set -euo pipefail

cd "$(dirname "$0")"

mkdir -p build dist

pubdate="$(date -u +%F)"
version="$(
  awk '
    /^\[package\]/ { in_package = 1; next }
    /^\[/ { in_package = 0 }
    in_package && /^version[[:space:]]*=/ {
      gsub(/"/, "", $3)
      print $3
      exit
    }
  ' ../../Cargo.toml
)"
title_stem="querygraph-stack"
visible_title="The QueryGraph Stack"

commit_suffix="$(git -C ../.. rev-parse --short HEAD)"
kindle_title="$title_stem ($version-$commit_suffix)"
version_subtitle="Stack release $version-$commit_suffix · built $pubdate"

{
  printf 'kindle_name: %s\n' "$kindle_title"
  printf 'built_at: %s\n' "$pubdate"
  printf 'epub_file: %s.epub\n' "$title_stem"
  printf 'pdf_file: %s.pdf\n' "$title_stem"
  printf 'kindle_link: %s.epub\n' "$kindle_title"
  printf 'pdf_link: %s.pdf\n' "$kindle_title"
} > dist/VERSION.md

sed "s|{{versionSubtitle}}|$version_subtitle|g" cover.md > build/cover.rendered.md

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT
sed '/^```{=typst}$/,/^```$/d' build/cover.rendered.md > "$tmpdir/cover.epub.md"

pandoc --from markdown+smart \
  --pdf-engine=typst \
  --output "$tmpdir/cover.pdf" \
  build/cover.rendered.md

pandoc --from markdown+smart \
  --to typst \
  --metadata-file metadata.yaml \
  --toc --toc-depth=2 \
  --output "$tmpdir/body.typ" \
  manuscript.md

{
  printf '#outline(title: [Contents])\n'
  printf '#pagebreak()\n\n'
  cat "$tmpdir/body.typ"
} > "$tmpdir/body-with-toc.typ"

typst compile "$tmpdir/body-with-toc.typ" "$tmpdir/body.pdf"
pdfunite "$tmpdir/cover.pdf" "$tmpdir/body.pdf" "dist/$title_stem.pdf"

pandoc --from markdown+smart \
  --metadata-file metadata.yaml \
  --metadata date="$pubdate" \
  --epub-title-page=false \
  --toc --toc-depth=2 \
  --css ../book/epub.css \
  --output "dist/$title_stem.epub" \
  "$tmpdir/cover.epub.md" manuscript.md

../book/fix_epub_layout.sh "dist/$title_stem.epub" "$kindle_title" "$visible_title"

find dist -maxdepth 1 -name "$title_stem (*).epub" -exec rm -f {} +
ln -s "$title_stem.epub" "dist/$kindle_title.epub"
find dist -maxdepth 1 -name "$title_stem (*).pdf" -exec rm -f {} +
ln -s "$title_stem.pdf" "dist/$kindle_title.pdf"

./check_epub_metadata.sh "dist/$title_stem.epub" "$kindle_title" "$visible_title"

EBOOK_CONVERT="${EBOOK_CONVERT:-}"
if [[ -z "$EBOOK_CONVERT" ]]; then
  if command -v ebook-convert >/dev/null 2>&1; then
    EBOOK_CONVERT="$(command -v ebook-convert)"
  elif [[ -x /Applications/calibre.app/Contents/MacOS/ebook-convert ]]; then
    EBOOK_CONVERT=/Applications/calibre.app/Contents/MacOS/ebook-convert
  fi
fi
if [[ -n "$EBOOK_CONVERT" ]]; then
  "$EBOOK_CONVERT" "dist/$title_stem.epub" "dist/$title_stem.mobi" >/dev/null
else
  echo "ebook-convert not found; skipping MOBI" >&2
fi

# ── Dual typesettings: -typst and -troff editions ──────────────────────────
# The canonical PDF above is the typst edition; alias it, and typeset a
# second PDF through groff/ms. Per the omnighost troff findings: -P-e embeds
# fonts (unembedded base-14 fonts cause word-gap artifacts), -t runs tbl for
# the guide's tables, -k runs preconv for the Unicode-dense text; homebrew's
# gropdf font map pins the ghostscript path from groff's build time, so it is
# regenerated against the ghostscript actually installed.
typst_title="$title_stem-typst ($version-$commit_suffix)"
troff_title="$title_stem-troff ($version-$commit_suffix)"

cp "dist/$title_stem.pdf" "dist/$title_stem-typst.pdf"
cp "dist/$title_stem.epub" "dist/$title_stem-typst.epub"
../book/fix_epub_layout.sh "dist/$title_stem-typst.epub" "$typst_title" \
  "$visible_title (typst)"

groff_font_dir="$(dirname "$(command -v groff)")/../share/groff/$(groff --version | awk 'NR==1{print $NF}')/font"
gs_font_dir="$(ls -d /opt/homebrew/Cellar/ghostscript/*/share/ghostscript/Resource/Font 2>/dev/null | sort -V | tail -1)"
if [[ -n "$gs_font_dir" && -f "$groff_font_dir/devpdf/download" ]]; then
  mkdir -p "$tmpdir/devpdf"
  sed -E "s|/opt/homebrew/Cellar/ghostscript/[^/]+/share/ghostscript/Resource/Font|$gs_font_dir|" \
    "$groff_font_dir/devpdf/download" > "$tmpdir/devpdf/download"
  export GROFF_FONT_PATH="$tmpdir"
fi

cat > "$tmpdir/cover.ms" <<COVER
.nr PS 10
.nr VS 12
.ds CH
.LP
.sp 2i
.ce 4
.ps 28
.B "$visible_title"
.ps 12
.sp 0.5v
The Definitive Guide to the Governed Semantic Lakehouse
.sp 0.4v
$version_subtitle \(em troff edition
.sp 2v
.ce 2
.ps 14
Alexy Khrabrov and Slava Tykhonov
.ps 11
querygraph.ai
.bp
COVER
# Strip code-fence language tags for the ms writer: pandoc's highlighted ms
# output uses *Tok string macros that are only defined in standalone mode,
# so annotated code blocks render blank in a non-standalone body.
sed -E 's/^```[a-zA-Z]+$/```/' manuscript.md \
  | pandoc --from markdown+smart --to ms > "$tmpdir/body.ms"
groff -Tpdf -P-e -k -t -ms "$tmpdir/cover.ms" "$tmpdir/body.ms" \
  > "dist/$title_stem-troff.pdf"
if command -v pdffonts >/dev/null 2>&1; then
  unembedded="$(pdffonts "dist/$title_stem-troff.pdf" | awk 'NR>2 && $(NF-4) == "no"' || true)"
  if [[ -n "$unembedded" ]]; then
    echo "ERROR: unembedded fonts in troff PDF:" >&2
    echo "$unembedded" >&2
    exit 1
  fi
fi
cp "dist/$title_stem.epub" "dist/$title_stem-troff.epub"
../book/fix_epub_layout.sh "dist/$title_stem-troff.epub" "$troff_title" \
  "$visible_title (troff)"

{
  printf 'kindle_name_typst: %s\n' "$typst_title"
  printf 'kindle_name_troff: %s\n' "$troff_title"
  printf 'epub_file_typst: %s-typst.epub\n' "$title_stem"
  printf 'pdf_file_typst: %s-typst.pdf\n' "$title_stem"
  printf 'epub_file_troff: %s-troff.epub\n' "$title_stem"
  printf 'pdf_file_troff: %s-troff.pdf\n' "$title_stem"
} >> dist/VERSION.md

books_dir="$HOME/icloud/books"
if [[ -d "$books_dir" ]]; then
  # Prune superseded versioned copies of this book's stems, then publish.
  find "$books_dir" -maxdepth 1 \
    \( -name "$title_stem (*" -o -name "$title_stem-typst (*" -o -name "$title_stem-troff (*" \) \
    ! -name "*($version-$commit_suffix)*" -exec rm -f {} +
  cp -L "dist/$kindle_title.epub" "$books_dir/$kindle_title.epub"
  cp -L "dist/$kindle_title.pdf"  "$books_dir/$kindle_title.pdf"
  cp "dist/$title_stem-typst.epub" "$books_dir/$typst_title.epub"
  cp "dist/$title_stem-typst.pdf"  "$books_dir/$typst_title.pdf"
  cp "dist/$title_stem-troff.epub" "$books_dir/$troff_title.epub"
  cp "dist/$title_stem-troff.pdf"  "$books_dir/$troff_title.pdf"
  echo "Published to $books_dir:"
  echo "  $kindle_title.{epub,pdf}"
  echo "  $typst_title.{epub,pdf}"
  echo "  $troff_title.{epub,pdf}"
fi

echo "Built:"
echo "  docs/guide/dist/$title_stem.pdf"
echo "  docs/guide/dist/$title_stem.epub"
echo "  docs/guide/dist/$title_stem-{typst,troff}.{pdf,epub}"
echo "  docs/guide/dist/$kindle_title.{epub,pdf} symlinks"
