#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: $0 primary.pdf [variant.pdf ...]" >&2
  exit 2
fi

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
cover_image="$repo_root/cover/querygraph-cover.png"

if [[ ! -f "$cover_image" ]]; then
  echo "cover image not found: $cover_image" >&2
  exit 2
fi

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

cover_typ="$tmpdir/cover.typ"
cover_pdf="$tmpdir/cover.pdf"
cat > "$cover_typ" <<EOF
#set page(width: 8.5in, height: 11in, margin: 0pt, fill: rgb("#071d26"))
#align(center + horizon)[
  #image("$cover_image", width: 100%, height: 100%, fit: "contain")
]
EOF
typst compile --root / "$cover_typ" "$cover_pdf"

has_image_cover() {
  pdfimages -f 1 -l 1 -list "$1" 2>/dev/null \
    | awk 'NR > 2 && $1 == 1 { found = 1 } END { exit(found ? 0 : 1) }'
}

index=0
for pdf in "$@"; do
  index=$((index + 1))
  if [[ ! -f "$pdf" ]]; then
    echo "PDF not found: $pdf" >&2
    exit 2
  fi

  pages="$(pdfinfo "$pdf" | awk '/^Pages:/ { print $2 }')"
  first_body_page=1
  if has_image_cover "$pdf"; then
    first_body_page=2
  fi
  if (( pages < first_body_page )); then
    echo "PDF has no body pages: $pdf" >&2
    exit 1
  fi

  body_pattern="$tmpdir/body-$index-%04d.pdf"
  pdfseparate -f "$first_body_page" -l "$pages" "$pdf" "$body_pattern" 2>/dev/null
  body_pages=("$tmpdir"/body-"$index"-*.pdf)
  fixed="$tmpdir/fixed-$index.pdf"
  pdfunite "$cover_pdf" "${body_pages[@]}" "$fixed" 2>/dev/null
  mv "$fixed" "$pdf"
  echo "PDF cover normalized: $pdf"
done
