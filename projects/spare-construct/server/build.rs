//! Rebuilds the server when a file of the page's core or tools is added or changed: `src/http.rs`
//! embeds `app/core` and `app/tools` as two trees (note A96), and a file added to a tree is not
//! otherwise seen by the compiler. A folder named here is scanned whole by Cargo.

fn main() {
    println!("cargo::rerun-if-changed=../app/core");
    println!("cargo::rerun-if-changed=../app/tools");
    println!("cargo::rerun-if-changed=build.rs");
}
