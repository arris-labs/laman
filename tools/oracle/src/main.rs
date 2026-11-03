//! The oracle: other Rust constraint solvers, run out of process on laman's
//! C1 fixtures. Step 1 is the skeleton: `oracle hello <solver>` solves a tiny
//! sketch through each solver's API and prints what that API exposes, to
//! back the expressibility matrix in `README.md`.

mod hello;

fn main() {
    let solver = std::env::args().nth(2).unwrap_or_default();
    match (std::env::args().nth(1).as_deref(), solver.as_str()) {
        (Some("hello"), "sketch-solver") => hello::sketch_solver(),
        (Some("hello"), "acs-solver") => hello::acs_solver(),
        (Some("hello"), "ezpz") => hello::ezpz(),
        _ => {
            eprintln!("usage: oracle hello <sketch-solver|acs-solver|ezpz>");
            std::process::exit(2);
        }
    }
}
