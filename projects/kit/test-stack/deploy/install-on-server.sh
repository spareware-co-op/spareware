#!/usr/bin/env bash
# Runs on the server, as root, from deploy.sh. Safe to run again: it updates what's there and
# never touches stored data. Everything but the web proxy listens on 127.0.0.1 only.
set -euo pipefail

src=/opt/spare-stack/src/projects/kit/test-stack
home=/opt/spare-stack
env_file=/etc/spare-stack/stack.env
. /etc/spare-stack/owner.env

export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y -qq build-essential pkg-config curl unzip caddy iptables-persistent

id spare >/dev/null 2>&1 || useradd --system --home-dir "$home" --shell /usr/sbin/nologin spare
mkdir -p "$home/bin" "$home/data" "$home/log" "$home/backups" "$home/config"
cp "$src"/config/pd.toml "$src"/config/tikv.toml "$home/config/"

# The server's own settings, generated once and kept on the server.
if [ ! -f /etc/spare-stack/rustfs.env ]; then
  umask 077
  access="$(head -c 16 /dev/urandom | od -An -tx1 | tr -d ' \n')"
  secret="$(head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n')"
  printf 'RUSTFS_ACCESS_KEY=%s\nRUSTFS_SECRET_KEY=%s\n' "$access" "$secret" >/etc/spare-stack/rustfs.env
fi
. /etc/spare-stack/rustfs.env
umask 077
{
  cat /etc/spare-stack/owner.env
  echo "STACK_BIND=127.0.0.1:8080"
  echo "STACK_PD=127.0.0.1:2379"
  echo "STACK_FILES_URL=s3://spare-files"
  echo "STACK_BACKUP_URL=${STACK_BACKUP_URL:-file://$home/backups}"
  echo "AWS_ENDPOINT=http://127.0.0.1:9100"
  echo "AWS_ALLOW_HTTP=true"
  echo "AWS_REGION=us-east-1"
  echo "AWS_ACCESS_KEY_ID=$RUSTFS_ACCESS_KEY"
  echo "AWS_SECRET_ACCESS_KEY=$RUSTFS_SECRET_KEY"
} >"$env_file"
umask 022

STACK_HOME="$home" bash "$src/scripts/install-binaries.sh"

# Build with the compiler the repository pins.
export RUSTUP_HOME="$home/rustup" CARGO_HOME="$home/cargo" CARGO_TARGET_DIR="$home/target"
if [ ! -x "$CARGO_HOME/bin/rustup" ]; then
  curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
fi
(cd "$src" && "$CARGO_HOME/bin/cargo" build --release --locked --bin test-stack)
install -m 755 "$CARGO_TARGET_DIR/release/test-stack" "$home/bin/test-stack.new"
mv "$home/bin/test-stack.new" "$home/bin/test-stack"
chown -R spare:spare "$home/data" "$home/log" "$home/backups"

cp "$src"/deploy/units/* /etc/systemd/system/
systemctl daemon-reload
systemctl enable --now spare-pd spare-tikv spare-rustfs spare-backup.timer

for _ in $(seq 1 90); do
  curl -fs http://127.0.0.1:2379/pd/api/v1/stores 2>/dev/null | grep -q '"state_name": "Up"' && break
  sleep 1
done
for _ in $(seq 1 30); do
  curl -s -o /dev/null http://127.0.0.1:9100/ && break
  sleep 1
done
for bucket in spare-files spare-backups; do
  curl -fsS -o /dev/null --aws-sigv4 "aws:amz:us-east-1:s3" \
    --user "$RUSTFS_ACCESS_KEY:$RUSTFS_SECRET_KEY" -X PUT "http://127.0.0.1:9100/$bucket" || true
done

systemd-run --wait --quiet --pipe --uid=spare --property=EnvironmentFile="$env_file" \
  "$home/bin/test-stack" seed
systemctl enable spare-server
systemctl restart spare-server

# The web address decides the proxy: https with a free certificate, or plain http for a test.
site="${STACK_PUBLIC_URL#https://}"
sed "s|SITE|$site|" "$src/deploy/Caddyfile" >/etc/caddy/Caddyfile
systemctl reload-or-restart caddy

# Oracle's Ubuntu images refuse every incoming port but 22 until told otherwise.
for port in 80 443; do
  iptables -C INPUT -p tcp --dport "$port" -j ACCEPT 2>/dev/null \
    || iptables -I INPUT 1 -p tcp --dport "$port" -j ACCEPT
done
netfilter-persistent save >/dev/null

for _ in $(seq 1 30); do
  curl -fs http://127.0.0.1:8080/health >/dev/null && break
  sleep 1
done
echo "health: $(curl -fsS http://127.0.0.1:8080/health)"
echo "done: $STACK_PUBLIC_URL"
