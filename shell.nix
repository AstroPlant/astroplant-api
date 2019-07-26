let
  moz_overlay = import (builtins.fetchTarball https://github.com/mozilla/nixpkgs-mozilla/archive/master.tar.gz);
  nixpkgs = import <nixpkgs> { overlays = [ moz_overlay ]; };
  rust-channel = (nixpkgs.rustChannelOf { date="2019-07-15"; channel = "nightly"; });
  my-rust = rust-channel.rust.override { extensions = [ "rust-src" ]; };
  my-rust-src = rust-channel.rust-src;
in
  with import <nixpkgs> {};
  pkgs.mkShell {
    buildInputs = [
      my-rust
      my-rust-src
      rustracer
      pkgconfig
      cmake
      bashInteractive
      cacert
      openssl
    ];

    shellHook = ''
      export RUST_SRC_PATH="${my-rust-src}/lib/rustlib/src/rust/src"
      export RUST_LOG=warn,astroplant-api=trace
    '';
  }
