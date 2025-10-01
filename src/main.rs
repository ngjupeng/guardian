mod masm;

use masm::{run, run_prove, run_verify};

fn main() {
    // run();
    let (outputs, proof) = run_prove().unwrap();
    run_verify(outputs, proof).unwrap();
}
