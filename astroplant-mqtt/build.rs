extern crate capnpc;

use capnpc::CompilerCommand;

fn main() {
    CompilerCommand::new()
        .file("proto/astroplant.capnp")
        .run()
        .expect("compiling schema");
}
