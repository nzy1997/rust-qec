#![no_main]

use libfuzzer_sys::fuzz_target;
use rstim::interactive_shot::{EditableShot, ExpansionLimits, NoiseOutcome};

fuzz_target!(|data: &[u8]| {
    let mut shot = EditableShot::open(
        "REPEAT 3 {\n DEPOLARIZE1(0.2) 0\n M 0\n DETECTOR rec[-1]\n R 0\n}\n",
        ExpansionLimits::default(),
        11,
    )
    .unwrap();
    let ids = shot
        .session()
        .catalog()
        .events()
        .iter()
        .map(|event| event.id.clone())
        .collect::<Vec<_>>();
    for (step, byte) in data.iter().copied().take(512).enumerate() {
        let id = &ids[step % ids.len()];
        match byte % 9 {
            0 => shot.set_noise(id, NoiseOutcome::Identity).unwrap(),
            1 => shot.set_noise(id, NoiseOutcome::X).unwrap(),
            2 => shot.set_noise(id, NoiseOutcome::Y).unwrap(),
            3 => shot.set_noise(id, NoiseOutcome::Z).unwrap(),
            4 => shot.restore_noise(id).unwrap(),
            5 => {
                let _ = shot.undo().unwrap();
            }
            6 => {
                let _ = shot.redo().unwrap();
            }
            7 => shot.sample(u64::from(byte) << 32 | step as u64).unwrap(),
            _ => shot.clear(u64::from(byte) << 32 | step as u64).unwrap(),
        }
        serde_json::to_vec(&shot.summary()).unwrap();
    }
});
