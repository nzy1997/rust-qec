export interface MeasuredCase {
  caseId: string;
  circuit: string;
  shots: number;
  measurements: number;
  detectors: number;
  observables: number;
  rank: number;
  freeWidth: number;
  detectorDensityPercent: number;
  rawB8Bytes: number;
  directZstdBytes: number;
  rsmpBytes: number;
  packThroughputBytesPerSecond: number;
  unpackThroughputBytesPerSecond: number;
  logicalBlockWorkingSetBytes: number;
  packShotsPerSecond: number;
  unpackShotsPerSecond: number;
}

export interface HighEntropyControl {
  label: string;
  shots: number;
  rawB8Bytes: number;
  directZstdBytes: number;
  rsmpBytes: number;
}

export interface Projection {
  shots: number;
  rawBytes: number;
  zstdBytes: number;
  rsmpBytes: number;
  packSeconds: number;
  unpackSeconds: number;
  rawSavingsBytes: number;
  zstdSavingsBytes: number;
  rsmpToRawPercent: number;
  rsmpToZstdPercent: number;
}

export const MEASURED_CASE: Readonly<MeasuredCase>;
export const HIGH_ENTROPY_CONTROL: Readonly<HighEntropyControl>;
export const READINESS_DISCLOSURE: Readonly<Record<string, string>>;
export const EVIDENCE_COUNTS: Readonly<Record<string, number>>;
export const LINKS: Readonly<Record<string, string>>;

export function projectShots(shots: number): Projection;
export function formatBytes(bytes: number): string;
export function formatDuration(seconds: number): string;
export function formatInteger(value: number): string;
export function formatPercent(numerator: number, denominator: number, digits?: number): string;
