# CSS Fixtures

APM P=96 fixtures are generated from the qec-code built-in CSS export:

```sh
cargo run -p qec-code -- code css apm_kasai:p=96 hx > rsinter/tests/fixtures/css/apm_p96_hx.json
cargo run -p qec-code -- code css apm_kasai:p=96 hz > rsinter/tests/fixtures/css/apm_p96_hz.json
```
