with import <nixpkgs> {};
let
  nixpkgs-mozilla = fetchFromGitHub {
    owner = "mozilla";
    repo = "nixpkgs-mozilla";
    rev = "e912ed483e980dfb4666ae0ed17845c4220e5e7c";
    sha256 = "08fvzb8w80bkkabc1iyhzd15f4sm7ra10jn32kfch5klgl0gj3j3";
  };
in
  with import "${nixpkgs-mozilla.out}/rust-overlay.nix" pkgs pkgs;
  let
    rust-channel = (rustChannelOf { date="2020-04-16"; channel = "nightly"; });
    my-rust = rust-channel.rust.override { extensions = [ "rust-src" ]; };
    my-rust-src = rust-channel.rust-src;
  in
    pkgs.mkShell {
      buildInputs = [
        my-rust
        my-rust-src
        rustfmt
        rustracer
        pkgconfig
        cmake
        bashInteractive
        cacert
        openssl
        postgresql_11
        capnproto
        # note: install diesel_cli with:
        # cargo install diesel_cli --no-default-features --features postgres
      ];

      shellHook = ''
        export RUST_SRC_PATH="${my-rust-src}/lib/rustlib/src/rust/src"
        export RUST_LOG=warn,astroplant_api=trace,astroplant_websocket=trace,astroplant_mqtt=debug
        export PATH="$HOME/.cargo/bin:$PATH"
      '';
    }
