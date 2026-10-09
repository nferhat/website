{
  lib,
  rustPlatform,
  makeWrapper,
  git,
  stdenv,
}:
rustPlatform.buildRustPackage {
  pname = "website";
  version = "0.1.0";

  src = lib.cleanSource ./.;
  cargoLock.lockFile = ./Cargo.lock;

  nativeBuildInputs = [makeWrapper];

  # NOTE: grammars get git-cloned and compiled with `cc` at runtime, so both
  # need to be in PATH, otherwise highlighting just dies
  postInstall = ''
    wrapProgram $out/bin/cli --prefix PATH : ${lib.makeBinPath [git stdenv.cc]}
  '';

  meta.mainProgram = "cli";
}
