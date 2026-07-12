# Fair CLI sampling benchmark

Case: stim_surface_d11_r100
Measured records: 14

| variant | sample_count | median_elapsed_ns | min_elapsed_ns | max_elapsed_ns |
| --- | ---: | ---: | ---: | ---: |
| stim-cli-b8 | 7 | 26704292 | 26308292 | 26863625 |
| rstim-cli-b8 | 7 | 18073833 | 17891500 | 18228916 |

## Baseline comparison

Baseline rstim/Stim ratio: 3.576x
Candidate rstim/Stim ratio: 0.677x
Change from baseline: -2.899x
Reference strategy: direct_inverse_repeat_folded

