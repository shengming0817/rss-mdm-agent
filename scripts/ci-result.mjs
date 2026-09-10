export function stepResult(name, command, result) {
  return {
    name,
    command,
    status: result.status,
    signal: result.signal,
    error:
      result.error?.message ??
      (result.signal
        ? `terminated by signal ${result.signal}`
        : result.status === null
          ? "process ended without an exit status"
          : undefined),
  };
}
