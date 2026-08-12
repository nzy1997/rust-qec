# Split precompile, sample, and b8 benchmark

| variant | sample_count | precompile_elapsed_ns | median_call_sample_elapsed_ns | median_call_b8_elapsed_ns | median_worker_total_elapsed_ns |
| --- | ---: | ---: | ---: | ---: | ---: |
| rstim-precompiled | 7 | 6110250 | 9543167 | 14671917 | 24215084 |
| stim-precompiled | 7 | 1259750 | 27372416 | 73417 | 27445833 |
| rstim-interpreted | 7 | 0 | 16483292 | 8445667 | 24928959 |
| stim-direct | 7 | 0 | 28254458 | 106125 | 28360583 |
| rstim-interpreted-atom-loss | 7 | 0 | 1394900375 | 8356166 | 1403256541 |

Measured records: 35
