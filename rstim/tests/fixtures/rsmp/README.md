# rsmp Fixture Catalog

This directory contains the small committed fixtures for `rstim/tests/fixtures/rsmp/catalog.json`.
The four known-answer b8 vectors are independent oracles for later rsmp work.

## known_mpad_multi

Measurements per shot are `[m0, m1, m2]` from `MPAD 0 1 0`. Detector bits are
`d0 = m0 xor m1` and `d1 = m1 xor m2`; observable bit is `l0 = m0 xor m2`.
For measurement bytes `02 03 06 07`, the detector bytes are `00 01 02 03` and
the observable bytes are `00 01 01 00`.

## known_mpp_multi_product

Measurements per shot are `[m0, m1, m2]` from `MPP Z0*Z1 Z0 Z1`. Detector bits
copy the three measurement bits, and the observable bit is `m1 xor m2`. For
measurement bytes `00 03 05 06`, the detector bytes are identical and the
observable bytes are `00 01 01 00`.

## known_heralded_erase

`HERALDED_ERASE` produces one herald measurement. The detector and observable
both reference `rec[-1]`, so measurements, detectors, and observables are all
`00 01 01 00`.

## known_heralded_pauli_channel_1

`HERALDED_PAULI_CHANNEL_1` produces one herald measurement. The detector and
observable both reference `rec[-1]`, so measurements, detectors, and
observables are all `00 01 00 01`.

## Stim Cross-Check

The independent command family is pinned to Stim 1.15.0:

```console
python3 -c 'import stim; print(stim.__version__)'
stim m2d --circuit <case>.stim --in <case>.measurements.b8 --in_format b8 --out <case>.detectors.check.b8 --out_format b8 --obs_out <case>.observables.check.b8 --obs_out_format b8
```
