#!/bin/bash

if [ "$(id -u)" -eq 0 ]; then
    echo "This script should not be run as root. Exiting."
    exit 1
fi

tmux has-session -t mydrive 2>/dev/null
if [[ $? == 0 ]]; then
    echo "MyDrive is currently running"

    while true; do
        read -p "Want to restart: (y/n) " ANSWER

        if [[ "$ANSWER" == "n" ]]; then
            echo "Exiting..."
            exit 0
        elif [[ "$ANSWER" == "y" ]]; then
            echo "Terminating running instance"
            tmux kill-session -t mydrive
            break
        fi
    done
fi

tmux new-session -d -s mydrive "cargo run --release src/main.rs"
echo "Started MyDrive"