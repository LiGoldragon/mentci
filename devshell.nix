{ pkgs, ... }:
let
  # Sibling repos under ~/git/ exposed as symlinks in ./repos/
  # at devshell entry. Multi-root workspace (workspace.code-workspace)
  # gives editors the same view via additional folders.
  linkedRepos = [
    "lore"
    # sema-ecosystem
    "criome"
    "nota"
    "nota-codec"
    "nota-derive"
    "nexus"
    "signal"
    "signal-forge"
    "sema"
    "nexus-cli"
    "signal-derive"
    "prism"
    "arca"
    "lojix-cli"
    "lojix-cli-v2"
    "forge"
    # mentci interaction surface
    "mentci-lib"
    "mentci-egui"
    # CriomOS cluster
    "CriomOS"
    "horizon-rs"
    "CriomOS-emacs"
    "CriomOS-home"
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
    pkgs.beads
    pkgs.dolt
  ];

  env = { };

  shellHook = ''
    ${linkSiblingRepos}
  '';
}
