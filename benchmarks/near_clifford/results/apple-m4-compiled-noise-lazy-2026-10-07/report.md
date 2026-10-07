# Experimental compiled near-Clifford raw-record comparison

Host: macOS-27.0.1-arm64-arm-64bit-Mach-O. Single CPU thread; full raw records, without postselection.
rstim API: CompiledNearCliffordExecutor. Every backend receives the identical native records-only circuit.
Rotation arithmetic: fused (rstim immutable plan policy; peer build flags are not attested).
Warm timings exclude output destruction; each raw observation accumulates at least 50 ms of public calls.
Tuning and correctness are outside all measured windows. Ranges describe process medians, not confidence intervals.

| Fixture | Shots | rstim ms | Fastest peer ms | rstim speedup | Paired speedup range | Fastest peer |
| --- | ---: | ---: | ---: | ---: | --- | --- |
| terminal | 1 | 0.000246 | 0.001533 | 6.2375× | 6.2000–6.2591× | symft |
| terminal | 64 | 0.003743 | 0.004917 | 1.3134× | 1.3026–1.3164× | clifft-scheduled |
| terminal | 1024 | 0.059039 | 0.049895 | 0.8451× | 0.8402–0.8499× | clifft |
| brick16 | 1 | 0.000322 | 0.001888 | 5.8603× | 5.7154–5.9418× | clifft-scheduled |
| brick16 | 64 | 0.005966 | 0.008835 | 1.4810× | 1.4679–1.4975× | clifft-scheduled |
| brick16 | 1024 | 0.095124 | 0.109679 | 1.1530× | 1.1503–1.1680× | clifft-scheduled |
| parity129 | 1 | 0.001844 | 0.002846 | 1.5436× | 1.5207–1.5950× | clifft-scheduled |
| parity129 | 64 | 0.012965 | 0.017970 | 1.3860× | 1.3605–1.3895× | clifft-scheduled |
| parity129 | 1024 | 0.212214 | 0.253506 | 1.1946× | 1.1840–1.2185× | clifft-scheduled |
| rounds129 | 1 | 0.001946 | 0.003018 | 1.5509× | 1.4997–1.5917× | clifft-scheduled |
| rounds129 | 64 | 0.009819 | 0.017632 | 1.7957× | 1.7873–1.8144× | clifft-scheduled |
| rounds129 | 1024 | 0.157548 | 0.240575 | 1.5270× | 1.5116–1.5365× | clifft-scheduled |
| qec32 | 1 | 0.000748 | 0.001925 | 2.5749× | 2.5121–2.6702× | clifft |
| qec32 | 64 | 0.005738 | 0.010866 | 1.8935× | 1.8807–1.9097× | clifft |
| qec32 | 1024 | 0.089418 | 0.130977 | 1.4648× | 1.4617–1.4709× | clifft-scheduled |
| msc3 | 1 | 0.001698 | 0.002334 | 1.3741× | 1.3523–1.3927× | clifft |
| msc3 | 64 | 0.014333 | 0.017920 | 1.2503× | 1.2408–1.2503× | symft |
| msc3 | 1024 | 0.228571 | 0.222046 | 0.9715× | 0.9668–1.0011× | clifft |
| msc5 | 1 | 0.032685 | 0.026104 | 0.7986× | 0.7975–0.7998× | clifft-scheduled |
| msc5 | 64 | 1.323494 | 1.202865 | 0.9089× | 0.9068–0.9112× | clifft-scheduled |
| msc5 | 1024 | 21.205875 | 18.129542 | 0.8549× | 0.8478–0.8623× | clifft-scheduled |

Cold and first-call observations are separate from warm throughput.

