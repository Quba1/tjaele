#!/bin/bash

set -eu

if [ "$EUID" -ne 0 ]
  then echo "Please run as sudo"
  exit
fi

mkdir -p /usr/local/etc/tjaele
cp ./utils/example_config.toml /usr/local/etc/tjaele/config.toml

cp ./target/release/tjaeled /usr/local/sbin/tjaeled
chmod +x /usr/local/sbin/tjaeled

cp ./target/release/tjaele /usr/local/bin/tjaele
chmod +x /usr/local/bin/tjaele

mkdir -p /usr/local/lib/systemd/system
cp ./utils/tjaele.service /usr/local/lib/systemd/system/tjaele.service

systemctl daemon-reload
systemctl enable tjaele
systemctl start tjaele

echo "Now edit config.toml file in /usr/local/etc/tjaele"
