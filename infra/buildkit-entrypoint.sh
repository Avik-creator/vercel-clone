#!/bin/sh
set -e
mkdir -p /home/appuser
chown 1001:1001 /home/appuser 2>/dev/null || true
exec buildkitd "$@"
