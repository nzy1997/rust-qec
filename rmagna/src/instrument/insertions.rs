//! Immutable bounded noise/decision insertion catalogs retained across branches.
use super::*;
use crate::unitary::{InsertionPreparationOptions, PreparedInsertions};

#[derive(Clone, Copy, Debug, Default)]
/// Select direct propagation or a bounded immutable fault/decision catalog.
pub enum InsertionPreparation {
    #[default]
    Direct,
    Prepared(InsertionPreparationOptions),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Retained catalog size and conservative table-preparation footprint estimate.
pub struct PreparedInsertionStatistics {
    pub catalog_entries: usize,
    pub estimated_peak_bytes: u128,
}
pub(super) struct InsertionCatalog {
    pub(super) plan: PreparedInsertions,
    pub(super) decisions: BTreeMap<(usize, usize), usize>,
}
pub(super) fn prepare(
    unitary: &CompiledUnitary,
    sites: &mut [NoiseSite],
    decisions: &BTreeMap<(usize, usize), usize>,
    options: InsertionPreparationOptions,
) -> Result<InsertionCatalog, InstrumentError> {
    let count = sites.iter().try_fold(decisions.len(), |n, s| {
        n.checked_add(s.alternatives.len())
            .ok_or(UnitaryError::DimensionOverflow)
    })?;
    // Reject before constructing the temporary input catalog as well as before
    // the unitary primitive allocates its suffix and retained-entry tables.
    unitary.insertion_preparation_requirement(count, options)?;
    let mut catalog = Vec::with_capacity(count);
    for site in sites {
        for alternative in Arc::make_mut(&mut site.alternatives) {
            alternative.catalog_index = Some(catalog.len());
            catalog.push(PauliInsertion {
                before_gate: site.before_gate,
                word: alternative.word.clone(),
            });
        }
    }
    let mut indices = BTreeMap::new();
    for (&key, &wire) in decisions {
        indices.insert(key, catalog.len());
        catalog.push(PauliInsertion {
            before_gate: 0,
            word: PauliWord::new(
                BitVector::from_indices(unitary.width(), [wire])?,
                BitVector::zeros(unitary.width())?,
                0,
            )?,
        });
    }
    Ok(InsertionCatalog {
        plan: unitary.prepare_insertions(&catalog, options)?,
        decisions: indices,
    })
}
