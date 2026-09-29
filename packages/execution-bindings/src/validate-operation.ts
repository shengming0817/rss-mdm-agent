// @ts-nocheck
// Generated from Rust execution-mcp schema. Do not edit.
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
  $defs: {
    RequestId: {
      description: "Local execution request identity.",
      maxLength: 128,
      minLength: 1,
      pattern: "^[A-Za-z0-9][A-Za-z0-9._:/-]*$",
      type: "string",
    },
  },
  $schema: "https://json-schema.org/draft/2020-12/schema",
  additionalProperties: false,
  description:
    "Authorized lookup/cancellation within the host-bound namespace.",
  properties: {
    operationRequestId: {
      $ref: "#/$defs/RequestId",
      description: "Business identity, never inferred from a JSON-RPC ID.",
    },
  },
  required: ["operationRequestId"],
  title: "OperationRequest",
  type: "object",
};
const schema32 = {
  description: "Local execution request identity.",
  maxLength: 128,
  minLength: 1,
  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/-]*$",
  type: "string",
};
const func1 = helper0;
const pattern4 = new RegExp("^[A-Za-z0-9][A-Za-z0-9._:/-]*$", "u");
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
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        data.operationRequestId === undefined &&
        (missing0 = "operationRequestId")
      ) {
        validate20.errors = [
          {
            instancePath,
            schemaPath: "#/required",
            keyword: "required",
            params: { missingProperty: missing0 },
            message: "must have required property '" + missing0 + "'",
          },
        ];
        return false;
      } else {
        const _errs1 = errors;
        for (const key0 in data) {
          if (!(key0 === "operationRequestId")) {
            validate20.errors = [
              {
                instancePath,
                schemaPath: "#/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key0 },
                message: "must NOT have additional properties",
              },
            ];
            return false;
            break;
          }
        }
        if (_errs1 === errors) {
          if (data.operationRequestId !== undefined) {
            let data0 = data.operationRequestId;
            const _errs3 = errors;
            if (errors === _errs3) {
              if (typeof data0 === "string") {
                if (func1(data0) > 128) {
                  validate20.errors = [
                    {
                      instancePath: instancePath + "/operationRequestId",
                      schemaPath: "#/$defs/RequestId/maxLength",
                      keyword: "maxLength",
                      params: { limit: 128 },
                      message: "must NOT have more than 128 characters",
                    },
                  ];
                  return false;
                } else {
                  if (func1(data0) < 1) {
                    validate20.errors = [
                      {
                        instancePath: instancePath + "/operationRequestId",
                        schemaPath: "#/$defs/RequestId/minLength",
                        keyword: "minLength",
                        params: { limit: 1 },
                        message: "must NOT have fewer than 1 characters",
                      },
                    ];
                    return false;
                  } else {
                    if (!pattern4.test(data0)) {
                      validate20.errors = [
                        {
                          instancePath: instancePath + "/operationRequestId",
                          schemaPath: "#/$defs/RequestId/pattern",
                          keyword: "pattern",
                          params: { pattern: "^[A-Za-z0-9][A-Za-z0-9._:/-]*$" },
                          message:
                            'must match pattern "' +
                            "^[A-Za-z0-9][A-Za-z0-9._:/-]*$" +
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
                    instancePath: instancePath + "/operationRequestId",
                    schemaPath: "#/$defs/RequestId/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
            }
          }
        }
      }
    } else {
      validate20.errors = [
        {
          instancePath,
          schemaPath: "#/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        },
      ];
      return false;
    }
  }
  validate20.errors = vErrors;
  return errors === 0;
}
validate20.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
