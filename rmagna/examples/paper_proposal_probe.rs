#[path = "../tests/support/paper_proposal_probe.rs"]
mod probe;

fn main() {
    println!("{}", probe::check());
}
