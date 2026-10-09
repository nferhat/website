{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix/monthly";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    nixpkgs,
    fenix,
    ...
  }: let
    systems = ["x86_64-linux" "aarch64-linux"];
    forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system} system);
  in {
    packages = forAllSystems (pkgs: _: {
      default = pkgs.callPackage ./default.nix {};
    });

    devShells = forAllSystems (pkgs: system: {
      default = pkgs.mkShell {
        packages = with pkgs; [
          pkg-config
          cmake
          shopify-cli
          (fenix.packages.${system}.complete.withComponents [
            "cargo"
            "clippy"
            "rust-src"
            "rustc"
            "rustfmt"
            "rust-analyzer"
            "rustc-codegen-cranelift-preview"
          ])
        ];
      };
    });
  };
}
