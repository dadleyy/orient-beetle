#!/bin/bash

if [[ -z `which fontgen` ]]; then
  echo "missing 'fontgen' in PATH"
  exit 1
fi

fontgen ../../.resources/Glyphter-font/Glyphter.ttf > ./include/glyphter_ttf.hpp
fontgen ../../.resources/Jellee_1223/TTF/Jellee-Bold.ttf > ./include/jellee_ttf.hpp
