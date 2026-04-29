{ pkgs, inputs, system, ... }:

# `checks.default` — linkFarm of the seven CANON crate checks.
#
# Each crate's `checks.default` lives in its own flake input;
# this file aggregates them so `nix build .#checks.<sys>.default`
# builds the whole workspace's unit-test surface as one path.
#
# The other checks in this directory ([`integration.nix`](./integration.nix),
# [`scenario-assert-node.nix`](./scenario-assert-node.nix),
# [`scenario-query-nodes.nix`](./scenario-query-nodes.nix),
# [`scenario-chain.nix`](./scenario-chain.nix)) are
# blueprint-discovered as their own top-level checks and run
# independently under `nix flake check`.

pkgs.linkFarm "mentci-workspace-crate-checks" [
  { name = "nota-derive"; path = inputs.nota-derive.checks.${system}.default; }
  { name = "nota-codec";  path = inputs.nota-codec.checks.${system}.default; }
  { name = "signal";      path = inputs.signal.checks.${system}.default; }
  { name = "sema";        path = inputs.sema.checks.${system}.default; }
  { name = "criome";      path = inputs.criome.checks.${system}.default; }
  { name = "nexus";       path = inputs.nexus.checks.${system}.default; }
  { name = "nexus-cli";   path = inputs.nexus-cli.checks.${system}.default; }
  { name = "mentci-lib";  path = inputs.mentci-lib.checks.${system}.default; }
  { name = "mentci-egui"; path = inputs.mentci-egui.checks.${system}.default; }
]
