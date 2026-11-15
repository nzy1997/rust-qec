#[path = "support/paper_proposal_probe.rs"]
mod probe;

#[test]
fn finite_paper_proposals_reweight_supplied_actions_and_hide_true_outcomes_from_decoders() {
    let report = probe::check();
    assert_eq!(report["status"], "PASS");
    assert_eq!(report["decoder_calls"], 768);
}
