#!/bin/sh
set -e

add_user_to_socket_group() {
  sock="$1"
  group_name="$2"
  if [ -S "$sock" ]; then
    gid=$(stat -c '%g' "$sock" 2>/dev/null || stat -f '%g' "$sock")
    if ! getent group "$group_name" >/dev/null 2>&1; then
      groupadd -g "$gid" "$group_name" 2>/dev/null || groupadd "$group_name"
    fi
    usermod -aG "$group_name" appuser 2>/dev/null || true
  fi
}

add_user_to_socket_group /var/run/docker.sock dockerhost

if [ -S /var/run/docker.sock ]; then
  if ! gosu appuser docker info >/dev/null 2>&1; then
    chmod 666 /var/run/docker.sock 2>/dev/null || true
  fi
fi

if [ -S /var/run/buildkit/buildkitd.sock ]; then
  chmod 666 /var/run/buildkit/buildkitd.sock 2>/dev/null || true
fi

mkdir -p /home/appuser
chown appuser:appuser /home/appuser 2>/dev/null || true

exec gosu appuser env HOME=/home/appuser "$@"
