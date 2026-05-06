// Tell cargo to rebuild if any file in the migrations directory changes.
// Without this, `sqlx::migrate!()` would silently embed stale SQL because
// the macro reads files at compile time but the macro itself sits in
// already-compiled code that cargo otherwise won't reconsider.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
