//! Bounded spelling suggestions shared by file and command-line validation.

/// Return a unique nearest ASCII name within two edits. Exact matches,
/// ambiguous candidates and names longer than 80 bytes receive no suggestion.
/// Candidate enumeration is caller-owned; each edit-distance workspace is
/// bounded independently of an untrusted input's length.
pub fn closest_name<'a>(
    name: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    if !name.is_ascii() || name.len() > 80 {
        return None;
    }
    let mut best = None;
    let mut score = 3;
    let mut ambiguous = false;
    for candidate in candidates {
        if !candidate.is_ascii() || candidate.len() > 80 || name.len().abs_diff(candidate.len()) > 2
        {
            continue;
        }
        let mut row = (0..=candidate.len()).collect::<Vec<_>>();
        for (i, a) in name.bytes().enumerate() {
            let mut diagonal = row[0];
            row[0] = i + 1;
            for (j, b) in candidate.bytes().enumerate() {
                let old = row[j + 1];
                row[j + 1] = (diagonal + usize::from(a != b))
                    .min(row[j] + 1)
                    .min(old + 1);
                diagonal = old;
            }
        }
        let distance = row[candidate.len()];
        if distance == 0 {
            return None;
        }
        if distance < score {
            score = distance;
            best = Some(candidate);
            ambiguous = false;
        } else if distance == score && best != Some(candidate) {
            ambiguous = true;
        }
    }
    if ambiguous { None } else { best }
}
