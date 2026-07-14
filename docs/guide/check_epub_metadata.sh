#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "usage: $0 path/to/book.epub expected-title" >&2
  exit 2
fi

epub="$1"
expected_title="$2"
epub_path="$(cd "$(dirname "$epub")" && pwd)/$(basename "$epub")"
dist_dir="$(dirname "$epub_path")"

if [[ ! -f "$epub_path" ]]; then
  echo "EPUB not found: $epub_path" >&2
  exit 2
fi

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT
unzip -q "$epub_path" -d "$tmpdir/book"

opf="$tmpdir/book/EPUB/content.opf"
toc="$tmpdir/book/EPUB/toc.ncx"
nav="$tmpdir/book/EPUB/nav.xhtml"
cover="$tmpdir/book/EPUB/text/cover.xhtml"
stylesheet="$tmpdir/book/EPUB/styles/stylesheet1.css"
opf_flat="$tmpdir/content.flat"
toc_flat="$tmpdir/toc.flat"
tr '\n\r\t' '   ' < "$opf" > "$opf_flat"
tr '\n\r\t' '   ' < "$toc" > "$toc_flat"

require_pattern() {
  local pattern="$1"
  local file="$2"
  local message="$3"
  if ! grep -Eq "$pattern" "$file"; then
    echo "EPUB metadata check failed: $message" >&2
    exit 1
  fi
}

reject_pattern() {
  local pattern="$1"
  local file="$2"
  local message="$3"
  if grep -Eq "$pattern" "$file"; then
    echo "EPUB metadata check failed: $message" >&2
    exit 1
  fi
}

regex_escape() {
  sed 's/[][(){}.^$*+?|\\]/\\&/g' <<< "$1"
}

expected_title_pattern="$(regex_escape "$expected_title")"
expected_stem="${expected_title% (*}"
stable_epub="$dist_dir/$expected_stem.epub"
version_marker="$dist_dir/VERSION.md"

require_pattern "<dc:title[^>]*>$expected_title_pattern</dc:title>" "$opf" "missing dc:title"
require_pattern "<meta[^>]*refines=\"#epub-title-1\"[^>]*property=\"file-as\"[^>]*>$expected_title_pattern</meta>" "$opf" "missing title sort metadata"
require_pattern '<dc:creator[^>]*>Alexy Khrabrov and Slava Tykhonov</dc:creator>' "$opf" "missing dc:creator"
require_pattern '<dc:publisher>First Pair Press</dc:publisher>' "$opf" "missing First Pair Press publisher metadata"
require_pattern '<dc:language>en-US</dc:language>' "$opf" "missing dc:language"
require_pattern '<dc:date[^>]*>[0-9]{4}-[0-9]{2}-[0-9]{2}</dc:date>' "$opf" "missing dc:date"
require_pattern '<meta[^>]+property="dcterms:modified"' "$opf" "missing dcterms:modified"
require_pattern '<meta name="cover" content="[^"]+" />' "$opf" "missing cover metadata"
require_pattern '<item properties="cover-image"[^>]*href="media/[^"]+"' "$opf" "missing cover-image manifest item"
require_pattern '<spine toc="ncx">[[:space:]]*<itemref idref="cover_xhtml" />[[:space:]]*<itemref idref="nav" linear="no" />[[:space:]]*<itemref idref="ch001_xhtml" />' "$opf_flat" "image cover is not first in the reading spine"
require_pattern '<docTitle>[[:space:]]*<text>The QueryGraph Stack</text>[[:space:]]*</docTitle>' "$toc_flat" "NCX title is not The QueryGraph Stack"
require_pattern '<title>The QueryGraph Stack</title>' "$nav" "nav document title is not The QueryGraph Stack"
require_pattern '<h1[^>]*>The QueryGraph Stack</h1>' "$nav" "nav heading is not The QueryGraph Stack"
require_pattern '<body id="cover">' "$cover" "cover XHTML is not the image cover"
require_pattern '<div id="cover-image">' "$cover" "cover XHTML is missing its image wrapper"
require_pattern '<svg[^>]*viewBox="0 0 1024 1536"' "$cover" "cover XHTML has the wrong geometry"
require_pattern '<image[^>]*xlink:href="\.\./media/[^"]+"' "$cover" "cover XHTML does not reference the cover image"
require_pattern 'div\.sourceCode' "$stylesheet" "stylesheet is missing sourceCode rules"
require_pattern 'line-height:[[:space:]]*1\.12' "$stylesheet" "stylesheet is missing compact code line-height"
require_pattern 'pre[[:space:]]*>[[:space:]]*code\.sourceCode[[:space:]]*>[[:space:]]*span:empty' "$stylesheet" "stylesheet is missing empty source-line rules"
require_pattern 'display:[[:space:]]*none' "$stylesheet" "stylesheet is missing empty source-line suppression"

