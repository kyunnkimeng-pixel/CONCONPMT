export const BYTES_PER_MEGABYTE = 1024 * 1024;

export function bytesToMegabytesInput(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 1) {
    return "";
  }

  const megabytes = bytes / BYTES_PER_MEGABYTE;
  const fractionDigits =
    megabytes >= 1 ? 2 : megabytes >= 0.01 ? 3 : 6;

  return trimTrailingZeros(megabytes.toFixed(fractionDigits));
}

export function megabytesInputToBytes(value: string): number | null {
  const normalized = value.trim().replace(",", ".");

  if (normalized === "") {
    return null;
  }

  if (!/^(?:\d+|\d*\.\d+)$/.test(normalized)) {
    return null;
  }

  const megabytes = Number.parseFloat(normalized);
  if (!Number.isFinite(megabytes) || megabytes <= 0) {
    return null;
  }

  return Math.max(1, Math.round(megabytes * BYTES_PER_MEGABYTE));
}

function trimTrailingZeros(value: string): string {
  return value.replace(/\.?0+$/, "");
}
