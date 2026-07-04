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

books_dir="$HOME/icloud/books"
if [[ -d "$books_dir" ]]; then
  cp -L "dist/$kindle_title.epub" "$books_dir/$kindle_title.epub"
  cp -L "dist/$kindle_title.pdf"  "$books_dir/$kindle_title.pdf"
  echo "Published to $books_dir:"
  echo "  $kindle_title.epub"
  echo "  $kindle_title.pdf"
fi

echo "Built:"
echo "  docs/guide/dist/$title_stem.pdf"
echo "  docs/guide/dist/$title_stem.epub"
echo "  docs/guide/dist/$kindle_title.{epub,pdf} symlinks"
