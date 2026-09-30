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
    Digest: {
      description:
        "Lowercase SHA-256 bytes expressed as hex; a digest alone grants no trust.",
      maxLength: 64,
      minLength: 64,
      pattern: "^[0-9a-f]{64}$",
      type: "string",
    },
    Id: {
      description:
        "Opaque local reference identifier; syntax validity is not authenticity.",
      maxLength: 128,
      minLength: 1,
      pattern: "^[A-Za-z0-9][A-Za-z0-9._:/-]*$",
      type: "string",
    },
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
    "A selection references exact backend content and carries no script or local catalog model.",
  properties: {
    attempt: { $ref: "#/$defs/Id", description: "Original backend attempt." },
    request: {
      $ref: "#/$defs/RequestId",
      description:
        "Original request shown with the offer, used unchanged for recovery.",
    },
    revision: {
      $ref: "#/$defs/Digest",
      description: "Revision shown to the user/model.",
    },
    task: { $ref: "#/$defs/Id", description: "Backend task identity." },
  },
  required: ["request", "task", "attempt", "revision"],
  title: "BackendSelection",
  type: "object",
};
const schema32 = {
  description:
    "Opaque local reference identifier; syntax validity is not authenticity.",
  maxLength: 128,
  minLength: 1,
  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/-]*$",
  type: "string",
};
const schema33 = {
  description: "Local execution request identity.",
  maxLength: 128,
  minLength: 1,
  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/-]*$",
  type: "string",
};
const schema34 = {
  description:
    "Lowercase SHA-256 bytes expressed as hex; a digest alone grants no trust.",
  maxLength: 64,
  minLength: 64,
  pattern: "^[0-9a-f]{64}$",
  type: "string",
};
const func1 = helper0;
const pattern4 = new RegExp("^[A-Za-z0-9][A-Za-z0-9._:/-]*$", "u");
const pattern6 = new RegExp("^[0-9a-f]{64}$", "u");
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
        (data.request === undefined && (missing0 = "request")) ||
        (data.task === undefined && (missing0 = "task")) ||
        (data.attempt === undefined && (missing0 = "attempt")) ||
        (data.revision === undefined && (missing0 = "revision"))
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
          if (
            !(
              key0 === "attempt" ||
              key0 === "request" ||
              key0 === "revision" ||
              key0 === "task"
            )
          ) {
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
          if (data.attempt !== undefined) {
            let data0 = data.attempt;
            const _errs2 = errors;
            const _errs3 = errors;
            if (errors === _errs3) {
              if (typeof data0 === "string") {
                if (func1(data0) > 128) {
                  validate20.errors = [
                    {
                      instancePath: instancePath + "/attempt",
                      schemaPath: "#/$defs/Id/maxLength",
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
                        instancePath: instancePath + "/attempt",
                        schemaPath: "#/$defs/Id/minLength",
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
                          instancePath: instancePath + "/attempt",
                          schemaPath: "#/$defs/Id/pattern",
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
                    instancePath: instancePath + "/attempt",
                    schemaPath: "#/$defs/Id/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.request !== undefined) {
              let data1 = data.request;
              const _errs5 = errors;
              const _errs6 = errors;
              if (errors === _errs6) {
                if (typeof data1 === "string") {
                  if (func1(data1) > 128) {
                    validate20.errors = [
                      {
                        instancePath: instancePath + "/request",
                        schemaPath: "#/$defs/RequestId/maxLength",
                        keyword: "maxLength",
                        params: { limit: 128 },
                        message: "must NOT have more than 128 characters",
                      },
                    ];
                    return false;
                  } else {
                    if (func1(data1) < 1) {
                      validate20.errors = [
                        {
                          instancePath: instancePath + "/request",
                          schemaPath: "#/$defs/RequestId/minLength",
                          keyword: "minLength",
                          params: { limit: 1 },
                          message: "must NOT have fewer than 1 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (!pattern4.test(data1)) {
                        validate20.errors = [
                          {
                            instancePath: instancePath + "/request",
                            schemaPath: "#/$defs/RequestId/pattern",
                            keyword: "pattern",
                            params: {
                              pattern: "^[A-Za-z0-9][A-Za-z0-9._:/-]*$",
                            },
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
                      instancePath: instancePath + "/request",
                      schemaPath: "#/$defs/RequestId/type",
                      keyword: "type",
                      params: { type: "string" },
                      message: "must be string",
                    },
                  ];
                  return false;
                }
              }
              var valid0 = _errs5 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.revision !== undefined) {
                let data2 = data.revision;
                const _errs8 = errors;
                const _errs9 = errors;
                if (errors === _errs9) {
                  if (typeof data2 === "string") {
                    if (func1(data2) > 64) {
                      validate20.errors = [
                        {
                          instancePath: instancePath + "/revision",
                          schemaPath: "#/$defs/Digest/maxLength",
                          keyword: "maxLength",
                          params: { limit: 64 },
                          message: "must NOT have more than 64 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (func1(data2) < 64) {
                        validate20.errors = [
                          {
                            instancePath: instancePath + "/revision",
                            schemaPath: "#/$defs/Digest/minLength",
                            keyword: "minLength",
                            params: { limit: 64 },
                            message: "must NOT have fewer than 64 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (!pattern6.test(data2)) {
                          validate20.errors = [
                            {
                              instancePath: instancePath + "/revision",
                              schemaPath: "#/$defs/Digest/pattern",
                              keyword: "pattern",
                              params: { pattern: "^[0-9a-f]{64}$" },
                              message:
                                'must match pattern "' + "^[0-9a-f]{64}$" + '"',
                            },
                          ];
                          return false;
                        }
                      }
                    }
                  } else {
                    validate20.errors = [
                      {
                        instancePath: instancePath + "/revision",
                        schemaPath: "#/$defs/Digest/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      },
                    ];
                    return false;
                  }
                }
                var valid0 = _errs8 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.task !== undefined) {
                  let data3 = data.task;
                  const _errs11 = errors;
                  const _errs12 = errors;
                  if (errors === _errs12) {
                    if (typeof data3 === "string") {
                      if (func1(data3) > 128) {
                        validate20.errors = [
                          {
                            instancePath: instancePath + "/task",
                            schemaPath: "#/$defs/Id/maxLength",
                            keyword: "maxLength",
                            params: { limit: 128 },
                            message: "must NOT have more than 128 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (func1(data3) < 1) {
                          validate20.errors = [
                            {
                              instancePath: instancePath + "/task",
                              schemaPath: "#/$defs/Id/minLength",
                              keyword: "minLength",
                              params: { limit: 1 },
                              message: "must NOT have fewer than 1 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (!pattern4.test(data3)) {
                            validate20.errors = [
                              {
                                instancePath: instancePath + "/task",
                                schemaPath: "#/$defs/Id/pattern",
                                keyword: "pattern",
                                params: {
                                  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/-]*$",
                                },
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
                          instancePath: instancePath + "/task",
                          schemaPath: "#/$defs/Id/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                  }
                  var valid0 = _errs11 === errors;
                } else {
                  var valid0 = true;
                }
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
