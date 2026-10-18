//! Direct Core oracle for the executable Swift Share Bundle smoke.
#[path = "../tests/support/share.rs"]
mod support;
fn main() {
    println!("{}", support::oracle());
}
