#!/usr/bin/env sh
set -e
DIR="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$DIR/certs"
openssl req -x509 -newkey rsa:4096 \
  -keyout "$DIR/certs/server-key.pem" \
  -out "$DIR/certs/server.pem" \
  -days 3650 -nodes \
  -subj "/CN=nats"
cp "$DIR/certs/server.pem" "$DIR/certs/ca.pem"
chmod 644 "$DIR/certs/server.pem" "$DIR/certs/ca.pem"
chmod 600 "$DIR/certs/server-key.pem"
echo "NATS TLS certs written to $DIR/certs/"
