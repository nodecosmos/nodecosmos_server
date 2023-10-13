#!/bin/bash
# Not used anymore
# It was only used to test monitoring stack locally.
# We will keep it for now as we might want to create production version of this script,
# where we will append scylla installation and use it to add new nodes to the cluster.

# Check if gpg is installed
if ! command -v gpg &> /dev/null; then
    # Update the package list
    apt update

    # Install gpg
    apt install -y gpg
fi

# Check and create directory if it doesn't exist
if [ ! -d "/etc/apt/keyrings" ]; then
    mkdir -p /etc/apt/keyrings
fi

# Check for the keyring and fetch keys if it doesn't exist
if [ ! -f "/etc/apt/keyrings/scylladb.gpg" ]; then
    gpg --homedir /tmp --no-default-keyring --keyring /etc/apt/keyrings/scylladb.gpg --keyserver hkp://keyserver.ubuntu.com:80 --recv-keys d0a112e067426ab2
fi

sudo wget -O /etc/apt/sources.list.d/scylla-manager.list http://downloads.scylladb.com/deb/ubuntu/scylladb-manager-3.2.list


# Check if scylla-manager-agent is installed
if ! dpkg -l | grep scylla-manager-agent &> /dev/null; then
  apt update
  apt install -y scylla-manager-agent
fi

FILE="/etc/scylla-manager-agent/scylla-manager-agent.yaml"
TOKEN="auth_token: QYkHdDUmj2lQXwg8wpzertKI0hdBLJuwTuT9Sx6Jzw211DyeHwakHVZZw8mE18wueXFtGgckzsPteRG2Ji0kwMfx5bbwGZ7zH7h1vc91LpsEfH59RNLku3ETWaUQzoqm"

# Check if the token exists in the file
if ! grep -q "$TOKEN" "$FILE"; then
    echo "$TOKEN" >> "$FILE"
fi


# Rsyslog for loki
FILE="/etc/rsyslog.d/scylla.conf"
LINE="if \$programname == 'scylla' then @@172.17.0.1:1514;RSYSLOG_SyslogProtocol23Format"

# Check if the line is present in the file
grep -qF "$LINE" $FILE

# $? is a special variable that holds the exit status of the last command executed
if [ $? -ne 0 ]; then
    # Line was not found, append it to the file
    echo "$LINE" >> $FILE
    echo "Line added to $FILE"
else
    echo "Line already exists in $FILE"
fi


# exist script
exit 0