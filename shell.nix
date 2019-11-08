with import <nixpkgs> {};
let
  nixpkgs-mozilla = fetchFromGitHub {
    owner = "mozilla";
    repo = "nixpkgs-mozilla";
    rev = "d46240e8755d91bc36c0c38621af72bf5c489e13";
    sha256 = "0icws1cbdscic8s8lx292chvh3fkkbjp571j89lmmha7vl2n71jg";
  };
in
  with import "${nixpkgs-mozilla.out}/rust-overlay.nix" pkgs pkgs;
  let
    rust-channel = (rustChannelOf { date="2019-11-03"; channel = "nightly"; });
    my-rust = rust-channel.rust.override { extensions = [ "rust-src" ]; };
    my-rust-src = rust-channel.rust-src;
  in
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
        postgresql_11
        capnproto
        # note: install diesel_cli with:
        # cargo install diesel_cli --no-default-features --features postgres
      ];

      shellHook = ''
        export RUST_SRC_PATH="${my-rust-src}/lib/rustlib/src/rust/src"
        export RUST_LOG=warn,astroplant_rs_api=trace
        export PATH="$HOME/.cargo/bin:$PATH"
      '';
    }
