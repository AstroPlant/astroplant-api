{
  description = "Services providing the AstroPlant API";
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";
    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
  };
  outputs = { self, nixpkgs, flake-utils, ... }:
    {
      overlays.default = final: prev: {
        astroplant-api = final.callPackage ./nix/build.nix { };
      };
    }
    //
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        packages.astroplant-api = pkgs.callPackage ./nix/build.nix { };
        packages.default = self.packages.${system}.astroplant-api;
        devShells.default = pkgs.mkShell {
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
