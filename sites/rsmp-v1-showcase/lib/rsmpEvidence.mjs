export const MEASURED_CASE = Object.freeze({
  caseId: "stim_surface_d11_r100",
  circuit: "surface-code d=11, rounds=100",
  shots: 1024,
  measurements: 12121,
  detectors: 12000,
  observables: 1,
  rank: 12000,
  freeWidth: 121,
  detectorDensityPercent: 1.5214,
  rawB8Bytes: 1552384,
  directZstdBytes: 325565,
  rsmpBytes: 186028,
  packThroughputBytesPerSecond: 15443821,
  unpackThroughputBytesPerSecond: 37218025,
  logicalBlockWorkingSetBytes: 11900649,
  packShotsPerSecond: 10187,
  unpackShotsPerSecond: 24550,
});

export const HIGH_ENTROPY_CONTROL = Object.freeze({
  label: "high_entropy_control",
  shots: 8192,
  rawB8Bytes: 1048576,
  directZstdBytes: 1048616,
  rsmpBytes: 1049064,
});

export const READINESS_DISCLOSURE = Object.freeze({
  cleanCheckout:
    "The complete 19-command readiness suite passes in a clean exported checkout.",
  currentTree:
    "An ordinary working tree can fail evidence validation when a different current target/release/rstim exists at the historical producer path.",
  interpretation:
    "That is an existence-dependent, non-hermetic checker behavior, not a recomputation failure of the committed compression arithmetic.",
});

export const EVIDENCE_COUNTS = Object.freeze({
  semanticRoles: 7,
  namedCorruptionRecipes: 27,
  generatedTruncations: 491,
  generatedBitFlips: 8,
  compatibilityFixtures: 1,
});

export const LINKS = Object.freeze({
  repository: "https://github.com/nzy1997/rstim",
  format: "https://github.com/nzy1997/rstim/blob/master/rstim/doc/rsmp-v1.md",
  cliGuide: "https://github.com/nzy1997/rstim/blob/master/rstim/doc/rsmp-cli.md",
  evidence:
    "https://github.com/nzy1997/rstim/tree/master/benchmarks/rstim_vs_stim_simulator/results/rsmp-v1",
  testReport:
    "https://github.com/nzy1997/rstim/blob/master/docs/test-reports/test-feature-20260724-rsmp-v1.md",
});

export function projectShots(shots) {
  if (!Number.isFinite(shots) || shots <= 0) {
    throw new RangeError("Expected a positive finite shot count.");
  }
  const scale = shots / MEASURED_CASE.shots;
  const rawBytes = MEASURED_CASE.rawB8Bytes * scale;
  const zstdBytes = MEASURED_CASE.directZstdBytes * scale;
  const rsmpBytes = MEASURED_CASE.rsmpBytes * scale;
  return {
    shots,
    rawBytes,
    zstdBytes,
    rsmpBytes,
    packSeconds: rawBytes / MEASURED_CASE.packThroughputBytesPerSecond,
    unpackSeconds: rawBytes / MEASURED_CASE.unpackThroughputBytesPerSecond,
    rawSavingsBytes: rawBytes - rsmpBytes,
    zstdSavingsBytes: zstdBytes - rsmpBytes,
    rsmpToRawPercent: (rsmpBytes / rawBytes) * 100,
    rsmpToZstdPercent: (rsmpBytes / zstdBytes) * 100,
  };
}

export function formatPercent(numerator, denominator, digits = 2) {
  if (!Number.isFinite(numerator) || !Number.isFinite(denominator) || denominator === 0) {
    throw new RangeError("Expected finite percent inputs with a nonzero denominator.");
  }
  return `${((numerator / denominator) * 100).toFixed(digits)}%`;
}

export function formatBytes(bytes) {
  if (!Number.isFinite(bytes) || bytes < 0) {
    throw new RangeError("Expected a finite nonnegative byte count.");
  }
  const units = ["B", "kB", "MB", "GB", "TB", "PB"];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1000 && unitIndex < units.length - 1) {
    value /= 1000;
    unitIndex += 1;
  }
  if (unitIndex === 0) {
    return `${Math.round(value).toLocaleString("en-US")} B`;
  }
  return `${value.toFixed(3)} ${units[unitIndex]}`;
}

export function formatInteger(value) {
  if (!Number.isFinite(value)) {
    throw new RangeError("Expected a finite number.");
  }
  return Math.round(value).toLocaleString("en-US");
}

export function formatDuration(seconds) {
  if (!Number.isFinite(seconds) || seconds < 0) {
    throw new RangeError("Expected a finite nonnegative duration.");
  }
  const roundedSeconds = Math.round(seconds);
  if (roundedSeconds < 60) {
    return `${roundedSeconds}s`;
  }
  if (roundedSeconds < 600) {
    const minutes = Math.floor(roundedSeconds / 60);
    const secondsRemainder = roundedSeconds % 60;
    return `${minutes}m ${secondsRemainder}s`;
  }
  const roundedMinutes = Math.round(seconds / 60);
  const hours = Math.floor(roundedMinutes / 60);
  const minutes = roundedMinutes % 60;
  return `${hours}h ${minutes}m`;
}
