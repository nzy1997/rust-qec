# Apple M4 master-to-compact-replay source effects

Baseline: merged master `e1453a7b634275d3055ef64b2f4a7b33d3995870`.
Candidate: clean measured `d989531d837fe874a77ac05745f5d77ea5639d52`.
All four warm/control campaigns retain all 24 cells and 288 events each:
48 independent four-seed records/counts/RNG validations followed by 240 timed
processes, five rotated/reversed paired rounds and seven observations at least
50 ms long. Both A/A controls use the exact candidate source and binary under
both labels. No peer or SOTA claim follows from these source-effect ratios.

## Complete warm and null-control results

Ratios are baseline time / candidate time. All median losses and process outliers
remain; a median gain alone does not establish a stable small source effect.

| Campaign | Circuit | Policy | Shots | Baseline / candidate | Paired range |
| --- | --- | --- | ---: | ---: | ---: |
| warm | MSC d3 | strict | 1 | 0.994865× | 0.842298–1.011487 |
| warm | MSC d3 | fused | 1 | 1.004954× | 0.940621–1.016508 |
| warm | MSC d3 | strict | 64 | 1.004962× | 0.502691–1.024938 |
| warm | MSC d3 | fused | 64 | 1.007404× | 0.999342–1.012512 |
| warm | MSC d3 | strict | 1024 | 1.011085× | 0.997271–1.023666 |
| warm | MSC d3 | fused | 1024 | 1.007216× | 0.992658–1.012781 |
| warm | MSC d5 | strict | 1 | 0.994711× | 0.953529–1.000252 |
| warm | MSC d5 | fused | 1 | 0.994539× | 0.989584–1.013173 |
| warm | MSC d5 | strict | 64 | 1.201203× | 1.190176–1.258074 |
| warm | MSC d5 | fused | 64 | 1.234027× | 0.472378–1.295307 |
| warm | MSC d5 | strict | 1024 | 1.223837× | 1.162752–1.571267 |
| warm | MSC d5 | fused | 1024 | 1.227795× | 1.226423–1.320032 |
| warm | surface d7 | strict | 1 | 1.005095× | 0.987896–1.012570 |
| warm | surface d7 | fused | 1 | 0.987227× | 0.981968–0.997969 |
| warm | surface d7 | strict | 64 | 1.007422× | 0.991526–1.016988 |
| warm | surface d7 | fused | 64 | 1.002365× | 0.996136–1.009819 |
| warm | surface d7 | strict | 1024 | 1.006569× | 1.002940–1.013101 |
| warm | surface d7 | fused | 1024 | 1.011858× | 0.998660–1.013657 |
| warm | surface d9 | strict | 1 | 1.000019× | 0.991006–1.019430 |
| warm | surface d9 | fused | 1 | 1.005969× | 0.975533–1.017251 |
| warm | surface d9 | strict | 64 | 1.015152× | 0.992084–1.015152 |
| warm | surface d9 | fused | 64 | 1.004510× | 0.989563–1.020925 |
| warm | surface d9 | strict | 1024 | 1.001257× | 0.990797–1.011532 |
| warm | surface d9 | fused | 1024 | 0.994517× | 0.994517–1.011442 |
| confirmation | MSC d3 | strict | 1 | 1.021147× | 0.933690–1.291075 |
| confirmation | MSC d3 | fused | 1 | 1.009371× | 1.000417–1.030322 |
| confirmation | MSC d3 | strict | 64 | 1.012055× | 0.996302–1.020495 |
| confirmation | MSC d3 | fused | 64 | 1.015706× | 0.995989–1.037392 |
| confirmation | MSC d3 | strict | 1024 | 1.008094× | 0.999928–1.023576 |
| confirmation | MSC d3 | fused | 1024 | 1.009383× | 0.992108–1.034691 |
| confirmation | MSC d5 | strict | 1 | 1.008169× | 1.001292–1.020397 |
| confirmation | MSC d5 | fused | 1 | 1.003484× | 0.961018–1.014815 |
| confirmation | MSC d5 | strict | 64 | 1.219694× | 1.214286–1.314074 |
| confirmation | MSC d5 | fused | 64 | 1.239121× | 1.221012–1.270789 |
| confirmation | MSC d5 | strict | 1024 | 1.202829× | 1.188482–1.222664 |
| confirmation | MSC d5 | fused | 1024 | 1.246541× | 1.228388–1.374840 |
| confirmation | surface d7 | strict | 1 | 0.994992× | 0.986938–0.999253 |
| confirmation | surface d7 | fused | 1 | 0.995397× | 0.979822–1.001293 |
| confirmation | surface d7 | strict | 64 | 1.008078× | 0.903536–1.016182 |
| confirmation | surface d7 | fused | 64 | 0.998511× | 0.985158–1.118770 |
| confirmation | surface d7 | strict | 1024 | 1.007564× | 0.983182–1.024189 |
| confirmation | surface d7 | fused | 1024 | 1.015468× | 0.968753–1.024365 |
| confirmation | surface d9 | strict | 1 | 1.025694× | 0.962873–1.490207 |
| confirmation | surface d9 | fused | 1 | 1.002166× | 0.975225–1.033326 |
| confirmation | surface d9 | strict | 64 | 1.009823× | 0.987637–1.019529 |
| confirmation | surface d9 | fused | 64 | 1.008545× | 0.800549–1.313352 |
| confirmation | surface d9 | strict | 1024 | 1.012580× | 1.001252–1.663633 |
| confirmation | surface d9 | fused | 1024 | 0.997302× | 0.995536–1.001384 |
| same-binary-warm | MSC d3 | strict | 1 | 0.998612× | 0.973588–1.013274 |
| same-binary-warm | MSC d3 | fused | 1 | 1.024723× | 0.989494–1.029844 |
| same-binary-warm | MSC d3 | strict | 64 | 0.994624× | 0.977682–1.012061 |
| same-binary-warm | MSC d3 | fused | 64 | 0.997134× | 0.992406–1.008196 |
| same-binary-warm | MSC d3 | strict | 1024 | 1.004881× | 0.984646–1.017083 |
| same-binary-warm | MSC d3 | fused | 1024 | 0.994543× | 0.993430–1.164091 |
| same-binary-warm | MSC d5 | strict | 1 | 1.002767× | 1.000167–1.010713 |
| same-binary-warm | MSC d5 | fused | 1 | 1.002304× | 0.993701–1.144503 |
| same-binary-warm | MSC d5 | strict | 64 | 1.005259× | 0.981986–1.028581 |
| same-binary-warm | MSC d5 | fused | 64 | 1.008517× | 0.957021–1.024378 |
| same-binary-warm | MSC d5 | strict | 1024 | 0.999234× | 0.973229–1.018072 |
| same-binary-warm | MSC d5 | fused | 1024 | 0.990089× | 0.989368–1.037536 |
| same-binary-warm | surface d7 | strict | 1 | 0.999502× | 0.829472–1.008268 |
| same-binary-warm | surface d7 | fused | 1 | 1.001388× | 0.939541–1.016633 |
| same-binary-warm | surface d7 | strict | 64 | 0.995217× | 0.983128–1.031684 |
| same-binary-warm | surface d7 | fused | 64 | 0.999566× | 0.965251–1.003419 |
| same-binary-warm | surface d7 | strict | 1024 | 1.003607× | 0.978075–1.004261 |
| same-binary-warm | surface d7 | fused | 1024 | 0.997232× | 0.990810–1.011279 |
| same-binary-warm | surface d9 | strict | 1 | 0.989908× | 0.907256–1.026996 |
| same-binary-warm | surface d9 | fused | 1 | 0.986107× | 0.971684–1.024661 |
| same-binary-warm | surface d9 | strict | 64 | 0.992603× | 0.992603–1.008081 |
| same-binary-warm | surface d9 | fused | 64 | 1.010607× | 0.995453–1.010607 |
| same-binary-warm | surface d9 | strict | 1024 | 0.994086× | 0.982478–1.015443 |
| same-binary-warm | surface d9 | fused | 1024 | 1.014166× | 0.985075–1.014531 |
| same-binary-confirmation | MSC d3 | strict | 1 | 0.998444× | 0.524767–1.017097 |
| same-binary-confirmation | MSC d3 | fused | 1 | 0.988395× | 0.959688–1.018486 |
| same-binary-confirmation | MSC d3 | strict | 64 | 1.004502× | 0.985524–1.030579 |
| same-binary-confirmation | MSC d3 | fused | 64 | 1.004010× | 1.001731–1.012751 |
| same-binary-confirmation | MSC d3 | strict | 1024 | 0.988454× | 0.972130–0.997031 |
| same-binary-confirmation | MSC d3 | fused | 1024 | 1.001297× | 0.990414–1.016499 |
| same-binary-confirmation | MSC d5 | strict | 1 | 0.989188× | 0.983975–0.999296 |
| same-binary-confirmation | MSC d5 | fused | 1 | 0.987220× | 0.982620–0.999328 |
| same-binary-confirmation | MSC d5 | strict | 64 | 0.991388× | 0.986246–0.999870 |
| same-binary-confirmation | MSC d5 | fused | 64 | 0.998671× | 0.964799–1.045485 |
| same-binary-confirmation | MSC d5 | strict | 1024 | 0.990852× | 0.964061–0.992531 |
| same-binary-confirmation | MSC d5 | fused | 1024 | 0.992055× | 0.987424–0.997772 |
| same-binary-confirmation | surface d7 | strict | 1 | 0.996983× | 0.993577–1.011577 |
| same-binary-confirmation | surface d7 | fused | 1 | 1.000802× | 0.523986–1.014332 |
| same-binary-confirmation | surface d7 | strict | 64 | 0.999313× | 0.996286–1.004150 |
| same-binary-confirmation | surface d7 | fused | 64 | 1.007390× | 0.986307–1.015185 |
| same-binary-confirmation | surface d7 | strict | 1024 | 1.005429× | 0.998630–1.008558 |
| same-binary-confirmation | surface d7 | fused | 1024 | 0.995918× | 0.985450–1.004991 |
| same-binary-confirmation | surface d9 | strict | 1 | 1.012935× | 0.986834–1.025863 |
| same-binary-confirmation | surface d9 | fused | 1 | 0.988582× | 0.981442–1.040753 |
| same-binary-confirmation | surface d9 | strict | 64 | 1.002307× | 0.968162–1.008470 |
| same-binary-confirmation | surface d9 | fused | 64 | 0.995620× | 0.962305–0.998627 |
| same-binary-confirmation | surface d9 | strict | 1024 | 0.985313× | 0.983610–1.007309 |
| same-binary-confirmation | surface d9 | fused | 1024 | 0.987762× | 0.939641–1.008571 |

