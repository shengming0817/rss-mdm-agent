export const toolCatalog = (names) => async () => ({
  ok: true,
  value: names.map((name) => ({
    name,
    inputSchema: { type: "object", additionalProperties: true },
  })),
});
