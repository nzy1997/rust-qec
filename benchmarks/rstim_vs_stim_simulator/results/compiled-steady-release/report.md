# Split precompile, sample, and b8 benchmark

| variant | sample_count | precompile_elapsed_ns | median_call_sample_elapsed_ns | median_call_b8_elapsed_ns | median_worker_total_elapsed_ns |
| --- | ---: | ---: | ---: | ---: | ---: |
| rstim-precompiled | 7 | 2784625 | 4048500 | 232500 | 4281000 |
| stim-precompiled | 7 | 1061625 | 14713791 | 41667 | 14755458 |
| rstim-interpreted | 7 | 0 | 7204833 | 247292 | 7452125 |
| stim-direct | 7 | 0 | 15819709 | 50458 | 15870167 |
| rstim-precompiled-atom-loss | 7 | 2890458 | 3911500 | 217292 | 4128792 |

Measured records: 35
