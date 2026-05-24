#!/bin/sh
set -e

if [ -S /var/run/docker.sock ]; then
  DOCKER_GID=$(stat -c '%g' /var/run/docker.sock 2>/dev/null || stat -f '%g' /var/run/docker.sock)
  if ! getent group dockerhost >/dev/null 2>&1; then
    groupadd -g "$DOCKER_GID" dockerhost 2>/dev/null || groupadd dockerhost
  fi
  usermod -aG dockerhost appuser 2>/dev/null || true
fi

exec gosu appuser "$@"
