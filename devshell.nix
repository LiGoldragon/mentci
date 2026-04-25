{ pkgs, inputs, system }:
let
  # Sibling repos under ~/git/ to expose as symlinks in ./repos/.
  # This list IS the canonical workspace manifest for agents.
  # Entries align with docs/workspace-manifest.md.
  # Direnv / nix develop entry creates the links.
  linkedRepos = [
    "tools-documentation"
    # --- sema-ecosystem CANON ---
    "criome"          # spec repo — runtime pillar
    "nota"            # spec repo — data grammar
    "nota-serde-core" # shared lexer + ser/de kernel
    "nota-serde"      # nota's public API
    "nexus"           # spec repo — messaging grammar
    "nexus-serde"     # nexus's public API
    "nexus-schema"    # record-kind vocabulary
    "sema"            # records DB (redb-backed)
    "nexusd"          # messenger daemon
    "nexus-cli"       # text client
    "rsc"             # records → Rust source projector
    "lojix-store"     # content-addressed filesystem (renamed from criome-store 2026-04-24)
    "lojix"           # TRANSITIONAL — Li's working deploy CLI (report 030)
    # --- CriomOS host (criome engine runs on criomos) ---
    "CriomOS"         # NixOS-based host OS for the sema ecosystem
    "horizon-rs"      # horizon projection library (lojix's deploy path links it)
    "CriomOS-emacs"   # emacs config as CriomOS module
    "CriomOS-home"    # home-manager config as CriomOS module
    # --- CANON-MISSING (repos don't exist yet; uncomment when scaffolded) ---
    # "criomed"       # sema's engine daemon
    # "signal"    # nexusd↔criomed contract
    # "lojix-msg"     # criomed↔lojixd contract (report 030 Phase B)
    # "lojixd"        # lojix daemon (report 030 Phase C)
  ];

  linkSiblingRepos = ''
    mkdir -p repos
    ${pkgs.lib.concatMapStringsSep "\n" (name: ''
      if [ -d "$HOME/git/${name}" ]; then
        ln -sfn "$HOME/git/${name}" "repos/${name}"
      else
        echo "warn: $HOME/git/${name} not found; skipping symlink" >&2
      fi
    '') linkedRepos}
  '';
in
pkgs.mkShell {
  packages = [
    inputs.mentci-tools.packages.${system}.beads
    inputs.mentci-tools.packages.${system}.dolt
  ];

  env = { };

  shellHook = ''
    ${linkSiblingRepos}
  '';
}
