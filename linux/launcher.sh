#!/bin/bash

PROGRAM=subrosa.x64
LIBRARY=libclient.so
GDB=gdb_old
if ! [ -f "$PROGRAM" -a -f "$LIBRARY" -a -f "$GDB" ]; then
  echo -e "\e[31mOne of the neccesary files for the launcher is missing ('$PROGRAM', '$LIBRARY' or '$GDB') in the current folder, please put the launcher in the root folder of Sub Rosa.\e[0m"
    exit 1
fi

PWD=$(pwd | sed 's/ /\\ /g')
alias cwd='printf "%q\n" "$(pwd)" | pbcopy'

LD_PRELOAD="./$LIBRARY" ./$PROGRAM
