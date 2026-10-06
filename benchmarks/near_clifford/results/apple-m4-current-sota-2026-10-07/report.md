# Exact near-Clifford raw-record comparison

Host: macOS-27.0.1-arm64-arm-64bit-Mach-O. Single CPU thread; full raw records, without postselection.
Native competitor circuits; rstim uses disclosed exact MPP/readout lowering.
Warm timings exclude output destruction; each raw observation accumulates at least 50 ms of public calls.
Tuning and correctness are outside all measured windows. Ranges describe process medians, not confidence intervals.

| Fixture | Shots | rstim ms | Fastest peer ms | rstim speedup | Paired speedup range | Fastest peer |
| --- | ---: | ---: | ---: | ---: | --- | --- |
| terminal | 1 | 0.000055 | 0.001577 | 28.4409× | 28.0343–30.2082× | symft |
| terminal | 64 | 0.002772 | 0.005341 | 1.9266× | 1.8413–2.4987× | clifft-scheduled |
| terminal | 1024 | 0.043327 | 0.051781 | 1.1951× | 1.0987–1.2075× | clifft |
| brick16 | 1 | 0.007283 | 0.001953 | 0.2682× | 0.2651–0.2730× | clifft-scheduled |
| brick16 | 64 | 0.464100 | 0.009081 | 0.0196× | 0.0196–0.0209× | clifft-scheduled |
| brick16 | 1024 | 7.295154 | 0.112884 | 0.0155× | 0.0147–0.0161× | clifft-scheduled |
| parity129 | 1 | 0.290547 | 0.002931 | 0.0101× | 0.0100–0.0103× | clifft-scheduled |
| parity129 | 64 | 18.735167 | 0.018324 | 0.0010× | 0.0009–0.0010× | clifft-scheduled |
| parity129 | 1024 | 542.357292 | 0.250739 | 0.0005× | 0.0005–0.0005× | clifft-scheduled |
| rounds129 | 1 | 0.620486 | 0.003050 | 0.0049× | 0.0048–0.0050× | clifft-scheduled |
| rounds129 | 64 | 70.810833 | 0.017646 | 0.0002× | 0.0002–0.0003× | clifft-scheduled |
| rounds129 | 1024 | 1239.589208 | 0.243853 | 0.0002× | 0.0002–0.0002× | clifft-scheduled |
| qec32 | 1 | 0.019956 | 0.001925 | 0.0965× | 0.0960–0.0989× | clifft-scheduled |
| qec32 | 64 | 1.269694 | 0.011026 | 0.0087× | 0.0083–0.0088× | clifft |
| qec32 | 1024 | 31.877271 | 0.197927 | 0.0062× | 0.0059–0.0069× | clifft-scheduled |
| msc3 | 1 | 0.071951 | 0.004072 | 0.0566× | 0.0500–0.0619× | clifft |
| msc3 | 64 | 4.011016 | 0.028576 | 0.0071× | 0.0066–0.0074× | symft |
| msc3 | 1024 | 80.645916 | 0.251262 | 0.0031× | 0.0028–0.0047× | clifft |
| msc5 | 1 | 0.945452 | 0.047234 | 0.0500× | 0.0445–0.0748× | clifft-scheduled |
| msc5 | 64 | 72.317792 | 2.549031 | 0.0352× | 0.0320–0.0384× | clifft |
| msc5 | 1024 | 1110.025208 | 40.836396 | 0.0368× | 0.0322–0.0368× | clifft |
