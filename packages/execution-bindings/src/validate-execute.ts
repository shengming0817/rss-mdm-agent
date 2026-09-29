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
    Authority: {
      description:
        "Separate namespaces: none of these serializable references authenticates its issuer.",
      oneOf: [
        {
          additionalProperties: false,
          description:
            "Local authority namespace, established and verified by a later trusted host.",
          properties: {
            id: {
              $ref: "#/$defs/Id",
              description:
                "Authority reference in this variant namespace; no proof of issuer authenticity.",
            },
            kind: { const: "local", type: "string" },
          },
          required: ["kind", "id"],
          type: "object",
        },
        {
          additionalProperties: false,
          description:
            "Enterprise authority with an explicit tenant; verified by the product identity owner.",
          properties: {
            id: {
              $ref: "#/$defs/Id",
              description:
                "Authority reference in this variant namespace; no proof of issuer authenticity.",
            },
            kind: { const: "enterprise", type: "string" },
            tenant: {
              $ref: "#/$defs/Id",
              description:
                "Explicit enterprise tenant reference; local/test authority never fabricates a tenant.",
            },
          },
          required: ["kind", "id", "tenant"],
          type: "object",
        },
        {
          additionalProperties: false,
          description:
            "Explicit test-only authority; cannot issue production or real-platform evidence.",
          properties: {
            id: {
              $ref: "#/$defs/Id",
              description:
                "Authority reference in this variant namespace; no proof of issuer authenticity.",
            },
            kind: { const: "test", type: "string" },
          },
          required: ["kind", "id"],
          type: "object",
        },
      ],
    },
    CatalogInput: {
      additionalProperties: false,
      description:
        "Exact catalog selection. The arguments retain original numeric tokens until C03 validates them.",
      properties: {
        arguments: {
          additionalProperties: true,
          description:
            "Original parameter object; schema/rules are obtained from execution_catalog.",
          type: "object",
        },
        catalog: {
          $ref: "#/$defs/CatalogRef",
          description: "Exact catalog identity and digest.",
        },
        itemId: { $ref: "#/$defs/Id", description: "Catalog item ID." },
        operationRequestId: {
          $ref: "#/$defs/RequestId",
          description:
            "Stable business identity, independent of the MCP request ID.",
        },
        variantId: { $ref: "#/$defs/Id", description: "Operation variant ID." },
      },
      required: [
        "operationRequestId",
        "catalog",
        "itemId",
        "variantId",
        "arguments",
      ],
      type: "object",
    },
    CatalogRef: {
      additionalProperties: false,
      description:
        "Exact catalog content identity. It is neither a signature nor an authorization token.",
      properties: {
        authority: {
          $ref: "#/$defs/Authority",
          description:
            "Complete namespace including enterprise tenant when applicable.",
        },
        digest: {
          $ref: "#/$defs/Digest",
          description: "Computed canonical catalog SHA-256.",
        },
        identity: {
          $ref: "#/$defs/VersionedRef",
          description:
            "Catalog ID/revision, not resource version or format version.",
        },
      },
      required: ["authority", "identity", "digest"],
      type: "object",
    },
    Digest: {
      description:
        "Lowercase SHA-256 bytes expressed as hex; a digest alone grants no trust.",
      maxLength: 64,
      minLength: 64,
      pattern: "^[0-9a-f]{64}$",
      type: "string",
    },
    ExactArtifactRef: {
      additionalProperties: false,
      description:
        "Resource revision and exact SHA-256 content identity; does not itself verify downloaded bytes.",
      properties: {
        resource: {
          $ref: "#/$defs/VersionedRef",
          description:
            "Exact resource ID and revision; no latest-version lookup is performed here.",
        },
        sha256: {
          $ref: "#/$defs/Digest",
          description:
            "Expected content SHA-256. The adapter must verify the actual bytes before use.",
        },
      },
      required: ["resource", "sha256"],
      type: "object",
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
    VersionedRef: {
      additionalProperties: false,
      description:
        "Exact resource/configuration reference. Resolution and authenticity belong to the owner.",
      properties: {
        id: {
          $ref: "#/$defs/Id",
          description:
            "Opaque reference identity; the revision must be supplied separately.",
        },
        revision: {
          $ref: "#/$defs/Id",
          description:
            "Exact immutable revision reference; does not resolve or follow a moving alias.",
        },
      },
      required: ["id", "revision"],
      type: "object",
    },
  },
  $schema: "https://json-schema.org/draft/2020-12/schema",
  description:
    "Untrusted proposal input; there is no approval, actor or permission field.",
  oneOf: [
    {
      additionalProperties: false,
      description: "A directory operation using the shared parameter grammar.",
      properties: {
        catalog: {
          additionalProperties: false,
          properties: {
            selection: {
              $ref: "#/$defs/CatalogInput",
              description: "Exact selection.",
            },
          },
          required: ["selection"],
          type: "object",
        },
      },
      required: ["catalog"],
      type: "object",
    },
    {
      additionalProperties: false,
      description:
        "New UTF-8 source. The service creates the immutable artifact; the adapter never writes it.",
      properties: {
        script: {
          additionalProperties: false,
          properties: {
            interpreter: {
              $ref: "#/$defs/ExactArtifactRef",
              description:
                "Explicit interpreter identity, subject to service validation.",
            },
            operationRequestId: {
              $ref: "#/$defs/RequestId",
              description: "Stable operation identity.",
            },
            sourceUtf8: {
              description: "Original UTF-8 script, never executed or echoed.",
              type: "string",
            },
          },
          required: ["operationRequestId", "sourceUtf8", "interpreter"],
          type: "object",
        },
      },
      required: ["script"],
      type: "object",
    },
  ],
  title: "ExecuteInput",
  type: "object",
};
const schema44 = {
  description: "Local execution request identity.",
  maxLength: 128,
  minLength: 1,
  pattern: "^[A-Za-z0-9][A-Za-z0-9._:/-]*$",
  type: "string",
};
const schema32 = {
  additionalProperties: false,
  description:
    "Exact catalog selection. The arguments retain original numeric tokens until C03 validates them.",
  properties: {
    arguments: {
      additionalProperties: true,
      description:
        "Original parameter object; schema/rules are obtained from execution_catalog.",
      type: "object",
    },
    catalog: {
      $ref: "#/$defs/CatalogRef",
      description: "Exact catalog identity and digest.",
    },
    itemId: { $ref: "#/$defs/Id", description: "Catalog item ID." },
    operationRequestId: {
      $ref: "#/$defs/RequestId",
      description:
        "Stable business identity, independent of the MCP request ID.",
    },
    variantId: { $ref: "#/$defs/Id", description: "Operation variant ID." },
  },
  required: [
    "operationRequestId",
    "catalog",
    "itemId",
    "variantId",
    "arguments",
  ],
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
const schema33 = {
  additionalProperties: false,
  description:
    "Exact catalog content identity. It is neither a signature nor an authorization token.",
  properties: {
    authority: {
      $ref: "#/$defs/Authority",
      description:
        "Complete namespace including enterprise tenant when applicable.",
    },
    digest: {
      $ref: "#/$defs/Digest",
      description: "Computed canonical catalog SHA-256.",
    },
    identity: {
      $ref: "#/$defs/VersionedRef",
      description:
        "Catalog ID/revision, not resource version or format version.",
    },
  },
  required: ["authority", "identity", "digest"],
  type: "object",
};
const schema39 = {
  description:
    "Lowercase SHA-256 bytes expressed as hex; a digest alone grants no trust.",
  maxLength: 64,
  minLength: 64,
  pattern: "^[0-9a-f]{64}$",
  type: "string",
};
const schema34 = {
  description:
    "Separate namespaces: none of these serializable references authenticates its issuer.",
  oneOf: [
    {
      additionalProperties: false,
      description:
        "Local authority namespace, established and verified by a later trusted host.",
      properties: {
        id: {
          $ref: "#/$defs/Id",
          description:
            "Authority reference in this variant namespace; no proof of issuer authenticity.",
        },
        kind: { const: "local", type: "string" },
      },
      required: ["kind", "id"],
      type: "object",
    },
    {
      additionalProperties: false,
      description:
        "Enterprise authority with an explicit tenant; verified by the product identity owner.",
      properties: {
        id: {
          $ref: "#/$defs/Id",
          description:
            "Authority reference in this variant namespace; no proof of issuer authenticity.",
        },
        kind: { const: "enterprise", type: "string" },
        tenant: {
          $ref: "#/$defs/Id",
          description:
            "Explicit enterprise tenant reference; local/test authority never fabricates a tenant.",
        },
      },
      required: ["kind", "id", "tenant"],
      type: "object",
    },
    {
      additionalProperties: false,
      description:
        "Explicit test-only authority; cannot issue production or real-platform evidence.",
      properties: {
        id: {
          $ref: "#/$defs/Id",
          description:
            "Authority reference in this variant namespace; no proof of issuer authenticity.",
        },
        kind: { const: "test", type: "string" },
      },
      required: ["kind", "id"],
      type: "object",
    },
  ],
};
const func1 = helper0;
const pattern4 = new RegExp("^[A-Za-z0-9][A-Za-z0-9._:/-]*$", "u");
function validate23(
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
  const evaluated0 = validate23.evaluated;
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
        (data.id === undefined && (missing0 = "id"))
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
          if (!(key0 === "id" || key0 === "kind")) {
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
          if (data.id !== undefined) {
            let data0 = data.id;
            const _errs4 = errors;
            const _errs5 = errors;
            if (errors === _errs5) {
              if (typeof data0 === "string") {
                if (func1(data0) > 128) {
                  const err2 = {
                    instancePath: instancePath + "/id",
                    schemaPath: "#/$defs/Id/maxLength",
                    keyword: "maxLength",
                    params: { limit: 128 },
                    message: "must NOT have more than 128 characters",
                  };
                  if (vErrors === null) {
                    vErrors = [err2];
                  } else {
                    vErrors.push(err2);
                  }
                  errors++;
                } else {
                  if (func1(data0) < 1) {
                    const err3 = {
                      instancePath: instancePath + "/id",
                      schemaPath: "#/$defs/Id/minLength",
                      keyword: "minLength",
                      params: { limit: 1 },
                      message: "must NOT have fewer than 1 characters",
                    };
                    if (vErrors === null) {
                      vErrors = [err3];
                    } else {
                      vErrors.push(err3);
                    }
                    errors++;
                  } else {
                    if (!pattern4.test(data0)) {
                      const err4 = {
                        instancePath: instancePath + "/id",
                        schemaPath: "#/$defs/Id/pattern",
                        keyword: "pattern",
                        params: { pattern: "^[A-Za-z0-9][A-Za-z0-9._:/-]*$" },
                        message:
                          'must match pattern "' +
                          "^[A-Za-z0-9][A-Za-z0-9._:/-]*$" +
                          '"',
                      };
                      if (vErrors === null) {
                        vErrors = [err4];
                      } else {
                        vErrors.push(err4);
                      }
                      errors++;
                    }
                  }
                }
              } else {
                const err5 = {
                  instancePath: instancePath + "/id",
                  schemaPath: "#/$defs/Id/type",
                  keyword: "type",
                  params: { type: "string" },
                  message: "must be string",
                };
                if (vErrors === null) {
                  vErrors = [err5];
                } else {
                  vErrors.push(err5);
                }
                errors++;
              }
            }
            var valid1 = _errs4 === errors;
          } else {
            var valid1 = true;
          }
          if (valid1) {
            if (data.kind !== undefined) {
              let data1 = data.kind;
              const _errs7 = errors;
              if (typeof data1 !== "string") {
                const err6 = {
                  instancePath: instancePath + "/kind",
                  schemaPath: "#/oneOf/0/properties/kind/type",
                  keyword: "type",
                  params: { type: "string" },
                  message: "must be string",
                };
                if (vErrors === null) {
                  vErrors = [err6];
                } else {
                  vErrors.push(err6);
                }
                errors++;
              }
              if ("local" !== data1) {
                const err7 = {
                  instancePath: instancePath + "/kind",
                  schemaPath: "#/oneOf/0/properties/kind/const",
                  keyword: "const",
                  params: { allowedValue: "local" },
                  message: "must be equal to constant",
                };
                if (vErrors === null) {
                  vErrors = [err7];
                } else {
                  vErrors.push(err7);
                }
                errors++;
              }
              var valid1 = _errs7 === errors;
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
        (data.id === undefined && (missing1 = "id")) ||
        (data.tenant === undefined && (missing1 = "tenant"))
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
          if (!(key1 === "id" || key1 === "kind" || key1 === "tenant")) {
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
          if (data.id !== undefined) {
            let data2 = data.id;
            const _errs12 = errors;
            const _errs13 = errors;
            if (errors === _errs13) {
              if (typeof data2 === "string") {
                if (func1(data2) > 128) {
                  const err11 = {
                    instancePath: instancePath + "/id",
                    schemaPath: "#/$defs/Id/maxLength",
                    keyword: "maxLength",
                    params: { limit: 128 },
                    message: "must NOT have more than 128 characters",
                  };
                  if (vErrors === null) {
                    vErrors = [err11];
                  } else {
                    vErrors.push(err11);
                  }
                  errors++;
                } else {
                  if (func1(data2) < 1) {
                    const err12 = {
                      instancePath: instancePath + "/id",
                      schemaPath: "#/$defs/Id/minLength",
                      keyword: "minLength",
                      params: { limit: 1 },
                      message: "must NOT have fewer than 1 characters",
                    };
                    if (vErrors === null) {
                      vErrors = [err12];
                    } else {
                      vErrors.push(err12);
                    }
                    errors++;
                  } else {
                    if (!pattern4.test(data2)) {
                      const err13 = {
                        instancePath: instancePath + "/id",
                        schemaPath: "#/$defs/Id/pattern",
                        keyword: "pattern",
                        params: { pattern: "^[A-Za-z0-9][A-Za-z0-9._:/-]*$" },
                        message:
                          'must match pattern "' +
                          "^[A-Za-z0-9][A-Za-z0-9._:/-]*$" +
                          '"',
                      };
                      if (vErrors === null) {
                        vErrors = [err13];
                      } else {
                        vErrors.push(err13);
                      }
                      errors++;
                    }
                  }
                }
              } else {
                const err14 = {
                  instancePath: instancePath + "/id",
                  schemaPath: "#/$defs/Id/type",
                  keyword: "type",
                  params: { type: "string" },
                  message: "must be string",
                };
                if (vErrors === null) {
                  vErrors = [err14];
                } else {
                  vErrors.push(err14);
                }
                errors++;
              }
            }
            var valid3 = _errs12 === errors;
          } else {
            var valid3 = true;
          }
          if (valid3) {
            if (data.kind !== undefined) {
              let data3 = data.kind;
              const _errs15 = errors;
              if (typeof data3 !== "string") {
                const err15 = {
                  instancePath: instancePath + "/kind",
                  schemaPath: "#/oneOf/1/properties/kind/type",
                  keyword: "type",
                  params: { type: "string" },
                  message: "must be string",
                };
                if (vErrors === null) {
                  vErrors = [err15];
                } else {
                  vErrors.push(err15);
                }
                errors++;
              }
              if ("enterprise" !== data3) {
                const err16 = {
                  instancePath: instancePath + "/kind",
                  schemaPath: "#/oneOf/1/properties/kind/const",
                  keyword: "const",
                  params: { allowedValue: "enterprise" },
                  message: "must be equal to constant",
                };
                if (vErrors === null) {
                  vErrors = [err16];
                } else {
                  vErrors.push(err16);
                }
                errors++;
              }
              var valid3 = _errs15 === errors;
            } else {
              var valid3 = true;
            }
            if (valid3) {
              if (data.tenant !== undefined) {
                let data4 = data.tenant;
                const _errs17 = errors;
                const _errs18 = errors;
                if (errors === _errs18) {
                  if (typeof data4 === "string") {
                    if (func1(data4) > 128) {
                      const err17 = {
                        instancePath: instancePath + "/tenant",
                        schemaPath: "#/$defs/Id/maxLength",
                        keyword: "maxLength",
                        params: { limit: 128 },
                        message: "must NOT have more than 128 characters",
                      };
                      if (vErrors === null) {
                        vErrors = [err17];
                      } else {
                        vErrors.push(err17);
                      }
                      errors++;
                    } else {
                      if (func1(data4) < 1) {
                        const err18 = {
                          instancePath: instancePath + "/tenant",
                          schemaPath: "#/$defs/Id/minLength",
                          keyword: "minLength",
                          params: { limit: 1 },
                          message: "must NOT have fewer than 1 characters",
                        };
                        if (vErrors === null) {
                          vErrors = [err18];
                        } else {
                          vErrors.push(err18);
                        }
                        errors++;
                      } else {
                        if (!pattern4.test(data4)) {
                          const err19 = {
                            instancePath: instancePath + "/tenant",
                            schemaPath: "#/$defs/Id/pattern",
                            keyword: "pattern",
                            params: {
                              pattern: "^[A-Za-z0-9][A-Za-z0-9._:/-]*$",
                            },
                            message:
                              'must match pattern "' +
                              "^[A-Za-z0-9][A-Za-z0-9._:/-]*$" +
                              '"',
                          };
                          if (vErrors === null) {
                            vErrors = [err19];
                          } else {
                            vErrors.push(err19);
                          }
                          errors++;
                        }
                      }
                    }
                  } else {
                    const err20 = {
                      instancePath: instancePath + "/tenant",
                      schemaPath: "#/$defs/Id/type",
                      keyword: "type",
                      params: { type: "string" },
                      message: "must be string",
                    };
                    if (vErrors === null) {
                      vErrors = [err20];
                    } else {
                      vErrors.push(err20);
                    }
                    errors++;
                  }
                }
                var valid3 = _errs17 === errors;
              } else {
                var valid3 = true;
              }
            }
          }
        }
      }
    } else {
      const err21 = {
        instancePath,
        schemaPath: "#/oneOf/1/type",
        keyword: "type",
        params: { type: "object" },
        message: "must be object",
      };
      if (vErrors === null) {
        vErrors = [err21];
      } else {
        vErrors.push(err21);
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
    const _errs20 = errors;
    if (errors === _errs20) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        let missing2;
        if (
          (data.kind === undefined && (missing2 = "kind")) ||
          (data.id === undefined && (missing2 = "id"))
        ) {
          const err22 = {
            instancePath,
            schemaPath: "#/oneOf/2/required",
            keyword: "required",
            params: { missingProperty: missing2 },
            message: "must have required property '" + missing2 + "'",
          };
          if (vErrors === null) {
            vErrors = [err22];
          } else {
            vErrors.push(err22);
          }
          errors++;
        } else {
          const _errs22 = errors;
          for (const key2 in data) {
            if (!(key2 === "id" || key2 === "kind")) {
              const err23 = {
                instancePath,
                schemaPath: "#/oneOf/2/additionalProperties",
                keyword: "additionalProperties",
                params: { additionalProperty: key2 },
                message: "must NOT have additional properties",
              };
              if (vErrors === null) {
                vErrors = [err23];
              } else {
                vErrors.push(err23);
              }
              errors++;
              break;
            }
          }
          if (_errs22 === errors) {
            if (data.id !== undefined) {
              let data5 = data.id;
              const _errs23 = errors;
              const _errs24 = errors;
              if (errors === _errs24) {
                if (typeof data5 === "string") {
                  if (func1(data5) > 128) {
                    const err24 = {
                      instancePath: instancePath + "/id",
                      schemaPath: "#/$defs/Id/maxLength",
                      keyword: "maxLength",
                      params: { limit: 128 },
                      message: "must NOT have more than 128 characters",
                    };
                    if (vErrors === null) {
                      vErrors = [err24];
                    } else {
                      vErrors.push(err24);
                    }
                    errors++;
                  } else {
                    if (func1(data5) < 1) {
                      const err25 = {
                        instancePath: instancePath + "/id",
                        schemaPath: "#/$defs/Id/minLength",
                        keyword: "minLength",
                        params: { limit: 1 },
                        message: "must NOT have fewer than 1 characters",
                      };
                      if (vErrors === null) {
                        vErrors = [err25];
                      } else {
                        vErrors.push(err25);
                      }
                      errors++;
                    } else {
                      if (!pattern4.test(data5)) {
                        const err26 = {
                          instancePath: instancePath + "/id",
                          schemaPath: "#/$defs/Id/pattern",
                          keyword: "pattern",
                          params: { pattern: "^[A-Za-z0-9][A-Za-z0-9._:/-]*$" },
                          message:
                            'must match pattern "' +
                            "^[A-Za-z0-9][A-Za-z0-9._:/-]*$" +
                            '"',
                        };
                        if (vErrors === null) {
                          vErrors = [err26];
                        } else {
                          vErrors.push(err26);
                        }
                        errors++;
                      }
                    }
                  }
                } else {
                  const err27 = {
                    instancePath: instancePath + "/id",
                    schemaPath: "#/$defs/Id/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  };
                  if (vErrors === null) {
                    vErrors = [err27];
                  } else {
                    vErrors.push(err27);
                  }
                  errors++;
                }
              }
              var valid6 = _errs23 === errors;
            } else {
              var valid6 = true;
            }
            if (valid6) {
              if (data.kind !== undefined) {
                let data6 = data.kind;
                const _errs26 = errors;
                if (typeof data6 !== "string") {
                  const err28 = {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/oneOf/2/properties/kind/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  };
                  if (vErrors === null) {
                    vErrors = [err28];
                  } else {
                    vErrors.push(err28);
                  }
                  errors++;
                }
                if ("test" !== data6) {
                  const err29 = {
                    instancePath: instancePath + "/kind",
                    schemaPath: "#/oneOf/2/properties/kind/const",
                    keyword: "const",
                    params: { allowedValue: "test" },
                    message: "must be equal to constant",
                  };
                  if (vErrors === null) {
                    vErrors = [err29];
                  } else {
                    vErrors.push(err29);
                  }
                  errors++;
                }
                var valid6 = _errs26 === errors;
              } else {
                var valid6 = true;
              }
            }
          }
        }
      } else {
        const err30 = {
          instancePath,
          schemaPath: "#/oneOf/2/type",
          keyword: "type",
          params: { type: "object" },
          message: "must be object",
        };
        if (vErrors === null) {
          vErrors = [err30];
        } else {
          vErrors.push(err30);
        }
        errors++;
      }
    }
    var _valid0 = _errs20 === errors;
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
    const err31 = {
      instancePath,
      schemaPath: "#/oneOf",
      keyword: "oneOf",
      params: { passingSchemas: passing0 },
      message: "must match exactly one schema in oneOf",
    };
    if (vErrors === null) {
      vErrors = [err31];
    } else {
      vErrors.push(err31);
    }
    errors++;
    validate23.errors = vErrors;
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
  validate23.errors = vErrors;
  evaluated0.props = props0;
  return errors === 0;
}
validate23.evaluated = { dynamicProps: true, dynamicItems: false };
const schema40 = {
  additionalProperties: false,
  description:
    "Exact resource/configuration reference. Resolution and authenticity belong to the owner.",
  properties: {
    id: {
      $ref: "#/$defs/Id",
      description:
        "Opaque reference identity; the revision must be supplied separately.",
    },
    revision: {
      $ref: "#/$defs/Id",
      description:
        "Exact immutable revision reference; does not resolve or follow a moving alias.",
    },
  },
  required: ["id", "revision"],
  type: "object",
};
function validate25(
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
  const evaluated0 = validate25.evaluated;
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
        validate25.errors = [
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
            validate25.errors = [
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
                  validate25.errors = [
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
                    validate25.errors = [
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
                      validate25.errors = [
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
                validate25.errors = [
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
                  if (func1(data1) > 128) {
                    validate25.errors = [
                      {
                        instancePath: instancePath + "/revision",
                        schemaPath: "#/$defs/Id/maxLength",
                        keyword: "maxLength",
                        params: { limit: 128 },
                        message: "must NOT have more than 128 characters",
                      },
                    ];
                    return false;
                  } else {
                    if (func1(data1) < 1) {
                      validate25.errors = [
                        {
                          instancePath: instancePath + "/revision",
                          schemaPath: "#/$defs/Id/minLength",
                          keyword: "minLength",
                          params: { limit: 1 },
                          message: "must NOT have fewer than 1 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (!pattern4.test(data1)) {
                        validate25.errors = [
                          {
                            instancePath: instancePath + "/revision",
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
                  validate25.errors = [
                    {
                      instancePath: instancePath + "/revision",
                      schemaPath: "#/$defs/Id/type",
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
      validate25.errors = [
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
  validate25.errors = vErrors;
  return errors === 0;
}
validate25.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const pattern8 = new RegExp("^[0-9a-f]{64}$", "u");
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
        (data.authority === undefined && (missing0 = "authority")) ||
        (data.identity === undefined && (missing0 = "identity")) ||
        (data.digest === undefined && (missing0 = "digest"))
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
          if (
            !(key0 === "authority" || key0 === "digest" || key0 === "identity")
          ) {
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
          if (data.authority !== undefined) {
            const _errs2 = errors;
            if (
              !validate23(data.authority, {
                instancePath: instancePath + "/authority",
                parentData: data,
                parentDataProperty: "authority",
                rootData,
                dynamicAnchors,
              })
            ) {
              vErrors =
                vErrors === null
                  ? validate23.errors
                  : vErrors.concat(validate23.errors);
              errors = vErrors.length;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.digest !== undefined) {
              let data1 = data.digest;
              const _errs3 = errors;
              const _errs4 = errors;
              if (errors === _errs4) {
                if (typeof data1 === "string") {
                  if (func1(data1) > 64) {
                    validate22.errors = [
                      {
                        instancePath: instancePath + "/digest",
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
                          instancePath: instancePath + "/digest",
                          schemaPath: "#/$defs/Digest/minLength",
                          keyword: "minLength",
                          params: { limit: 64 },
                          message: "must NOT have fewer than 64 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (!pattern8.test(data1)) {
                        validate22.errors = [
                          {
                            instancePath: instancePath + "/digest",
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
                      instancePath: instancePath + "/digest",
                      schemaPath: "#/$defs/Digest/type",
                      keyword: "type",
                      params: { type: "string" },
                      message: "must be string",
                    },
                  ];
                  return false;
                }
              }
              var valid0 = _errs3 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.identity !== undefined) {
                const _errs6 = errors;
                if (
                  !validate25(data.identity, {
                    instancePath: instancePath + "/identity",
                    parentData: data,
                    parentDataProperty: "identity",
                    rootData,
                    dynamicAnchors,
                  })
                ) {
                  vErrors =
                    vErrors === null
                      ? validate25.errors
                      : vErrors.concat(validate25.errors);
                  errors = vErrors.length;
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
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
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.operationRequestId === undefined &&
          (missing0 = "operationRequestId")) ||
        (data.catalog === undefined && (missing0 = "catalog")) ||
        (data.itemId === undefined && (missing0 = "itemId")) ||
        (data.variantId === undefined && (missing0 = "variantId")) ||
        (data.arguments === undefined && (missing0 = "arguments"))
      ) {
        validate21.errors = [
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
              key0 === "arguments" ||
              key0 === "catalog" ||
              key0 === "itemId" ||
              key0 === "operationRequestId" ||
              key0 === "variantId"
            )
          ) {
            validate21.errors = [
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
          if (data.arguments !== undefined) {
            let data0 = data.arguments;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (data0 && typeof data0 == "object" && !Array.isArray(data0)) {
              } else {
                validate21.errors = [
                  {
                    instancePath: instancePath + "/arguments",
                    schemaPath: "#/properties/arguments/type",
                    keyword: "type",
                    params: { type: "object" },
                    message: "must be object",
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
            if (data.catalog !== undefined) {
              const _errs5 = errors;
              if (
                !validate22(data.catalog, {
                  instancePath: instancePath + "/catalog",
                  parentData: data,
                  parentDataProperty: "catalog",
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
              var valid0 = _errs5 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.itemId !== undefined) {
                let data2 = data.itemId;
                const _errs6 = errors;
                const _errs7 = errors;
                if (errors === _errs7) {
                  if (typeof data2 === "string") {
                    if (func1(data2) > 128) {
                      validate21.errors = [
                        {
                          instancePath: instancePath + "/itemId",
                          schemaPath: "#/$defs/Id/maxLength",
                          keyword: "maxLength",
                          params: { limit: 128 },
                          message: "must NOT have more than 128 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (func1(data2) < 1) {
                        validate21.errors = [
                          {
                            instancePath: instancePath + "/itemId",
                            schemaPath: "#/$defs/Id/minLength",
                            keyword: "minLength",
                            params: { limit: 1 },
                            message: "must NOT have fewer than 1 characters",
                          },
                        ];
                        return false;
                      } else {
                        if (!pattern4.test(data2)) {
                          validate21.errors = [
                            {
                              instancePath: instancePath + "/itemId",
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
                    validate21.errors = [
                      {
                        instancePath: instancePath + "/itemId",
                        schemaPath: "#/$defs/Id/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      },
                    ];
                    return false;
                  }
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.operationRequestId !== undefined) {
                  let data3 = data.operationRequestId;
                  const _errs9 = errors;
                  const _errs10 = errors;
                  if (errors === _errs10) {
                    if (typeof data3 === "string") {
                      if (func1(data3) > 128) {
                        validate21.errors = [
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
                        if (func1(data3) < 1) {
                          validate21.errors = [
                            {
                              instancePath:
                                instancePath + "/operationRequestId",
                              schemaPath: "#/$defs/RequestId/minLength",
                              keyword: "minLength",
                              params: { limit: 1 },
                              message: "must NOT have fewer than 1 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (!pattern4.test(data3)) {
                            validate21.errors = [
                              {
                                instancePath:
                                  instancePath + "/operationRequestId",
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
                      validate21.errors = [
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
                  var valid0 = _errs9 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.variantId !== undefined) {
                    let data4 = data.variantId;
                    const _errs12 = errors;
                    const _errs13 = errors;
                    if (errors === _errs13) {
                      if (typeof data4 === "string") {
                        if (func1(data4) > 128) {
                          validate21.errors = [
                            {
                              instancePath: instancePath + "/variantId",
                              schemaPath: "#/$defs/Id/maxLength",
                              keyword: "maxLength",
                              params: { limit: 128 },
                              message: "must NOT have more than 128 characters",
                            },
                          ];
                          return false;
                        } else {
                          if (func1(data4) < 1) {
                            validate21.errors = [
                              {
                                instancePath: instancePath + "/variantId",
                                schemaPath: "#/$defs/Id/minLength",
                                keyword: "minLength",
                                params: { limit: 1 },
                                message:
                                  "must NOT have fewer than 1 characters",
                              },
                            ];
                            return false;
                          } else {
                            if (!pattern4.test(data4)) {
                              validate21.errors = [
                                {
                                  instancePath: instancePath + "/variantId",
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
                        validate21.errors = [
                          {
                            instancePath: instancePath + "/variantId",
                            schemaPath: "#/$defs/Id/type",
                            keyword: "type",
                            params: { type: "string" },
                            message: "must be string",
                          },
                        ];
                        return false;
                      }
                    }
                    var valid0 = _errs12 === errors;
                  } else {
                    var valid0 = true;
                  }
                }
              }
            }
          }
        }
      }
    } else {
      validate21.errors = [
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
  validate21.errors = vErrors;
  return errors === 0;
}
validate21.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema46 = {
  additionalProperties: false,
  description:
    "Resource revision and exact SHA-256 content identity; does not itself verify downloaded bytes.",
  properties: {
    resource: {
      $ref: "#/$defs/VersionedRef",
      description:
        "Exact resource ID and revision; no latest-version lookup is performed here.",
    },
    sha256: {
      $ref: "#/$defs/Digest",
      description:
        "Expected content SHA-256. The adapter must verify the actual bytes before use.",
    },
  },
  required: ["resource", "sha256"],
  type: "object",
};
function validate29(
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
  const evaluated0 = validate29.evaluated;
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
        (data.resource === undefined && (missing0 = "resource")) ||
        (data.sha256 === undefined && (missing0 = "sha256"))
      ) {
        validate29.errors = [
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
          if (!(key0 === "resource" || key0 === "sha256")) {
            validate29.errors = [
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
          if (data.resource !== undefined) {
            const _errs2 = errors;
            if (
              !validate25(data.resource, {
                instancePath: instancePath + "/resource",
                parentData: data,
                parentDataProperty: "resource",
                rootData,
                dynamicAnchors,
              })
            ) {
              vErrors =
                vErrors === null
                  ? validate25.errors
                  : vErrors.concat(validate25.errors);
              errors = vErrors.length;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.sha256 !== undefined) {
              let data1 = data.sha256;
              const _errs3 = errors;
              const _errs4 = errors;
              if (errors === _errs4) {
                if (typeof data1 === "string") {
                  if (func1(data1) > 64) {
                    validate29.errors = [
                      {
                        instancePath: instancePath + "/sha256",
                        schemaPath: "#/$defs/Digest/maxLength",
                        keyword: "maxLength",
                        params: { limit: 64 },
                        message: "must NOT have more than 64 characters",
                      },
                    ];
                    return false;
                  } else {
                    if (func1(data1) < 64) {
                      validate29.errors = [
                        {
                          instancePath: instancePath + "/sha256",
                          schemaPath: "#/$defs/Digest/minLength",
                          keyword: "minLength",
                          params: { limit: 64 },
                          message: "must NOT have fewer than 64 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (!pattern8.test(data1)) {
                        validate29.errors = [
                          {
                            instancePath: instancePath + "/sha256",
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
                  validate29.errors = [
                    {
                      instancePath: instancePath + "/sha256",
                      schemaPath: "#/$defs/Digest/type",
                      keyword: "type",
                      params: { type: "string" },
                      message: "must be string",
                    },
                  ];
                  return false;
                }
              }
              var valid0 = _errs3 === errors;
            } else {
              var valid0 = true;
            }
          }
        }
      }
    } else {
      validate29.errors = [
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
  validate29.errors = vErrors;
  return errors === 0;
}
validate29.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
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
  if (!(data && typeof data == "object" && !Array.isArray(data))) {
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
  const _errs1 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs2 = errors;
  if (errors === _errs2) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.catalog === undefined && (missing0 = "catalog")) {
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
        const _errs4 = errors;
        for (const key0 in data) {
          if (!(key0 === "catalog")) {
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
        if (_errs4 === errors) {
          if (data.catalog !== undefined) {
            let data0 = data.catalog;
            const _errs5 = errors;
            if (errors === _errs5) {
              if (data0 && typeof data0 == "object" && !Array.isArray(data0)) {
                let missing1;
                if (data0.selection === undefined && (missing1 = "selection")) {
                  const err2 = {
                    instancePath: instancePath + "/catalog",
                    schemaPath: "#/oneOf/0/properties/catalog/required",
                    keyword: "required",
                    params: { missingProperty: missing1 },
                    message: "must have required property '" + missing1 + "'",
                  };
                  if (vErrors === null) {
                    vErrors = [err2];
                  } else {
                    vErrors.push(err2);
                  }
                  errors++;
                } else {
                  const _errs7 = errors;
                  for (const key1 in data0) {
                    if (!(key1 === "selection")) {
                      const err3 = {
                        instancePath: instancePath + "/catalog",
                        schemaPath:
                          "#/oneOf/0/properties/catalog/additionalProperties",
                        keyword: "additionalProperties",
                        params: { additionalProperty: key1 },
                        message: "must NOT have additional properties",
                      };
                      if (vErrors === null) {
                        vErrors = [err3];
                      } else {
                        vErrors.push(err3);
                      }
                      errors++;
                      break;
                    }
                  }
                  if (_errs7 === errors) {
                    if (data0.selection !== undefined) {
                      if (
                        !validate21(data0.selection, {
                          instancePath: instancePath + "/catalog/selection",
                          parentData: data0,
                          parentDataProperty: "selection",
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
                const err4 = {
                  instancePath: instancePath + "/catalog",
                  schemaPath: "#/oneOf/0/properties/catalog/type",
                  keyword: "type",
                  params: { type: "object" },
                  message: "must be object",
                };
                if (vErrors === null) {
                  vErrors = [err4];
                } else {
                  vErrors.push(err4);
                }
                errors++;
              }
            }
          }
        }
      }
    } else {
      const err5 = {
        instancePath,
        schemaPath: "#/oneOf/0/type",
        keyword: "type",
        params: { type: "object" },
        message: "must be object",
      };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
  }
  var _valid0 = _errs2 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
    var props0 = true;
  }
  const _errs9 = errors;
  if (errors === _errs9) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing2;
      if (data.script === undefined && (missing2 = "script")) {
        const err6 = {
          instancePath,
          schemaPath: "#/oneOf/1/required",
          keyword: "required",
          params: { missingProperty: missing2 },
          message: "must have required property '" + missing2 + "'",
        };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      } else {
        const _errs11 = errors;
        for (const key2 in data) {
          if (!(key2 === "script")) {
            const err7 = {
              instancePath,
              schemaPath: "#/oneOf/1/additionalProperties",
              keyword: "additionalProperties",
              params: { additionalProperty: key2 },
              message: "must NOT have additional properties",
            };
            if (vErrors === null) {
              vErrors = [err7];
            } else {
              vErrors.push(err7);
            }
            errors++;
            break;
          }
        }
        if (_errs11 === errors) {
          if (data.script !== undefined) {
            let data2 = data.script;
            const _errs12 = errors;
            if (errors === _errs12) {
              if (data2 && typeof data2 == "object" && !Array.isArray(data2)) {
                let missing3;
                if (
                  (data2.operationRequestId === undefined &&
                    (missing3 = "operationRequestId")) ||
                  (data2.sourceUtf8 === undefined &&
                    (missing3 = "sourceUtf8")) ||
                  (data2.interpreter === undefined &&
                    (missing3 = "interpreter"))
                ) {
                  const err8 = {
                    instancePath: instancePath + "/script",
                    schemaPath: "#/oneOf/1/properties/script/required",
                    keyword: "required",
                    params: { missingProperty: missing3 },
                    message: "must have required property '" + missing3 + "'",
                  };
                  if (vErrors === null) {
                    vErrors = [err8];
                  } else {
                    vErrors.push(err8);
                  }
                  errors++;
                } else {
                  const _errs14 = errors;
                  for (const key3 in data2) {
                    if (
                      !(
                        key3 === "interpreter" ||
                        key3 === "operationRequestId" ||
                        key3 === "sourceUtf8"
                      )
                    ) {
                      const err9 = {
                        instancePath: instancePath + "/script",
                        schemaPath:
                          "#/oneOf/1/properties/script/additionalProperties",
                        keyword: "additionalProperties",
                        params: { additionalProperty: key3 },
                        message: "must NOT have additional properties",
                      };
                      if (vErrors === null) {
                        vErrors = [err9];
                      } else {
                        vErrors.push(err9);
                      }
                      errors++;
                      break;
                    }
                  }
                  if (_errs14 === errors) {
                    if (data2.interpreter !== undefined) {
                      const _errs15 = errors;
                      if (
                        !validate29(data2.interpreter, {
                          instancePath: instancePath + "/script/interpreter",
                          parentData: data2,
                          parentDataProperty: "interpreter",
                          rootData,
                          dynamicAnchors,
                        })
                      ) {
                        vErrors =
                          vErrors === null
                            ? validate29.errors
                            : vErrors.concat(validate29.errors);
                        errors = vErrors.length;
                      }
                      var valid4 = _errs15 === errors;
                    } else {
                      var valid4 = true;
                    }
                    if (valid4) {
                      if (data2.operationRequestId !== undefined) {
                        let data4 = data2.operationRequestId;
                        const _errs16 = errors;
                        const _errs17 = errors;
                        if (errors === _errs17) {
                          if (typeof data4 === "string") {
                            if (func1(data4) > 128) {
                              const err10 = {
                                instancePath:
                                  instancePath + "/script/operationRequestId",
                                schemaPath: "#/$defs/RequestId/maxLength",
                                keyword: "maxLength",
                                params: { limit: 128 },
                                message:
                                  "must NOT have more than 128 characters",
                              };
                              if (vErrors === null) {
                                vErrors = [err10];
                              } else {
                                vErrors.push(err10);
                              }
                              errors++;
                            } else {
                              if (func1(data4) < 1) {
                                const err11 = {
                                  instancePath:
                                    instancePath + "/script/operationRequestId",
                                  schemaPath: "#/$defs/RequestId/minLength",
                                  keyword: "minLength",
                                  params: { limit: 1 },
                                  message:
                                    "must NOT have fewer than 1 characters",
                                };
                                if (vErrors === null) {
                                  vErrors = [err11];
                                } else {
                                  vErrors.push(err11);
                                }
                                errors++;
                              } else {
                                if (!pattern4.test(data4)) {
                                  const err12 = {
                                    instancePath:
                                      instancePath +
                                      "/script/operationRequestId",
                                    schemaPath: "#/$defs/RequestId/pattern",
                                    keyword: "pattern",
                                    params: {
                                      pattern: "^[A-Za-z0-9][A-Za-z0-9._:/-]*$",
                                    },
                                    message:
                                      'must match pattern "' +
                                      "^[A-Za-z0-9][A-Za-z0-9._:/-]*$" +
                                      '"',
                                  };
                                  if (vErrors === null) {
                                    vErrors = [err12];
                                  } else {
                                    vErrors.push(err12);
                                  }
                                  errors++;
                                }
                              }
                            }
                          } else {
                            const err13 = {
                              instancePath:
                                instancePath + "/script/operationRequestId",
                              schemaPath: "#/$defs/RequestId/type",
                              keyword: "type",
                              params: { type: "string" },
                              message: "must be string",
                            };
                            if (vErrors === null) {
                              vErrors = [err13];
                            } else {
                              vErrors.push(err13);
                            }
                            errors++;
                          }
                        }
                        var valid4 = _errs16 === errors;
                      } else {
                        var valid4 = true;
                      }
                      if (valid4) {
                        if (data2.sourceUtf8 !== undefined) {
                          const _errs19 = errors;
                          if (typeof data2.sourceUtf8 !== "string") {
                            const err14 = {
                              instancePath: instancePath + "/script/sourceUtf8",
                              schemaPath:
                                "#/oneOf/1/properties/script/properties/sourceUtf8/type",
                              keyword: "type",
                              params: { type: "string" },
                              message: "must be string",
                            };
                            if (vErrors === null) {
                              vErrors = [err14];
                            } else {
                              vErrors.push(err14);
                            }
                            errors++;
                          }
                          var valid4 = _errs19 === errors;
                        } else {
                          var valid4 = true;
                        }
                      }
                    }
                  }
                }
              } else {
                const err15 = {
                  instancePath: instancePath + "/script",
                  schemaPath: "#/oneOf/1/properties/script/type",
                  keyword: "type",
                  params: { type: "object" },
                  message: "must be object",
                };
                if (vErrors === null) {
                  vErrors = [err15];
                } else {
                  vErrors.push(err15);
                }
                errors++;
              }
            }
          }
        }
      }
    } else {
      const err16 = {
        instancePath,
        schemaPath: "#/oneOf/1/type",
        keyword: "type",
        params: { type: "object" },
        message: "must be object",
      };
      if (vErrors === null) {
        vErrors = [err16];
      } else {
        vErrors.push(err16);
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
  }
  if (!valid0) {
    const err17 = {
      instancePath,
      schemaPath: "#/oneOf",
      keyword: "oneOf",
      params: { passingSchemas: passing0 },
      message: "must match exactly one schema in oneOf",
    };
    if (vErrors === null) {
      vErrors = [err17];
    } else {
      vErrors.push(err17);
    }
    errors++;
    validate20.errors = vErrors;
    return false;
  } else {
    errors = _errs1;
    if (vErrors !== null) {
      if (_errs1) {
        vErrors.length = _errs1;
      } else {
        vErrors = null;
      }
    }
  }
  validate20.errors = vErrors;
  evaluated0.props = props0;
  return errors === 0;
}
validate20.evaluated = { dynamicProps: true, dynamicItems: false };