reject_pattern 'UNTITLED|Unknown' "$opf" "fallback OPF metadata found"
reject_pattern 'UNTITLED|Unknown' "$toc" "fallback NCX metadata found"
reject_pattern 'UNTITLED|Unknown' "$nav" "fallback nav metadata found"
reject_pattern 'display:[[:space:]]*flex' "$cover" "cover uses flexbox"

cover_href="$(sed -n 's/.*properties="cover-image"[^>]*href="\([^"]*\)".*/\1/p' "$opf" | head -n 1)"
[[ -n "$cover_href" ]] || { echo "EPUB metadata check failed: cover image href is missing" >&2; exit 1; }
if ! cmp -s "$tmpdir/book/EPUB/$cover_href" "$(cd "$(dirname "$0")/../.." && pwd)/cover/querygraph-cover.png"; then
  echo "EPUB metadata check failed: packaged cover differs from cover/querygraph-cover.png" >&2
  exit 1
fi

if [[ -e "$tmpdir/book/EPUB/text/title_page.xhtml" ]]; then
  echo "EPUB metadata check failed: generated empty title_page.xhtml is present" >&2
  exit 1
fi

[[ -f "$stable_epub" ]] || { echo "EPUB metadata check failed: missing stable EPUB $stable_epub" >&2; exit 1; }
cmp -s "$epub_path" "$stable_epub" || { echo "EPUB metadata check failed: stable EPUB differs" >&2; exit 1; }
[[ -f "$version_marker" ]] || { echo "EPUB metadata check failed: VERSION.md is missing" >&2; exit 1; }

kindle_link="$(awk -F': ' '/^kindle_link:/ { print $2 }' "$version_marker")"
epub_link="$(awk -F': ' '/^epub_link:/ { print $2 }' "$version_marker")"
pdf_link="$(awk -F': ' '/^pdf_link:/ { print $2 }' "$version_marker")"
for link in "$kindle_link" "$epub_link"; do
  [[ -L "$dist_dir/$link" ]] || { echo "EPUB metadata check failed: missing EPUB symlink $link" >&2; exit 1; }
  [[ "$(readlink "$dist_dir/$link")" == "$expected_stem.epub" ]] || { echo "EPUB metadata check failed: bad EPUB symlink $link" >&2; exit 1; }
done
[[ -e "$dist_dir/$pdf_link" ]] || { echo "EPUB metadata check failed: missing PDF link $pdf_link" >&2; exit 1; }

require_pattern "^kindle_name: $expected_title_pattern$" "$version_marker" "VERSION.md missing Kindle name"
require_pattern '^version_stamp: [0-9]+\.[0-9]+\.[0-9]+-[0-9a-z]+$' "$version_marker" "VERSION.md missing version stamp"
require_pattern '^built_at: [0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$' "$version_marker" "VERSION.md missing build timestamp"
require_pattern "^epub_file: $(regex_escape "$(basename "$stable_epub")")$" "$version_marker" "VERSION.md missing stable EPUB filename"
require_pattern "^pdf_file: $(regex_escape "$expected_stem.pdf")$" "$version_marker" "VERSION.md missing stable PDF filename"

echo "EPUB metadata check passed: $epub_path"
