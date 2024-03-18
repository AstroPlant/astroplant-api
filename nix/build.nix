{ pkgs
, rustPlatform
, pkg-config
, capnproto
, openssl
, postgresql
, ...
}:
let
  astroplant-api = rustPlatform.buildRustPackage rec {
    pname = "astroplant-api";
    version = "1.0.0.alpha-1";

    src = ../.;
    cargoLock = { lockFile = ../Cargo.lock; };

    depsBuildBuild = [ capnproto ];
    nativeBuildInputs = [ pkg-config ];
    buildInputs = [
      openssl
      postgresql
    ];
  };
  astroplant-mqtt-ingest = rustPlatform.buildRustPackage rec {
    pname = "astroplant-mqtt-ingest";
    version = "1.0.0.alpha-1";

    src = ../.;
    buildAndTestSubdir = "astroplant-mqtt-ingest";
    cargoLock = { lockFile = ../Cargo.lock; };

    depsBuildBuild = [ capnproto ];
    nativeBuildInputs = [ pkg-config ];
    buildInputs = [
      openssl
      postgresql
    ];
  };
in
pkgs.buildEnv {
  name = "astroplant";
  paths = [
    astroplant-api
    astroplant-mqtt-ingest
  ];
}
