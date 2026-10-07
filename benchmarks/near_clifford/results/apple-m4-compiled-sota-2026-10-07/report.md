# Experimental compiled near-Clifford raw-record comparison

Host: macOS-27.0.1-arm64-arm-64bit-Mach-O. Single CPU thread; full raw records, without postselection.
rstim API: CompiledNearCliffordExecutor. Every backend receives the identical native records-only circuit.
Rotation arithmetic: fused (rstim immutable plan policy; peer build flags are not attested).
Warm timings exclude output destruction; each raw observation accumulates at least 50 ms of public calls.
Tuning and correctness are outside all measured windows. Ranges describe process medians, not confidence intervals.

| Fixture | Shots | rstim ms | Fastest peer ms | rstim speedup | Paired speedup range | Fastest peer |
| --- | ---: | ---: | ---: | ---: | --- | --- |
| terminal | 1 | 0.000313 | 0.001993 | 6.3694× | 6.3025–6.5477× | symft |
| terminal | 64 | 0.005136 | 0.006176 | 1.2026× | 1.1524–1.2937× | clifft-scheduled |
| terminal | 1024 | 0.082475 | 0.063984 | 0.7758× | 0.7717–0.7943× | clifft-scheduled |
| brick16 | 1 | 0.000435 | 0.002404 | 5.5261× | 5.2435–5.6090× | clifft |
| brick16 | 64 | 0.007901 | 0.011372 | 1.4394× | 1.3475–1.5006× | clifft-scheduled |
| brick16 | 1024 | 0.131280 | 0.140370 | 1.0692× | 0.9243–1.0745× | clifft |
| parity129 | 1 | 0.002356 | 0.003656 | 1.5515× | 1.3996–1.5800× | clifft-scheduled |
| parity129 | 64 | 0.017511 | 0.022158 | 1.2653× | 1.2365–1.3535× | clifft-scheduled |
| parity129 | 1024 | 0.277462 | 0.307012 | 1.1065× | 1.0821–1.1077× | clifft-scheduled |
| rounds129 | 1 | 0.002594 | 0.003666 | 1.4132× | 1.3874–1.4628× | clifft-scheduled |
| rounds129 | 64 | 0.011511 | 0.021163 | 1.8385× | 1.6375–1.8596× | clifft-scheduled |
| rounds129 | 1024 | 0.191233 | 0.289766 | 1.5153× | 1.4777–1.5167× | clifft-scheduled |
| qec32 | 1 | 0.000917 | 0.002420 | 2.6379× | 2.5351–2.7978× | clifft |
| qec32 | 64 | 0.008995 | 0.013042 | 1.4499× | 1.4426–1.5263× | clifft |
| qec32 | 1024 | 0.145264 | 0.153442 | 1.0563× | 1.0486–1.0828× | clifft |
| msc3 | 1 | 0.002334 | 0.002992 | 1.2822× | 1.2553–1.4520× | clifft |
| msc3 | 64 | 0.022363 | 0.022588 | 1.0101× | 0.9953–1.1000× | symft |
| msc3 | 1024 | 0.360910 | 0.289553 | 0.8023× | 0.7974–0.8129× | clifft-scheduled |
| msc5 | 1 | 0.044676 | 0.034925 | 0.7817× | 0.7387–0.8276× | clifft-scheduled |
| msc5 | 64 | 1.654832 | 1.563583 | 0.9449× | 0.9262–0.9449× | clifft |
| msc5 | 1024 | 27.632271 | 24.548875 | 0.8884× | 0.8380–0.9074× | clifft-scheduled |

Cold and first-call observations are separate from warm throughput.

