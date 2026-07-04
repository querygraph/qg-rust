#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 || $# -gt 3 ]]; then
  echo "usage: $0 path/to/querygraph.epub [kindle-title] [visible-title]" >&2
  exit 2
fi

epub="$1"
expected_title="${2:-querygraph}"
visible_title="${3:-Querygraph}"
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
  ' "$(dirname "$0")/metadata.yaml"
)"
dist_dir="$(dirname "$epub")"
stable="$dist_dir/$title_stem.epub"
versioned="$dist_dir/$expected_title.epub"
marker="$dist_dir/VERSION.md"

if [[ ! -f "$epub" ]]; then
  echo "EPUB not found: $epub" >&2
  exit 2
fi

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT
unzip -q "$epub" -d "$tmpdir"

opf="$tmpdir/EPUB/content.opf"
nav="$tmpdir/EPUB/nav.xhtml"
toc="$tmpdir/EPUB/toc.ncx"
cover="$tmpdir/EPUB/text/ch001.xhtml"
css="$tmpdir/EPUB/styles/stylesheet1.css"

require() {
  local pattern="$1"
  local file="$2"
  local message="$3"
  if ! perl -0ne "if (m{$pattern}s) { \$ok = 1 } END { exit(\$ok ? 0 : 1) }" "$file"; then
    echo "EPUB metadata check failed: $message" >&2
    exit 1
  fi
}

reject() {
  local pattern="$1"
  local file="$2"
  local message="$3"
  if perl -0ne "if (m{$pattern}s) { \$bad = 1 } END { exit(\$bad ? 0 : 1) }" "$file"; then
    echo "EPUB metadata check failed: $message" >&2
    exit 1
  fi
}

escaped_title="$(printf '%s' "$expected_title" | perl -0777 -ne 'print quotemeta($_)')"
escaped_visible="$(printf '%s' "$visible_title" | perl -0777 -ne 'print quotemeta($_)')"

require "<dc:title[^>]*>$escaped_title</dc:title>" "$opf" "OPF title is not $expected_title"
require "<meta refines=\"\\#epub-title-1\" property=\"file-as\">$escaped_title</meta>" "$opf" "OPF file-as title is not $expected_title"
require "<dc:creator[^>]*>Alexy Khrabrov and Slava Tykhonov</dc:creator>" "$opf" "OPF creator is not Alexy Khrabrov and Slava Tykhonov"
require "<dc:language[^>]*>en-US</dc:language>" "$opf" "OPF language is not en-US"
require "<docTitle>\\s*<text>$escaped_visible</text>\\s*</docTitle>" "$toc" "NCX title is not $visible_title"
require "<title>$escaped_visible</title>" "$nav" "nav document title is not $visible_title"
require "<h1[^>]*>$escaped_visible</h1>" "$nav" "nav heading is not $visible_title"
require "<body epub:type=\"frontmatter\">" "$cover" "cover body is not frontmatter"
require "Alexy Khrabrov and Slava Tykhonov" "$cover" "cover authors are not Alexy Khrabrov and Slava Tykhonov"
require "querygraph\\.ai" "$cover" "cover site is not querygraph.ai"
reject "chiefscientist\\.org" "$cover" "cover still references chiefscientist.org"
reject "<h1 class=\"unnumbered\">$escaped_visible</h1>" "$cover" "generated cover heading remains in cover XHTML"
reject "display:\\s*flex" "$cover" "cover uses flexbox"
require "pre > code\\.sourceCode > span:empty" "$css" "compact code-block CSS missing"

if [[ ! -f "$stable" ]]; then
  echo "EPUB metadata check failed: missing stable EPUB $stable" >&2
  exit 1
fi
if ! cmp -s "$epub" "$stable"; then
  echo "EPUB metadata check failed: $epub and $stable differ" >&2
  exit 1
fi
if [[ ! -L "$versioned" ]]; then
  echo "EPUB metadata check failed: missing versioned symlink $versioned" >&2
  exit 1
fi
if [[ "$(readlink "$versioned")" != "$title_stem.epub" ]]; then
  echo "EPUB metadata check failed: $versioned does not point to $title_stem.epub" >&2
  exit 1
fi
if [[ ! -f "$marker" ]]; then
  echo "EPUB metadata check failed: missing $marker" >&2
  exit 1
fi
grep -Fx "kindle_name: $expected_title" "$marker" >/dev/null
grep -Fx "epub_file: $title_stem.epub" "$marker" >/dev/null
grep -Fx "kindle_link: $expected_title.epub" "$marker" >/dev/null

echo "EPUB metadata check passed: $epub"
