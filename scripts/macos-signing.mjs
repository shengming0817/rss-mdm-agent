// ref: Apple TN2206, Code Signing In Depth — designated requirements and self-signed identities.
export function macosSigningArguments({
  file,
  identity,
  identifier,
  entitlements,
  timestamp = false,
}) {
  if (!identifier || !/^[A-Za-z0-9][A-Za-z0-9.-]+$/.test(identifier))
    throw new Error("An explicit stable code-signing identifier is required");
  return [
    "--force",
    "--sign",
    identity,
    "--identifier",
    identifier,
    ...(entitlements
      ? ["--entitlements", entitlements]
      : ["--preserve-metadata=entitlements"]),
    ...(identity === "-" ? [] : ["--options", "runtime"]),
    ...(timestamp ? ["--timestamp"] : []),
    file,
  ];
}
