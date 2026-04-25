{ pkgs, inputs, system }:
let
  # Sibling repos under ~/git/ to expose as symlinks in ./repos/.
  # This list IS the canonical workspace manifest for agents.
  # Entries align with docs/workspace-manifest.md.
  # Direnv / nix develop entry creates the links.
  linkedRepos = [
    "tools-documentation"
    # --- sema-ecosystem CANON ---
    "criome"          # spec + criome daemon (sema's engine; six-step validator pipeline)
    "nota"            # spec repo — data grammar
    "nota-serde-core" # shared lexer + ser/de kernel
    "nota-serde"      # nota's public API
    "nexus"           # the nexus language — spec + translator daemon (renamed from nexusd 2026-04-25; absorbed former nexus spec repo into spec/)
    "nexus-serde"     # nexus's public API
    # nexus-schema SHELVED 2026-04-25 — types absorbed into signal
    "signal"          # nexus↔criome messaging schema (rkyv)
    "sema"            # records DB (redb-backed)
    "nexus-cli"       # text client
    "rsc"             # records → Rust source projector
    "lojix-store"     # content-addressed filesystem (renamed from criome-store 2026-04-24)
    "lojix-cli"       # TRANSITIONAL — Li's working deploy CLI (renamed from lojix 2026-04-25)
    "lojix"           # the lojix daemon (forge + store + deploy actors)
    "lojix-schema"    # criome↔lojix contract types (verbs + spec/outcome shapes)
    # --- CriomOS host (criome engine runs on criomos) ---
    "CriomOS"         # NixOS-based host OS for the sema ecosystem
    "horizon-rs"      # horizon projection library (lojix-cli's deploy path links it)
    "CriomOS-emacs"   # emacs config as CriomOS module
    "CriomOS-home"    # home-manager config as CriomOS module
    # --- CANON-MISSING (none currently; all 2026-04-25 scaffolds landed) ---
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
