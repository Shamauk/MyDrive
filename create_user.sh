#!/bin/bash

isUsernameTaken() {
    numAccounts=$(awk -F '|' -v user="$1" '{if ($1 == user) { print $1 }}' users.csv | wc -l)

    if [[ numAccounts -eq 0 ]]; then
        echo false
    else
        echo true
    fi
}

createUserFileIfNonExistent() {
    if [[ ! -e "users.csv" ]]; then
        touch users.csv
    fi
}

main() {
    createUserFileIfNonExistent

    while true; do
        read -p "Enter a username: " USERNAME

        if [[ $(isUsernameTaken $USERNAME) == true ]]; then
            echo "Username already taken, please type a new one"
        else
            break
        fi
    done

    read -p "Enter a password: " PASSWORD

    SALT=$(openssl rand -hex 16)
    HASH=$(echo -n "$PASSWORD" | argon2 "$SALT" -i -l 32 -m 12 -p 1 -t 3 | grep "Encoded:" | awk '{ print $2 }' )
    echo "$USERNAME|$HASH" >> users.csv
    echo "User added successfully"
}

main 