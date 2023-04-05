#!/bin/bash

if [ "$(id -u)" -eq 0 ]; then
    echo "This script should not be run as root. Exiting."
    exit 1
fi

main() {
    installLibraries
    installRust
    setupDirectory
    createKey
    makeKeyReadWrite
    createConfigurationFile
    echo "Finished"
}

installLibraries() {
    if [ -x "$(command -v brew)" ]; then 
        brew update
        brew install openssl@1.1
        brew install argon2
        brew install tmux
    elif [ -x "$(command -v apt-get)" ]; then
        sudo apt-get update
        sudo apt-get upgrade
        sudo apt-get install pkg-config libssl-dev libsqlite3-dev build-essential tmux
        sudo apt-get -y install argon2
    elif [ -x "$(command -v pacman)"]; then
        sudo pacman -Syu
        sudo pacman -S pkg-config openssl argon2 tmux
    elif [ -x "$(command -v dnf)"]; then
        sudo dnf update
        sudo dnf install pkg-config openssl-devel sqlite-devel make automake gcc gcc-c+ tmux
        sudo dnf -y install argon2
    elif [ -x "$(command -v apk)"]; then
        sudo apk update
        apk add pkgconfig openssl-dev argon2 tmux
    else
        echo "Please install OpenSSL and try again"
        exit 1
    fi
}

installRust() {
    if [ -x "$(command -v rustc)" ]; then
        echo "Rust is already installed. Skipping installation."
    else
        echo "Installing Rust..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
        rustup default stable
    fi
}

setupDirectory() {
    mkdir ssl
}

createKey() {
    openssl genrsa -out ssl/private_key.pem 2048
    openssl req -new -key ssl/private_key.pem -out ssl/csr.pem -subj "/CN=$(curl -s https://api.ipify.org)/"
    openssl x509 -req -days 365 -in ssl/csr.pem -signkey ssl/private_key.pem -out ssl/certificate.pem 
    openssl x509 -in ssl/certificate.pem -out ssl/certificate.cer -outform DER
}

makeKeyReadWrite() {
    chmod +r ssl/certificate.cer
    chmod +r ssl/certificate.pem
    chmod +r ssl/private_key.pem
    chmod +r ssl/csr.pem
}

createConfigurationFile() {
    while true; do
        read -p "Enter private port: " PORT

        if [[ -z "$PORT" ]]; then
            echo "Please enter a non empty response"
        else
            break
        fi
    done

    while true; do
        read -p "Enter file storage point: " DIR

        if [[ -z "$DIR" ]]; then
            echo "Please enter a non empty response"
        else
            break
        fi
    done

    if [[ ! -d "$DIR" ]]; then
        mkdir -p "$DIR/mydrive"
    fi

    SECRET_KEY=$(openssl rand -hex 32)

    if [[ -f "Rocket.toml" ]]; then
        rm Rocket.toml
    fi

    touch Rocket.toml
    echo "[default]
port = $PORT
directory = \"$DIR/mydrive\"
secret_key = \"$SECRET_KEY\"

[default.tls]
key = \"./ssl/private_key.pem\"
certs = \"./ssl/certificate.pem\"" >> Rocket.toml
}

main
