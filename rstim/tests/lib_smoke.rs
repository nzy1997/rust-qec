use rstim::version;

#[test]
fn version_is_nonempty() {
    assert!(!version().is_empty());
}
