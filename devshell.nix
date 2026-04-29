{ pkgs, inputs, system }:
let
  # Sibling repos under ~/git/ exposed as symlinks in ./repos/
  # at devshell entry. Multi-root workspace (mentci.code-workspace)
  # gives editors the same view via additional folders.
  linkedRepos = [
    "tools-documentation"
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
    "prism"
    "arca"
    "lojix-cli"
    "forge"
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
    inputs.mentci-tools.packages.${system}.beads
    inputs.mentci-tools.packages.${system}.dolt
  ];

  env = { };

  shellHook = ''
    ${linkSiblingRepos}
  '';
}
