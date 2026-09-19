// @ts-nocheck
// @generated from canonical schemas; do not edit.

"use strict";
export const validate = validate20;
export default validate20;
const schema31 = {
  title: "A2UI (Agent to UI) Client-to-Server Event Schema",
  description: "Describes a JSON payload for a client-to-server event message.",
  type: "object",
  minProperties: 2,
  maxProperties: 2,
  properties: {
    version: { enum: ["v0.9", "v0.9.1"] },
    action: {
      type: "object",
      description: "Reports a user-initiated action from a component.",
      properties: {
        name: {
          type: "string",
          description:
            "The name of the action, taken from the component's action.event.name property.",
        },
        surfaceId: {
          type: "string",
          description: "The id of the surface where the event originated.",
        },
        sourceComponentId: {
          type: "string",
          description: "The id of the component that triggered the event.",
        },
        timestamp: {
          type: "string",
          format: "date-time",
          description: "An ISO 8601 timestamp of when the event occurred.",
        },
        context: {
          type: "object",
          description:
            "A JSON object containing the key-value pairs from the component's action.event.context, after resolving all data bindings.",
          additionalProperties: true,
        },
      },
      required: [
        "name",
        "surfaceId",
        "sourceComponentId",
        "timestamp",
        "context",
      ],
    },
    error: {
      description: "Reports a client-side error.",
      oneOf: [
        {
          type: "object",
          title: "Validation Failed Error",
          properties: {
            code: { const: "VALIDATION_FAILED" },
            surfaceId: {
              type: "string",
              description: "The id of the surface where the error occurred.",
            },
            path: {
              type: "string",
              description:
                "The JSON pointer to the field that failed validation (e.g. '/components/0/text').",
            },
            message: {
              type: "string",
              description:
                "A short one or two sentence description of why validation failed.",
            },
          },
          required: ["code", "path", "message", "surfaceId"],
          additionalProperties: false,
        },
        {
          type: "object",
          title: "Generic Error",
          properties: {
            code: { not: { const: "VALIDATION_FAILED" } },
            message: {
              type: "string",
              description:
                "A short one or two sentence description of why the error occurred.",
            },
            surfaceId: {
              type: "string",
              description: "The id of the surface where the error occurred.",
            },
          },
          required: ["code", "surfaceId", "message"],
          additionalProperties: true,
        },
      ],
    },
  },
  oneOf: [
    { required: ["action", "version"] },
    { required: ["error", "version"] },
  ],
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
  const _errs1 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs2 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    let missing0;
    if (
      (data.action === undefined && (missing0 = "action")) ||
      (data.version === undefined && (missing0 = "version"))
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
    }
  }
  var _valid0 = _errs2 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    let missing1;
    if (
      (data.error === undefined && (missing1 = "error")) ||
      (data.version === undefined && (missing1 = "version"))
    ) {
      const err1 = {
        instancePath,
        schemaPath: "#/oneOf/1/required",
        keyword: "required",
        params: { missingProperty: missing1 },
        message: "must have required property '" + missing1 + "'",
      };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
  }
  if (!valid0) {
    const err2 = {
      instancePath,
      schemaPath: "#/oneOf",
      keyword: "oneOf",
      params: { passingSchemas: passing0 },
      message: "must match exactly one schema in oneOf",
    };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
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
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      if (Object.keys(data).length > 2) {
        validate20.errors = [
          {
            instancePath,
            schemaPath: "#/maxProperties",
            keyword: "maxProperties",
            params: { limit: 2 },
            message: "must NOT have more than 2 properties",
          },
        ];
        return false;
      } else {
        if (Object.keys(data).length < 2) {
          validate20.errors = [
            {
              instancePath,
              schemaPath: "#/minProperties",
              keyword: "minProperties",
              params: { limit: 2 },
              message: "must NOT have fewer than 2 properties",
            },
          ];
          return false;
        } else {
          if (data.version !== undefined) {
            let data0 = data.version;
            const _errs4 = errors;
            if (!(data0 === "v0.9" || data0 === "v0.9.1")) {
              validate20.errors = [
                {
                  instancePath: instancePath + "/version",
                  schemaPath: "#/properties/version/enum",
                  keyword: "enum",
                  params: { allowedValues: schema31.properties.version.enum },
                  message: "must be equal to one of the allowed values",
                },
              ];
              return false;
            }
            var valid1 = _errs4 === errors;
          } else {
            var valid1 = true;
          }
          if (valid1) {
            if (data.action !== undefined) {
              let data1 = data.action;
              const _errs5 = errors;
              if (errors === _errs5) {
                if (
                  data1 &&
                  typeof data1 == "object" &&
                  !Array.isArray(data1)
                ) {
                  let missing2;
                  if (
                    (data1.name === undefined && (missing2 = "name")) ||
                    (data1.surfaceId === undefined &&
                      (missing2 = "surfaceId")) ||
                    (data1.sourceComponentId === undefined &&
                      (missing2 = "sourceComponentId")) ||
                    (data1.timestamp === undefined &&
                      (missing2 = "timestamp")) ||
                    (data1.context === undefined && (missing2 = "context"))
                  ) {
                    validate20.errors = [
                      {
                        instancePath: instancePath + "/action",
                        schemaPath: "#/properties/action/required",
                        keyword: "required",
                        params: { missingProperty: missing2 },
                        message:
                          "must have required property '" + missing2 + "'",
                      },
                    ];
                    return false;
                  } else {
                    if (data1.name !== undefined) {
                      const _errs7 = errors;
                      if (typeof data1.name !== "string") {
                        validate20.errors = [
                          {
                            instancePath: instancePath + "/action/name",
                            schemaPath:
                              "#/properties/action/properties/name/type",
                            keyword: "type",
                            params: { type: "string" },
                            message: "must be string",
                          },
                        ];
                        return false;
                      }
                      var valid2 = _errs7 === errors;
                    } else {
                      var valid2 = true;
                    }
                    if (valid2) {
                      if (data1.surfaceId !== undefined) {
                        const _errs9 = errors;
                        if (typeof data1.surfaceId !== "string") {
                          validate20.errors = [
                            {
                              instancePath: instancePath + "/action/surfaceId",
                              schemaPath:
                                "#/properties/action/properties/surfaceId/type",
                              keyword: "type",
                              params: { type: "string" },
                              message: "must be string",
                            },
                          ];
                          return false;
                        }
                        var valid2 = _errs9 === errors;
                      } else {
                        var valid2 = true;
                      }
                      if (valid2) {
                        if (data1.sourceComponentId !== undefined) {
                          const _errs11 = errors;
                          if (typeof data1.sourceComponentId !== "string") {
                            validate20.errors = [
                              {
                                instancePath:
                                  instancePath + "/action/sourceComponentId",
                                schemaPath:
                                  "#/properties/action/properties/sourceComponentId/type",
                                keyword: "type",
                                params: { type: "string" },
                                message: "must be string",
                              },
                            ];
                            return false;
                          }
                          var valid2 = _errs11 === errors;
                        } else {
                          var valid2 = true;
                        }
                        if (valid2) {
                          if (data1.timestamp !== undefined) {
                            const _errs13 = errors;
                            if (errors === _errs13) {
                              if (errors === _errs13) {
                                if (!(typeof data1.timestamp === "string")) {
                                  validate20.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/action/timestamp",
                                      schemaPath:
                                        "#/properties/action/properties/timestamp/type",
                                      keyword: "type",
                                      params: { type: "string" },
                                      message: "must be string",
                                    },
                                  ];
                                  return false;
                                }
                              }
                            }
                            var valid2 = _errs13 === errors;
                          } else {
                            var valid2 = true;
                          }
                          if (valid2) {
                            if (data1.context !== undefined) {
                              let data6 = data1.context;
                              const _errs15 = errors;
                              if (errors === _errs15) {
                                if (
                                  data6 &&
                                  typeof data6 == "object" &&
                                  !Array.isArray(data6)
                                ) {
                                } else {
                                  validate20.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/action/context",
                                      schemaPath:
                                        "#/properties/action/properties/context/type",
                                      keyword: "type",
                                      params: { type: "object" },
                                      message: "must be object",
                                    },
                                  ];
                                  return false;
                                }
                              }
                              var valid2 = _errs15 === errors;
                            } else {
                              var valid2 = true;
                            }
                          }
                        }
                      }
                    }
                  }
                } else {
                  validate20.errors = [
                    {
                      instancePath: instancePath + "/action",
                      schemaPath: "#/properties/action/type",
                      keyword: "type",
                      params: { type: "object" },
                      message: "must be object",
                    },
                  ];
                  return false;
                }
              }
              var valid1 = _errs5 === errors;
            } else {
              var valid1 = true;
            }
            if (valid1) {
              if (data.error !== undefined) {
                let data7 = data.error;
                const _errs18 = errors;
                const _errs19 = errors;
                let valid3 = false;
                let passing1 = null;
                const _errs20 = errors;
                if (errors === _errs20) {
                  if (
                    data7 &&
                    typeof data7 == "object" &&
                    !Array.isArray(data7)
                  ) {
                    let missing3;
                    if (
                      (data7.code === undefined && (missing3 = "code")) ||
                      (data7.path === undefined && (missing3 = "path")) ||
                      (data7.message === undefined && (missing3 = "message")) ||
                      (data7.surfaceId === undefined &&
                        (missing3 = "surfaceId"))
                    ) {
                      const err3 = {
                        instancePath: instancePath + "/error",
                        schemaPath: "#/properties/error/oneOf/0/required",
                        keyword: "required",
                        params: { missingProperty: missing3 },
                        message:
                          "must have required property '" + missing3 + "'",
                      };
                      if (vErrors === null) {
                        vErrors = [err3];
                      } else {
                        vErrors.push(err3);
                      }
                      errors++;
                    } else {
                      const _errs22 = errors;
                      for (const key0 in data7) {
                        if (
                          !(
                            key0 === "code" ||
                            key0 === "surfaceId" ||
                            key0 === "path" ||
                            key0 === "message"
                          )
                        ) {
                          const err4 = {
                            instancePath: instancePath + "/error",
                            schemaPath:
                              "#/properties/error/oneOf/0/additionalProperties",
                            keyword: "additionalProperties",
                            params: { additionalProperty: key0 },
                            message: "must NOT have additional properties",
                          };
                          if (vErrors === null) {
                            vErrors = [err4];
                          } else {
                            vErrors.push(err4);
                          }
                          errors++;
                          break;
                        }
                      }
                      if (_errs22 === errors) {
                        if (data7.code !== undefined) {
                          const _errs23 = errors;
                          if ("VALIDATION_FAILED" !== data7.code) {
                            const err5 = {
                              instancePath: instancePath + "/error/code",
                              schemaPath:
                                "#/properties/error/oneOf/0/properties/code/const",
                              keyword: "const",
                              params: { allowedValue: "VALIDATION_FAILED" },
                              message: "must be equal to constant",
                            };
                            if (vErrors === null) {
                              vErrors = [err5];
                            } else {
                              vErrors.push(err5);
                            }
                            errors++;
                          }
                          var valid4 = _errs23 === errors;
                        } else {
                          var valid4 = true;
                        }
                        if (valid4) {
                          if (data7.surfaceId !== undefined) {
                            const _errs24 = errors;
                            if (typeof data7.surfaceId !== "string") {
                              const err6 = {
                                instancePath: instancePath + "/error/surfaceId",
                                schemaPath:
                                  "#/properties/error/oneOf/0/properties/surfaceId/type",
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
                            var valid4 = _errs24 === errors;
                          } else {
                            var valid4 = true;
                          }
                          if (valid4) {
                            if (data7.path !== undefined) {
                              const _errs26 = errors;
                              if (typeof data7.path !== "string") {
                                const err7 = {
                                  instancePath: instancePath + "/error/path",
                                  schemaPath:
                                    "#/properties/error/oneOf/0/properties/path/type",
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
                              var valid4 = _errs26 === errors;
                            } else {
                              var valid4 = true;
                            }
                            if (valid4) {
                              if (data7.message !== undefined) {
                                const _errs28 = errors;
                                if (typeof data7.message !== "string") {
                                  const err8 = {
                                    instancePath:
                                      instancePath + "/error/message",
                                    schemaPath:
                                      "#/properties/error/oneOf/0/properties/message/type",
                                    keyword: "type",
                                    params: { type: "string" },
                                    message: "must be string",
                                  };
                                  if (vErrors === null) {
                                    vErrors = [err8];
                                  } else {
                                    vErrors.push(err8);
                                  }
                                  errors++;
                                }
                                var valid4 = _errs28 === errors;
                              } else {
                                var valid4 = true;
                              }
                            }
                          }
                        }
                      }
                    }
                  } else {
                    const err9 = {
                      instancePath: instancePath + "/error",
                      schemaPath: "#/properties/error/oneOf/0/type",
                      keyword: "type",
                      params: { type: "object" },
                      message: "must be object",
                    };
                    if (vErrors === null) {
                      vErrors = [err9];
                    } else {
                      vErrors.push(err9);
                    }
                    errors++;
                  }
                }
                var _valid1 = _errs20 === errors;
                if (_valid1) {
                  valid3 = true;
                  passing1 = 0;
                  var props0 = true;
                }
                const _errs30 = errors;
                if (errors === _errs30) {
                  if (
                    data7 &&
                    typeof data7 == "object" &&
                    !Array.isArray(data7)
                  ) {
                    let missing4;
                    if (
                      (data7.code === undefined && (missing4 = "code")) ||
                      (data7.surfaceId === undefined &&
                        (missing4 = "surfaceId")) ||
                      (data7.message === undefined && (missing4 = "message"))
                    ) {
                      const err10 = {
                        instancePath: instancePath + "/error",
                        schemaPath: "#/properties/error/oneOf/1/required",
                        keyword: "required",
                        params: { missingProperty: missing4 },
                        message:
                          "must have required property '" + missing4 + "'",
                      };
                      if (vErrors === null) {
                        vErrors = [err10];
                      } else {
                        vErrors.push(err10);
                      }
                      errors++;
                    } else {
                      if (data7.code !== undefined) {
                        const _errs33 = errors;
                        const _errs34 = errors;
                        const _errs35 = errors;
                        if ("VALIDATION_FAILED" !== data7.code) {
                          const err11 = {};
                          if (vErrors === null) {
                            vErrors = [err11];
                          } else {
                            vErrors.push(err11);
                          }
                          errors++;
                        }
                        var valid6 = _errs35 === errors;
                        if (valid6) {
                          const err12 = {
                            instancePath: instancePath + "/error/code",
                            schemaPath:
                              "#/properties/error/oneOf/1/properties/code/not",
                            keyword: "not",
                            params: {},
                            message: "must NOT be valid",
                          };
                          if (vErrors === null) {
                            vErrors = [err12];
                          } else {
                            vErrors.push(err12);
                          }
                          errors++;
                        } else {
                          errors = _errs34;
                          if (vErrors !== null) {
                            if (_errs34) {
                              vErrors.length = _errs34;
                            } else {
                              vErrors = null;
                            }
                          }
                        }
                        var valid5 = _errs33 === errors;
                      } else {
                        var valid5 = true;
                      }
                      if (valid5) {
                        if (data7.message !== undefined) {
                          const _errs36 = errors;
                          if (typeof data7.message !== "string") {
                            const err13 = {
                              instancePath: instancePath + "/error/message",
                              schemaPath:
                                "#/properties/error/oneOf/1/properties/message/type",
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
                          var valid5 = _errs36 === errors;
                        } else {
                          var valid5 = true;
                        }
                        if (valid5) {
                          if (data7.surfaceId !== undefined) {
                            const _errs38 = errors;
                            if (typeof data7.surfaceId !== "string") {
                              const err14 = {
                                instancePath: instancePath + "/error/surfaceId",
                                schemaPath:
                                  "#/properties/error/oneOf/1/properties/surfaceId/type",
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
                            var valid5 = _errs38 === errors;
                          } else {
                            var valid5 = true;
                          }
                        }
                      }
                    }
                  } else {
                    const err15 = {
                      instancePath: instancePath + "/error",
                      schemaPath: "#/properties/error/oneOf/1/type",
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
                var _valid1 = _errs30 === errors;
                if (_valid1 && valid3) {
                  valid3 = false;
                  passing1 = [passing1, 1];
                } else {
                  if (_valid1) {
                    valid3 = true;
                    passing1 = 1;
                    if (props0 !== true) {
                      props0 = true;
                    }
                  }
                }
                if (!valid3) {
                  const err16 = {
                    instancePath: instancePath + "/error",
                    schemaPath: "#/properties/error/oneOf",
                    keyword: "oneOf",
                    params: { passingSchemas: passing1 },
                    message: "must match exactly one schema in oneOf",
                  };
                  if (vErrors === null) {
                    vErrors = [err16];
                  } else {
                    vErrors.push(err16);
                  }
                  errors++;
                  validate20.errors = vErrors;
                  return false;
                } else {
                  errors = _errs19;
                  if (vErrors !== null) {
                    if (_errs19) {
                      vErrors.length = _errs19;
                    } else {
                      vErrors = null;
                    }
                  }
                }
                var valid1 = _errs18 === errors;
              } else {
                var valid1 = true;
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
  props: { version: true, action: true, error: true },
  dynamicProps: false,
  dynamicItems: false,
};
