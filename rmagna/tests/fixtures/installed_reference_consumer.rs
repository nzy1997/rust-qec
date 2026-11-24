// SPDX-License-Identifier: MPL-2.0
// Installed public-API adaptation of the pinned conditional-noise reuse method.
use rmagna::{
    field::{ExactField, FieldArithmetic},
    file_api::{FileEnumerationOptions, FileLoadOptions, FileTask, FileTaskOptions},
    instrument::QueryPreparation,
    noise::NoiseSampling,
    runtime::{ControllerRegistry, HistorySampler},
    task::{RunStatus, TaskRunOptions},
    unitary::{CompilerStrategy, QueryWorkStatistics},
};
use std::{collections::BTreeMap, path::Path};

fn factories(s: QueryWorkStatistics) -> [u64; 10] {
    assert!(!s.saturated);
    assert!(s.live_affine_geometries.is_some());
    [
        s.affine_closures_prepared,
        s.affine_geometry_preparations,
        s.phase_maps_prepared,
        s.phase_resources_prepared,
        s.support_projections_prepared,
        s.support_boundary_maps_prepared,
        s.support_contractions_prepared,
        s.support_phase_maps_prepared,
        s.support_phase_resources_prepared,
        s.support_parents_prepared,
    ]
}

fn load(options: FileTaskOptions) -> FileTask {
    FileTask::load(
        Path::new("conditional.magna"),
        Some(Path::new("conditional.json")),
        &ControllerRegistry::default(),
        NoiseSampling::Physical,
        options,
        &FileLoadOptions::default(),
        BTreeMap::new(),
    )
    .unwrap()
}

fn main() {
    let run = TaskRunOptions {
        shots: 64,
        seed: 3,
        ..Default::default()
    };
    // The public file API defaults to cold queries. Observe this honestly;
    // it is not the explicitly prepared interval qualified below.
    let cold = load(FileTaskOptions::default()).compile().unwrap();
    let before = cold.query_work_statistics();
    let preparation = cold.prepare_sampling(&Default::default()).unwrap();
    preparation.raw_result().unwrap();
    assert!(preparation.is_complete());
    assert!(!preparation.stage().unwrap().all_families_retained());
    assert!(preparation.stage().unwrap().counts.cold > 0);
    let result = cold.run(&run).unwrap();
    assert_eq!(result.raw_result().completed, 64);
    println!(
        "default-cold before={before:?}; after64={:?}; preparation={:?}",
        cold.query_work_statistics(),
        preparation.stage()
    );

    for sampler in [HistorySampler::ExactMarginal, HistorySampler::AffineSupport] {
        for compiler in [
            CompilerStrategy::ParityCopies,
            CompilerStrategy::SharedCommutingLayer,
        ] {
            let mut options = FileTaskOptions::default();
            options.evaluation.runtime.history_sampler = sampler;
            options.evaluation.runtime.compile.compiler_strategy = compiler;
            options.evaluation.runtime.compile.probability_preparation =
                QueryPreparation::Native(Default::default());
            options.evaluation.runtime.compile.density_preparation =
                QueryPreparation::Native(Default::default());
            options.evaluation.runtime.compile.moment_preparation =
                QueryPreparation::Native(Default::default());
            let compiled = load(options).compile().unwrap();
            let cold_work = compiled.query_work_statistics();
            let cold_storage = compiled.reference_storage_statistics().unwrap();
            let cold_assignments = compiled.source_insertion_assignments().unwrap();
            assert_eq!(
                cold_work,
                compiled.query_work_statistics(),
                "inspection is read-only"
            );
            assert_eq!(
                cold_storage,
                compiled.reference_storage_statistics().unwrap()
            );
            assert_eq!(
                cold_assignments,
                compiled.source_insertion_assignments().unwrap()
            );
            let preparation = compiled.prepare_sampling(&Default::default()).unwrap();
            preparation.raw_result().unwrap();
            assert!(preparation.is_complete());
            assert!(preparation.stage().unwrap().all_families_retained());
            assert_eq!(
                preparation.stage().unwrap().execution_route,
                "causal_fixed_template"
            );
            let prepared = compiled.query_work_statistics();
            let storage = compiled.reference_storage_statistics().unwrap();
            let assignments = compiled.source_insertion_assignments().unwrap();
            let source = prepared.source.unwrap();
            let target = prepared.target.unwrap();
            assert_ne!(
                factories(cold_work.source.unwrap()),
                factories(source),
                "preparation must observe real construction"
            );
            assert!(source.affine_closures_prepared > 0);
            assert!(source.phase_maps_prepared > 0);
            assert!(source.phase_resources_prepared > 0);
            // Roles are one retained source insertion program and one captured
            // ideal target, with query families attached to the source. Native's
            // three private parents are not a Rust object-count invariant.
            for owner in [storage.source.unwrap(), storage.target.unwrap()] {
                assert!(owner.physical_width > 0);
                assert!(owner.unique_reference_boundaries <= 2);
                assert!(owner.live_geometries <= owner.geometry_weak_slots);
            }
            let report = compiled.run(&run).unwrap();
            let result = report.raw_result();
            assert_eq!(result.status, RunStatus::Complete);
            assert_eq!(
                (result.completed, result.failed, result.not_attempted),
                (64, 0, 0)
            );
            assert_eq!(result.completed_network_compilations, 0);
            assert_eq!(result.execution_costs.compilation.attempted, 0);
            assert!(result.completed_history_groups > 0);
            assert_eq!(
                result.statistics.as_ref().unwrap().accepted,
                ExactField::one()
            );
            let after = compiled.query_work_statistics();
            let after_source = after.source.unwrap();
            let after_storage = compiled.reference_storage_statistics().unwrap();
            let after_assignments = compiled.source_insertion_assignments().unwrap();
            assert!(after_assignments > assignments);
            println!(
                "case {sampler:?}/{compiler:?}: cold work={cold_work:?}; storage={cold_storage:?}; assignments={cold_assignments}; prepared work={prepared:?}; storage={storage:?}; assignments={assignments}; after64 work={after:?}; storage={:?}; assignments={after_assignments}; source_compilations={}; history_groups={}",
                compiled.reference_storage_statistics().unwrap(),
                result.completed_network_compilations,
                result.completed_history_groups
            );
            // Enumeration owns additional query layouts: snapshot the draw
            // interval first and never conceal this later work as sampling.
            let mut enumeration = FileEnumerationOptions::default();
            enumeration.enumeration.noise.max_weight = 2;
            let exact = compiled.enumerate(&enumeration).unwrap();
            let masses = &exact.raw_result().unwrap().masses;
            assert_eq!(masses.accepted, ExactField::one());
            assert_eq!(
                masses.error,
                FieldArithmetic::default()
                    .from_rational("1/12".parse().unwrap())
                    .unwrap()
            );
            println!("exact acceptance=1 error=1/12 before reuse assertions");
            // These snapshots precede enumeration. A numerically equivalent
            // reconstruction must still fail the reuse contract.
            assert_eq!(
                factories(source),
                factories(after_source),
                "hot draw reconstructed a query factory"
            );
            assert_eq!(factories(target), factories(after.target.unwrap()));
            assert_eq!(source.cold_evaluations, after_source.cold_evaluations);
            assert!(after_source.fixed_evaluations > source.fixed_evaluations);
            assert!(after_source.closure_evaluations > source.closure_evaluations);
            assert_eq!(
                storage, after_storage,
                "retained structure grew during draws"
            );
            println!(
                "PASS {sampler:?}/{compiler:?}: same-owner 64 draws; acceptance=1 error=1/12; hot factories=0 source recompilations=0"
            );
        }
    }
}
