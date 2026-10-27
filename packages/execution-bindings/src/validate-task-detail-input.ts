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
    QueryRef: {
      additionalProperties: false,
      description: "Exact service-owned query definition.",
      properties: {
        id: { $ref: "#/$defs/Id", description: "Stable resource identity." },
        revision: {
          $ref: "#/$defs/Digest",
          description: "Digest of the complete current definition.",
        },
      },
      required: ["id", "revision"],
      type: "object",
    },
    RequestId: {
      description: "Local execution request identity.",
      maxLength: 128,
      minLength: 1,
      pattern: "^[A-Za-z0-9][A-Za-z0-9._:/-]*$",
      type: "string",
    },
    TaskRef: {
      description:
        "Exact original owner identity, scoped by the authenticated native connection.",
      oneOf: [
        {
          additionalProperties: false,
          description: "Original execution or pre-execution request.",
          properties: {
            kind: { const: "request", type: "string" },
            request: {
              $ref: "#/$defs/RequestId",
              description: "Original immutable request identity.",
            },
          },
          required: ["kind", "request"],
          type: "object",
        },
        {
          additionalProperties: false,
          description:
            "Original periodic collection definition/session, without generating tasks per sample.",
          properties: {
            kind: { const: "sampling", type: "string" },
            query: {
              $ref: "#/$defs/QueryRef",
              description: "Exact original versioned collection definition.",
            },
          },
          required: ["kind", "query"],
          type: "object",
        },
        {
          additionalProperties: false,
          description: "Source-qualified server activity identity.",
          properties: {
            id: {
              description: "Original producer ID, never a display name.",
              type: "string",
            },
            kind: { const: "activity", type: "string" },
          },
          required: ["kind", "id"],
          type: "object",
        },
      ],
    },
  },
  $schema: "https://json-schema.org/draft/2020-12/schema",
  additionalProperties: false,
  properties: { reference: { $ref: "#/$defs/TaskRef" } },
  required: ["reference"],
  title: "TaskDetailQuery",
  type: "object",
};
const schema32 = {
  description:
    "Exact original owner identity, scoped by the authenticated native connection.",
  oneOf: [
    {
      additionalProperties: false,
      description: "Original execution or pre-execution request.",
      properties: {
        kind: { const: "request", type: "string" },
        request: {
          $ref: "#/$defs/RequestId",
          description: "Original immutable request identity.",
        },
      },
      required: ["kind", "request"],
      type: "object",
    },
    {
      additionalProperties: false,
      description:
        "Original periodic collection definition/session, without generating tasks per sample.",
      properties: {
        kind: { const: "sampling", type: "string" },
        query: {
          $ref: "#/$defs/QueryRef",
          description: "Exact original versioned collection definition.",
        },
      },
      required: ["kind", "query"],
      type: "object",
    },
    {
      additionalProperties: false,
      description: "Source-qualified server activity identity.",
      properties: {
        id: {
          description: "Original producer ID, never a display name.",
          type: "string",
        },
        kind: { const: "activity", type: "string" },
      },
      required: ["kind", "id"],
      type: "object",
    },
  ],
};
const schema33 = {
  description: "Local execution request identity.",
  maxLength: 128,
  minLength: 1,
  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/-]*$",
  type: "string",
};
const func1 = helper0;
const pattern4 = new RegExp("^[A-Za-z0-9][A-Za-z0-9._:/-]*$", "u");
const schema34 = {
  additionalProperties: false,
  description: "Exact service-owned query definition.",
  properties: {
    id: { $ref: "#/$defs/Id", description: "Stable resource identity." },
    revision: {
      $ref: "#/$defs/Digest",
      description: "Digest of the complete current definition.",
    },
  },
  required: ["id", "revision"],
  type: "object",
};
const schema35 = {
  description:
    "Opaque local reference identifier; syntax validity is not authenticity.",
  maxLength: 128,
  minLength: 1,
  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/-]*$",
  type: "string",
};
const schema36 = {
  description:
    "Lowercase SHA-256 bytes expressed as hex; a digest alone grants no trust.",
  maxLength: 64,
  minLength: 64,
  pattern: "^[0-9a-f]{64}$",
  type: "string",
};
const pattern6 = new RegExp("^[0-9a-f]{64}$", "u");
function validate22(
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
  const evaluated0 = validate22.evaluated;
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
        (data.id === undefined && (missing0 = "id")) ||
        (data.revision === undefined && (missing0 = "revision"))
      ) {
        validate22.errors = [
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
          if (!(key0 === "id" || key0 === "revision")) {
            validate22.errors = [
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
          if (data.id !== undefined) {
            let data0 = data.id;
            const _errs2 = errors;
            const _errs3 = errors;
            if (errors === _errs3) {
              if (typeof data0 === "string") {
                if (func1(data0) > 128) {
                  validate22.errors = [
                    {
                      instancePath: instancePath + "/id",
                      schemaPath: "#/$defs/Id/maxLength",
                      keyword: "maxLength",
                      params: { limit: 128 },
                      message: "must NOT have more than 128 characters",
                    },
                  ];
                  return false;
                } else {
                  if (func1(data0) < 1) {
                    validate22.errors = [
                      {
                        instancePath: instancePath + "/id",
                        schemaPath: "#/$defs/Id/minLength",
                        keyword: "minLength",
                        params: { limit: 1 },
                        message: "must NOT have fewer than 1 characters",
                      },
                    ];
                    return false;
                  } else {
                    if (!pattern4.test(data0)) {
                      validate22.errors = [
                        {
                          instancePath: instancePath + "/id",
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
                validate22.errors = [
                  {
                    instancePath: instancePath + "/id",
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
            if (data.revision !== undefined) {
              let data1 = data.revision;
              const _errs5 = errors;
              const _errs6 = errors;
              if (errors === _errs6) {
                if (typeof data1 === "string") {
                  if (func1(data1) > 64) {
                    validate22.errors = [
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
                    if (func1(data1) < 64) {
                      validate22.errors = [
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
                      if (!pattern6.test(data1)) {
                        validate22.errors = [
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
                  validate22.errors = [
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
              var valid0 = _errs5 === errors;
            } else {
              var valid0 = true;
            }
          }
        }
      }
    } else {
      validate22.errors = [
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
  validate22.errors = vErrors;
  return errors === 0;
}
validate22.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
function validate21(
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
  const evaluated0 = validate21.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (errors === _errs1) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.kind === undefined && (missing0 = "kind")) ||
        (data.request === undefined && (missing0 = "request"))
      ) {
        const err0 = {
          instancePath,
          schemaPath: "#/oneOf/0/required",
          keyword: "required",
          params: { missingProperty: missing0 },
          message: "must have required property '" + missing0 + "'",
        };
        if (vErrors === null) {
          vErrors = [err0];
        } else {
          vErrors.push(err0);
        }
        errors++;
      } else {
        const _errs3 = errors;
        for (const key0 in data) {
          if (!(key0 === "kind" || key0 === "request")) {
            const err1 = {
              instancePath,
              schemaPath: "#/oneOf/0/additionalProperties",
              keyword: "additionalProperties",
              params: { additionalProperty: key0 },
              message: "must NOT have additional properties",
            };
            if (vErrors === null) {
              vErrors = [err1];
            } else {
              vErrors.push(err1);
            }
            errors++;
            break;
          }
        }
        if (_errs3 === errors) {
          if (data.kind !== undefined) {
            let data0 = data.kind;
            const _errs4 = errors;
            if (typeof data0 !== "string") {
              const err2 = {
                instancePath: instancePath + "/kind",
                schemaPath: "#/oneOf/0/properties/kind/type",
                keyword: "type",
                params: { type: "string" },
                message: "must be string",
              };
              if (vErrors === null) {
                vErrors = [err2];
              } else {
                vErrors.push(err2);
              }
              errors++;
            }
            if ("request" !== data0) {
              const err3 = {
                instancePath: instancePath + "/kind",
                schemaPath: "#/oneOf/0/properties/kind/const",
                keyword: "const",
                params: { allowedValue: "request" },
                message: "must be equal to constant",
              };
              if (vErrors === null) {
                vErrors = [err3];
              } else {
                vErrors.push(err3);
              }
              errors++;
            }
            var valid1 = _errs4 === errors;
          } else {
            var valid1 = true;
          }
          if (valid1) {
            if (data.request !== undefined) {
              let data1 = data.request;
              const _errs6 = errors;
              const _errs7 = errors;
              if (errors === _errs7) {
                if (typeof data1 === "string") {
                  if (func1(data1) > 128) {
                    const err4 = {
                      instancePath: instancePath + "/request",
                      schemaPath: "#/$defs/RequestId/maxLength",
                      keyword: "maxLength",
                      params: { limit: 128 },
                      message: "must NOT have more than 128 characters",
                    };
                    if (vErrors === null) {
                      vErrors = [err4];
                    } else {
                      vErrors.push(err4);
                    }
                    errors++;
                  } else {
                    if (func1(data1) < 1) {
                      const err5 = {
                        instancePath: instancePath + "/request",
                        schemaPath: "#/$defs/RequestId/minLength",
                        keyword: "minLength",
                        params: { limit: 1 },
                        message: "must NOT have fewer than 1 characters",
                      };
                      if (vErrors === null) {
                        vErrors = [err5];
                      } else {
                        vErrors.push(err5);
                      }
                      errors++;
                    } else {
                      if (!pattern4.test(data1)) {
                        const err6 = {
                          instancePath: instancePath + "/request",
                          schemaPath: "#/$defs/RequestId/pattern",
                          keyword: "pattern",
                          params: { pattern: "^[A-Za-z0-9][A-Za-z0-9._:/-]*$" },
                          message:
                            'must match pattern "' +
                            "^[A-Za-z0-9][A-Za-z0-9._:/-]*$" +
                            '"',
                        };
                        if (vErrors === null) {
                          vErrors = [err6];
                        } else {
                          vErrors.push(err6);
                        }
                        errors++;
                      }
                    }
                  }
                } else {
                  const err7 = {
                    instancePath: instancePath + "/request",
                    schemaPath: "#/$defs/RequestId/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  };
                  if (vErrors === null) {
                    vErrors = [err7];
                  } else {
                    vErrors.push(err7);
                  }
                  errors++;
                }
              }
              var valid1 = _errs6 === errors;
            } else {
              var valid1 = true;
            }
          }
        }
      }
    } else {
      const err8 = {
        instancePath,
        schemaPath: "#/oneOf/0/type",
        keyword: "type",
        params: { type: "object" },
        message: "must be object",
      };
      if (vErrors === null) {
        vErrors = [err8];
      } else {
        vErrors.push(err8);
      }
      errors++;
    }
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
    var props0 = true;
  }
  const _errs9 = errors;
  if (errors === _errs9) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing1;
      if (
        (data.kind === undefined && (missing1 = "kind")) ||
        (data.query === undefined && (missing1 = "query"))
      ) {
        const err9 = {
          instancePath,
          schemaPath: "#/oneOf/1/required",
          keyword: "required",
          params: { missingProperty: missing1 },
          message: "must have required property '" + missing1 + "'",
        };
        if (vErrors === null) {
          vErrors = [err9];
        } else {
          vErrors.push(err9);
        }
        errors++;
      } else {
        const _errs11 = errors;
        for (const key1 in data) {
          if (!(key1 === "kind" || key1 === "query")) {
            const err10 = {
              instancePath,
              schemaPath: "#/oneOf/1/additionalProperties",
              keyword: "additionalProperties",
              params: { additionalProperty: key1 },
              message: "must NOT have additional properties",
            };
            if (vErrors === null) {
              vErrors = [err10];
            } else {
              vErrors.push(err10);
            }
            errors++;
            break;
          }
        }
        if (_errs11 === errors) {
          if (data.kind !== undefined) {
            let data2 = data.kind;
            const _errs12 = errors;
            if (typeof data2 !== "string") {
              const err11 = {
                instancePath: instancePath + "/kind",
                schemaPath: "#/oneOf/1/properties/kind/type",
                keyword: "type",
                params: { type: "string" },
                message: "must be string",
              };
              if (vErrors === null) {
                vErrors = [err11];
              } else {
                vErrors.push(err11);
              }
              errors++;
            }
            if ("sampling" !== data2) {
              const err12 = {
                instancePath: instancePath + "/kind",
                schemaPath: "#/oneOf/1/properties/kind/const",
                keyword: "const",
                params: { allowedValue: "sampling" },
                message: "must be equal to constant",
              };
              if (vErrors === null) {
                vErrors = [err12];
              } else {
                vErrors.push(err12);
              }
              errors++;
            }
            var valid3 = _errs12 === errors;
          } else {
            var valid3 = true;
          }
          if (valid3) {
            if (data.query !== undefined) {
              const _errs14 = errors;
              if (
                !validate22(data.query, {
                  instancePath: instancePath + "/query",
                  parentData: data,
                  parentDataProperty: "query",
                  rootData,
                  dynamicAnchors,
                })
              ) {
                vErrors =
                  vErrors === null
                    ? validate22.errors
                    : vErrors.concat(validate22.errors);
                errors = vErrors.length;
              }
              var valid3 = _errs14 === errors;
            } else {
              var valid3 = true;
            }
          }
        }
      }
    } else {
      const err13 = {
        instancePath,
        schemaPath: "#/oneOf/1/type",
        keyword: "type",
        params: { type: "object" },
        message: "must be object",
      };
      if (vErrors === null) {
        vErrors = [err13];
      } else {
        vErrors.push(err13);
      }
      errors++;
    }
  }
  var _valid0 = _errs9 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      if (props0 !== true) {
        props0 = true;
      }
    }
    const _errs15 = errors;
    if (errors === _errs15) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        let missing2;
        if (
          (data.kind === undefined && (missing2 = "kind")) ||
          (data.id === undefined && (missing2 = "id"))
        ) {
          const err14 = {
            instancePath,
            schemaPath: "#/oneOf/2/required",
            keyword: "required",
            params: { missingProperty: missing2 },
            message: "must have required property '" + missing2 + "'",
          };
          if (vErrors === null) {
            vErrors = [err14];
          } else {
            vErrors.push(err14);
          }
          errors++;
        } else {
          const _errs17 = errors;
          for (const key2 in data) {
            if (!(key2 === "id" || key2 === "kind")) {
              const err15 = {
                instancePath,
                schemaPath: "#/oneOf/2/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key2 },
                message: "must NOT have additional properties",
              };
              if (vErrors === null) {
                vErrors = [err15];
              } else {
                vErrors.push(err15);
              }
              errors++;
              break;
            }
          }
          if (_errs17 === errors) {
            if (data.id !== undefined) {
              const _errs18 = errors;
              if (typeof data.id !== "string") {
                const err16 = {
                  instancePath: instancePath + "/id",
                  schemaPath: "#/oneOf/2/properties/id/type",
                  keyword: "type",
                  params: { type: "string" },
                  message: "must be string",
                };
                if (vErrors === null) {
                  vErrors = [err16];
                } else {
                  vErrors.push(err16);
                }
                errors++;
              }
              var valid4 = _errs18 === errors;
            } else {
              var valid4 = true;
            }
            if (valid4) {
              if (data.kind !== undefined) {
                let data5 = data.kind;
                const _errs20 = errors;
                if (typeof data5 !== "string") {
                  const err17 = {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/oneOf/2/properties/kind/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  };
                  if (vErrors === null) {
                    vErrors = [err17];
                  } else {
                    vErrors.push(err17);
                  }
                  errors++;
                }
                if ("activity" !== data5) {
                  const err18 = {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/oneOf/2/properties/kind/const",
                    keyword: "const",
                    params: { allowedValue: "activity" },
                    message: "must be equal to constant",
                  };
                  if (vErrors === null) {
                    vErrors = [err18];
                  } else {
                    vErrors.push(err18);
                  }
                  errors++;
                }
                var valid4 = _errs20 === errors;
              } else {
                var valid4 = true;
              }
            }
          }
        }
      } else {
        const err19 = {
          instancePath,
          schemaPath: "#/oneOf/2/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        };
        if (vErrors === null) {
          vErrors = [err19];
        } else {
          vErrors.push(err19);
        }
        errors++;
      }
    }
    var _valid0 = _errs15 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
        if (props0 !== true) {
          props0 = true;
        }
      }
    }
  }
  if (!valid0) {
    const err20 = {
      instancePath,
      schemaPath: "#/oneOf",
      keyword: "oneOf",
      params: { passingSchemas: passing0 },
      message: "must match exactly one schema in oneOf",
    };
    if (vErrors === null) {
      vErrors = [err20];
    } else {
      vErrors.push(err20);
    }
    errors++;
    validate21.errors = vErrors;
    return false;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate21.errors = vErrors;
  evaluated0.props = props0;
  return errors === 0;
}
validate21.evaluated = { dynamicProps: true, dynamicItems: false };
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
      if (data.reference === undefined && (missing0 = "reference")) {
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
          if (!(key0 === "reference")) {
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
          if (data.reference !== undefined) {
            if (
              !validate21(data.reference, {
                instancePath: instancePath + "/reference",
                parentData: data,
                parentDataProperty: "reference",
                rootData,
                dynamicAnchors,
              })
            ) {
              vErrors =
                vErrors === null
                  ? validate21.errors
                  : vErrors.concat(validate21.errors);
              errors = vErrors.length;
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
