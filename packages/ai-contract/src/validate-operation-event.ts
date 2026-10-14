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
  $schema: "https://json-schema.org/draft/2020-12/schema",
  $defs: {
    OperationValue: {
      anyOf: [
        {
          anyOf: [
            { type: "string" },
            { type: "number" },
            { type: "boolean" },
            { type: "null" },
          ],
        },
        {
          type: "array",
          items: { type: "string", maxLength: 4096 },
          maxItems: 128,
        },
      ],
    },
    OperationField: {
      type: "object",
      properties: {
        id: {
          type: "string",
          minLength: 1,
          maxLength: 128,
          pattern: "^[a-zA-Z0-9_-]+$",
        },
        label: { type: "string", maxLength: 256 },
        component: {
          enum: [
            "TextField",
            "NumberField",
            "SelectField",
            "Switch",
            "Checkbox",
            "RadioGroup",
            "PathPicker",
            "DateTimeField",
            "SecretField",
          ],
        },
        required: { type: "boolean" },
        readonly: { type: "boolean" },
        reason: { type: "string", maxLength: 4096 },
        min: { type: "number" },
        max: { type: "number" },
        maxLength: { type: "integer", minimum: 1, maximum: 8192 },
        options: {
          type: "array",
          items: {
            type: "object",
            properties: {
              id: {
                type: "string",
                minLength: 1,
                maxLength: 128,
                pattern: "^[a-zA-Z0-9_-]+$",
              },
              label: { type: "string", maxLength: 256 },
              disabledReason: { type: "string", maxLength: 4096 },
            },
            required: ["id", "label"],
            additionalProperties: false,
          },
          maxItems: 128,
        },
        submitMode: { enum: ["draft", "immediate"] },
        section: {
          type: "string",
          minLength: 1,
          maxLength: 128,
          pattern: "^[a-zA-Z0-9_-]+$",
        },
      },
      required: ["id", "label", "component"],
      additionalProperties: false,
    },
    OperationRule: {
      type: "object",
      properties: {
        field: {
          type: "string",
          minLength: 1,
          maxLength: 128,
          pattern: "^[a-zA-Z0-9_-]+$",
        },
        when: {
          type: "string",
          minLength: 1,
          maxLength: 128,
          pattern: "^[a-zA-Z0-9_-]+$",
        },
        equals: {
          anyOf: [
            { type: "string" },
            { type: "number" },
            { type: "boolean" },
            { type: "null" },
          ],
        },
        kind: { enum: ["required", "equals"] },
        message: { type: "string", maxLength: 256 },
      },
      required: ["field", "when", "equals", "kind", "message"],
      additionalProperties: false,
    },
    OperationPresentation: {
      type: "object",
      properties: {
        version: { const: 1 },
        template: {
          enum: [
            "SettingsForm",
            "ObjectActions",
            "ResourceSelection",
            "ChangeReview",
            "TaskExecution",
            "GuidedFlow",
            "OperationApproval",
          ],
        },
        fields: {
          type: "array",
          items: { $ref: "#/$defs/OperationField" },
          maxItems: 128,
        },
        rules: {
          type: "array",
          items: { $ref: "#/$defs/OperationRule" },
          maxItems: 128,
        },
        sections: {
          type: "array",
          items: {
            type: "object",
            properties: {
              id: {
                type: "string",
                minLength: 1,
                maxLength: 128,
                pattern: "^[a-zA-Z0-9_-]+$",
              },
              label: { type: "string", maxLength: 256 },
            },
            required: ["id", "label"],
            additionalProperties: false,
          },
          maxItems: 16,
        },
        layout: { enum: ["sections", "tabs", "disclosure"] },
        selection: { enum: ["single", "multiple"] },
        steps: {
          type: "array",
          items: {
            type: "object",
            properties: {
              id: {
                type: "string",
                minLength: 1,
                maxLength: 128,
                pattern: "^[a-zA-Z0-9_-]+$",
              },
              label: { type: "string", maxLength: 256 },
              fields: {
                type: "array",
                items: {
                  type: "string",
                  minLength: 1,
                  maxLength: 128,
                  pattern: "^[a-zA-Z0-9_-]+$",
                },
                maxItems: 128,
              },
            },
            required: ["id", "label", "fields"],
            additionalProperties: false,
          },
          maxItems: 16,
        },
      },
      required: ["version", "template", "fields", "rules", "sections"],
      additionalProperties: false,
    },
    OperationScope: {
      type: "object",
      properties: {
        authority: { type: "string", maxLength: 256 },
        tenant: { type: "string", maxLength: 256 },
        principal: { type: "string", maxLength: 256 },
        session: { type: "string", maxLength: 256 },
        target: { type: "string", maxLength: 256 },
        instance: {
          type: "string",
          minLength: 1,
          maxLength: 128,
          pattern: "^[a-zA-Z0-9_-]+$",
        },
      },
      required: [
        "authority",
        "tenant",
        "principal",
        "session",
        "target",
        "instance",
      ],
      additionalProperties: false,
    },
    OperationObject: {
      type: "object",
      properties: {
        id: {
          type: "string",
          minLength: 1,
          maxLength: 128,
          pattern: "^[a-zA-Z0-9_-]+$",
        },
        label: { type: "string", maxLength: 256 },
        detail: { type: "string", maxLength: 4096 },
        disabledReason: { type: "string", maxLength: 4096 },
        status: { type: "string", maxLength: 256 },
      },
      required: ["id", "label", "detail"],
      additionalProperties: false,
    },
    OperationInteraction: {
      type: "object",
      properties: {
        id: {
          type: "string",
          minLength: 1,
          maxLength: 128,
          pattern: "^[a-zA-Z0-9_-]+$",
        },
        question: { type: "string", maxLength: 4096 },
        options: {
          type: "array",
          items: {
            type: "object",
            properties: {
              id: {
                type: "string",
                minLength: 1,
                maxLength: 128,
                pattern: "^[a-zA-Z0-9_-]+$",
              },
              label: { type: "string", maxLength: 256 },
            },
            required: ["id", "label"],
            additionalProperties: false,
          },
          maxItems: 16,
        },
        expiresAtMs: { type: "integer", minimum: 0 },
      },
      required: ["id", "question", "options", "expiresAtMs"],
      additionalProperties: false,
    },
    OperationSnapshot: {
      type: "object",
      properties: {
        scope: { $ref: "#/$defs/OperationScope" },
        revision: { type: "integer", minimum: 0 },
        baseline: { type: "string", maxLength: 256 },
        title: { type: "string", maxLength: 256 },
        source: { type: "string", maxLength: 256 },
        targetLabel: { type: "string", maxLength: 256 },
        impact: { type: "string", maxLength: 4096 },
        current: {
          type: "object",
          maxProperties: 128,
          propertyNames: {
            type: "string",
            minLength: 1,
            maxLength: 128,
            pattern: "^[a-zA-Z0-9_-]+$",
          },
          additionalProperties: { $ref: "#/$defs/OperationValue" },
        },
        proposal: {
          type: "object",
          maxProperties: 128,
          propertyNames: {
            type: "string",
            minLength: 1,
            maxLength: 128,
            pattern: "^[a-zA-Z0-9_-]+$",
          },
          additionalProperties: { $ref: "#/$defs/OperationValue" },
        },
        phase: {
          enum: [
            "ready",
            "submitting",
            "waiting",
            "running",
            "failed",
            "unknown",
            "succeeded",
            "pending_effect",
            "cancelled",
            "denied",
          ],
        },
        reason: { type: "string", maxLength: 4096 },
        allowed: {
          type: "array",
          items: { enum: ["submit", "cancel", "refresh", "answer"] },
          maxItems: 4,
        },
        approval: {
          type: "object",
          properties: {
            user: { type: "string", maxLength: 256 },
            enterprise: { type: "string", maxLength: 256 },
            elevation: { type: "string", maxLength: 256 },
          },
          required: ["user", "enterprise", "elevation"],
          additionalProperties: false,
        },
        objects: {
          type: "array",
          items: { $ref: "#/$defs/OperationObject" },
          maxItems: 128,
        },
        logs: {
          type: "array",
          items: { type: "string", maxLength: 2048 },
          maxItems: 200,
        },
        progress: {
          type: "object",
          properties: {
            completed: { type: "number", minimum: 0 },
            total: { type: "number", exclusiveMinimum: 0 },
          },
          required: ["completed", "total"],
          additionalProperties: false,
        },
        result: { type: "string", maxLength: 4096 },
        interaction: { $ref: "#/$defs/OperationInteraction" },
      },
      required: [
        "scope",
        "revision",
        "baseline",
        "title",
        "source",
        "targetLabel",
        "impact",
        "current",
        "proposal",
        "phase",
        "allowed",
        "approval",
        "objects",
        "logs",
      ],
      additionalProperties: false,
    },
    OperationEvent: {
      type: "object",
      properties: {
        id: {
          type: "string",
          minLength: 1,
          maxLength: 128,
          pattern: "^[a-zA-Z0-9_-]+$",
        },
        instance: {
          type: "string",
          minLength: 1,
          maxLength: 128,
          pattern: "^[a-zA-Z0-9_-]+$",
        },
        revision: { type: "integer", minimum: 0 },
        kind: { enum: ["submit", "cancel", "refresh", "answer"] },
        values: {
          type: "object",
          maxProperties: 128,
          propertyNames: {
            type: "string",
            minLength: 1,
            maxLength: 128,
            pattern: "^[a-zA-Z0-9_-]+$",
          },
          additionalProperties: { $ref: "#/$defs/OperationValue" },
        },
        selection: {
          type: "array",
          items: {
            type: "string",
            minLength: 1,
            maxLength: 128,
            pattern: "^[a-zA-Z0-9_-]+$",
          },
          maxItems: 128,
        },
        secretRefs: {
          type: "object",
          maxProperties: 128,
          propertyNames: {
            type: "string",
            minLength: 1,
            maxLength: 128,
            pattern: "^[a-zA-Z0-9_-]+$",
          },
          additionalProperties: {
            type: "string",
            minLength: 1,
            maxLength: 128,
            pattern: "^[a-zA-Z0-9_-]+$",
          },
        },
        interactionId: {
          type: "string",
          minLength: 1,
          maxLength: 128,
          pattern: "^[a-zA-Z0-9_-]+$",
        },
        answer: {
          type: "string",
          minLength: 1,
          maxLength: 128,
          pattern: "^[a-zA-Z0-9_-]+$",
        },
      },
      required: [
        "id",
        "instance",
        "revision",
        "kind",
        "values",
        "selection",
        "secretRefs",
      ],
      additionalProperties: false,
    },
  },
  $ref: "#/$defs/OperationEvent",
};
const schema32 = {
  type: "object",
  properties: {
    id: {
      type: "string",
      minLength: 1,
      maxLength: 128,
      pattern: "^[a-zA-Z0-9_-]+$",
    },
    instance: {
      type: "string",
      minLength: 1,
      maxLength: 128,
      pattern: "^[a-zA-Z0-9_-]+$",
    },
    revision: { type: "integer", minimum: 0 },
    kind: { enum: ["submit", "cancel", "refresh", "answer"] },
    values: {
      type: "object",
      maxProperties: 128,
      propertyNames: {
        type: "string",
        minLength: 1,
        maxLength: 128,
        pattern: "^[a-zA-Z0-9_-]+$",
      },
      additionalProperties: { $ref: "#/$defs/OperationValue" },
    },
    selection: {
      type: "array",
      items: {
        type: "string",
        minLength: 1,
        maxLength: 128,
        pattern: "^[a-zA-Z0-9_-]+$",
      },
      maxItems: 128,
    },
    secretRefs: {
      type: "object",
      maxProperties: 128,
      propertyNames: {
        type: "string",
        minLength: 1,
        maxLength: 128,
        pattern: "^[a-zA-Z0-9_-]+$",
      },
      additionalProperties: {
        type: "string",
        minLength: 1,
        maxLength: 128,
        pattern: "^[a-zA-Z0-9_-]+$",
      },
    },
    interactionId: {
      type: "string",
      minLength: 1,
      maxLength: 128,
      pattern: "^[a-zA-Z0-9_-]+$",
    },
    answer: {
      type: "string",
      minLength: 1,
      maxLength: 128,
      pattern: "^[a-zA-Z0-9_-]+$",
    },
  },
  required: [
    "id",
    "instance",
    "revision",
    "kind",
    "values",
    "selection",
    "secretRefs",
  ],
  additionalProperties: false,
};
const schema33 = {
  anyOf: [
    {
      anyOf: [
        { type: "string" },
        { type: "number" },
        { type: "boolean" },
        { type: "null" },
      ],
    },
    {
      type: "array",
      items: { type: "string", maxLength: 4096 },
      maxItems: 128,
    },
  ],
};
const func1 = Object.prototype.hasOwnProperty;
const func2 = helper0;
const pattern4 = new RegExp("^[a-zA-Z0-9_-]+$", "u");
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
        (data.id === undefined && (missing0 = "id")) ||
        (data.instance === undefined && (missing0 = "instance")) ||
        (data.revision === undefined && (missing0 = "revision")) ||
        (data.kind === undefined && (missing0 = "kind")) ||
        (data.values === undefined && (missing0 = "values")) ||
        (data.selection === undefined && (missing0 = "selection")) ||
        (data.secretRefs === undefined && (missing0 = "secretRefs"))
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
          if (!func1.call(schema32.properties, key0)) {
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
          if (data.id !== undefined) {
            let data0 = data.id;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (typeof data0 === "string") {
                if (func2(data0) > 128) {
                  validate21.errors = [
                    {
                      instancePath: instancePath + "/id",
                      schemaPath: "#/properties/id/maxLength",
                      keyword: "maxLength",
                      params: { limit: 128 },
                      message: "must NOT have more than 128 characters",
                    },
                  ];
                  return false;
                } else {
                  if (func2(data0) < 1) {
                    validate21.errors = [
                      {
                        instancePath: instancePath + "/id",
                        schemaPath: "#/properties/id/minLength",
                        keyword: "minLength",
                        params: { limit: 1 },
                        message: "must NOT have fewer than 1 characters",
                      },
                    ];
                    return false;
                  } else {
                    if (!pattern4.test(data0)) {
                      validate21.errors = [
                        {
                          instancePath: instancePath + "/id",
                          schemaPath: "#/properties/id/pattern",
                          keyword: "pattern",
                          params: { pattern: "^[a-zA-Z0-9_-]+$" },
                          message:
                            'must match pattern "' + "^[a-zA-Z0-9_-]+$" + '"',
                        },
                      ];
                      return false;
                    }
                  }
                }
              } else {
                validate21.errors = [
                  {
                    instancePath: instancePath + "/id",
                    schemaPath: "#/properties/id/type",
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
            if (data.instance !== undefined) {
              let data1 = data.instance;
              const _errs4 = errors;
              if (errors === _errs4) {
                if (typeof data1 === "string") {
                  if (func2(data1) > 128) {
                    validate21.errors = [
                      {
                        instancePath: instancePath + "/instance",
                        schemaPath: "#/properties/instance/maxLength",
                        keyword: "maxLength",
                        params: { limit: 128 },
                        message: "must NOT have more than 128 characters",
                      },
                    ];
                    return false;
                  } else {
                    if (func2(data1) < 1) {
                      validate21.errors = [
                        {
                          instancePath: instancePath + "/instance",
                          schemaPath: "#/properties/instance/minLength",
                          keyword: "minLength",
                          params: { limit: 1 },
                          message: "must NOT have fewer than 1 characters",
                        },
                      ];
                      return false;
                    } else {
                      if (!pattern4.test(data1)) {
                        validate21.errors = [
                          {
                            instancePath: instancePath + "/instance",
                            schemaPath: "#/properties/instance/pattern",
                            keyword: "pattern",
                            params: { pattern: "^[a-zA-Z0-9_-]+$" },
                            message:
                              'must match pattern "' + "^[a-zA-Z0-9_-]+$" + '"',
                          },
                        ];
                        return false;
                      }
                    }
                  }
                } else {
                  validate21.errors = [
                    {
                      instancePath: instancePath + "/instance",
                      schemaPath: "#/properties/instance/type",
                      keyword: "type",
                      params: { type: "string" },
                      message: "must be string",
                    },
                  ];
                  return false;
                }
              }
              var valid0 = _errs4 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.revision !== undefined) {
                let data2 = data.revision;
                const _errs6 = errors;
                if (
                  !(
                    typeof data2 == "number" &&
                    !(data2 % 1) &&
                    !isNaN(data2) &&
                    isFinite(data2)
                  )
                ) {
                  validate21.errors = [
                    {
                      instancePath: instancePath + "/revision",
                      schemaPath: "#/properties/revision/type",
                      keyword: "type",
                      params: { type: "integer" },
                      message: "must be integer",
                    },
                  ];
                  return false;
                }
                if (errors === _errs6) {
                  if (typeof data2 == "number" && isFinite(data2)) {
                    if (data2 < 0 || isNaN(data2)) {
                      validate21.errors = [
                        {
                          instancePath: instancePath + "/revision",
                          schemaPath: "#/properties/revision/minimum",
                          keyword: "minimum",
                          params: { comparison: ">=", limit: 0 },
                          message: "must be >= 0",
                        },
                      ];
                      return false;
                    }
                  }
                }
                var valid0 = _errs6 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.kind !== undefined) {
                  let data3 = data.kind;
                  const _errs8 = errors;
                  if (
                    !(
                      data3 === "submit" ||
                      data3 === "cancel" ||
                      data3 === "refresh" ||
                      data3 === "answer"
                    )
                  ) {
                    validate21.errors = [
                      {
                        instancePath: instancePath + "/kind",
                        schemaPath: "#/properties/kind/enum",
                        keyword: "enum",
                        params: {
                          allowedValues: schema32.properties.kind.enum,
                        },
                        message: "must be equal to one of the allowed values",
                      },
                    ];
                    return false;
                  }
                  var valid0 = _errs8 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.values !== undefined) {
                    let data4 = data.values;
                    const _errs9 = errors;
                    if (errors === _errs9) {
                      if (
                        data4 &&
                        typeof data4 == "object" &&
                        !Array.isArray(data4)
                      ) {
                        if (Object.keys(data4).length > 128) {
                          validate21.errors = [
                            {
                              instancePath: instancePath + "/values",
                              schemaPath: "#/properties/values/maxProperties",
                              keyword: "maxProperties",
                              params: { limit: 128 },
                              message: "must NOT have more than 128 properties",
                            },
                          ];
                          return false;
                        } else {
                          for (const key1 in data4) {
                            const _errs11 = errors;
                            if (errors === _errs11) {
                              if (typeof key1 === "string") {
                                if (func2(key1) > 128) {
                                  const err0 = {
                                    instancePath: instancePath + "/values",
                                    schemaPath:
                                      "#/properties/values/propertyNames/maxLength",
                                    keyword: "maxLength",
                                    params: { limit: 128 },
                                    message:
                                      "must NOT have more than 128 characters",
                                    propertyName: key1,
                                  };
                                  if (vErrors === null) {
                                    vErrors = [err0];
                                  } else {
                                    vErrors.push(err0);
                                  }
                                  errors++;
                                } else {
                                  if (func2(key1) < 1) {
                                    const err1 = {
                                      instancePath: instancePath + "/values",
                                      schemaPath:
                                        "#/properties/values/propertyNames/minLength",
                                      keyword: "minLength",
                                      params: { limit: 1 },
                                      message:
                                        "must NOT have fewer than 1 characters",
                                      propertyName: key1,
                                    };
                                    if (vErrors === null) {
                                      vErrors = [err1];
                                    } else {
                                      vErrors.push(err1);
                                    }
                                    errors++;
                                  } else {
                                    if (!pattern4.test(key1)) {
                                      const err2 = {
                                        instancePath: instancePath + "/values",
                                        schemaPath:
                                          "#/properties/values/propertyNames/pattern",
                                        keyword: "pattern",
                                        params: { pattern: "^[a-zA-Z0-9_-]+$" },
                                        message:
                                          'must match pattern "' +
                                          "^[a-zA-Z0-9_-]+$" +
                                          '"',
                                        propertyName: key1,
                                      };
                                      if (vErrors === null) {
                                        vErrors = [err2];
                                      } else {
                                        vErrors.push(err2);
                                      }
                                      errors++;
                                    }
                                  }
                                }
                              } else {
                                const err3 = {
                                  instancePath: instancePath + "/values",
                                  schemaPath:
                                    "#/properties/values/propertyNames/type",
                                  keyword: "type",
                                  params: { type: "string" },
                                  message: "must be string",
                                  propertyName: key1,
                                };
                                if (vErrors === null) {
                                  vErrors = [err3];
                                } else {
                                  vErrors.push(err3);
                                }
                                errors++;
                              }
                            }
                            var valid1 = _errs11 === errors;
                            if (!valid1) {
                              const err4 = {
                                instancePath: instancePath + "/values",
                                schemaPath: "#/properties/values/propertyNames",
                                keyword: "propertyNames",
                                params: { propertyName: key1 },
                                message: "property name must be valid",
                              };
                              if (vErrors === null) {
                                vErrors = [err4];
                              } else {
                                vErrors.push(err4);
                              }
                              errors++;
                              validate21.errors = vErrors;
                              return false;
                              break;
                            }
                          }
                          if (valid1) {
                            for (const key2 in data4) {
                              let data5 = data4[key2];
                              const _errs14 = errors;
                              const _errs16 = errors;
                              let valid4 = false;
                              const _errs17 = errors;
                              const _errs18 = errors;
                              let valid5 = false;
                              const _errs19 = errors;
                              if (typeof data5 !== "string") {
                                const err5 = {
                                  instancePath:
                                    instancePath +
                                    "/values/" +
                                    key2
                                      .replace(/~/g, "~0")
                                      .replace(/\//g, "~1"),
                                  schemaPath:
                                    "#/$defs/OperationValue/anyOf/0/anyOf/0/type",
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
                              var _valid1 = _errs19 === errors;
                              valid5 = valid5 || _valid1;
                              const _errs21 = errors;
                              if (
                                !(typeof data5 == "number" && isFinite(data5))
                              ) {
                                const err6 = {
                                  instancePath:
                                    instancePath +
                                    "/values/" +
                                    key2
                                      .replace(/~/g, "~0")
                                      .replace(/\//g, "~1"),
                                  schemaPath:
                                    "#/$defs/OperationValue/anyOf/0/anyOf/1/type",
                                  keyword: "type",
                                  params: { type: "number" },
                                  message: "must be number",
                                };
                                if (vErrors === null) {
                                  vErrors = [err6];
                                } else {
                                  vErrors.push(err6);
                                }
                                errors++;
                              }
                              var _valid1 = _errs21 === errors;
                              valid5 = valid5 || _valid1;
                              const _errs23 = errors;
                              if (typeof data5 !== "boolean") {
                                const err7 = {
                                  instancePath:
                                    instancePath +
                                    "/values/" +
                                    key2
                                      .replace(/~/g, "~0")
                                      .replace(/\//g, "~1"),
                                  schemaPath:
                                    "#/$defs/OperationValue/anyOf/0/anyOf/2/type",
                                  keyword: "type",
                                  params: { type: "boolean" },
                                  message: "must be boolean",
                                };
                                if (vErrors === null) {
                                  vErrors = [err7];
                                } else {
                                  vErrors.push(err7);
                                }
                                errors++;
                              }
                              var _valid1 = _errs23 === errors;
                              valid5 = valid5 || _valid1;
                              const _errs25 = errors;
                              if (data5 !== null) {
                                const err8 = {
                                  instancePath:
                                    instancePath +
                                    "/values/" +
                                    key2
                                      .replace(/~/g, "~0")
                                      .replace(/\//g, "~1"),
                                  schemaPath:
                                    "#/$defs/OperationValue/anyOf/0/anyOf/3/type",
                                  keyword: "type",
                                  params: { type: "null" },
                                  message: "must be null",
                                };
                                if (vErrors === null) {
                                  vErrors = [err8];
                                } else {
                                  vErrors.push(err8);
                                }
                                errors++;
                              }
                              var _valid1 = _errs25 === errors;
                              valid5 = valid5 || _valid1;
                              if (!valid5) {
                                const err9 = {
                                  instancePath:
                                    instancePath +
                                    "/values/" +
                                    key2
                                      .replace(/~/g, "~0")
                                      .replace(/\//g, "~1"),
                                  schemaPath:
                                    "#/$defs/OperationValue/anyOf/0/anyOf",
                                  keyword: "anyOf",
                                  params: {},
                                  message: "must match a schema in anyOf",
                                };
                                if (vErrors === null) {
                                  vErrors = [err9];
                                } else {
                                  vErrors.push(err9);
                                }
                                errors++;
                              } else {
                                errors = _errs18;
                                if (vErrors !== null) {
                                  if (_errs18) {
                                    vErrors.length = _errs18;
                                  } else {
                                    vErrors = null;
                                  }
                                }
                              }
                              var _valid0 = _errs17 === errors;
                              valid4 = valid4 || _valid0;
                              const _errs27 = errors;
                              if (errors === _errs27) {
                                if (Array.isArray(data5)) {
                                  if (data5.length > 128) {
                                    const err10 = {
                                      instancePath:
                                        instancePath +
                                        "/values/" +
                                        key2
                                          .replace(/~/g, "~0")
                                          .replace(/\//g, "~1"),
                                      schemaPath:
                                        "#/$defs/OperationValue/anyOf/1/maxItems",
                                      keyword: "maxItems",
                                      params: { limit: 128 },
                                      message:
                                        "must NOT have more than 128 items",
                                    };
                                    if (vErrors === null) {
                                      vErrors = [err10];
                                    } else {
                                      vErrors.push(err10);
                                    }
                                    errors++;
                                  } else {
                                    var valid6 = true;
                                    const len0 = data5.length;
                                    for (let i0 = 0; i0 < len0; i0++) {
                                      let data6 = data5[i0];
                                      const _errs29 = errors;
                                      if (errors === _errs29) {
                                        if (typeof data6 === "string") {
                                          if (func2(data6) > 4096) {
                                            const err11 = {
                                              instancePath:
                                                instancePath +
                                                "/values/" +
                                                key2
                                                  .replace(/~/g, "~0")
                                                  .replace(/\//g, "~1") +
                                                "/" +
                                                i0,
                                              schemaPath:
                                                "#/$defs/OperationValue/anyOf/1/items/maxLength",
                                              keyword: "maxLength",
                                              params: { limit: 4096 },
                                              message:
                                                "must NOT have more than 4096 characters",
                                            };
                                            if (vErrors === null) {
                                              vErrors = [err11];
                                            } else {
                                              vErrors.push(err11);
                                            }
                                            errors++;
                                          }
                                        } else {
                                          const err12 = {
                                            instancePath:
                                              instancePath +
                                              "/values/" +
                                              key2
                                                .replace(/~/g, "~0")
                                                .replace(/\//g, "~1") +
                                              "/" +
                                              i0,
                                            schemaPath:
                                              "#/$defs/OperationValue/anyOf/1/items/type",
                                            keyword: "type",
                                            params: { type: "string" },
                                            message: "must be string",
                                          };
                                          if (vErrors === null) {
                                            vErrors = [err12];
                                          } else {
                                            vErrors.push(err12);
                                          }
                                          errors++;
                                        }
                                      }
                                      var valid6 = _errs29 === errors;
                                      if (!valid6) {
                                        break;
                                      }
                                    }
                                  }
                                } else {
                                  const err13 = {
                                    instancePath:
                                      instancePath +
                                      "/values/" +
                                      key2
                                        .replace(/~/g, "~0")
                                        .replace(/\//g, "~1"),
                                    schemaPath:
                                      "#/$defs/OperationValue/anyOf/1/type",
                                    keyword: "type",
                                    params: { type: "array" },
                                    message: "must be array",
                                  };
                                  if (vErrors === null) {
                                    vErrors = [err13];
                                  } else {
                                    vErrors.push(err13);
                                  }
                                  errors++;
                                }
                              }
                              var _valid0 = _errs27 === errors;
                              valid4 = valid4 || _valid0;
                              if (!valid4) {
                                const err14 = {
                                  instancePath:
                                    instancePath +
                                    "/values/" +
                                    key2
                                      .replace(/~/g, "~0")
                                      .replace(/\//g, "~1"),
                                  schemaPath: "#/$defs/OperationValue/anyOf",
                                  keyword: "anyOf",
                                  params: {},
                                  message: "must match a schema in anyOf",
                                };
                                if (vErrors === null) {
                                  vErrors = [err14];
                                } else {
                                  vErrors.push(err14);
                                }
                                errors++;
                                validate21.errors = vErrors;
                                return false;
                              } else {
                                errors = _errs16;
                                if (vErrors !== null) {
                                  if (_errs16) {
                                    vErrors.length = _errs16;
                                  } else {
                                    vErrors = null;
                                  }
                                }
                              }
                              var valid2 = _errs14 === errors;
                              if (!valid2) {
                                break;
                              }
                            }
                          }
                        }
                      } else {
                        validate21.errors = [
                          {
                            instancePath: instancePath + "/values",
                            schemaPath: "#/properties/values/type",
                            keyword: "type",
                            params: { type: "object" },
                            message: "must be object",
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
                    if (data.selection !== undefined) {
                      let data7 = data.selection;
                      const _errs31 = errors;
                      if (errors === _errs31) {
                        if (Array.isArray(data7)) {
                          if (data7.length > 128) {
                            validate21.errors = [
                              {
                                instancePath: instancePath + "/selection",
                                schemaPath: "#/properties/selection/maxItems",
                                keyword: "maxItems",
                                params: { limit: 128 },
                                message: "must NOT have more than 128 items",
                              },
                            ];
                            return false;
                          } else {
                            var valid7 = true;
                            const len1 = data7.length;
                            for (let i1 = 0; i1 < len1; i1++) {
                              let data8 = data7[i1];
                              const _errs33 = errors;
                              if (errors === _errs33) {
                                if (typeof data8 === "string") {
                                  if (func2(data8) > 128) {
                                    validate21.errors = [
                                      {
                                        instancePath:
                                          instancePath + "/selection/" + i1,
                                        schemaPath:
                                          "#/properties/selection/items/maxLength",
                                        keyword: "maxLength",
                                        params: { limit: 128 },
                                        message:
                                          "must NOT have more than 128 characters",
                                      },
                                    ];
                                    return false;
                                  } else {
                                    if (func2(data8) < 1) {
                                      validate21.errors = [
                                        {
                                          instancePath:
                                            instancePath + "/selection/" + i1,
                                          schemaPath:
                                            "#/properties/selection/items/minLength",
                                          keyword: "minLength",
                                          params: { limit: 1 },
                                          message:
                                            "must NOT have fewer than 1 characters",
                                        },
                                      ];
                                      return false;
                                    } else {
                                      if (!pattern4.test(data8)) {
                                        validate21.errors = [
                                          {
                                            instancePath:
                                              instancePath + "/selection/" + i1,
                                            schemaPath:
                                              "#/properties/selection/items/pattern",
                                            keyword: "pattern",
                                            params: {
                                              pattern: "^[a-zA-Z0-9_-]+$",
                                            },
                                            message:
                                              'must match pattern "' +
                                              "^[a-zA-Z0-9_-]+$" +
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
                                      instancePath:
                                        instancePath + "/selection/" + i1,
                                      schemaPath:
                                        "#/properties/selection/items/type",
                                      keyword: "type",
                                      params: { type: "string" },
                                      message: "must be string",
                                    },
                                  ];
                                  return false;
                                }
                              }
                              var valid7 = _errs33 === errors;
                              if (!valid7) {
                                break;
                              }
                            }
                          }
                        } else {
                          validate21.errors = [
                            {
                              instancePath: instancePath + "/selection",
                              schemaPath: "#/properties/selection/type",
                              keyword: "type",
                              params: { type: "array" },
                              message: "must be array",
                            },
                          ];
                          return false;
                        }
                      }
                      var valid0 = _errs31 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.secretRefs !== undefined) {
                        let data9 = data.secretRefs;
                        const _errs35 = errors;
                        if (errors === _errs35) {
                          if (
                            data9 &&
                            typeof data9 == "object" &&
                            !Array.isArray(data9)
                          ) {
                            if (Object.keys(data9).length > 128) {
                              validate21.errors = [
                                {
                                  instancePath: instancePath + "/secretRefs",
                                  schemaPath:
                                    "#/properties/secretRefs/maxProperties",
                                  keyword: "maxProperties",
                                  params: { limit: 128 },
                                  message:
                                    "must NOT have more than 128 properties",
                                },
                              ];
                              return false;
                            } else {
                              for (const key3 in data9) {
                                const _errs37 = errors;
                                if (errors === _errs37) {
                                  if (typeof key3 === "string") {
                                    if (func2(key3) > 128) {
                                      const err15 = {
                                        instancePath:
                                          instancePath + "/secretRefs",
                                        schemaPath:
                                          "#/properties/secretRefs/propertyNames/maxLength",
                                        keyword: "maxLength",
                                        params: { limit: 128 },
                                        message:
                                          "must NOT have more than 128 characters",
                                        propertyName: key3,
                                      };
                                      if (vErrors === null) {
                                        vErrors = [err15];
                                      } else {
                                        vErrors.push(err15);
                                      }
                                      errors++;
                                    } else {
                                      if (func2(key3) < 1) {
                                        const err16 = {
                                          instancePath:
                                            instancePath + "/secretRefs",
                                          schemaPath:
                                            "#/properties/secretRefs/propertyNames/minLength",
                                          keyword: "minLength",
                                          params: { limit: 1 },
                                          message:
                                            "must NOT have fewer than 1 characters",
                                          propertyName: key3,
                                        };
                                        if (vErrors === null) {
                                          vErrors = [err16];
                                        } else {
                                          vErrors.push(err16);
                                        }
                                        errors++;
                                      } else {
                                        if (!pattern4.test(key3)) {
                                          const err17 = {
                                            instancePath:
                                              instancePath + "/secretRefs",
                                            schemaPath:
                                              "#/properties/secretRefs/propertyNames/pattern",
                                            keyword: "pattern",
                                            params: {
                                              pattern: "^[a-zA-Z0-9_-]+$",
                                            },
                                            message:
                                              'must match pattern "' +
                                              "^[a-zA-Z0-9_-]+$" +
                                              '"',
                                            propertyName: key3,
                                          };
                                          if (vErrors === null) {
                                            vErrors = [err17];
                                          } else {
                                            vErrors.push(err17);
                                          }
                                          errors++;
                                        }
                                      }
                                    }
                                  } else {
                                    const err18 = {
                                      instancePath:
                                        instancePath + "/secretRefs",
                                      schemaPath:
                                        "#/properties/secretRefs/propertyNames/type",
                                      keyword: "type",
                                      params: { type: "string" },
                                      message: "must be string",
                                      propertyName: key3,
                                    };
                                    if (vErrors === null) {
                                      vErrors = [err18];
                                    } else {
                                      vErrors.push(err18);
                                    }
                                    errors++;
                                  }
                                }
                                var valid8 = _errs37 === errors;
                                if (!valid8) {
                                  const err19 = {
                                    instancePath: instancePath + "/secretRefs",
                                    schemaPath:
                                      "#/properties/secretRefs/propertyNames",
                                    keyword: "propertyNames",
                                    params: { propertyName: key3 },
                                    message: "property name must be valid",
                                  };
                                  if (vErrors === null) {
                                    vErrors = [err19];
                                  } else {
                                    vErrors.push(err19);
                                  }
                                  errors++;
                                  validate21.errors = vErrors;
                                  return false;
                                  break;
                                }
                              }
                              if (valid8) {
                                for (const key4 in data9) {
                                  let data10 = data9[key4];
                                  const _errs40 = errors;
                                  if (errors === _errs40) {
                                    if (typeof data10 === "string") {
                                      if (func2(data10) > 128) {
                                        validate21.errors = [
                                          {
                                            instancePath:
                                              instancePath +
                                              "/secretRefs/" +
                                              key4
                                                .replace(/~/g, "~0")
                                                .replace(/\//g, "~1"),
                                            schemaPath:
                                              "#/properties/secretRefs/additionalProperties/maxLength",
                                            keyword: "maxLength",
                                            params: { limit: 128 },
                                            message:
                                              "must NOT have more than 128 characters",
                                          },
                                        ];
                                        return false;
                                      } else {
                                        if (func2(data10) < 1) {
                                          validate21.errors = [
                                            {
                                              instancePath:
                                                instancePath +
                                                "/secretRefs/" +
                                                key4
                                                  .replace(/~/g, "~0")
                                                  .replace(/\//g, "~1"),
                                              schemaPath:
                                                "#/properties/secretRefs/additionalProperties/minLength",
                                              keyword: "minLength",
                                              params: { limit: 1 },
                                              message:
                                                "must NOT have fewer than 1 characters",
                                            },
                                          ];
                                          return false;
                                        } else {
                                          if (!pattern4.test(data10)) {
                                            validate21.errors = [
                                              {
                                                instancePath:
                                                  instancePath +
                                                  "/secretRefs/" +
                                                  key4
                                                    .replace(/~/g, "~0")
                                                    .replace(/\//g, "~1"),
                                                schemaPath:
                                                  "#/properties/secretRefs/additionalProperties/pattern",
                                                keyword: "pattern",
                                                params: {
                                                  pattern: "^[a-zA-Z0-9_-]+$",
                                                },
                                                message:
                                                  'must match pattern "' +
                                                  "^[a-zA-Z0-9_-]+$" +
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
                                          instancePath:
                                            instancePath +
                                            "/secretRefs/" +
                                            key4
                                              .replace(/~/g, "~0")
                                              .replace(/\//g, "~1"),
                                          schemaPath:
                                            "#/properties/secretRefs/additionalProperties/type",
                                          keyword: "type",
                                          params: { type: "string" },
                                          message: "must be string",
                                        },
                                      ];
                                      return false;
                                    }
                                  }
                                  var valid9 = _errs40 === errors;
                                  if (!valid9) {
                                    break;
                                  }
                                }
                              }
                            }
                          } else {
                            validate21.errors = [
                              {
                                instancePath: instancePath + "/secretRefs",
                                schemaPath: "#/properties/secretRefs/type",
                                keyword: "type",
                                params: { type: "object" },
                                message: "must be object",
                              },
                            ];
                            return false;
                          }
                        }
                        var valid0 = _errs35 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.interactionId !== undefined) {
                          let data11 = data.interactionId;
                          const _errs42 = errors;
                          if (errors === _errs42) {
                            if (typeof data11 === "string") {
                              if (func2(data11) > 128) {
                                validate21.errors = [
                                  {
                                    instancePath:
                                      instancePath + "/interactionId",
                                    schemaPath:
                                      "#/properties/interactionId/maxLength",
                                    keyword: "maxLength",
                                    params: { limit: 128 },
                                    message:
                                      "must NOT have more than 128 characters",
                                  },
                                ];
                                return false;
                              } else {
                                if (func2(data11) < 1) {
                                  validate21.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/interactionId",
                                      schemaPath:
                                        "#/properties/interactionId/minLength",
                                      keyword: "minLength",
                                      params: { limit: 1 },
                                      message:
                                        "must NOT have fewer than 1 characters",
                                    },
                                  ];
                                  return false;
                                } else {
                                  if (!pattern4.test(data11)) {
                                    validate21.errors = [
                                      {
                                        instancePath:
                                          instancePath + "/interactionId",
                                        schemaPath:
                                          "#/properties/interactionId/pattern",
                                        keyword: "pattern",
                                        params: { pattern: "^[a-zA-Z0-9_-]+$" },
                                        message:
                                          'must match pattern "' +
                                          "^[a-zA-Z0-9_-]+$" +
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
                                  instancePath: instancePath + "/interactionId",
                                  schemaPath: "#/properties/interactionId/type",
                                  keyword: "type",
                                  params: { type: "string" },
                                  message: "must be string",
                                },
                              ];
                              return false;
                            }
                          }
                          var valid0 = _errs42 === errors;
                        } else {
                          var valid0 = true;
                        }
                        if (valid0) {
                          if (data.answer !== undefined) {
                            let data12 = data.answer;
                            const _errs44 = errors;
                            if (errors === _errs44) {
                              if (typeof data12 === "string") {
                                if (func2(data12) > 128) {
                                  validate21.errors = [
                                    {
                                      instancePath: instancePath + "/answer",
                                      schemaPath:
                                        "#/properties/answer/maxLength",
                                      keyword: "maxLength",
                                      params: { limit: 128 },
                                      message:
                                        "must NOT have more than 128 characters",
                                    },
                                  ];
                                  return false;
                                } else {
                                  if (func2(data12) < 1) {
                                    validate21.errors = [
                                      {
                                        instancePath: instancePath + "/answer",
                                        schemaPath:
                                          "#/properties/answer/minLength",
                                        keyword: "minLength",
                                        params: { limit: 1 },
                                        message:
                                          "must NOT have fewer than 1 characters",
                                      },
                                    ];
                                    return false;
                                  } else {
                                    if (!pattern4.test(data12)) {
                                      validate21.errors = [
                                        {
                                          instancePath:
                                            instancePath + "/answer",
                                          schemaPath:
                                            "#/properties/answer/pattern",
                                          keyword: "pattern",
                                          params: {
                                            pattern: "^[a-zA-Z0-9_-]+$",
                                          },
                                          message:
                                            'must match pattern "' +
                                            "^[a-zA-Z0-9_-]+$" +
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
                                    instancePath: instancePath + "/answer",
                                    schemaPath: "#/properties/answer/type",
                                    keyword: "type",
                                    params: { type: "string" },
                                    message: "must be string",
                                  },
                                ];
                                return false;
                              }
                            }
                            var valid0 = _errs44 === errors;
                          } else {
                            var valid0 = true;
                          }
                        }
                      }
                    }
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
  if (
    !validate21(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate21.errors : vErrors.concat(validate21.errors);
    errors = vErrors.length;
  }
  validate20.errors = vErrors;
  return errors === 0;
}
validate20.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
