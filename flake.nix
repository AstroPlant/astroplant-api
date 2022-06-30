{
  description = "An API to interface with the AstroPlant back-end";
  inputs.flake-utils.url = "github:numtide/flake-utils";
  inputs.flake-compat = {
    url = "github:edolstra/flake-compat";
    flake = false;
  };
  inputs.naersk.url = "github:nix-community/naersk";
  outputs = { self, nixpkgs, flake-utils, naersk, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        naersk-lib = naersk.lib.${system};
      in
      {
        packages.astroplant = naersk-lib.buildPackage {
          pname = "astroplant";
          root = ./.;
          depsBuildBuild = with pkgs; [
            capnproto
          ];
          nativeBuildInputs = with pkgs; [
            pkgconfig
          ];
          buildInputs = with pkgs; [
            openssl
            postgresql
          ];
          doCheck = true;
        };
        defaultPackage = self.packages.${system}.astroplant;
        devShell = pkgs.mkShell {
          buildInputs = with pkgs; [
            cargo
            clippy
            rust-analyzer
            rustc
            rustfmt
            pkgconfig
            openssl
            capnproto
            postgresql
            diesel-cli
          ];
          shellHook = ''
            export RUST_SRC_PATH="${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
            export RUST_LOG="warn,astroplant_mqtt=trace,astroplant_api=trace";
          '';
        };
      });
}
