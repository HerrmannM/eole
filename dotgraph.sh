#!/bin/bash
DIR="generated"

for filename in $DIR/*.dot; do
  [ -f "$filename" ] || continue
  echo $filename
  dot -Tpdf "$filename" -o $DIR/$(basename "$filename" .dot).pdf
done

# Test if dir is empty or not
if [ "$(ls -A $DIR)" ]; then
  cd $DIR
  pdfunite *.pdf out.pdf
fi
