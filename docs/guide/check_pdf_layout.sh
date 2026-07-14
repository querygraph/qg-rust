#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: $0 primary.pdf [variant.pdf ...]" >&2
  exit 2
fi

for pdf in "$@"; do
  if [[ ! -f "$pdf" ]]; then
    echo "PDF layout check failed: missing $pdf" >&2
    exit 2
  fi

  pages="$(pdfinfo "$pdf" | awk '/^Pages:/ { print $2 }')"
  if (( pages < 2 )); then
    echo "PDF layout check failed: expected a cover and body in $pdf" >&2
    exit 1
  fi

  page_sizes="$(pdfinfo -f 1 -l "$pages" -box "$pdf" 2>/dev/null)"
  size_count="$(awk '/^Page +[0-9]+ size:/ { count++ } END { print count + 0 }' <<< "$page_sizes")"
  if [[ "$size_count" -ne "$pages" ]]; then
    echo "PDF layout check failed: found $size_count page sizes for $pages pages in $pdf" >&2
    exit 1
  fi
  bad_size="$(awk '/^Page +[0-9]+ size:/ && ($4 != 612 || $6 != 792) { print; exit }' <<< "$page_sizes")"
  if [[ -n "$bad_size" ]]; then
    echo "PDF layout check failed: non-letter page in $pdf: $bad_size" >&2
    exit 1
  fi

  if ! pdfimages -f 1 -l 1 -list "$pdf" 2>/dev/null \
      | awk 'NR > 2 && $1 == 1 { found = 1 } END { exit(found ? 0 : 1) }'; then
    echo "PDF layout check failed: page 1 has no image cover in $pdf" >&2
    exit 1
  fi

  if [[ -n "$(pdftotext -f 1 -l 1 "$pdf" - 2>/dev/null | tr -d '[:space:]')" ]]; then
    echo "PDF layout check failed: page 1 contains generated text in $pdf" >&2
    exit 1
  fi

  echo "PDF layout check passed: $pdf ($pages pages, letter, image cover)"
done
