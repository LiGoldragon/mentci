{
  description = "mentci — sema-ecosystem dev workspace";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs?ref=nixos-unstable";

    blueprint.url = "github:numtide/blueprint";
    blueprint.inputs.nixpkgs.follows = "nixpkgs";

    mentci-tools.url = "github:LiGoldragon/mentci-tools";
    mentci-tools.inputs.nixpkgs.follows = "nixpkgs";
    mentci-tools.inputs.blueprint.follows = "blueprint";

    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";

    crane.url = "github:ipetkov/crane";

    # ─── canonical sema-ecosystem crates ─────────────────────
    # Each crate's flake exposes `checks.default` which the
    # workspace-level `checks/default.nix` aggregates so that
    # `nix flake check` from mentci runs every crate's tests
    # in a single sandboxed pass.
    nota-derive = {
      url = "github:LiGoldragon/nota-derive";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.fenix.follows = "fenix";
      inputs.crane.follows = "crane";
    };
    nota-codec = {
      url = "github:LiGoldragon/nota-codec";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.fenix.follows = "fenix";
      inputs.crane.follows = "crane";
    };
    signal = {
      url = "github:LiGoldragon/signal";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.fenix.follows = "fenix";
      inputs.crane.follows = "crane";
    };
    sema = {
      url = "github:LiGoldragon/sema";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.fenix.follows = "fenix";
      inputs.crane.follows = "crane";
    };
    criome = {
      url = "github:LiGoldragon/criome";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.fenix.follows = "fenix";
      inputs.crane.follows = "crane";
    };
    nexus = {
      url = "github:LiGoldragon/nexus";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.fenix.follows = "fenix";
      inputs.crane.follows = "crane";
    };
    nexus-cli = {
      url = "github:LiGoldragon/nexus-cli";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.fenix.follows = "fenix";
      inputs.crane.follows = "crane";
    };
    mentci-lib = {
      url = "github:LiGoldragon/mentci-lib";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.fenix.follows = "fenix";
      inputs.crane.follows = "crane";
    };
    mentci-egui = {
      url = "github:LiGoldragon/mentci-egui";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.fenix.follows = "fenix";
      inputs.crane.follows = "crane";
    };
  };

  outputs = inputs: inputs.blueprint { inherit inputs; };
}
