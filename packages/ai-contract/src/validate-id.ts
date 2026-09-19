// @ts-nocheck
// @generated from canonical schemas; do not edit.
const helper0 = function ucs2length(str) {
  const len = str.length;
  let length = 0;
  let pos = 0;
  let value;
  while (pos < len) {
    length++;
    value = str.charCodeAt(pos++);
    if (value >= 0xd800 && value <= 0xdbff && pos < len) {
      // high surrogate, and there is a next character
      value = str.charCodeAt(pos);
      if ((value & 0xfc00) === 0xdc00) pos++; // low surrogate
    }
  }
  return length;
};
("use strict");
export const validate = validate20;
export default validate20;
const schema31 = {
  type: "string",
  minLength: 1,
  maxLength: 128,
  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$",
  description:
    "Opaque ASCII correlation identifier (1–128 characters); never an authentication credential.",
};
const func1 = helper0;
const pattern4 = new RegExp("^[A-Za-z0-9][A-Za-z0-9._:/+-]*$", "u");
function validate20(
  data,
  {
    instancePath = "",
    parentData,
    parentDataProperty,
    rootData = data,
    dynamicAnchors = {},
  } = {},
) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate20.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (typeof data === "string") {
      if (func1(data) > 128) {
        validate20.errors = [
          {
            instancePath,
            schemaPath: "#/maxLength",
            keyword: "maxLength",
            params: { limit: 128 },
            message: "must NOT have more than 128 characters",
          },
        ];
        return false;
      } else {
        if (func1(data) < 1) {
          validate20.errors = [
            {
              instancePath,
              schemaPath: "#/minLength",
              keyword: "minLength",
              params: { limit: 1 },
              message: "must NOT have fewer than 1 characters",
            },
          ];
          return false;
        } else {
          if (!pattern4.test(data)) {
            validate20.errors = [
              {
                instancePath,
                schemaPath: "#/pattern",
                keyword: "pattern",
                params: { pattern: "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" },
                message:
                  'must match pattern "' +
                  "^[A-Za-z0-9][A-Za-z0-9._:/+-]*$" +
                  '"',
              },
            ];
            return false;
          }
        }
      }
    } else {
      validate20.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "string" },
          message: "must be string",
        },
      ];
      return false;
    }
  }
  validate20.errors = vErrors;
  return errors === 0;
}
validate20.evaluated = { dynamicProps: false, dynamicItems: false };
