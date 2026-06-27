# Bravyi BB Circuit BP+OSD Source Contract

## Upstream Pin

The comparison contract is pinned to
`sbravyi/BivariateBicycleCodes` at commit
`fa77e3333d3ec44c79d8f914dd24c040d1da471b`.

Pinned tree:
https://github.com/sbravyi/BivariateBicycleCodes/tree/fa77e3333d3ec44c79d8f914dd24c040d1da471b

## Result Row And Failure Unit

The upstream result row columns are `physical_error_rate`,
`num_syndrome_cycles`, `num_monte_carlo_trials`, and
`num_failed_trials`. The failure unit is one Monte Carlo trial, not one
cycle or one decoder call.

Source:
https://github.com/sbravyi/BivariateBicycleCodes/blob/fa77e3333d3ec44c79d8f914dd24c040d1da471b/README.md#L16-L21
(`README.md` lines 16-21).

## BP/OSD Parameters

The upstream Python replay uses min-sum BP with `max_iter=10000`,
`osd_method=osd_cs`, `osd_order=7`, and `ms_scaling_factor=0`.

Source:
https://github.com/sbravyi/BivariateBicycleCodes/blob/fa77e3333d3ec44c79d8f914dd24c040d1da471b/decoder_run.py#L67-L72
(`decoder_run.py` lines 67-72 and 329-349).

## Cycle Convention

The configured noisy-cycle field is `num_cycles`. The circuit construction
adds two noiseless tail cycles to close the measurement history.

Source:
https://github.com/sbravyi/BivariateBicycleCodes/blob/fa77e3333d3ec44c79d8f914dd24c040d1da471b/decoder_setup.py#L511-L618
(`decoder_setup.py` lines 511-618).

## Failure Predicate

Trials decode Z first. X is decoded only if Z succeeds. A failed trial is one
where Z fails, or where X fails after Z succeeds.

Source:
https://github.com/sbravyi/BivariateBicycleCodes/blob/fa77e3333d3ec44c79d8f914dd24c040d1da471b/decoder_run.py#L364-L415
(`decoder_run.py` lines 364-415).

## Source References

- `README.md` lines 16-21:
  https://github.com/sbravyi/BivariateBicycleCodes/blob/fa77e3333d3ec44c79d8f914dd24c040d1da471b/README.md#L16-L21
- `decoder_setup.py` lines 511-618:
  https://github.com/sbravyi/BivariateBicycleCodes/blob/fa77e3333d3ec44c79d8f914dd24c040d1da471b/decoder_setup.py#L511-L618
- `decoder_run.py` lines 67-72 and 329-349:
  https://github.com/sbravyi/BivariateBicycleCodes/blob/fa77e3333d3ec44c79d8f914dd24c040d1da471b/decoder_run.py#L67-L72
- `decoder_run.py` lines 364-415:
  https://github.com/sbravyi/BivariateBicycleCodes/blob/fa77e3333d3ec44c79d8f914dd24c040d1da471b/decoder_run.py#L364-L415
