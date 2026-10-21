// Current schema identity and generation consistency, independent of Git history.
export function checkCurrentContract(current) {
  const version = current.$defs.Negotiation.properties.contractVersion.const;
  if (version !== 1) throw new Error("Unsupported current AI contract version");
  function records(node) {
    if (!node || typeof node !== "object") return;
    if (
      node.properties?.schemaVersion &&
      node.properties.schemaVersion.const !== version
    )
      throw new Error("AI record version differs from negotiation");
    for (const child of Object.values(node)) records(child);
  }
  records(current);
  if (current.$id !== `urn:rss-mdm-agent:ai-runtime:${version}`)
    throw new Error("AI schema identity differs from negotiation");
}
