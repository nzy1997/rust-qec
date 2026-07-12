# Packed Reference-Build Evidence

| variant | count | min_elapsed_ns | median_elapsed_ns | max_elapsed_ns | backend | parse_count | final_reference_build_count | byte_sha256 |
| --- | ---: | ---: | ---: | ---: | --- | ---: | ---: | --- |
| stim-reference-b8 | 7 | 916000 | 940375 | 946334 | stim_reference | 1 | 9 | d95f3eacd05c1ca0d3a90e4a48e1d68b7ef5f2d817da11121ba4b77454b24d3d |
| rstim-canonical-reference-b8 | 7 | 69634958 | 70486208 | 71083208 | canonical_roundtrip | 1 | 9 | d95f3eacd05c1ca0d3a90e4a48e1d68b7ef5f2d817da11121ba4b77454b24d3d |
| rstim-direct-repeat-reference-b8 | 7 | 3252167 | 3293042 | 3323875 | direct_inverse_repeat_folded | 1 | 9 | d95f3eacd05c1ca0d3a90e4a48e1d68b7ef5f2d817da11121ba4b77454b24d3d |

direct_speedup=21.404588