D5 bulk gains reproduce in both A/B campaigns. Other median and paired losses
remain visible above; A/A cells with every pair on one side of one demonstrate
that this protocol cannot turn every small shift into a source claim.

The warm d5/64 Fused experiment retains a paired ratio 0.4723776 despite its
median gain. The M4 is shared and unpinned; all ranges are descriptive.

## Complete fresh-seed cold phases

The cold run retains 48 four-seed validation events and 240 processes, each with
32 independent fresh seeds 739..770: 7680 observations. Every observation compiles
a fresh plan, prepares a sampler and takes its first counts call. No warm-duration
minimum applies. Counts, sixteen RNG continuation words and the 64 MiB cache cap
must agree across source roles. Phase sum excludes process startup and destruction;
it is not an end-to-end latency measurement.

| Circuit | Policy | Shots | Phase | Baseline / candidate | Paired range |
| --- | --- | ---: | --- | ---: | ---: |
| MSC d3 | strict | 1 | compile_ns | 0.982075× | 0.956152–1.009191 |
| MSC d3 | strict | 1 | prepare_ns | 0.937031× | 0.777333–1.248876 |
| MSC d3 | strict | 1 | first_ns | 0.879125× | 0.820975–1.173702 |
| MSC d3 | strict | 1 | phase_sum_ns | 0.982144× | 0.955836–1.011575 |
| MSC d3 | fused | 1 | compile_ns | 0.977882× | 0.958528–0.995914 |
| MSC d3 | fused | 1 | prepare_ns | 1.028914× | 0.881523–1.167200 |
| MSC d3 | fused | 1 | first_ns | 0.872147× | 0.812054–0.987132 |
| MSC d3 | fused | 1 | phase_sum_ns | 0.977503× | 0.959468–0.996186 |
| MSC d3 | strict | 64 | compile_ns | 0.995432× | 0.965487–1.015059 |
| MSC d3 | strict | 64 | prepare_ns | 1.000000× | 0.800000–1.000000 |
| MSC d3 | strict | 64 | first_ns | 1.022788× | 0.938019–1.106810 |
| MSC d3 | strict | 64 | phase_sum_ns | 0.998264× | 0.963880–1.018848 |
| MSC d3 | fused | 64 | compile_ns | 0.984218× | 0.967400–1.006035 |
| MSC d3 | fused | 64 | prepare_ns | 0.969455× | 0.778000–1.125281 |
| MSC d3 | fused | 64 | first_ns | 0.992095× | 0.918107–1.019782 |
| MSC d3 | fused | 64 | phase_sum_ns | 0.985167× | 0.970584–1.006137 |
| MSC d3 | strict | 1024 | compile_ns | 1.005187× | 0.952644–1.025853 |
| MSC d3 | strict | 1024 | prepare_ns | 1.033308× | 0.847384–1.193648 |
| MSC d3 | strict | 1024 | first_ns | 1.003622× | 0.985273–1.027020 |
| MSC d3 | strict | 1024 | phase_sum_ns | 1.004201× | 0.962000–1.020975 |
| MSC d3 | fused | 1024 | compile_ns | 0.997817× | 0.962225–1.007397 |
| MSC d3 | fused | 1024 | prepare_ns | 1.032533× | 0.881523–1.129357 |
| MSC d3 | fused | 1024 | first_ns | 1.007784× | 0.981244–1.031552 |
| MSC d3 | fused | 1024 | phase_sum_ns | 1.000269× | 0.970468–1.013170 |
| MSC d5 | strict | 1 | compile_ns | 0.974752× | 0.970776–0.987440 |
| MSC d5 | strict | 1 | prepare_ns | 0.973120× | 0.947943–0.986720 |
| MSC d5 | strict | 1 | first_ns | 1.036498× | 1.011340–1.125742 |
| MSC d5 | strict | 1 | phase_sum_ns | 0.977497× | 0.970467–0.990800 |
| MSC d5 | fused | 1 | compile_ns | 0.977706× | 0.969418–0.979399 |
| MSC d5 | fused | 1 | prepare_ns | 0.961139× | 0.861501–1.034667 |
| MSC d5 | fused | 1 | first_ns | 0.899076× | 0.857122–1.117817 |
| MSC d5 | fused | 1 | phase_sum_ns | 0.976631× | 0.968575–0.978411 |
| MSC d5 | strict | 64 | compile_ns | 0.969818× | 0.959154–1.007895 |
| MSC d5 | strict | 64 | prepare_ns | 1.026560× | 0.946720–1.054531 |
| MSC d5 | strict | 64 | first_ns | 1.046352× | 0.995973–1.062176 |
| MSC d5 | strict | 64 | phase_sum_ns | 0.986416× | 0.957807–1.013162 |
| MSC d5 | fused | 64 | compile_ns | 0.974822× | 0.960934–0.979470 |
| MSC d5 | fused | 64 | prepare_ns | 0.986880× | 0.924058–1.006603 |
| MSC d5 | fused | 64 | first_ns | 0.991930× | 0.947186–1.013259 |
| MSC d5 | fused | 64 | phase_sum_ns | 0.975875× | 0.959015–0.987277 |
| MSC d5 | strict | 1024 | compile_ns | 0.973340× | 0.962878–0.983034 |
| MSC d5 | strict | 1024 | prepare_ns | 1.064994× | 0.802427–1.138833 |
| MSC d5 | strict | 1024 | first_ns | 1.066314× | 1.028227–1.092553 |
| MSC d5 | strict | 1024 | phase_sum_ns | 1.009686× | 0.999938–1.014091 |
| MSC d5 | fused | 1024 | compile_ns | 0.974730× | 0.965473–0.987709 |
| MSC d5 | fused | 1024 | prepare_ns | 0.974784× | 0.956476–1.118217 |
| MSC d5 | fused | 1024 | first_ns | 1.069257× | 1.057330–1.140799 |
| MSC d5 | fused | 1024 | phase_sum_ns | 1.011224× | 1.000251–1.050682 |
| surface d7 | strict | 1 | compile_ns | 0.996664× | 0.989611–1.000313 |
| surface d7 | strict | 1 | prepare_ns | 0.967581× | 0.870940–1.082183 |
| surface d7 | strict | 1 | first_ns | 0.993007× | 0.976424–1.012936 |
| surface d7 | strict | 1 | phase_sum_ns | 0.997377× | 0.991015–1.000210 |
| surface d7 | fused | 1 | compile_ns | 0.999543× | 0.985087–1.013831 |
| surface d7 | fused | 1 | prepare_ns | 1.006974× | 0.920371–1.068851 |
| surface d7 | fused | 1 | first_ns | 1.019992× | 1.005475–1.041165 |
| surface d7 | fused | 1 | phase_sum_ns | 0.996912× | 0.985638–1.011064 |
| surface d7 | strict | 64 | compile_ns | 0.995637× | 0.983838–1.006455 |
| surface d7 | strict | 64 | prepare_ns | 1.033440× | 0.877053–1.088490 |
| surface d7 | strict | 64 | first_ns | 0.997561× | 0.988688–1.024547 |
| surface d7 | strict | 64 | phase_sum_ns | 0.995521× | 0.986756–1.008003 |
| surface d7 | fused | 64 | compile_ns | 0.997582× | 0.994412–1.041434 |
| surface d7 | fused | 64 | prepare_ns | 1.073776× | 0.981449–1.132740 |
| surface d7 | fused | 64 | first_ns | 1.029920× | 1.012095–1.092214 |
| surface d7 | fused | 64 | phase_sum_ns | 1.000156× | 0.997435–1.047659 |
| surface d7 | strict | 1024 | compile_ns | 0.997631× | 0.983807–1.002828 |
| surface d7 | strict | 1024 | prepare_ns | 0.929418× | 0.869358–1.073440 |
| surface d7 | strict | 1024 | first_ns | 1.010712× | 0.984659–1.040166 |
| surface d7 | strict | 1024 | phase_sum_ns | 0.999637× | 0.980475–1.007168 |
| surface d7 | fused | 1024 | compile_ns | 0.996504× | 0.992967–1.005378 |
| surface d7 | fused | 1024 | prepare_ns | 0.999841× | 0.867092–1.090970 |
| surface d7 | fused | 1024 | first_ns | 1.017288× | 1.003336–1.024677 |
| surface d7 | fused | 1024 | phase_sum_ns | 1.000025× | 0.994695–1.005830 |
| surface d9 | strict | 1 | compile_ns | 0.995391× | 0.974732–1.009599 |
| surface d9 | strict | 1 | prepare_ns | 0.996583× | 0.989324–1.031336 |
| surface d9 | strict | 1 | first_ns | 0.967791× | 0.869517–1.038744 |
| surface d9 | strict | 1 | phase_sum_ns | 0.995166× | 0.967852–1.010948 |
| surface d9 | fused | 1 | compile_ns | 0.994775× | 0.992650–1.007772 |
| surface d9 | fused | 1 | prepare_ns | 1.000169× | 0.978871–1.025121 |
| surface d9 | fused | 1 | first_ns | 0.964358× | 0.855556–1.044976 |
| surface d9 | fused | 1 | phase_sum_ns | 0.991589× | 0.989811–1.011374 |
| surface d9 | strict | 64 | compile_ns | 0.994158× | 0.994158–0.998060 |
| surface d9 | strict | 64 | prepare_ns | 0.986083× | 0.975237–0.990370 |
| surface d9 | strict | 64 | first_ns | 0.970915× | 0.923134–1.065928 |
| surface d9 | strict | 64 | phase_sum_ns | 0.993523× | 0.989935–0.995277 |
| surface d9 | fused | 64 | compile_ns | 0.992697× | 0.903387–0.999828 |
| surface d9 | fused | 64 | prepare_ns | 0.978874× | 0.957447–1.090514 |
| surface d9 | fused | 64 | first_ns | 0.886392× | 0.747285–0.996376 |
| surface d9 | fused | 64 | phase_sum_ns | 0.989639× | 0.892742–0.995540 |
| surface d9 | strict | 1024 | compile_ns | 0.992753× | 0.984571–1.007900 |
| surface d9 | strict | 1024 | prepare_ns | 0.947765× | 0.932479–0.996784 |
| surface d9 | strict | 1024 | first_ns | 0.943832× | 0.843399–1.077840 |
| surface d9 | strict | 1024 | phase_sum_ns | 0.979243× | 0.972241–1.003836 |
| surface d9 | fused | 1024 | compile_ns | 0.986049× | 0.983172–0.997433 |
| surface d9 | fused | 1024 | prepare_ns | 0.969490× | 0.928544–1.033927 |
| surface d9 | fused | 1024 | first_ns | 0.894873× | 0.888858–1.070085 |
| surface d9 | fused | 1024 | phase_sum_ns | 0.977357× | 0.977357–1.002815 |

