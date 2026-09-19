/** A bounded display model for ordinary structured questions; it carries no approval semantics. */
export interface Question {
  question: string;
  header: string;
  options: { label: string; description: string }[];
  multiSelect: boolean;
}
/** Unsupported provider payloads remain display-only. Provider adapters still validate answers. */
export function questions(
  request: Record<string, unknown>,
): Question[] | undefined {
  const rows = request.questions;
  if (!Array.isArray(rows) || rows.length < 1 || rows.length > 4) return;
  const result: Question[] = [];
  for (const row of rows) {
    if (
      !row ||
      typeof row !== "object" ||
      typeof row.question !== "string" ||
      !row.question ||
      row.question.length > 8192 ||
      typeof row.header !== "string" ||
      row.header.length > 128 ||
      typeof row.multiSelect !== "boolean" ||
      !Array.isArray(row.options) ||
      row.options.length < 2 ||
      row.options.length > 4
    )
      return;
    if (result.some((q) => q.question === row.question)) return;
    const options: Question["options"] = [];
    for (const option of row.options) {
      if (
        !option ||
        typeof option.label !== "string" ||
        !option.label ||
        option.label.length > 1024 ||
        typeof option.description !== "string" ||
        option.description.length > 8192
      )
        return;
      options.push({ label: option.label, description: option.description });
    }
    result.push({
      question: row.question,
      header: row.header,
      options,
      multiSelect: row.multiSelect,
    });
  }
  return result;
}
