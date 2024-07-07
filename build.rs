#[cfg(target_os = "macos")]
fn main() {
    println!(
        "cargo:rustc-link-arg=-Wl,-rpath,/Library/Developer/CommandLineTools/Library/Frameworks"
    );
}


#[cfg(not(target_os = "macos"))]
fn main() {
}