D5/1024 first-call ratios are 1.066314/1.069257 (Strict/Fused), while phase sums
are only 1.009686/1.011224. D5/1 and d5/64 phase sums are below one in both
policies. The warm improvement does not establish a comparable cold-total gain.

## Diagnostic direction and retention

A separate current d5/1024 Strict/Fused diagnostic campaign ran in
[workflow 37751156104](https://github.com/nzy1997/rust-qec/actions/runs/37751156104).
Its whole-process cpu-clock profiles include startup, warmup and teardown and are
all `performance_valid=false`. Candidate leaf samples concentrate on rotation,
probability and projection kernels; this identifies an experiment direction,
not elapsed phase fractions, measured gains or peer leadership. Raw perf data,
stack exports and actual ELF bytes remain in the workflow artifact. The perf
wrapper is identified; the underlying perf ELF lacks separate SHA attestation.

All original schemas, drivers, headers, closures, events, seven probe inputs and
build logs per role remain unchanged. Historical source snapshots are evidence,
not runnable relocated Cargo projects. Producer tags/branches remain. Native
binaries/disassemblies stay in original ignored evidence/workflow artifacts;
portable replay checks byte-identifying receipts without hashing absent binaries.
M4 cold and warm binary identities are checked separately. Recollection requires
new clean checkouts, corrected output paths and new measured identities.

```sh
python3 benchmarks/near_clifford/verify_compact_zero_spans_scouts.py
python3 -O benchmarks/near_clifford/verify_compact_zero_spans_scouts.py
python3 benchmarks/near_clifford/test_compact_zero_spans_scouts.py
python3 -O benchmarks/near_clifford/test_compact_zero_spans_scouts.py
```

Replay checks all sources/probes, driver digests, receipts, exact schedules,
validations, observations and derived summaries. Resealed adversarial cases must
fail in normal and optimized Python. The cold-prefix option preserves the older
240-event cold contracts by default. Formal independent full PR review and final
head CI are separate merge gates.