| Fixture | Shots | Backend | Compile ms | Prepare ms | First ms | Peak active rank | Cache reserved bytes |
| --- | ---: | --- | ---: | ---: | ---: | ---: | ---: |
| terminal | 1 | rstim | 0.159167 | 0.000542 | 0.002125 | 1 | 4112 |
| terminal | 1 | clifft | 0.080041 | included in compile | 0.003083 | 1 | n/a |
| terminal | 1 | clifft-scheduled | 0.146041 | included in compile | 0.003333 | 1 | n/a |
| terminal | 1 | symft | 0.237792 | included in compile | 0.002417 | 8 | n/a |
| terminal | 64 | rstim | 0.156834 | 0.000500 | 0.014583 | 1 | 4112 |
| terminal | 64 | clifft | 0.082084 | included in compile | 0.007542 | 1 | n/a |
| terminal | 64 | clifft-scheduled | 0.142541 | included in compile | 0.007500 | 1 | n/a |
| terminal | 64 | symft | 0.230209 | included in compile | 0.040666 | 8 | n/a |
| terminal | 1024 | rstim | 0.157375 | 0.000500 | 0.151750 | 1 | 4112 |
| terminal | 1024 | clifft | 0.088834 | included in compile | 0.054250 | 1 | n/a |
| terminal | 1024 | clifft-scheduled | 0.148625 | included in compile | 0.053625 | 1 | n/a |
| terminal | 1024 | symft | 0.231833 | included in compile | 1.079208 | 8 | n/a |
| brick16 | 1 | rstim | 0.205000 | 0.000500 | 0.005708 | 3 | 11701896 |
| brick16 | 1 | clifft | 0.189792 | included in compile | 0.006125 | 3 | n/a |
| brick16 | 1 | clifft-scheduled | 0.416292 | included in compile | 0.003500 | 3 | n/a |
| brick16 | 1 | symft | 0.206208 | included in compile | 0.003666 | 15 | n/a |
| brick16 | 64 | rstim | 0.113583 | 0.000333 | 0.043375 | 3 | 11702472 |
| brick16 | 64 | clifft | 0.100417 | included in compile | 0.011625 | 3 | n/a |
| brick16 | 64 | clifft-scheduled | 0.237500 | included in compile | 0.011625 | 3 | n/a |
| brick16 | 64 | symft | 0.369083 | included in compile | 0.028583 | 15 | n/a |
| brick16 | 1024 | rstim | 0.200458 | 0.000542 | 0.937000 | 3 | 11702472 |
| brick16 | 1024 | clifft | 0.106833 | included in compile | 0.116666 | 3 | n/a |
| brick16 | 1024 | clifft-scheduled | 0.229833 | included in compile | 0.119459 | 3 | n/a |
| brick16 | 1024 | symft | 0.197917 | included in compile | 0.397750 | 15 | n/a |
| parity129 | 1 | rstim | 2.213667 | 0.000750 | 0.011917 | 7 | 361592 |
| parity129 | 1 | clifft | 1.193167 | included in compile | 0.004750 | 7 | n/a |
| parity129 | 1 | clifft-scheduled | 3.271334 | included in compile | 0.005583 | 5 | n/a |
| parity129 | 1 | symft | 1.422958 | included in compile | 0.014083 | 12 | n/a |
| parity129 | 64 | rstim | 2.160875 | 0.000834 | 0.032000 | 7 | 374904 |
| parity129 | 64 | clifft | 0.807541 | included in compile | 0.031417 | 7 | n/a |
| parity129 | 64 | clifft-scheduled | 2.252750 | included in compile | 0.026000 | 5 | n/a |
| parity129 | 64 | symft | 1.439542 | included in compile | 0.046750 | 12 | n/a |
| parity129 | 1024 | rstim | 1.229834 | 0.000458 | 0.278250 | 7 | 373752 |
| parity129 | 1024 | clifft | 0.814375 | included in compile | 0.420792 | 7 | n/a |
| parity129 | 1024 | clifft-scheduled | 2.272125 | included in compile | 0.254250 | 5 | n/a |
| parity129 | 1024 | symft | 2.821959 | included in compile | 0.666542 | 12 | n/a |
| rounds129 | 1 | rstim | 1.740875 | 0.000834 | 0.008875 | 3 | 188176 |
| rounds129 | 1 | clifft | 0.672042 | included in compile | 0.011333 | 3 | n/a |
| rounds129 | 1 | clifft-scheduled | 1.853542 | included in compile | 0.005791 | 2 | n/a |
| rounds129 | 1 | symft | 1.166083 | included in compile | 0.008083 | 12 | n/a |
| rounds129 | 64 | rstim | 1.726792 | 0.002167 | 0.039875 | 3 | 188176 |
| rounds129 | 64 | clifft | 0.712375 | included in compile | 0.027334 | 3 | n/a |
| rounds129 | 64 | clifft-scheduled | 2.839417 | included in compile | 0.022125 | 2 | n/a |
| rounds129 | 64 | symft | 1.196750 | included in compile | 0.077375 | 12 | n/a |
| rounds129 | 1024 | rstim | 1.357417 | 0.000500 | 0.205084 | 3 | 188176 |
| rounds129 | 1024 | clifft | 0.673708 | included in compile | 0.485042 | 3 | n/a |
| rounds129 | 1024 | clifft-scheduled | 1.841084 | included in compile | 0.246583 | 2 | n/a |
| rounds129 | 1024 | symft | 1.150042 | included in compile | 0.680250 | 12 | n/a |
| qec32 | 1 | rstim | 0.456708 | 0.001750 | 0.001333 | 0 | 10928 |
| qec32 | 1 | clifft | 0.401666 | included in compile | 0.007416 | 0 | n/a |
| qec32 | 1 | clifft-scheduled | 0.365375 | included in compile | 0.006875 | 0 | n/a |
| qec32 | 1 | symft | 0.748041 | included in compile | 0.003250 | 1 | n/a |
| qec32 | 64 | rstim | 0.449291 | 0.001459 | 0.009416 | 0 | 10928 |
| qec32 | 64 | clifft | 0.373542 | included in compile | 0.012208 | 0 | n/a |
| qec32 | 64 | clifft-scheduled | 0.375041 | included in compile | 0.013833 | 0 | n/a |
| qec32 | 64 | symft | 0.763583 | included in compile | 0.023917 | 1 | n/a |
| qec32 | 1024 | rstim | 0.965291 | 0.002042 | 0.211583 | 0 | 10928 |
| qec32 | 1024 | clifft | 0.347333 | included in compile | 0.132667 | 0 | n/a |
| qec32 | 1024 | clifft-scheduled | 0.670917 | included in compile | 0.132208 | 0 | n/a |
| qec32 | 1024 | symft | 0.754750 | included in compile | 0.216084 | 1 | n/a |
| msc3 | 1 | rstim | 2.504583 | 0.002250 | 0.006250 | 4 | 10469376 |
| msc3 | 1 | clifft | 1.142000 | included in compile | 0.005042 | 4 | n/a |
| msc3 | 1 | clifft-scheduled | 1.570958 | included in compile | 0.007166 | 4 | n/a |
| msc3 | 1 | symft | 2.910292 | included in compile | 0.007667 | 4 | n/a |
| msc3 | 64 | rstim | 1.418708 | 0.002541 | 0.044000 | 4 | 22148096 |
| msc3 | 64 | clifft | 1.100416 | included in compile | 0.049000 | 4 | n/a |
| msc3 | 64 | clifft-scheduled | 1.577958 | included in compile | 0.051167 | 4 | n/a |
| msc3 | 64 | symft | 1.513542 | included in compile | 0.044000 | 4 | n/a |
| msc3 | 1024 | rstim | 2.831959 | 0.003750 | 0.387000 | 4 | 22255168 |
| msc3 | 1024 | clifft | 1.114584 | included in compile | 0.523125 | 4 | n/a |
| msc3 | 1024 | clifft-scheduled | 1.701041 | included in compile | 0.232000 | 4 | n/a |
| msc3 | 1024 | symft | 1.547500 | included in compile | 0.261375 | 4 | n/a |
| msc5 | 1 | rstim | 17.226625 | 0.010750 | 0.180083 | 10 | 67108592 |
| msc5 | 1 | clifft | 10.465125 | included in compile | 0.059542 | 10 | n/a |
| msc5 | 1 | clifft-scheduled | 14.727917 | included in compile | 0.065125 | 10 | n/a |
| msc5 | 1 | symft | 13.711500 | included in compile | 0.077042 | 10 | n/a |
| msc5 | 64 | rstim | 16.195042 | 0.009709 | 3.736250 | 10 | 67102576 |
| msc5 | 64 | clifft | 13.367917 | included in compile | 1.332833 | 10 | n/a |
| msc5 | 64 | clifft-scheduled | 13.756917 | included in compile | 1.326375 | 10 | n/a |
| msc5 | 64 | symft | 15.827834 | included in compile | 1.588250 | 10 | n/a |
| msc5 | 1024 | rstim | 13.291542 | 0.003167 | 31.626417 | 10 | 67101712 |
| msc5 | 1024 | clifft | 12.099917 | included in compile | 26.296583 | 10 | n/a |
| msc5 | 1024 | clifft-scheduled | 13.911416 | included in compile | 25.213709 | 10 | n/a |
| msc5 | 1024 | symft | 15.945250 | included in compile | 28.878584 | 10 | n/a |
