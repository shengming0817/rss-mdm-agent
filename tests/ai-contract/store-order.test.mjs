import { test } from "node:test";
import {
  MemorySessionStore,
  runStoreConformance,
} from "../../packages/ai-contract/dist/testing/index.js";
test("shared store conformance permits unordered interaction snapshots", () =>
  runStoreConformance(() => {
    const store = new MemorySessionStore(),
      snapshot = store.snapshot.bind(store);
    store.snapshot = async (...args) => {
      const result = await snapshot(...args);
      return result.ok
        ? {
            ok: true,
            value: {
              ...result.value,
              interactions: result.value.interactions.toReversed(),
            },
          }
        : result;
    };
    return store;
  }));