| Fixture | Shots | Backend | Compile ms | Prepare ms | First ms | Peak active rank | Cache reserved bytes |
| --- | ---: | --- | ---: | ---: | ---: | ---: | ---: |
| terminal | 1 | rstim | 0.079208 | 0.000333 | 0.001000 | 1 | 4112 |
| terminal | 1 | clifft | 0.069833 | included in compile | 0.002792 | 1 | n/a |
| terminal | 1 | clifft-scheduled | 0.136458 | included in compile | 0.003167 | 1 | n/a |
| terminal | 1 | symft | 0.219250 | included in compile | 0.002250 | 8 | n/a |
| terminal | 64 | rstim | 0.078584 | 0.000292 | 0.006458 | 1 | 4112 |
| terminal | 64 | clifft | 0.075125 | included in compile | 0.007292 | 1 | n/a |
| terminal | 64 | clifft-scheduled | 0.131792 | included in compile | 0.006625 | 1 | n/a |
| terminal | 64 | symft | 0.217459 | included in compile | 0.039667 | 8 | n/a |
| terminal | 1024 | rstim | 0.078209 | 0.000291 | 0.062125 | 1 | 4112 |
| terminal | 1024 | clifft | 0.071958 | included in compile | 0.052958 | 1 | n/a |
| terminal | 1024 | clifft-scheduled | 0.128417 | included in compile | 0.052083 | 1 | n/a |
| terminal | 1024 | symft | 0.212333 | included in compile | 0.519959 | 8 | n/a |
| brick16 | 1 | rstim | 0.097625 | 0.000250 | 0.003083 | 3 | 11701896 |
| brick16 | 1 | clifft | 0.085292 | included in compile | 0.003000 | 3 | n/a |
| brick16 | 1 | clifft-scheduled | 0.214375 | included in compile | 0.003292 | 3 | n/a |
| brick16 | 1 | symft | 0.186375 | included in compile | 0.003500 | 15 | n/a |
| brick16 | 64 | rstim | 0.095875 | 0.000250 | 0.040333 | 3 | 11702472 |
| brick16 | 64 | clifft | 0.089125 | included in compile | 0.011291 | 3 | n/a |
| brick16 | 64 | clifft-scheduled | 0.214709 | included in compile | 0.011375 | 3 | n/a |
| brick16 | 64 | symft | 0.180458 | included in compile | 0.027750 | 15 | n/a |
| brick16 | 1024 | rstim | 0.094208 | 0.000250 | 0.391958 | 3 | 11702472 |
| brick16 | 1024 | clifft | 0.088334 | included in compile | 0.114375 | 3 | n/a |
| brick16 | 1024 | clifft-scheduled | 0.213875 | included in compile | 0.112708 | 3 | n/a |
| brick16 | 1024 | symft | 0.181209 | included in compile | 0.362333 | 15 | n/a |
| parity129 | 1 | rstim | 1.086000 | 0.000375 | 0.008292 | 7 | 363608 |
| parity129 | 1 | clifft | 0.762459 | included in compile | 0.004541 | 7 | n/a |
| parity129 | 1 | clifft-scheduled | 2.183459 | included in compile | 0.004667 | 5 | n/a |
| parity129 | 1 | symft | 1.386084 | included in compile | 0.006042 | 12 | n/a |
| parity129 | 64 | rstim | 1.107208 | 0.001167 | 0.030458 | 7 | 375192 |
| parity129 | 64 | clifft | 0.771583 | included in compile | 0.030791 | 7 | n/a |
| parity129 | 64 | clifft-scheduled | 2.241417 | included in compile | 0.023459 | 5 | n/a |
| parity129 | 64 | symft | 1.402625 | included in compile | 0.045541 | 12 | n/a |
| parity129 | 1024 | rstim | 1.177625 | 0.000459 | 0.257125 | 7 | 373752 |
| parity129 | 1024 | clifft | 0.833667 | included in compile | 0.424000 | 7 | n/a |
| parity129 | 1024 | clifft-scheduled | 2.292666 | included in compile | 0.257250 | 5 | n/a |
| parity129 | 1024 | symft | 1.435375 | included in compile | 0.631583 | 12 | n/a |
| rounds129 | 1 | rstim | 0.930208 | 0.000458 | 0.005708 | 3 | 188176 |
| rounds129 | 1 | clifft | 0.694667 | included in compile | 0.004709 | 3 | n/a |
| rounds129 | 1 | clifft-scheduled | 1.892791 | included in compile | 0.006209 | 2 | n/a |
| rounds129 | 1 | symft | 1.174667 | included in compile | 0.007084 | 12 | n/a |
| rounds129 | 64 | rstim | 0.977750 | 0.000500 | 0.029292 | 3 | 188176 |
| rounds129 | 64 | clifft | 0.692625 | included in compile | 0.022000 | 3 | n/a |
| rounds129 | 64 | clifft-scheduled | 1.860667 | included in compile | 0.021375 | 2 | n/a |
| rounds129 | 64 | symft | 1.170834 | included in compile | 0.048500 | 12 | n/a |
| rounds129 | 1024 | rstim | 0.959667 | 0.000458 | 0.201625 | 3 | 188176 |
| rounds129 | 1024 | clifft | 0.673209 | included in compile | 0.279791 | 3 | n/a |
| rounds129 | 1024 | clifft-scheduled | 1.844958 | included in compile | 0.245167 | 2 | n/a |
| rounds129 | 1024 | symft | 1.161125 | included in compile | 0.682417 | 12 | n/a |
| qec32 | 1 | rstim | 0.440791 | 0.001500 | 0.001333 | 0 | 10928 |
| qec32 | 1 | clifft | 0.349959 | included in compile | 0.003625 | 0 | n/a |
| qec32 | 1 | clifft-scheduled | 0.362333 | included in compile | 0.003792 | 0 | n/a |
| qec32 | 1 | symft | 0.740792 | included in compile | 0.002958 | 1 | n/a |
| qec32 | 64 | rstim | 0.435458 | 0.001458 | 0.008333 | 0 | 10928 |
| qec32 | 64 | clifft | 0.359417 | included in compile | 0.012292 | 0 | n/a |
| qec32 | 64 | clifft-scheduled | 0.357917 | included in compile | 0.012417 | 0 | n/a |
| qec32 | 64 | symft | 0.742750 | included in compile | 0.017000 | 1 | n/a |
| qec32 | 1024 | rstim | 0.436583 | 0.000458 | 0.094750 | 0 | 10928 |
| qec32 | 1024 | clifft | 0.350500 | included in compile | 0.133000 | 0 | n/a |
| qec32 | 1024 | clifft-scheduled | 0.351208 | included in compile | 0.131583 | 0 | n/a |
| qec32 | 1024 | symft | 0.738792 | included in compile | 0.213083 | 1 | n/a |
| msc3 | 1 | rstim | 1.335375 | 0.002542 | 0.005667 | 4 | 11602208 |
| msc3 | 1 | clifft | 1.115708 | included in compile | 0.004792 | 4 | n/a |
| msc3 | 1 | clifft-scheduled | 1.556291 | included in compile | 0.005042 | 4 | n/a |
| msc3 | 1 | symft | 1.510417 | included in compile | 0.007208 | 4 | n/a |
| msc3 | 64 | rstim | 1.371292 | 0.002209 | 0.039833 | 4 | 26038560 |
| msc3 | 64 | clifft | 1.117083 | included in compile | 0.022166 | 4 | n/a |
| msc3 | 64 | clifft-scheduled | 1.558542 | included in compile | 0.022459 | 4 | n/a |
| msc3 | 64 | symft | 1.484542 | included in compile | 0.019709 | 4 | n/a |
| msc3 | 1024 | rstim | 1.346791 | 0.002167 | 0.336208 | 4 | 27061248 |
| msc3 | 1024 | clifft | 1.081958 | included in compile | 0.228375 | 4 | n/a |
| msc3 | 1024 | clifft-scheduled | 1.576500 | included in compile | 0.223917 | 4 | n/a |
| msc3 | 1024 | symft | 1.536042 | included in compile | 0.245167 | 4 | n/a |
| msc5 | 1 | rstim | 13.157458 | 0.001750 | 0.069458 | 10 | 67108592 |
| msc5 | 1 | clifft | 10.520292 | included in compile | 0.031708 | 10 | n/a |
| msc5 | 1 | clifft-scheduled | 13.554958 | included in compile | 0.034750 | 10 | n/a |
| msc5 | 1 | symft | 10.533750 | included in compile | 0.066667 | 10 | n/a |
| msc5 | 64 | rstim | 13.271958 | 0.001792 | 3.399000 | 10 | 67102576 |
| msc5 | 64 | clifft | 10.453833 | included in compile | 1.243875 | 10 | n/a |
| msc5 | 64 | clifft-scheduled | 13.511541 | included in compile | 1.211000 | 10 | n/a |
| msc5 | 64 | symft | 10.530500 | included in compile | 1.467417 | 10 | n/a |
| msc5 | 1024 | rstim | 13.150459 | 0.001750 | 24.893875 | 10 | 67101712 |
| msc5 | 1024 | clifft | 10.442833 | included in compile | 18.609792 | 10 | n/a |
| msc5 | 1024 | clifft-scheduled | 13.606708 | included in compile | 18.152750 | 10 | n/a |
| msc5 | 1024 | symft | 10.436125 | included in compile | 23.079417 | 10 | n/a |
