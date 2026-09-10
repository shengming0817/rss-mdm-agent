export const DEFAULT_TOP_RATIO = 0.5;

export const MIN_RATIO = 0.1;

export function normalizeMinRatio(minRatio: number): number {
  if (!Number.isFinite(minRatio)) return MIN_RATIO;
  return Math.min(Math.max(minRatio, 0), 0.5);
}

export function clampRatio(ratio: number, minRatio: number): number {
  const lo = normalizeMinRatio(minRatio);
  const hi = 1 - lo;
  if (Number.isNaN(ratio) || ratio < lo) return lo;
  if (ratio > hi) return hi;
  return ratio;
}

export function ratioFromPointer(
  clientY: number,
  rectTop: number,
  rectHeight: number,
  minRatio: number,
): number {
  if (!Number.isFinite(rectHeight) || rectHeight <= 0)
    return clampRatio(0, minRatio);
  return clampRatio((clientY - rectTop) / rectHeight, minRatio);
}
