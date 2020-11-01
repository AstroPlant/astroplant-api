{
  description = "An API to interface with the AstroPlant back-end";
  inputs.flake-utils.url = "github:numtide/flake-utils";
  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let pkgs = nixpkgs.legacyPackages.${system};
      in {
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
