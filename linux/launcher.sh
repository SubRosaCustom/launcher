#!/bin/bash

PROGRAM=subrosa.x64
LIBRARY=libclient.so

if ! [ -f "$PROGRAM" -a -f "$LIBRARY" ]; then
  echo -e "\e[31mOne of the neccesary files for the launcher is missing ('$PROGRAM' or '$LIBRARY') in the current folder, please put the launcher in the root folder of Sub Rosa.\e[0m"
    exit 1
fi

LD_PRELOAD="./$LIBRARY" ./$PROGRAM
