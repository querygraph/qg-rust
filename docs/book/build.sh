#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

mkdir -p build dist
html_emitter="${HTML_BOOK_EMITTER:-$HOME/src/firstpair/publishing/scripts/emit-html-book.sh}"

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
title_stem="$(
  awk -F: '
    $1 ~ /^[[:space:]]*title_stem[[:space:]]*$/ {
      value = $2
      sub(/^[[:space:]]*/, "", value)
      sub(/[[:space:]]*$/, "", value)
      gsub(/^["'\''"]|["'\''"]$/, "", value)
      print value
      exit
    }
  ' metadata.yaml
)"
visible_title="$(
  awk -F: '
    $1 ~ /^[[:space:]]*title[[:space:]]*$/ {
      value = $2
      sub(/^[[:space:]]*/, "", value)
      sub(/[[:space:]]*$/, "", value)
      gsub(/^["'\''"]|["'\''"]$/, "", value)
      print value
      exit
    }
  ' metadata.yaml
)"

if [[ -z "$version" || -z "$title_stem" || -z "$visible_title" ]]; then
  echo "could not read package version, title_stem, or title from book metadata" >&2
  exit 1
fi

commit_suffix="$(git -C ../.. rev-parse --short HEAD)"
if [[ -z "$commit_suffix" ]]; then
  echo "could not read git commit suffix" >&2
  exit 1
fi

kindle_title="$title_stem ($version-$commit_suffix)"
{
  printf 'kindle_name: %s\n' "$kindle_title"
  printf 'built_at: %s\n' "$pubdate"
  printf 'epub_file: %s.epub\n' "$title_stem"
  printf 'kindle_link: %s.epub\n' "$kindle_title"
} > dist/VERSION.md

node build.mjs
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
  --resource-path build \
  --output "$tmpdir/body.typ" \
  build/manuscript.rendered.md

{
  printf '#outline(title: [Contents])\n'
  printf '#pagebreak()\n\n'
  cat "$tmpdir/body.typ"
} > "$tmpdir/body-with-toc.typ"

cp -R build/diagrams "$tmpdir/diagrams"
typst compile "$tmpdir/body-with-toc.typ" "$tmpdir/body.pdf"
pdfunite "$tmpdir/cover.pdf" "$tmpdir/body.pdf" "dist/$title_stem.pdf"

pandoc --from markdown+smart \
  --metadata-file metadata.yaml \
  --metadata date="$pubdate" \
  --epub-title-page=false \
  --toc --toc-depth=2 \
  --css epub.css \
  --resource-path build \
  --output "dist/$title_stem.epub" \
  "$tmpdir/cover.epub.md" build/manuscript.rendered.md

./fix_epub_layout.sh "dist/$title_stem.epub" "$kindle_title" "$visible_title"

find dist -maxdepth 1 -name "$title_stem (*).epub" -exec rm -f {} +
ln -s "$title_stem.epub" "dist/$kindle_title.epub"

./check_epub_metadata.sh "dist/$title_stem.epub" "$kindle_title" "$visible_title"

# Maintain a versioned symlink for the PDF too (mirrors the EPUB symlink), so
# both formats carry the `stem (version-hash)` name.
find dist -maxdepth 1 -name "$title_stem (*).pdf" -exec rm -f {} +
ln -s "$title_stem.pdf" "dist/$kindle_title.pdf"

if [[ ! -x "$html_emitter" ]]; then
  echo "missing HTML book emitter: $html_emitter" >&2
  exit 1
fi
REPO_ROOT="$(cd ../.. && pwd)" \
  BOOK_ROOT="docs/book" \
  BOOK_DIST_DIR="$PWD/dist" \
  BOOK_BUILD_DIR="$PWD/build" \
  BOOK_METADATA="$PWD/metadata.yaml" \
  BOOK_HTML_COVER="$PWD/build/cover.rendered.md" \
  BOOK_HTML_MANUSCRIPT="$PWD/build/manuscript.rendered.md" \
  BOOK_HTML_CSS="$PWD/epub.css" \
  BOOK_STEM="$title_stem" \
  BOOK_VISIBLE_TITLE="$visible_title" \
  BOOK_VERSION="$version" \
  BOOK_VERSION_STAMP="$version-$commit_suffix" \
  BOOK_HTML_RESOURCE_PATH="$PWD/build:$PWD:$(cd ../.. && pwd)" \
  "$html_emitter"

EBOOK_CONVERT="${EBOOK_CONVERT:-}"
if [[ -z "$EBOOK_CONVERT" ]]; then
  if command -v ebook-convert >/dev/null 2>&1; then
    EBOOK_CONVERT="$(command -v ebook-convert)"
  elif [[ -x /Applications/calibre.app/Contents/MacOS/ebook-convert ]]; then
    EBOOK_CONVERT=/Applications/calibre.app/Contents/MacOS/ebook-convert
  else
    echo "ebook-convert not found; cannot produce MOBI" >&2
    exit 1
  fi
fi

"$EBOOK_CONVERT" "dist/$title_stem.epub" "dist/$title_stem.mobi"

# Publish both versioned formats to the local iCloud books library, if present.
# Dereference the symlinks so the destination holds regular, versioned files.
books_dir="$HOME/icloud/books"
if [[ -d "$books_dir" && -w "$books_dir" ]]; then
  if find "$books_dir" -maxdepth 1 -name "$title_stem (*" \
    ! -name "*($version-$commit_suffix)*" -exec rm -f {} + &&
    cp -L "dist/$kindle_title.epub" "$books_dir/$kindle_title.epub" &&
    cp -L "dist/$kindle_title.pdf"  "$books_dir/$kindle_title.pdf"; then
    echo "Published to $books_dir:"
    echo "  $kindle_title.epub"
    echo "  $kindle_title.pdf"
  else
    echo "Skipped library publish: copy to $books_dir failed"
  fi
else
  echo "Skipped library publish: $books_dir not present or not writable"
fi

echo "Built:"
echo "  docs/book/dist/$title_stem.pdf"
echo "  docs/book/dist/$title_stem.epub"
echo "  docs/book/dist/$kindle_title.epub -> $title_stem.epub"
echo "  docs/book/dist/$kindle_title.pdf -> $title_stem.pdf"
echo "  docs/book/dist/$title_stem.html"
echo "  docs/book/dist/$title_stem.mobi"
echo "  docs/book/dist/VERSION.md"
echo "  docs/book/diagrams/*.mmd and *.png"
echo "  docs/blog/assets/querygraph/diagrams/*.mmd and *.png"
