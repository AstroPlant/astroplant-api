{ rustPlatform
, pkgconfig
, capnproto
, openssl
, postgresql
, ...
}:
rustPlatform.buildRustPackage rec {
  pname = "astroplant-api";
  version = "1.0.0.alpha-1";

  src = ../.;
  cargoLock = { lockFile = ../Cargo.lock; };

  depsBuildBuild = [ capnproto ];
  nativeBuildInputs = [ pkgconfig ];
  buildInputs = [
    openssl
    postgresql
  ];
}
