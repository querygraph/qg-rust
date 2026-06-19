#!/usr/bin/env bash
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

echo "Built:"
echo "  docs/book/dist/$title_stem.pdf"
echo "  docs/book/dist/$title_stem.epub"
echo "  docs/book/dist/$kindle_title.epub -> $title_stem.epub"
echo "  docs/book/dist/$title_stem.mobi"
echo "  docs/book/dist/VERSION.md"
echo "  docs/book/diagrams/*.mmd and *.png"
echo "  docs/blog/assets/querygraph/diagrams/*.mmd and *.png"
