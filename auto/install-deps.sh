#!/bin/bash

if [[ -z `which arduino-cli` ]]; then
  echo "Missing arduino-cli"
  exit 1
fi

arduino-cli lib uninstall DFRobot_GDL
arduino-cli lib install --git-url https://github.com/dadleyy/DFRobot_GDL.git
