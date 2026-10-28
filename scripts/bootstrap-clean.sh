#!/usr/bin/env bash
# scripts/bootstrap.sh from nothing: a fresh node:22-bookworm container, this repo cloned from
# GitHub (Docker cannot run inside it, so the AVM prover's image is not covered)
set -euo pipefail
docker run --rm node:22-bookworm bash -lc '
  set -e
  apt-get update -qq && apt-get install -y -qq python3 python3-venv python3-pip git curl openssl build-essential pkg-config libssl-dev >/dev/null
  cd /root && mkdir -p w && cd w
  git clone -q https://github.com/abutlabs/aztec-on-jam && cd aztec-on-jam
  git log --oneline -1
  start=$(date +%s)
  scripts/bootstrap.sh || echo "bootstrap exit $?"
  echo "elapsed $(( $(date +%s) - start )) s"
'
