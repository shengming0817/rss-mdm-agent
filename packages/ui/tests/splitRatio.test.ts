import { expect, it } from "vitest";
import {
  clampRatio,
  normalizeMinRatio,
  ratioFromPointer,
} from "../src/internal/splitRatio";
it.each([NaN, Infinity, -Infinity])(
  "normalizes invalid minimum %s",
  (value) => {
    expect(normalizeMinRatio(value)).toBe(0.1);
    expect(clampRatio(0.99, value)).toBe(0.9);
  },
);
it("clamps unusable bounds and positions", () => {
  expect(normalizeMinRatio(-1)).toBe(0);
  expect(normalizeMinRatio(2)).toBe(0.5);
  expect(clampRatio(NaN, 0.1)).toBe(0.1);
  expect(clampRatio(Infinity, 0.1)).toBe(0.9);
  expect(clampRatio(-Infinity, 0.1)).toBe(0.1);
  expect(ratioFromPointer(150, 100, 100, 0.1)).toBe(0.5);
  for (const height of [0, -1, NaN, Infinity])
    expect(ratioFromPointer(150, 100, height, 0.1)).toBe(0.1);
});
