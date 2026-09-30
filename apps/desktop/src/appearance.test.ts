import { it, expect, vi } from "vitest";
import { createAppearance } from "./appearance";
it("starts solid, uses one in-flight read and ignores a late result after disposal", async () => {
  let finish!: (value: {
    materialEnabled: boolean;
    reducedMotion: boolean;
    highContrast: boolean;
  }) => void;
  const read = vi.fn(
    () =>
      new Promise<{
        materialEnabled: boolean;
        reducedMotion: boolean;
        highContrast: boolean;
      }>((resolve) => {
        finish = resolve;
      }),
  );
  const c = createAppearance(read);
  expect(c.state.materialEnabled).toBe(false);
  const first = c.refresh();
  void c.refresh();
  expect(read).toHaveBeenCalledTimes(1);
  c.dispose();
  finish({ materialEnabled: true, reducedMotion: false, highContrast: false });
  await first;
  expect(c.state.materialEnabled).toBe(false);
});
it("failed reads restore solid even after material was enabled", async () => {
  const read = vi
    .fn()
    .mockResolvedValueOnce({
      materialEnabled: true,
      reducedMotion: false,
      highContrast: false,
    })
    .mockRejectedValueOnce(new Error("unavailable"));
  const c = createAppearance(read);
  await c.refresh();
  expect(c.state.materialEnabled).toBe(true);
  await c.refresh();
  expect(c.state.materialEnabled).toBe(false);
  c.dispose();
});
