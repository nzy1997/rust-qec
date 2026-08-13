# Split precompile, sample, and b8 benchmark

| variant | sample_count | precompile_elapsed_ns | median_call_sample_elapsed_ns | median_call_b8_elapsed_ns | median_worker_total_elapsed_ns |
| --- | ---: | ---: | ---: | ---: | ---: |
| rstim-precompiled | 7 | 2924750 | 4207833 | 272667 | 4480500 |
| stim-precompiled | 7 | 1114916 | 16092542 | 35042 | 16127584 |
| rstim-interpreted | 7 | 0 | 7329792 | 198750 | 7528542 |
| stim-direct | 7 | 0 | 16774916 | 40416 | 16815332 |
| rstim-precompiled-atom-loss | 7 | 2978625 | 4002167 | 263541 | 4265708 |

Measured records: 35
