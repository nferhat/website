{ nixpkgs ? import <nixpkgs>, ... }:

let
  fenixMonthly = builtins.fetchTarball "https://github.com/nix-community/fenix/archive/monthly.tar.gz";
  pkgs = nixpkgs { overlays = [(import "${fenixMonthly}/overlay.nix")]; };
  lib = pkgs.lib;
in pkgs.mkShell rec {
  packages = with pkgs; [
    pkg-config
    cmake
    (fenix.complete.withComponents [
      "cargo"
      "clippy"
      "rust-src"
      "rustc"
      "rustfmt"
      "rust-analyzer"
      "rustc-codegen-cranelift-preview"
    ])
  ];

  buildInputs = with pkgs; [shopify-cli];
  LD_LIBRARY_PATH = lib.makeLibraryPath buildInputs;
}
