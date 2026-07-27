# Perturbed HGP Source-Grounding Decision Record

contract_version: 1
family_id = "perturbed_hgp"
selection_status = "explicitly_unsupported"
disposition_decision = "remain_deferred_unsupported"

## Searched Terminology

The source-grounding review searched the following terminology in relevant
literature, project context, and public code indexes:

- "perturbed HGP"
- "perturbed_hgp"
- "perturbed hypergraph product"
- "perturbation hypergraph product quantum LDPC"
- "cross swap"
- "H_X H_Z^T local repair"
- "GitHub code search"

## Source Log

- Error Correction Zoo, Hypergraph product (HGP) code,
  https://errorcorrectionzoo.org/c/hypergraph_product
- Tillich and Zemor, *Quantum LDPC codes with positive rate and minimum
  distance proportional to n^{1/2}*, arXiv:0903.0566.
- Okada and Kasai, *Random Construction of Quantum LDPC Codes*,
  arXiv:2511.04634.
- Freire, Delfosse, and Leverrier, *Optimizing hypergraph product codes with
  random walks, simulated annealing and reinforcement learning*,
  arXiv:2501.09622.
- Tan and Stambler, *Effective Distance of Higher Dimensional HGPs and
  Weight-Reduced Quantum LDPC Codes*, arXiv:2409.02193.
- Kasai, *Breaking the Orthogonality Barrier in Quantum LDPC Codes*,
  arXiv:2601.08824.
- GitHub code search results for "perturbed_hgp", "perturbed HGP", and
  "cross swap" with "H_X" and "H_Z".

The review did not identify a source that both names a construction
"Perturbed HGP" and specifies a unique, reproducible HGP-specific
perturbation rule suitable for this repository.

## Candidate Definitions And Dispositions

| Candidate | Disposition | Rationale |
| --- | --- | --- |
| Standard hypergraph product | Rejected | already implemented by #556; not perturbed |
| Okada-Kasai cross-swap repair | Rejected | generic CSS pair perturbation, not uniquely HGP-specific and not named perturbed HGP |
| HGP optimization by random walks | Rejected | optimizer/search over HGP instances, not a versioned perturbation constructor |
| weight-reduced HGP | Rejected | check-weight transformation/syndrome-extraction analysis, not a two-input perturbed-HGP constructor |
| active-orthogonality APM-LDPC | Rejected | APM-LDPC construction, not an HGP construction |

## Disposition Decision

No construction is selected. The name `perturbed_hgp` has no unique primary
definition in the reviewed sources. Choosing one of the nearby constructions
would silently define a new family rather than implement an established one.
The family therefore remains explicitly unsupported and deferred until a
maintainer identifies and approves a unique primary definition.

## No Selected Construction

selected_primary_source = none
perturbation_rule = none
positive_fixture = none
negative_fixture = none
orthogonality_preservation_rule = none
pure_rust_input_schema = none

## Would-Be Selected Contract Requirements

A future selected construction must provide all of the following before a
runtime interface is proposed:

- versioned pure-Rust input schema
- orthogonality-preservation rule
- one exact positive fixture
- one deliberately nonorthogonal negative fixture
- resource limits

The selected source must also make the relationship to a standard HGP input
and every allowed perturbation operation unambiguous and reproducible.

## Provenance And License Compatibility

This decision record contains a bibliographic and behavioral summary, not
copied implementation text. Any future implementation must record the source
provenance of its algorithm and fixtures. Repository contributions must remain
compatible with Apache-2.0. Reference material, including Error Correction Zoo
content, may be subject to Creative Commons terms and must be used in a manner
compatible with those terms; source licenses must be checked before importing
code, tables, or fixture data.

## Follow-Up Scope

No implementation issue is filed. Follow-up begins only after maintainers
select a unique primary source and convert the would-be requirements into a
versioned runtime contract with reviewed fixtures and limits.

## Deferred Runtime Status

`perturbed_hgp` is documentation-only and is not a callable CSS construction.
No callable runtime stub is provided.
