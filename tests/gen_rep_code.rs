use rstim::r#gen::repetition_code_memory;

#[test]
fn rep_code_still_works_via_gen_module() {
    let instrs = repetition_code_memory(3, 2, 0.0);
    assert!(!instrs.is_empty());
}
