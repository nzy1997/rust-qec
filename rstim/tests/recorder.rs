use rstim::recorder::Recorder;

#[test]
fn recorder_rec_offsets_work() {
    let mut r = Recorder::default();
    r.push(false);
    r.push(true);
    assert_eq!(r.rec(-1).unwrap(), true);
    assert_eq!(r.rec(-2).unwrap(), false);
}
