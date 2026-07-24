import assert from "node:assert/strict";
import test from "node:test";
import {
  HIGH_ENTROPY_CONTROL,
  MEASURED_CASE,
  formatBytes,
  formatDuration,
  formatPercent,
  projectShots,
} from "../lib/rsmpEvidence.mjs";

test("keeps the pinned measured RSMP compression values exact", () => {
  assert.equal(MEASURED_CASE.rawB8Bytes, 1552384);
  assert.equal(MEASURED_CASE.directZstdBytes, 325565);
  assert.equal(MEASURED_CASE.rsmpBytes, 186028);
  assert.equal(
    formatPercent(MEASURED_CASE.rsmpBytes, MEASURED_CASE.rawB8Bytes, 2),
    "11.98%",
  );
  assert.equal(
    formatPercent(MEASURED_CASE.rsmpBytes, MEASURED_CASE.directZstdBytes, 2),
    "57.14%",
  );
  assert.equal(HIGH_ENTROPY_CONTROL.rsmpBytes, 1049064);
  assert.equal(
    formatPercent(HIGH_ENTROPY_CONTROL.rsmpBytes, HIGH_ENTROPY_CONTROL.rawB8Bytes, 4),
    "100.0465%",
  );
});

test("projects large shot counts from the 1024-shot evidence only", () => {
  const oneMillion = projectShots(1_000_000);
  assert.equal(formatBytes(oneMillion.rawBytes), "1.516 GB");
  assert.equal(formatBytes(oneMillion.rsmpBytes), "181.668 MB");
  assert.equal(formatDuration(oneMillion.packSeconds), "1m 38s");

  const hundredMillion = projectShots(100_000_000);
  assert.equal(formatBytes(hundredMillion.rsmpBytes), "18.167 GB");
  assert.equal(formatDuration(hundredMillion.packSeconds), "2h 44m");

  const billion = projectShots(1_000_000_000);
  assert.equal(formatBytes(billion.rawBytes), "1.516 TB");
  assert.equal(formatBytes(billion.rsmpBytes), "181.668 GB");
  assert.equal(formatDuration(billion.unpackSeconds), "11h 19m");
});

test("rejects invalid estimator input without NaN or infinity", () => {
  for (const bad of [0, -1, Number.NaN, Number.POSITIVE_INFINITY]) {
    assert.throws(() => projectShots(bad), /positive finite shot count/);
  }
});
