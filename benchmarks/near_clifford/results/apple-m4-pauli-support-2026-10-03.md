# Pauli reconstruction support controls

Baseline `68b907dc9f6c3e938a98a07624592b0865ed6852`; candidate `2538142f96c9009795aa1f1348ba45f3a16b4fc0`.
Apple M4; three alternating process pairs, three repetitions. Paired ranges are not confidence intervals.

1024 direct Y-probability queries per repetition after 32 warmup queries; preparation/physics/counters untimed.

| Frame | Width | Selected rows | Entries | Baseline ms | Candidate ms | Speedup | Paired range |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| star | 63 | 64 | 4032 | 1.5384 | 1.0892 | 1.412× | 1.404–1.567× |
| star | 64 | 65 | 4160 | 1.1600 | 0.7617 | 1.523× | 1.516–1.565× |
| star | 65 | 66 | 4290 | 1.2828 | 1.2105 | 1.060× | 1.042–1.504× |
| star | 127 | 128 | 16256 | 4.3936 | 2.9400 | 1.494× | 1.438–1.521× |
| star | 128 | 129 | 16512 | 4.0847 | 1.6750 | 2.439× | 2.392–2.564× |
| star | 129 | 130 | 16770 | 4.0974 | 1.7343 | 2.363× | 2.362–2.374× |
| star | 193 | 194 | 37442 | 8.9040 | 2.9099 | 3.060× | 2.937–3.076× |
| chain | 63 | 3 | 189 | 0.3617 | 0.3373 | 1.072× | 1.069–1.132× |
| chain | 64 | 3 | 192 | 0.3457 | 0.3125 | 1.106× | 1.081–1.112× |
| chain | 65 | 3 | 195 | 0.4231 | 0.3919 | 1.080× | 1.065–1.129× |
| chain | 127 | 3 | 381 | 0.7252 | 0.6887 | 1.053× | 1.037–1.078× |
| chain | 128 | 3 | 384 | 0.7189 | 0.6583 | 1.092× | 1.069–1.105× |
| chain | 129 | 3 | 387 | 0.7125 | 0.6650 | 1.071× | 1.071–1.114× |
| chain | 193 | 3 | 579 | 1.1350 | 1.0072 | 1.127× | 1.111–1.132× |
