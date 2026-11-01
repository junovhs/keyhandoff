#!/usr/bin/env sh
# Builds paper/paper.tex and paper/paper.pdf from paper/draft.md.
# Needs pandoc (3.x) and tectonic on PATH. paper.tex is what goes to arXiv.
set -eu
cd "$(dirname "$0")"
pandoc draft.md --from markdown --to latex --standalone \
  --shift-heading-level-by=-1 --columns=40 --lua-filter=arxiv.lua \
  -V documentclass=article -V classoption=11pt -V geometry:margin=1in \
  -V colorlinks=true -V linkcolor=black -V urlcolor=blue \
  -V header-includes="\\usepackage{xurl}" \
  --output paper.tex
tectonic paper.tex
echo "wrote paper/paper.tex and paper/paper.pdf"
