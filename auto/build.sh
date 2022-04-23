#!/bin/bash

if [[ -z "$(which arduino-cli)" ]]; then
  echo "Please install arduino-cli before continuing"
  exit 1
fi

if [[ -z "${SEEDUINO_XIAO}" ]]; then
  echo "target: dfrobot esp32"
  sleep 1

  arduino-cli compile \
    -v \
    -b DFRobot:esp32:esp32-e \
    --clean \
    --output-dir ./target/arduino/debug \
    src/orient-beetle

  if [[ $? -eq 0 ]]; then
    echo "compile complete"
  else
    echo "compile failed"
    exit 1
  fi
else
  echo "target: seeduino xiao"
  sleep 1

  arduino-cli compile \
    -v \
    -b Seeeduino:samd:seeed_XIAO_m0 \
    --output-dir ./target/arduino/debug \
    src/orient-beetle
fi


if [[ -z "${1}" ]]; then
  exit 0
fi

if [[ -z "${SEEDUINO_XIAO}" ]]; then
  arduino-cli upload \
    -v \
    -b DFRobot:esp32:esp32-e \
    --input-dir ./target/arduino/debug \
    -p $1
else
  arduino-cli upload \
    -v \
    -b Seeeduino:samd:seeed_XIAO_m0 \
    --input-dir ./target/arduino/debug \
    -p $1
fi
