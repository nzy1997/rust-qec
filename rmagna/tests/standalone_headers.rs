//! Compile-only witness: the upstream header checks assert type availability.
#[path = "fixtures/standalone_consumer.rs"]
mod consumer;

#[test]
fn four_public_facades_type_check_without_python() {
    consumer::main();
}
