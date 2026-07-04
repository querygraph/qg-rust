#!/usr/bin/env bash
# Build the stack one-pager in three typesettings: HTML (source-of-truth
# styling), typst PDF, and troff PDF.
#
# The troff invocation follows the omnighost troff findings:
#   -P-e  embed fonts in the PDF (unembedded fonts make viewers substitute
#         different metrics — the classic word-gap bug);
#   -t    run the tbl preprocessor for the component table;
#   the .ms source sets .na (ragged right) because the text is dense with
#   unhyphenatable tokens.
# The build asserts every font in the troff PDF is embedded.
set -euo pipefail

cd "$(dirname "$0")"

typst compile querygraph-stack.typ querygraph-stack-typst.pdf
echo "built querygraph-stack-typst.pdf"

# Homebrew's groff ships a devpdf `download` file pinned to the ghostscript
# version present when groff was built; after a ghostscript upgrade the
# Type 1 font paths dangle and gropdf silently emits unembedded fonts.
# Rewrite the paths against the ghostscript actually installed and hand the
# corrected copy to gropdf via GROFF_FONT_PATH.
groff_font_dir="$(groff --version >/dev/null && echo "$(dirname "$(command -v groff)")/../share/groff/$(groff --version | awk 'NR==1{print $NF}')/font")"
gs_font_dir="$(ls -d /opt/homebrew/Cellar/ghostscript/*/share/ghostscript/Resource/Font 2>/dev/null | sort -V | tail -1)"
fontwork="$(mktemp -d)"
trap 'rm -rf "$fontwork"' EXIT
mkdir -p "$fontwork/devpdf"
if [[ -n "$gs_font_dir" && -f "$groff_font_dir/devpdf/download" ]]; then
  sed -E "s|/opt/homebrew/Cellar/ghostscript/[^/]+/share/ghostscript/Resource/Font|$gs_font_dir|" \
    "$groff_font_dir/devpdf/download" > "$fontwork/devpdf/download"
  export GROFF_FONT_PATH="$fontwork"
fi

groff -Tpdf -P-e -t -ms querygraph-stack.ms > querygraph-stack-troff.pdf
echo "built querygraph-stack-troff.pdf"

if command -v pdffonts >/dev/null 2>&1; then
  unembedded="$(pdffonts querygraph-stack-troff.pdf | awk 'NR>2 && $(NF-4) == "no"' || true)"
  if [[ -n "$unembedded" ]]; then
    echo "ERROR: unembedded fonts in troff PDF:" >&2
    echo "$unembedded" >&2
    exit 1
  fi
  echo "troff PDF font-embedding check passed"
else
  echo "pdffonts not found; skipping embed check" >&2
fi

echo "HTML edition: querygraph-stack.html (open directly or print to PDF)"
