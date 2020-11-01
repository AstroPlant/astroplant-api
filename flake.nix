{
  description = "An API to interface with the AstroPlant back-end";
  inputs.flake-utils.url = "github:numtide/flake-utils";
  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let pkgs = nixpkgs.legacyPackages.${system};
      in {
        packages.astroplant-api = pkgs.rustPlatform.buildRustPackage rec {
          pname = "astroplant-api";
          version = "1.0.0-alpha";
          src = ./.;
          cargoSha256 = "sha256-keRUIlicOsmudky2HiOyMyFoEtrseRHhrt2rhAozguc=";
          nativeBuildInputs = with pkgs; [ pkgconfig capnproto ];
          buildInputs = with pkgs; [ openssl postgresql ];
        };
        defaultPackage = self.packages.${system}.astroplant-api;
        devShell = pkgs.mkShell {
          buildInputs = with pkgs; [
            cargo
            rls
            rustfmt
            pkgconfig
            openssl
            capnproto
            postgresql
            (diesel-cli.override {
              postgresqlSupport = true;
              sqliteSupport = false;
              mysqlSupport = false;
            })
          ];
        };
      });
}
