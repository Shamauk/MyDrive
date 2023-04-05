#!/bin/bash

DB_FILE="mydb.db"

RESULT=$(sqlite3 "${DB_FILE}" "SELECT name FROM sqlite_master WHERE type='table' AND name='users';")

if [[ -z $RESULT ]]; then
    echo "Please run application for first time before adding new users"
    exit 1
fi 

read -p "Enter an email: " EMAIL
read -p "Enter a password: " PASSWORD

SALT=$(openssl rand -base64 16)
HASH=$(echo -n "$PASSWORD" | argon2 "$SALT" -i -l 32 | grep "Encoded:" | awk '{ print $2 }' )
output=$(sqlite3 "$DB_FILE" "UPDATE users SET password = '$HASH' WHERE email = '$EMAIL'; SELECT changes();" 2>&1)
exitstatus=$?

if [ $exitstatus -ne 0 ]; then
    echo "Unable to update password, ensure user exists"
    echo "$output"
elif [ $output -eq '0' ]; then 
    echo "Erro: No user exists"
else
    echo "Updated password successfully"
fi