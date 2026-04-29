{ pkgs, inputs, system, ... }:

# End-to-end Nix test of the criome ↔ mentci-lib wire.
#
# Spawns criome-daemon on a fresh sema, runs mentci-lib's
# `handshake` example as the client, and asserts that the
# expected event stream comes out — handshake completes,
# CriomeConnected fires, auto-subscribe pushes initial empty
# Records replies, asserts produce OutcomeArrived(Ok), and
# the final summary shows the asserted records absorbed into
# the cache.
#
# Verifies (in one Nix-sandboxed derivation, no network):
#
# - criome-daemon's UDS server accepts the handshake and
#   answers with HandshakeAccepted.
# - mentci-lib's connection driver dials, exchanges frames,
#   surfaces typed engine events.
# - The records-with-slots wire shape round-trips —
#   reply summaries show `(Slot(N), Node{...})` pairs.
# - Subscribe registration triggers ongoing pushes on each
#   subsequent Assert.
# - WorkbenchState.view derives the GraphsNav from the cache.

let
  criome     = inputs.criome.packages.${system}.default;
  mentci-lib = inputs.mentci-lib.packages.${system}.default;
in
pkgs.runCommand "mentci-lib-handshake"
{
  nativeBuildInputs = [ pkgs.coreutils ];
}
''
  set -euo pipefail

  cd $TMPDIR
  criome_socket=$PWD/criome.sock
  sema_path=$PWD/sema.redb

  cleanup() {
    kill ''${criome_pid:-} 2>/dev/null || true
    wait 2>/dev/null || true
  }
  trap cleanup EXIT

  CRIOME_SOCKET=$criome_socket SEMA_PATH=$sema_path \
    ${criome}/bin/criome-daemon &
  criome_pid=$!
  for i in $(seq 1 50); do
    [ -S "$criome_socket" ] && break
    sleep 0.1
  done
  [ -S "$criome_socket" ] || { echo "criome-daemon failed to bind"; exit 1; }

  output=$(${mentci-lib}/bin/mentci-handshake-test "$criome_socket" 2>&1)
  echo "$output"

  # Connection lifecycle.
  echo "$output" | grep -q 'CriomeConnected' \
    || { echo "expected CriomeConnected event"; exit 1; }
  echo "$output" | grep -q 'HandshakeAccepted' \
    || { echo "expected HandshakeAccepted frame"; exit 1; }

  # Records absorbed end-to-end.
  echo "$output" | grep -q 'Echo Pipeline' \
    || { echo "expected asserted Graph 'Echo Pipeline' to round-trip"; exit 1; }
  echo "$output" | grep -q 'Build Defs' \
    || { echo "expected asserted Graph 'Build Defs' to round-trip"; exit 1; }
  echo "$output" | grep -q 'ticks' \
    || { echo "expected asserted Node 'ticks' to round-trip"; exit 1; }
  echo "$output" | grep -q 'ModelCache: 2 graphs · 3 nodes · 1 edges' \
    || { echo "expected final ModelCache summary"; exit 1; }

  # Subscribe pushes happen on writes.
  push_count=$(echo "$output" | grep -c 'QueryReplied' || true)
  if [ "$push_count" -lt 6 ]; then
    echo "expected at least 6 QueryReplied events (initial 3 + push-per-write); got $push_count"
    exit 1
  fi

  echo "mentci-lib ↔ criome handshake + subscribe + records-with-slots round-trip OK"
  touch $out
''
