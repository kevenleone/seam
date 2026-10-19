// Prevents an extra console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // The launcher in resources/seam passes `--cli`; Finder and `open` never do.
    if std::env::args().nth(1).as_deref() == Some("--cli") {
        seam_lib::cli::main()
    }
    seam_lib::run()
}
