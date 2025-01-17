#!/bin/bash

set -eu

if [ "$EUID" -ne 0 ]
  then echo "Please run as sudo"
  exit
fi

mkdir -p /usr/local/etc/tjaele
cp ./example_config.toml /usr/local/etc/tjaele/config.toml

mkdir -p /usr/local/bin
cp ./target/release/tjaele /usr/local/bin/tjaele
chmod +x /usr/local/bin/tjaele

mkdir -p /usr/local/lib/systemd/system
cp ./tjaele.service /usr/local/lib/systemd/system/tjaele.service

systemctl daemon-reload
systemctl enable tjaele
systemctl start tjaele

echo "Now edit config.toml file in /usr/local/etc/tjaele"
