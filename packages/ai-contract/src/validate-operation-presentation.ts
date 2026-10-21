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
      additionalProperties: true,
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
      additionalProperties: true,
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
      additionalProperties: true,
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
            "execution_ended",
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
        expiresAtMs: { type: "integer", minimum: 0, maximum: 9007199254740991 },
        presentation: {
          type: "object",
          properties: {
            title: { type: "string", maxLength: 256 },
            targetLabel: { type: "string", maxLength: 256 },
            source: { type: "string", maxLength: 256 },
            impact: { type: "string", maxLength: 4096 },
          },
          required: ["title", "targetLabel", "source", "impact"],
          additionalProperties: true,
        },
        secretFields: {
          type: "array",
          items: { type: "string", minLength: 1, maxLength: 128 },
          maxItems: 128,
          uniqueItems: true,
        },
      },
      required: [
        "scope",
        "revision",
        "baseline",
        "current",
        "proposal",
        "phase",
        "allowed",
        "approval",
        "objects",
        "logs",
        "presentation",
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
  $ref: "#/$defs/OperationPresentation",
};
const schema32 = {
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
  additionalProperties: true,
};
const schema33 = {
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
  additionalProperties: true,
};
const schema34 = {
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
};
const func1 = helper0;
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
        (data.version === undefined && (missing0 = "version")) ||
        (data.template === undefined && (missing0 = "template")) ||
        (data.fields === undefined && (missing0 = "fields")) ||
        (data.rules === undefined && (missing0 = "rules")) ||
        (data.sections === undefined && (missing0 = "sections"))
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
        if (data.version !== undefined) {
          const _errs2 = errors;
          if (1 !== data.version) {
            validate21.errors = [
              {
                instancePath: instancePath + "/version",
                schemaPath: "#/properties/version/const",
                keyword: "const",
                params: { allowedValue: 1 },
                message: "must be equal to constant",
              },
            ];
            return false;
          }
          var valid0 = _errs2 === errors;
        } else {
          var valid0 = true;
        }
        if (valid0) {
          if (data.template !== undefined) {
            let data1 = data.template;
            const _errs3 = errors;
            if (
              !(
                data1 === "SettingsForm" ||
                data1 === "ObjectActions" ||
                data1 === "ResourceSelection" ||
                data1 === "ChangeReview" ||
                data1 === "TaskExecution" ||
                data1 === "GuidedFlow" ||
                data1 === "OperationApproval"
              )
            ) {
              validate21.errors = [
                {
                  instancePath: instancePath + "/template",
                  schemaPath: "#/properties/template/enum",
                  keyword: "enum",
                  params: { allowedValues: schema32.properties.template.enum },
                  message: "must be equal to one of the allowed values",
                },
              ];
              return false;
            }
            var valid0 = _errs3 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.fields !== undefined) {
              let data2 = data.fields;
              const _errs4 = errors;
              if (errors === _errs4) {
                if (Array.isArray(data2)) {
                  if (data2.length > 128) {
                    validate21.errors = [
                      {
                        instancePath: instancePath + "/fields",
                        schemaPath: "#/properties/fields/maxItems",
                        keyword: "maxItems",
                        params: { limit: 128 },
                        message: "must NOT have more than 128 items",
                      },
                    ];
                    return false;
                  } else {
                    var valid1 = true;
                    const len0 = data2.length;
                    for (let i0 = 0; i0 < len0; i0++) {
                      let data3 = data2[i0];
                      const _errs6 = errors;
                      const _errs7 = errors;
                      if (errors === _errs7) {
                        if (
                          data3 &&
                          typeof data3 == "object" &&
                          !Array.isArray(data3)
                        ) {
                          let missing1;
                          if (
                            (data3.id === undefined && (missing1 = "id")) ||
                            (data3.label === undefined &&
                              (missing1 = "label")) ||
                            (data3.component === undefined &&
                              (missing1 = "component"))
                          ) {
                            validate21.errors = [
                              {
                                instancePath: instancePath + "/fields/" + i0,
                                schemaPath: "#/$defs/OperationField/required",
                                keyword: "required",
                                params: { missingProperty: missing1 },
                                message:
                                  "must have required property '" +
                                  missing1 +
                                  "'",
                              },
                            ];
                            return false;
                          } else {
                            if (data3.id !== undefined) {
                              let data4 = data3.id;
                              const _errs10 = errors;
                              if (errors === _errs10) {
                                if (typeof data4 === "string") {
                                  if (func1(data4) > 128) {
                                    validate21.errors = [
                                      {
                                        instancePath:
                                          instancePath +
                                          "/fields/" +
                                          i0 +
                                          "/id",
                                        schemaPath:
                                          "#/$defs/OperationField/properties/id/maxLength",
                                        keyword: "maxLength",
                                        params: { limit: 128 },
                                        message:
                                          "must NOT have more than 128 characters",
                                      },
                                    ];
                                    return false;
                                  } else {
                                    if (func1(data4) < 1) {
                                      validate21.errors = [
                                        {
                                          instancePath:
                                            instancePath +
                                            "/fields/" +
                                            i0 +
                                            "/id",
                                          schemaPath:
                                            "#/$defs/OperationField/properties/id/minLength",
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
                                            instancePath:
                                              instancePath +
                                              "/fields/" +
                                              i0 +
                                              "/id",
                                            schemaPath:
                                              "#/$defs/OperationField/properties/id/pattern",
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
                                        instancePath + "/fields/" + i0 + "/id",
                                      schemaPath:
                                        "#/$defs/OperationField/properties/id/type",
                                      keyword: "type",
                                      params: { type: "string" },
                                      message: "must be string",
                                    },
                                  ];
                                  return false;
                                }
                              }
                              var valid3 = _errs10 === errors;
                            } else {
                              var valid3 = true;
                            }
                            if (valid3) {
                              if (data3.label !== undefined) {
                                let data5 = data3.label;
                                const _errs12 = errors;
                                if (errors === _errs12) {
                                  if (typeof data5 === "string") {
                                    if (func1(data5) > 256) {
                                      validate21.errors = [
                                        {
                                          instancePath:
                                            instancePath +
                                            "/fields/" +
                                            i0 +
                                            "/label",
                                          schemaPath:
                                            "#/$defs/OperationField/properties/label/maxLength",
                                          keyword: "maxLength",
                                          params: { limit: 256 },
                                          message:
                                            "must NOT have more than 256 characters",
                                        },
                                      ];
                                      return false;
                                    }
                                  } else {
                                    validate21.errors = [
                                      {
                                        instancePath:
                                          instancePath +
                                          "/fields/" +
                                          i0 +
                                          "/label",
                                        schemaPath:
                                          "#/$defs/OperationField/properties/label/type",
                                        keyword: "type",
                                        params: { type: "string" },
                                        message: "must be string",
                                      },
                                    ];
                                    return false;
                                  }
                                }
                                var valid3 = _errs12 === errors;
                              } else {
                                var valid3 = true;
                              }
                              if (valid3) {
                                if (data3.component !== undefined) {
                                  let data6 = data3.component;
                                  const _errs14 = errors;
                                  if (
                                    !(
                                      data6 === "TextField" ||
                                      data6 === "NumberField" ||
                                      data6 === "SelectField" ||
                                      data6 === "Switch" ||
                                      data6 === "Checkbox" ||
                                      data6 === "RadioGroup" ||
                                      data6 === "PathPicker" ||
                                      data6 === "DateTimeField" ||
                                      data6 === "SecretField"
                                    )
                                  ) {
                                    validate21.errors = [
                                      {
                                        instancePath:
                                          instancePath +
                                          "/fields/" +
                                          i0 +
                                          "/component",
                                        schemaPath:
                                          "#/$defs/OperationField/properties/component/enum",
                                        keyword: "enum",
                                        params: {
                                          allowedValues:
                                            schema33.properties.component.enum,
                                        },
                                        message:
                                          "must be equal to one of the allowed values",
                                      },
                                    ];
                                    return false;
                                  }
                                  var valid3 = _errs14 === errors;
                                } else {
                                  var valid3 = true;
                                }
                                if (valid3) {
                                  if (data3.required !== undefined) {
                                    const _errs15 = errors;
                                    if (typeof data3.required !== "boolean") {
                                      validate21.errors = [
                                        {
                                          instancePath:
                                            instancePath +
                                            "/fields/" +
                                            i0 +
                                            "/required",
                                          schemaPath:
                                            "#/$defs/OperationField/properties/required/type",
                                          keyword: "type",
                                          params: { type: "boolean" },
                                          message: "must be boolean",
                                        },
                                      ];
                                      return false;
                                    }
                                    var valid3 = _errs15 === errors;
                                  } else {
                                    var valid3 = true;
                                  }
                                  if (valid3) {
                                    if (data3.readonly !== undefined) {
                                      const _errs17 = errors;
                                      if (typeof data3.readonly !== "boolean") {
                                        validate21.errors = [
                                          {
                                            instancePath:
                                              instancePath +
                                              "/fields/" +
                                              i0 +
                                              "/readonly",
                                            schemaPath:
                                              "#/$defs/OperationField/properties/readonly/type",
                                            keyword: "type",
                                            params: { type: "boolean" },
                                            message: "must be boolean",
                                          },
                                        ];
                                        return false;
                                      }
                                      var valid3 = _errs17 === errors;
                                    } else {
                                      var valid3 = true;
                                    }
                                    if (valid3) {
                                      if (data3.reason !== undefined) {
                                        let data9 = data3.reason;
                                        const _errs19 = errors;
                                        if (errors === _errs19) {
                                          if (typeof data9 === "string") {
                                            if (func1(data9) > 4096) {
                                              validate21.errors = [
                                                {
                                                  instancePath:
                                                    instancePath +
                                                    "/fields/" +
                                                    i0 +
                                                    "/reason",
                                                  schemaPath:
                                                    "#/$defs/OperationField/properties/reason/maxLength",
                                                  keyword: "maxLength",
                                                  params: { limit: 4096 },
                                                  message:
                                                    "must NOT have more than 4096 characters",
                                                },
                                              ];
                                              return false;
                                            }
                                          } else {
                                            validate21.errors = [
                                              {
                                                instancePath:
                                                  instancePath +
                                                  "/fields/" +
                                                  i0 +
                                                  "/reason",
                                                schemaPath:
                                                  "#/$defs/OperationField/properties/reason/type",
                                                keyword: "type",
                                                params: { type: "string" },
                                                message: "must be string",
                                              },
                                            ];
                                            return false;
                                          }
                                        }
                                        var valid3 = _errs19 === errors;
                                      } else {
                                        var valid3 = true;
                                      }
                                      if (valid3) {
                                        if (data3.min !== undefined) {
                                          let data10 = data3.min;
                                          const _errs21 = errors;
                                          if (
                                            !(
                                              typeof data10 == "number" &&
                                              isFinite(data10)
                                            )
                                          ) {
                                            validate21.errors = [
                                              {
                                                instancePath:
                                                  instancePath +
                                                  "/fields/" +
                                                  i0 +
                                                  "/min",
                                                schemaPath:
                                                  "#/$defs/OperationField/properties/min/type",
                                                keyword: "type",
                                                params: { type: "number" },
                                                message: "must be number",
                                              },
                                            ];
                                            return false;
                                          }
                                          var valid3 = _errs21 === errors;
                                        } else {
                                          var valid3 = true;
                                        }
                                        if (valid3) {
                                          if (data3.max !== undefined) {
                                            let data11 = data3.max;
                                            const _errs23 = errors;
                                            if (
                                              !(
                                                typeof data11 == "number" &&
                                                isFinite(data11)
                                              )
                                            ) {
                                              validate21.errors = [
                                                {
                                                  instancePath:
                                                    instancePath +
                                                    "/fields/" +
                                                    i0 +
                                                    "/max",
                                                  schemaPath:
                                                    "#/$defs/OperationField/properties/max/type",
                                                  keyword: "type",
                                                  params: { type: "number" },
                                                  message: "must be number",
                                                },
                                              ];
                                              return false;
                                            }
                                            var valid3 = _errs23 === errors;
                                          } else {
                                            var valid3 = true;
                                          }
                                          if (valid3) {
                                            if (data3.maxLength !== undefined) {
                                              let data12 = data3.maxLength;
                                              const _errs25 = errors;
                                              if (
                                                !(
                                                  typeof data12 == "number" &&
                                                  !(data12 % 1) &&
                                                  !isNaN(data12) &&
                                                  isFinite(data12)
                                                )
                                              ) {
                                                validate21.errors = [
                                                  {
                                                    instancePath:
                                                      instancePath +
                                                      "/fields/" +
                                                      i0 +
                                                      "/maxLength",
                                                    schemaPath:
                                                      "#/$defs/OperationField/properties/maxLength/type",
                                                    keyword: "type",
                                                    params: { type: "integer" },
                                                    message: "must be integer",
                                                  },
                                                ];
                                                return false;
                                              }
                                              if (errors === _errs25) {
                                                if (
                                                  typeof data12 == "number" &&
                                                  isFinite(data12)
                                                ) {
                                                  if (
                                                    data12 > 8192 ||
                                                    isNaN(data12)
                                                  ) {
                                                    validate21.errors = [
                                                      {
                                                        instancePath:
                                                          instancePath +
                                                          "/fields/" +
                                                          i0 +
                                                          "/maxLength",
                                                        schemaPath:
                                                          "#/$defs/OperationField/properties/maxLength/maximum",
                                                        keyword: "maximum",
                                                        params: {
                                                          comparison: "<=",
                                                          limit: 8192,
                                                        },
                                                        message:
                                                          "must be <= 8192",
                                                      },
                                                    ];
                                                    return false;
                                                  } else {
                                                    if (
                                                      data12 < 1 ||
                                                      isNaN(data12)
                                                    ) {
                                                      validate21.errors = [
                                                        {
                                                          instancePath:
                                                            instancePath +
                                                            "/fields/" +
                                                            i0 +
                                                            "/maxLength",
                                                          schemaPath:
                                                            "#/$defs/OperationField/properties/maxLength/minimum",
                                                          keyword: "minimum",
                                                          params: {
                                                            comparison: ">=",
                                                            limit: 1,
                                                          },
                                                          message:
                                                            "must be >= 1",
                                                        },
                                                      ];
                                                      return false;
                                                    }
                                                  }
                                                }
                                              }
                                              var valid3 = _errs25 === errors;
                                            } else {
                                              var valid3 = true;
                                            }
                                            if (valid3) {
                                              if (data3.options !== undefined) {
                                                let data13 = data3.options;
                                                const _errs27 = errors;
                                                if (errors === _errs27) {
                                                  if (Array.isArray(data13)) {
                                                    if (data13.length > 128) {
                                                      validate21.errors = [
                                                        {
                                                          instancePath:
                                                            instancePath +
                                                            "/fields/" +
                                                            i0 +
                                                            "/options",
                                                          schemaPath:
                                                            "#/$defs/OperationField/properties/options/maxItems",
                                                          keyword: "maxItems",
                                                          params: {
                                                            limit: 128,
                                                          },
                                                          message:
                                                            "must NOT have more than 128 items",
                                                        },
                                                      ];
                                                      return false;
                                                    } else {
                                                      var valid4 = true;
                                                      const len1 =
                                                        data13.length;
                                                      for (
                                                        let i1 = 0;
                                                        i1 < len1;
                                                        i1++
                                                      ) {
                                                        let data14 = data13[i1];
                                                        const _errs29 = errors;
                                                        if (
                                                          errors === _errs29
                                                        ) {
                                                          if (
                                                            data14 &&
                                                            typeof data14 ==
                                                              "object" &&
                                                            !Array.isArray(
                                                              data14,
                                                            )
                                                          ) {
                                                            let missing2;
                                                            if (
                                                              (data14.id ===
                                                                undefined &&
                                                                (missing2 =
                                                                  "id")) ||
                                                              (data14.label ===
                                                                undefined &&
                                                                (missing2 =
                                                                  "label"))
                                                            ) {
                                                              validate21.errors =
                                                                [
                                                                  {
                                                                    instancePath:
                                                                      instancePath +
                                                                      "/fields/" +
                                                                      i0 +
                                                                      "/options/" +
                                                                      i1,
                                                                    schemaPath:
                                                                      "#/$defs/OperationField/properties/options/items/required",
                                                                    keyword:
                                                                      "required",
                                                                    params: {
                                                                      missingProperty:
                                                                        missing2,
                                                                    },
                                                                    message:
                                                                      "must have required property '" +
                                                                      missing2 +
                                                                      "'",
                                                                  },
                                                                ];
                                                              return false;
                                                            } else {
                                                              const _errs31 =
                                                                errors;
                                                              for (const key0 in data14) {
                                                                if (
                                                                  !(
                                                                    key0 ===
                                                                      "id" ||
                                                                    key0 ===
                                                                      "label" ||
                                                                    key0 ===
                                                                      "disabledReason"
                                                                  )
                                                                ) {
                                                                  validate21.errors =
                                                                    [
                                                                      {
                                                                        instancePath:
                                                                          instancePath +
                                                                          "/fields/" +
                                                                          i0 +
                                                                          "/options/" +
                                                                          i1,
                                                                        schemaPath:
                                                                          "#/$defs/OperationField/properties/options/items/additionalProperties",
                                                                        keyword:
                                                                          "additionalProperties",
                                                                        params:
                                                                          {
                                                                            additionalProperty:
                                                                              key0,
                                                                          },
                                                                        message:
                                                                          "must NOT have additional properties",
                                                                      },
                                                                    ];
                                                                  return false;
                                                                  break;
                                                                }
                                                              }
                                                              if (
                                                                _errs31 ===
                                                                errors
                                                              ) {
                                                                if (
                                                                  data14.id !==
                                                                  undefined
                                                                ) {
                                                                  let data15 =
                                                                    data14.id;
                                                                  const _errs32 =
                                                                    errors;
                                                                  if (
                                                                    errors ===
                                                                    _errs32
                                                                  ) {
                                                                    if (
                                                                      typeof data15 ===
                                                                      "string"
                                                                    ) {
                                                                      if (
                                                                        func1(
                                                                          data15,
                                                                        ) > 128
                                                                      ) {
                                                                        validate21.errors =
                                                                          [
                                                                            {
                                                                              instancePath:
                                                                                instancePath +
                                                                                "/fields/" +
                                                                                i0 +
                                                                                "/options/" +
                                                                                i1 +
                                                                                "/id",
                                                                              schemaPath:
                                                                                "#/$defs/OperationField/properties/options/items/properties/id/maxLength",
                                                                              keyword:
                                                                                "maxLength",
                                                                              params:
                                                                                {
                                                                                  limit: 128,
                                                                                },
                                                                              message:
                                                                                "must NOT have more than 128 characters",
                                                                            },
                                                                          ];
                                                                        return false;
                                                                      } else {
                                                                        if (
                                                                          func1(
                                                                            data15,
                                                                          ) < 1
                                                                        ) {
                                                                          validate21.errors =
                                                                            [
                                                                              {
                                                                                instancePath:
                                                                                  instancePath +
                                                                                  "/fields/" +
                                                                                  i0 +
                                                                                  "/options/" +
                                                                                  i1 +
                                                                                  "/id",
                                                                                schemaPath:
                                                                                  "#/$defs/OperationField/properties/options/items/properties/id/minLength",
                                                                                keyword:
                                                                                  "minLength",
                                                                                params:
                                                                                  {
                                                                                    limit: 1,
                                                                                  },
                                                                                message:
                                                                                  "must NOT have fewer than 1 characters",
                                                                              },
                                                                            ];
                                                                          return false;
                                                                        } else {
                                                                          if (
                                                                            !pattern4.test(
                                                                              data15,
                                                                            )
                                                                          ) {
                                                                            validate21.errors =
                                                                              [
                                                                                {
                                                                                  instancePath:
                                                                                    instancePath +
                                                                                    "/fields/" +
                                                                                    i0 +
                                                                                    "/options/" +
                                                                                    i1 +
                                                                                    "/id",
                                                                                  schemaPath:
                                                                                    "#/$defs/OperationField/properties/options/items/properties/id/pattern",
                                                                                  keyword:
                                                                                    "pattern",
                                                                                  params:
                                                                                    {
                                                                                      pattern:
                                                                                        "^[a-zA-Z0-9_-]+$",
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
                                                                      validate21.errors =
                                                                        [
                                                                          {
                                                                            instancePath:
                                                                              instancePath +
                                                                              "/fields/" +
                                                                              i0 +
                                                                              "/options/" +
                                                                              i1 +
                                                                              "/id",
                                                                            schemaPath:
                                                                              "#/$defs/OperationField/properties/options/items/properties/id/type",
                                                                            keyword:
                                                                              "type",
                                                                            params:
                                                                              {
                                                                                type: "string",
                                                                              },
                                                                            message:
                                                                              "must be string",
                                                                          },
                                                                        ];
                                                                      return false;
                                                                    }
                                                                  }
                                                                  var valid5 =
                                                                    _errs32 ===
                                                                    errors;
                                                                } else {
                                                                  var valid5 = true;
                                                                }
                                                                if (valid5) {
                                                                  if (
                                                                    data14.label !==
                                                                    undefined
                                                                  ) {
                                                                    let data16 =
                                                                      data14.label;
                                                                    const _errs34 =
                                                                      errors;
                                                                    if (
                                                                      errors ===
                                                                      _errs34
                                                                    ) {
                                                                      if (
                                                                        typeof data16 ===
                                                                        "string"
                                                                      ) {
                                                                        if (
                                                                          func1(
                                                                            data16,
                                                                          ) >
                                                                          256
                                                                        ) {
                                                                          validate21.errors =
                                                                            [
                                                                              {
                                                                                instancePath:
                                                                                  instancePath +
                                                                                  "/fields/" +
                                                                                  i0 +
                                                                                  "/options/" +
                                                                                  i1 +
                                                                                  "/label",
                                                                                schemaPath:
                                                                                  "#/$defs/OperationField/properties/options/items/properties/label/maxLength",
                                                                                keyword:
                                                                                  "maxLength",
                                                                                params:
                                                                                  {
                                                                                    limit: 256,
                                                                                  },
                                                                                message:
                                                                                  "must NOT have more than 256 characters",
                                                                              },
                                                                            ];
                                                                          return false;
                                                                        }
                                                                      } else {
                                                                        validate21.errors =
                                                                          [
                                                                            {
                                                                              instancePath:
                                                                                instancePath +
                                                                                "/fields/" +
                                                                                i0 +
                                                                                "/options/" +
                                                                                i1 +
                                                                                "/label",
                                                                              schemaPath:
                                                                                "#/$defs/OperationField/properties/options/items/properties/label/type",
                                                                              keyword:
                                                                                "type",
                                                                              params:
                                                                                {
                                                                                  type: "string",
                                                                                },
                                                                              message:
                                                                                "must be string",
                                                                            },
                                                                          ];
                                                                        return false;
                                                                      }
                                                                    }
                                                                    var valid5 =
                                                                      _errs34 ===
                                                                      errors;
                                                                  } else {
                                                                    var valid5 = true;
                                                                  }
                                                                  if (valid5) {
                                                                    if (
                                                                      data14.disabledReason !==
                                                                      undefined
                                                                    ) {
                                                                      let data17 =
                                                                        data14.disabledReason;
                                                                      const _errs36 =
                                                                        errors;
                                                                      if (
                                                                        errors ===
                                                                        _errs36
                                                                      ) {
                                                                        if (
                                                                          typeof data17 ===
                                                                          "string"
                                                                        ) {
                                                                          if (
                                                                            func1(
                                                                              data17,
                                                                            ) >
                                                                            4096
                                                                          ) {
                                                                            validate21.errors =
                                                                              [
                                                                                {
                                                                                  instancePath:
                                                                                    instancePath +
                                                                                    "/fields/" +
                                                                                    i0 +
                                                                                    "/options/" +
                                                                                    i1 +
                                                                                    "/disabledReason",
                                                                                  schemaPath:
                                                                                    "#/$defs/OperationField/properties/options/items/properties/disabledReason/maxLength",
                                                                                  keyword:
                                                                                    "maxLength",
                                                                                  params:
                                                                                    {
                                                                                      limit: 4096,
                                                                                    },
                                                                                  message:
                                                                                    "must NOT have more than 4096 characters",
                                                                                },
                                                                              ];
                                                                            return false;
                                                                          }
                                                                        } else {
                                                                          validate21.errors =
                                                                            [
                                                                              {
                                                                                instancePath:
                                                                                  instancePath +
                                                                                  "/fields/" +
                                                                                  i0 +
                                                                                  "/options/" +
                                                                                  i1 +
                                                                                  "/disabledReason",
                                                                                schemaPath:
                                                                                  "#/$defs/OperationField/properties/options/items/properties/disabledReason/type",
                                                                                keyword:
                                                                                  "type",
                                                                                params:
                                                                                  {
                                                                                    type: "string",
                                                                                  },
                                                                                message:
                                                                                  "must be string",
                                                                              },
                                                                            ];
                                                                          return false;
                                                                        }
                                                                      }
                                                                      var valid5 =
                                                                        _errs36 ===
                                                                        errors;
                                                                    } else {
                                                                      var valid5 = true;
                                                                    }
                                                                  }
                                                                }
                                                              }
                                                            }
                                                          } else {
                                                            validate21.errors =
                                                              [
                                                                {
                                                                  instancePath:
                                                                    instancePath +
                                                                    "/fields/" +
                                                                    i0 +
                                                                    "/options/" +
                                                                    i1,
                                                                  schemaPath:
                                                                    "#/$defs/OperationField/properties/options/items/type",
                                                                  keyword:
                                                                    "type",
                                                                  params: {
                                                                    type: "object",
                                                                  },
                                                                  message:
                                                                    "must be object",
                                                                },
                                                              ];
                                                            return false;
                                                          }
                                                        }
                                                        var valid4 =
                                                          _errs29 === errors;
                                                        if (!valid4) {
                                                          break;
                                                        }
                                                      }
                                                    }
                                                  } else {
                                                    validate21.errors = [
                                                      {
                                                        instancePath:
                                                          instancePath +
                                                          "/fields/" +
                                                          i0 +
                                                          "/options",
                                                        schemaPath:
                                                          "#/$defs/OperationField/properties/options/type",
                                                        keyword: "type",
                                                        params: {
                                                          type: "array",
                                                        },
                                                        message:
                                                          "must be array",
                                                      },
                                                    ];
                                                    return false;
                                                  }
                                                }
                                                var valid3 = _errs27 === errors;
                                              } else {
                                                var valid3 = true;
                                              }
                                              if (valid3) {
                                                if (
                                                  data3.submitMode !== undefined
                                                ) {
                                                  let data18 = data3.submitMode;
                                                  const _errs38 = errors;
                                                  if (
                                                    !(
                                                      data18 === "draft" ||
                                                      data18 === "immediate"
                                                    )
                                                  ) {
                                                    validate21.errors = [
                                                      {
                                                        instancePath:
                                                          instancePath +
                                                          "/fields/" +
                                                          i0 +
                                                          "/submitMode",
                                                        schemaPath:
                                                          "#/$defs/OperationField/properties/submitMode/enum",
                                                        keyword: "enum",
                                                        params: {
                                                          allowedValues:
                                                            schema33.properties
                                                              .submitMode.enum,
                                                        },
                                                        message:
                                                          "must be equal to one of the allowed values",
                                                      },
                                                    ];
                                                    return false;
                                                  }
                                                  var valid3 =
                                                    _errs38 === errors;
                                                } else {
                                                  var valid3 = true;
                                                }
                                                if (valid3) {
                                                  if (
                                                    data3.section !== undefined
                                                  ) {
                                                    let data19 = data3.section;
                                                    const _errs39 = errors;
                                                    if (errors === _errs39) {
                                                      if (
                                                        typeof data19 ===
                                                        "string"
                                                      ) {
                                                        if (
                                                          func1(data19) > 128
                                                        ) {
                                                          validate21.errors = [
                                                            {
                                                              instancePath:
                                                                instancePath +
                                                                "/fields/" +
                                                                i0 +
                                                                "/section",
                                                              schemaPath:
                                                                "#/$defs/OperationField/properties/section/maxLength",
                                                              keyword:
                                                                "maxLength",
                                                              params: {
                                                                limit: 128,
                                                              },
                                                              message:
                                                                "must NOT have more than 128 characters",
                                                            },
                                                          ];
                                                          return false;
                                                        } else {
                                                          if (
                                                            func1(data19) < 1
                                                          ) {
                                                            validate21.errors =
                                                              [
                                                                {
                                                                  instancePath:
                                                                    instancePath +
                                                                    "/fields/" +
                                                                    i0 +
                                                                    "/section",
                                                                  schemaPath:
                                                                    "#/$defs/OperationField/properties/section/minLength",
                                                                  keyword:
                                                                    "minLength",
                                                                  params: {
                                                                    limit: 1,
                                                                  },
                                                                  message:
                                                                    "must NOT have fewer than 1 characters",
                                                                },
                                                              ];
                                                            return false;
                                                          } else {
                                                            if (
                                                              !pattern4.test(
                                                                data19,
                                                              )
                                                            ) {
                                                              validate21.errors =
                                                                [
                                                                  {
                                                                    instancePath:
                                                                      instancePath +
                                                                      "/fields/" +
                                                                      i0 +
                                                                      "/section",
                                                                    schemaPath:
                                                                      "#/$defs/OperationField/properties/section/pattern",
                                                                    keyword:
                                                                      "pattern",
                                                                    params: {
                                                                      pattern:
                                                                        "^[a-zA-Z0-9_-]+$",
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
                                                              "/fields/" +
                                                              i0 +
                                                              "/section",
                                                            schemaPath:
                                                              "#/$defs/OperationField/properties/section/type",
                                                            keyword: "type",
                                                            params: {
                                                              type: "string",
                                                            },
                                                            message:
                                                              "must be string",
                                                          },
                                                        ];
                                                        return false;
                                                      }
                                                    }
                                                    var valid3 =
                                                      _errs39 === errors;
                                                  } else {
                                                    var valid3 = true;
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
                            }
                          }
                        } else {
                          validate21.errors = [
                            {
                              instancePath: instancePath + "/fields/" + i0,
                              schemaPath: "#/$defs/OperationField/type",
                              keyword: "type",
                              params: { type: "object" },
                              message: "must be object",
                            },
                          ];
                          return false;
                        }
                      }
                      var valid1 = _errs6 === errors;
                      if (!valid1) {
                        break;
                      }
                    }
                  }
                } else {
                  validate21.errors = [
                    {
                      instancePath: instancePath + "/fields",
                      schemaPath: "#/properties/fields/type",
                      keyword: "type",
                      params: { type: "array" },
                      message: "must be array",
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
              if (data.rules !== undefined) {
                let data20 = data.rules;
                const _errs41 = errors;
                if (errors === _errs41) {
                  if (Array.isArray(data20)) {
                    if (data20.length > 128) {
                      validate21.errors = [
                        {
                          instancePath: instancePath + "/rules",
                          schemaPath: "#/properties/rules/maxItems",
                          keyword: "maxItems",
                          params: { limit: 128 },
                          message: "must NOT have more than 128 items",
                        },
                      ];
                      return false;
                    } else {
                      var valid6 = true;
                      const len2 = data20.length;
                      for (let i2 = 0; i2 < len2; i2++) {
                        let data21 = data20[i2];
                        const _errs43 = errors;
                        const _errs44 = errors;
                        if (errors === _errs44) {
                          if (
                            data21 &&
                            typeof data21 == "object" &&
                            !Array.isArray(data21)
                          ) {
                            let missing3;
                            if (
                              (data21.field === undefined &&
                                (missing3 = "field")) ||
                              (data21.when === undefined &&
                                (missing3 = "when")) ||
                              (data21.equals === undefined &&
                                (missing3 = "equals")) ||
                              (data21.kind === undefined &&
                                (missing3 = "kind")) ||
                              (data21.message === undefined &&
                                (missing3 = "message"))
                            ) {
                              validate21.errors = [
                                {
                                  instancePath: instancePath + "/rules/" + i2,
                                  schemaPath: "#/$defs/OperationRule/required",
                                  keyword: "required",
                                  params: { missingProperty: missing3 },
                                  message:
                                    "must have required property '" +
                                    missing3 +
                                    "'",
                                },
                              ];
                              return false;
                            } else {
                              const _errs46 = errors;
                              for (const key1 in data21) {
                                if (
                                  !(
                                    key1 === "field" ||
                                    key1 === "when" ||
                                    key1 === "equals" ||
                                    key1 === "kind" ||
                                    key1 === "message"
                                  )
                                ) {
                                  validate21.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/rules/" + i2,
                                      schemaPath:
                                        "#/$defs/OperationRule/additionalProperties",
                                      keyword: "additionalProperties",
                                      params: { additionalProperty: key1 },
                                      message:
                                        "must NOT have additional properties",
                                    },
                                  ];
                                  return false;
                                  break;
                                }
                              }
                              if (_errs46 === errors) {
                                if (data21.field !== undefined) {
                                  let data22 = data21.field;
                                  const _errs47 = errors;
                                  if (errors === _errs47) {
                                    if (typeof data22 === "string") {
                                      if (func1(data22) > 128) {
                                        validate21.errors = [
                                          {
                                            instancePath:
                                              instancePath +
                                              "/rules/" +
                                              i2 +
                                              "/field",
                                            schemaPath:
                                              "#/$defs/OperationRule/properties/field/maxLength",
                                            keyword: "maxLength",
                                            params: { limit: 128 },
                                            message:
                                              "must NOT have more than 128 characters",
                                          },
                                        ];
                                        return false;
                                      } else {
                                        if (func1(data22) < 1) {
                                          validate21.errors = [
                                            {
                                              instancePath:
                                                instancePath +
                                                "/rules/" +
                                                i2 +
                                                "/field",
                                              schemaPath:
                                                "#/$defs/OperationRule/properties/field/minLength",
                                              keyword: "minLength",
                                              params: { limit: 1 },
                                              message:
                                                "must NOT have fewer than 1 characters",
                                            },
                                          ];
                                          return false;
                                        } else {
                                          if (!pattern4.test(data22)) {
                                            validate21.errors = [
                                              {
                                                instancePath:
                                                  instancePath +
                                                  "/rules/" +
                                                  i2 +
                                                  "/field",
                                                schemaPath:
                                                  "#/$defs/OperationRule/properties/field/pattern",
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
                                            "/rules/" +
                                            i2 +
                                            "/field",
                                          schemaPath:
                                            "#/$defs/OperationRule/properties/field/type",
                                          keyword: "type",
                                          params: { type: "string" },
                                          message: "must be string",
                                        },
                                      ];
                                      return false;
                                    }
                                  }
                                  var valid8 = _errs47 === errors;
                                } else {
                                  var valid8 = true;
                                }
                                if (valid8) {
                                  if (data21.when !== undefined) {
                                    let data23 = data21.when;
                                    const _errs49 = errors;
                                    if (errors === _errs49) {
                                      if (typeof data23 === "string") {
                                        if (func1(data23) > 128) {
                                          validate21.errors = [
                                            {
                                              instancePath:
                                                instancePath +
                                                "/rules/" +
                                                i2 +
                                                "/when",
                                              schemaPath:
                                                "#/$defs/OperationRule/properties/when/maxLength",
                                              keyword: "maxLength",
                                              params: { limit: 128 },
                                              message:
                                                "must NOT have more than 128 characters",
                                            },
                                          ];
                                          return false;
                                        } else {
                                          if (func1(data23) < 1) {
                                            validate21.errors = [
                                              {
                                                instancePath:
                                                  instancePath +
                                                  "/rules/" +
                                                  i2 +
                                                  "/when",
                                                schemaPath:
                                                  "#/$defs/OperationRule/properties/when/minLength",
                                                keyword: "minLength",
                                                params: { limit: 1 },
                                                message:
                                                  "must NOT have fewer than 1 characters",
                                              },
                                            ];
                                            return false;
                                          } else {
                                            if (!pattern4.test(data23)) {
                                              validate21.errors = [
                                                {
                                                  instancePath:
                                                    instancePath +
                                                    "/rules/" +
                                                    i2 +
                                                    "/when",
                                                  schemaPath:
                                                    "#/$defs/OperationRule/properties/when/pattern",
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
                                              "/rules/" +
                                              i2 +
                                              "/when",
                                            schemaPath:
                                              "#/$defs/OperationRule/properties/when/type",
                                            keyword: "type",
                                            params: { type: "string" },
                                            message: "must be string",
                                          },
                                        ];
                                        return false;
                                      }
                                    }
                                    var valid8 = _errs49 === errors;
                                  } else {
                                    var valid8 = true;
                                  }
                                  if (valid8) {
                                    if (data21.equals !== undefined) {
                                      let data24 = data21.equals;
                                      const _errs51 = errors;
                                      const _errs52 = errors;
                                      let valid9 = false;
                                      const _errs53 = errors;
                                      if (typeof data24 !== "string") {
                                        const err0 = {
                                          instancePath:
                                            instancePath +
                                            "/rules/" +
                                            i2 +
                                            "/equals",
                                          schemaPath:
                                            "#/$defs/OperationRule/properties/equals/anyOf/0/type",
                                          keyword: "type",
                                          params: { type: "string" },
                                          message: "must be string",
                                        };
                                        if (vErrors === null) {
                                          vErrors = [err0];
                                        } else {
                                          vErrors.push(err0);
                                        }
                                        errors++;
                                      }
                                      var _valid0 = _errs53 === errors;
                                      valid9 = valid9 || _valid0;
                                      const _errs55 = errors;
                                      if (
                                        !(
                                          typeof data24 == "number" &&
                                          isFinite(data24)
                                        )
                                      ) {
                                        const err1 = {
                                          instancePath:
                                            instancePath +
                                            "/rules/" +
                                            i2 +
                                            "/equals",
                                          schemaPath:
                                            "#/$defs/OperationRule/properties/equals/anyOf/1/type",
                                          keyword: "type",
                                          params: { type: "number" },
                                          message: "must be number",
                                        };
                                        if (vErrors === null) {
                                          vErrors = [err1];
                                        } else {
                                          vErrors.push(err1);
                                        }
                                        errors++;
                                      }
                                      var _valid0 = _errs55 === errors;
                                      valid9 = valid9 || _valid0;
                                      const _errs57 = errors;
                                      if (typeof data24 !== "boolean") {
                                        const err2 = {
                                          instancePath:
                                            instancePath +
                                            "/rules/" +
                                            i2 +
                                            "/equals",
                                          schemaPath:
                                            "#/$defs/OperationRule/properties/equals/anyOf/2/type",
                                          keyword: "type",
                                          params: { type: "boolean" },
                                          message: "must be boolean",
                                        };
                                        if (vErrors === null) {
                                          vErrors = [err2];
                                        } else {
                                          vErrors.push(err2);
                                        }
                                        errors++;
                                      }
                                      var _valid0 = _errs57 === errors;
                                      valid9 = valid9 || _valid0;
                                      const _errs59 = errors;
                                      if (data24 !== null) {
                                        const err3 = {
                                          instancePath:
                                            instancePath +
                                            "/rules/" +
                                            i2 +
                                            "/equals",
                                          schemaPath:
                                            "#/$defs/OperationRule/properties/equals/anyOf/3/type",
                                          keyword: "type",
                                          params: { type: "null" },
                                          message: "must be null",
                                        };
                                        if (vErrors === null) {
                                          vErrors = [err3];
                                        } else {
                                          vErrors.push(err3);
                                        }
                                        errors++;
                                      }
                                      var _valid0 = _errs59 === errors;
                                      valid9 = valid9 || _valid0;
                                      if (!valid9) {
                                        const err4 = {
                                          instancePath:
                                            instancePath +
                                            "/rules/" +
                                            i2 +
                                            "/equals",
                                          schemaPath:
                                            "#/$defs/OperationRule/properties/equals/anyOf",
                                          keyword: "anyOf",
                                          params: {},
                                          message:
                                            "must match a schema in anyOf",
                                        };
                                        if (vErrors === null) {
                                          vErrors = [err4];
                                        } else {
                                          vErrors.push(err4);
                                        }
                                        errors++;
                                        validate21.errors = vErrors;
                                        return false;
                                      } else {
                                        errors = _errs52;
                                        if (vErrors !== null) {
                                          if (_errs52) {
                                            vErrors.length = _errs52;
                                          } else {
                                            vErrors = null;
                                          }
                                        }
                                      }
                                      var valid8 = _errs51 === errors;
                                    } else {
                                      var valid8 = true;
                                    }
                                    if (valid8) {
                                      if (data21.kind !== undefined) {
                                        let data25 = data21.kind;
                                        const _errs61 = errors;
                                        if (
                                          !(
                                            data25 === "required" ||
                                            data25 === "equals"
                                          )
                                        ) {
                                          validate21.errors = [
                                            {
                                              instancePath:
                                                instancePath +
                                                "/rules/" +
                                                i2 +
                                                "/kind",
                                              schemaPath:
                                                "#/$defs/OperationRule/properties/kind/enum",
                                              keyword: "enum",
                                              params: {
                                                allowedValues:
                                                  schema34.properties.kind.enum,
                                              },
                                              message:
                                                "must be equal to one of the allowed values",
                                            },
                                          ];
                                          return false;
                                        }
                                        var valid8 = _errs61 === errors;
                                      } else {
                                        var valid8 = true;
                                      }
                                      if (valid8) {
                                        if (data21.message !== undefined) {
                                          let data26 = data21.message;
                                          const _errs62 = errors;
                                          if (errors === _errs62) {
                                            if (typeof data26 === "string") {
                                              if (func1(data26) > 256) {
                                                validate21.errors = [
                                                  {
                                                    instancePath:
                                                      instancePath +
                                                      "/rules/" +
                                                      i2 +
                                                      "/message",
                                                    schemaPath:
                                                      "#/$defs/OperationRule/properties/message/maxLength",
                                                    keyword: "maxLength",
                                                    params: { limit: 256 },
                                                    message:
                                                      "must NOT have more than 256 characters",
                                                  },
                                                ];
                                                return false;
                                              }
                                            } else {
                                              validate21.errors = [
                                                {
                                                  instancePath:
                                                    instancePath +
                                                    "/rules/" +
                                                    i2 +
                                                    "/message",
                                                  schemaPath:
                                                    "#/$defs/OperationRule/properties/message/type",
                                                  keyword: "type",
                                                  params: { type: "string" },
                                                  message: "must be string",
                                                },
                                              ];
                                              return false;
                                            }
                                          }
                                          var valid8 = _errs62 === errors;
                                        } else {
                                          var valid8 = true;
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
                                instancePath: instancePath + "/rules/" + i2,
                                schemaPath: "#/$defs/OperationRule/type",
                                keyword: "type",
                                params: { type: "object" },
                                message: "must be object",
                              },
                            ];
                            return false;
                          }
                        }
                        var valid6 = _errs43 === errors;
                        if (!valid6) {
                          break;
                        }
                      }
                    }
                  } else {
                    validate21.errors = [
                      {
                        instancePath: instancePath + "/rules",
                        schemaPath: "#/properties/rules/type",
                        keyword: "type",
                        params: { type: "array" },
                        message: "must be array",
                      },
                    ];
                    return false;
                  }
                }
                var valid0 = _errs41 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.sections !== undefined) {
                  let data27 = data.sections;
                  const _errs64 = errors;
                  if (errors === _errs64) {
                    if (Array.isArray(data27)) {
                      if (data27.length > 16) {
                        validate21.errors = [
                          {
                            instancePath: instancePath + "/sections",
                            schemaPath: "#/properties/sections/maxItems",
                            keyword: "maxItems",
                            params: { limit: 16 },
                            message: "must NOT have more than 16 items",
                          },
                        ];
                        return false;
                      } else {
                        var valid10 = true;
                        const len3 = data27.length;
                        for (let i3 = 0; i3 < len3; i3++) {
                          let data28 = data27[i3];
                          const _errs66 = errors;
                          if (errors === _errs66) {
                            if (
                              data28 &&
                              typeof data28 == "object" &&
                              !Array.isArray(data28)
                            ) {
                              let missing4;
                              if (
                                (data28.id === undefined &&
                                  (missing4 = "id")) ||
                                (data28.label === undefined &&
                                  (missing4 = "label"))
                              ) {
                                validate21.errors = [
                                  {
                                    instancePath:
                                      instancePath + "/sections/" + i3,
                                    schemaPath:
                                      "#/properties/sections/items/required",
                                    keyword: "required",
                                    params: { missingProperty: missing4 },
                                    message:
                                      "must have required property '" +
                                      missing4 +
                                      "'",
                                  },
                                ];
                                return false;
                              } else {
                                const _errs68 = errors;
                                for (const key2 in data28) {
                                  if (!(key2 === "id" || key2 === "label")) {
                                    validate21.errors = [
                                      {
                                        instancePath:
                                          instancePath + "/sections/" + i3,
                                        schemaPath:
                                          "#/properties/sections/items/additionalProperties",
                                        keyword: "additionalProperties",
                                        params: { additionalProperty: key2 },
                                        message:
                                          "must NOT have additional properties",
                                      },
                                    ];
                                    return false;
                                    break;
                                  }
                                }
                                if (_errs68 === errors) {
                                  if (data28.id !== undefined) {
                                    let data29 = data28.id;
                                    const _errs69 = errors;
                                    if (errors === _errs69) {
                                      if (typeof data29 === "string") {
                                        if (func1(data29) > 128) {
                                          validate21.errors = [
                                            {
                                              instancePath:
                                                instancePath +
                                                "/sections/" +
                                                i3 +
                                                "/id",
                                              schemaPath:
                                                "#/properties/sections/items/properties/id/maxLength",
                                              keyword: "maxLength",
                                              params: { limit: 128 },
                                              message:
                                                "must NOT have more than 128 characters",
                                            },
                                          ];
                                          return false;
                                        } else {
                                          if (func1(data29) < 1) {
                                            validate21.errors = [
                                              {
                                                instancePath:
                                                  instancePath +
                                                  "/sections/" +
                                                  i3 +
                                                  "/id",
                                                schemaPath:
                                                  "#/properties/sections/items/properties/id/minLength",
                                                keyword: "minLength",
                                                params: { limit: 1 },
                                                message:
                                                  "must NOT have fewer than 1 characters",
                                              },
                                            ];
                                            return false;
                                          } else {
                                            if (!pattern4.test(data29)) {
                                              validate21.errors = [
                                                {
                                                  instancePath:
                                                    instancePath +
                                                    "/sections/" +
                                                    i3 +
                                                    "/id",
                                                  schemaPath:
                                                    "#/properties/sections/items/properties/id/pattern",
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
                                              "/sections/" +
                                              i3 +
                                              "/id",
                                            schemaPath:
                                              "#/properties/sections/items/properties/id/type",
                                            keyword: "type",
                                            params: { type: "string" },
                                            message: "must be string",
                                          },
                                        ];
                                        return false;
                                      }
                                    }
                                    var valid11 = _errs69 === errors;
                                  } else {
                                    var valid11 = true;
                                  }
                                  if (valid11) {
                                    if (data28.label !== undefined) {
                                      let data30 = data28.label;
                                      const _errs71 = errors;
                                      if (errors === _errs71) {
                                        if (typeof data30 === "string") {
                                          if (func1(data30) > 256) {
                                            validate21.errors = [
                                              {
                                                instancePath:
                                                  instancePath +
                                                  "/sections/" +
                                                  i3 +
                                                  "/label",
                                                schemaPath:
                                                  "#/properties/sections/items/properties/label/maxLength",
                                                keyword: "maxLength",
                                                params: { limit: 256 },
                                                message:
                                                  "must NOT have more than 256 characters",
                                              },
                                            ];
                                            return false;
                                          }
                                        } else {
                                          validate21.errors = [
                                            {
                                              instancePath:
                                                instancePath +
                                                "/sections/" +
                                                i3 +
                                                "/label",
                                              schemaPath:
                                                "#/properties/sections/items/properties/label/type",
                                              keyword: "type",
                                              params: { type: "string" },
                                              message: "must be string",
                                            },
                                          ];
                                          return false;
                                        }
                                      }
                                      var valid11 = _errs71 === errors;
                                    } else {
                                      var valid11 = true;
                                    }
                                  }
                                }
                              }
                            } else {
                              validate21.errors = [
                                {
                                  instancePath:
                                    instancePath + "/sections/" + i3,
                                  schemaPath:
                                    "#/properties/sections/items/type",
                                  keyword: "type",
                                  params: { type: "object" },
                                  message: "must be object",
                                },
                              ];
                              return false;
                            }
                          }
                          var valid10 = _errs66 === errors;
                          if (!valid10) {
                            break;
                          }
                        }
                      }
                    } else {
                      validate21.errors = [
                        {
                          instancePath: instancePath + "/sections",
                          schemaPath: "#/properties/sections/type",
                          keyword: "type",
                          params: { type: "array" },
                          message: "must be array",
                        },
                      ];
                      return false;
                    }
                  }
                  var valid0 = _errs64 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.layout !== undefined) {
                    let data31 = data.layout;
                    const _errs73 = errors;
                    if (
                      !(
                        data31 === "sections" ||
                        data31 === "tabs" ||
                        data31 === "disclosure"
                      )
                    ) {
                      validate21.errors = [
                        {
                          instancePath: instancePath + "/layout",
                          schemaPath: "#/properties/layout/enum",
                          keyword: "enum",
                          params: {
                            allowedValues: schema32.properties.layout.enum,
                          },
                          message: "must be equal to one of the allowed values",
                        },
                      ];
                      return false;
                    }
                    var valid0 = _errs73 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.selection !== undefined) {
                      let data32 = data.selection;
                      const _errs74 = errors;
                      if (!(data32 === "single" || data32 === "multiple")) {
                        validate21.errors = [
                          {
                            instancePath: instancePath + "/selection",
                            schemaPath: "#/properties/selection/enum",
                            keyword: "enum",
                            params: {
                              allowedValues: schema32.properties.selection.enum,
                            },
                            message:
                              "must be equal to one of the allowed values",
                          },
                        ];
                        return false;
                      }
                      var valid0 = _errs74 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.steps !== undefined) {
                        let data33 = data.steps;
                        const _errs75 = errors;
                        if (errors === _errs75) {
                          if (Array.isArray(data33)) {
                            if (data33.length > 16) {
                              validate21.errors = [
                                {
                                  instancePath: instancePath + "/steps",
                                  schemaPath: "#/properties/steps/maxItems",
                                  keyword: "maxItems",
                                  params: { limit: 16 },
                                  message: "must NOT have more than 16 items",
                                },
                              ];
                              return false;
                            } else {
                              var valid12 = true;
                              const len4 = data33.length;
                              for (let i4 = 0; i4 < len4; i4++) {
                                let data34 = data33[i4];
                                const _errs77 = errors;
                                if (errors === _errs77) {
                                  if (
                                    data34 &&
                                    typeof data34 == "object" &&
                                    !Array.isArray(data34)
                                  ) {
                                    let missing5;
                                    if (
                                      (data34.id === undefined &&
                                        (missing5 = "id")) ||
                                      (data34.label === undefined &&
                                        (missing5 = "label")) ||
                                      (data34.fields === undefined &&
                                        (missing5 = "fields"))
                                    ) {
                                      validate21.errors = [
                                        {
                                          instancePath:
                                            instancePath + "/steps/" + i4,
                                          schemaPath:
                                            "#/properties/steps/items/required",
                                          keyword: "required",
                                          params: { missingProperty: missing5 },
                                          message:
                                            "must have required property '" +
                                            missing5 +
                                            "'",
                                        },
                                      ];
                                      return false;
                                    } else {
                                      const _errs79 = errors;
                                      for (const key3 in data34) {
                                        if (
                                          !(
                                            key3 === "id" ||
                                            key3 === "label" ||
                                            key3 === "fields"
                                          )
                                        ) {
                                          validate21.errors = [
                                            {
                                              instancePath:
                                                instancePath + "/steps/" + i4,
                                              schemaPath:
                                                "#/properties/steps/items/additionalProperties",
                                              keyword: "additionalProperties",
                                              params: {
                                                additionalProperty: key3,
                                              },
                                              message:
                                                "must NOT have additional properties",
                                            },
                                          ];
                                          return false;
                                          break;
                                        }
                                      }
                                      if (_errs79 === errors) {
                                        if (data34.id !== undefined) {
                                          let data35 = data34.id;
                                          const _errs80 = errors;
                                          if (errors === _errs80) {
                                            if (typeof data35 === "string") {
                                              if (func1(data35) > 128) {
                                                validate21.errors = [
                                                  {
                                                    instancePath:
                                                      instancePath +
                                                      "/steps/" +
                                                      i4 +
                                                      "/id",
                                                    schemaPath:
                                                      "#/properties/steps/items/properties/id/maxLength",
                                                    keyword: "maxLength",
                                                    params: { limit: 128 },
                                                    message:
                                                      "must NOT have more than 128 characters",
                                                  },
                                                ];
                                                return false;
                                              } else {
                                                if (func1(data35) < 1) {
                                                  validate21.errors = [
                                                    {
                                                      instancePath:
                                                        instancePath +
                                                        "/steps/" +
                                                        i4 +
                                                        "/id",
                                                      schemaPath:
                                                        "#/properties/steps/items/properties/id/minLength",
                                                      keyword: "minLength",
                                                      params: { limit: 1 },
                                                      message:
                                                        "must NOT have fewer than 1 characters",
                                                    },
                                                  ];
                                                  return false;
                                                } else {
                                                  if (!pattern4.test(data35)) {
                                                    validate21.errors = [
                                                      {
                                                        instancePath:
                                                          instancePath +
                                                          "/steps/" +
                                                          i4 +
                                                          "/id",
                                                        schemaPath:
                                                          "#/properties/steps/items/properties/id/pattern",
                                                        keyword: "pattern",
                                                        params: {
                                                          pattern:
                                                            "^[a-zA-Z0-9_-]+$",
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
                                                    "/steps/" +
                                                    i4 +
                                                    "/id",
                                                  schemaPath:
                                                    "#/properties/steps/items/properties/id/type",
                                                  keyword: "type",
                                                  params: { type: "string" },
                                                  message: "must be string",
                                                },
                                              ];
                                              return false;
                                            }
                                          }
                                          var valid13 = _errs80 === errors;
                                        } else {
                                          var valid13 = true;
                                        }
                                        if (valid13) {
                                          if (data34.label !== undefined) {
                                            let data36 = data34.label;
                                            const _errs82 = errors;
                                            if (errors === _errs82) {
                                              if (typeof data36 === "string") {
                                                if (func1(data36) > 256) {
                                                  validate21.errors = [
                                                    {
                                                      instancePath:
                                                        instancePath +
                                                        "/steps/" +
                                                        i4 +
                                                        "/label",
                                                      schemaPath:
                                                        "#/properties/steps/items/properties/label/maxLength",
                                                      keyword: "maxLength",
                                                      params: { limit: 256 },
                                                      message:
                                                        "must NOT have more than 256 characters",
                                                    },
                                                  ];
                                                  return false;
                                                }
                                              } else {
                                                validate21.errors = [
                                                  {
                                                    instancePath:
                                                      instancePath +
                                                      "/steps/" +
                                                      i4 +
                                                      "/label",
                                                    schemaPath:
                                                      "#/properties/steps/items/properties/label/type",
                                                    keyword: "type",
                                                    params: { type: "string" },
                                                    message: "must be string",
                                                  },
                                                ];
                                                return false;
                                              }
                                            }
                                            var valid13 = _errs82 === errors;
                                          } else {
                                            var valid13 = true;
                                          }
                                          if (valid13) {
                                            if (data34.fields !== undefined) {
                                              let data37 = data34.fields;
                                              const _errs84 = errors;
                                              if (errors === _errs84) {
                                                if (Array.isArray(data37)) {
                                                  if (data37.length > 128) {
                                                    validate21.errors = [
                                                      {
                                                        instancePath:
                                                          instancePath +
                                                          "/steps/" +
                                                          i4 +
                                                          "/fields",
                                                        schemaPath:
                                                          "#/properties/steps/items/properties/fields/maxItems",
                                                        keyword: "maxItems",
                                                        params: { limit: 128 },
                                                        message:
                                                          "must NOT have more than 128 items",
                                                      },
                                                    ];
                                                    return false;
                                                  } else {
                                                    var valid14 = true;
                                                    const len5 = data37.length;
                                                    for (
                                                      let i5 = 0;
                                                      i5 < len5;
                                                      i5++
                                                    ) {
                                                      let data38 = data37[i5];
                                                      const _errs86 = errors;
                                                      if (errors === _errs86) {
                                                        if (
                                                          typeof data38 ===
                                                          "string"
                                                        ) {
                                                          if (
                                                            func1(data38) > 128
                                                          ) {
                                                            validate21.errors =
                                                              [
                                                                {
                                                                  instancePath:
                                                                    instancePath +
                                                                    "/steps/" +
                                                                    i4 +
                                                                    "/fields/" +
                                                                    i5,
                                                                  schemaPath:
                                                                    "#/properties/steps/items/properties/fields/items/maxLength",
                                                                  keyword:
                                                                    "maxLength",
                                                                  params: {
                                                                    limit: 128,
                                                                  },
                                                                  message:
                                                                    "must NOT have more than 128 characters",
                                                                },
                                                              ];
                                                            return false;
                                                          } else {
                                                            if (
                                                              func1(data38) < 1
                                                            ) {
                                                              validate21.errors =
                                                                [
                                                                  {
                                                                    instancePath:
                                                                      instancePath +
                                                                      "/steps/" +
                                                                      i4 +
                                                                      "/fields/" +
                                                                      i5,
                                                                    schemaPath:
                                                                      "#/properties/steps/items/properties/fields/items/minLength",
                                                                    keyword:
                                                                      "minLength",
                                                                    params: {
                                                                      limit: 1,
                                                                    },
                                                                    message:
                                                                      "must NOT have fewer than 1 characters",
                                                                  },
                                                                ];
                                                              return false;
                                                            } else {
                                                              if (
                                                                !pattern4.test(
                                                                  data38,
                                                                )
                                                              ) {
                                                                validate21.errors =
                                                                  [
                                                                    {
                                                                      instancePath:
                                                                        instancePath +
                                                                        "/steps/" +
                                                                        i4 +
                                                                        "/fields/" +
                                                                        i5,
                                                                      schemaPath:
                                                                        "#/properties/steps/items/properties/fields/items/pattern",
                                                                      keyword:
                                                                        "pattern",
                                                                      params: {
                                                                        pattern:
                                                                          "^[a-zA-Z0-9_-]+$",
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
                                                                "/steps/" +
                                                                i4 +
                                                                "/fields/" +
                                                                i5,
                                                              schemaPath:
                                                                "#/properties/steps/items/properties/fields/items/type",
                                                              keyword: "type",
                                                              params: {
                                                                type: "string",
                                                              },
                                                              message:
                                                                "must be string",
                                                            },
                                                          ];
                                                          return false;
                                                        }
                                                      }
                                                      var valid14 =
                                                        _errs86 === errors;
                                                      if (!valid14) {
                                                        break;
                                                      }
                                                    }
                                                  }
                                                } else {
                                                  validate21.errors = [
                                                    {
                                                      instancePath:
                                                        instancePath +
                                                        "/steps/" +
                                                        i4 +
                                                        "/fields",
                                                      schemaPath:
                                                        "#/properties/steps/items/properties/fields/type",
                                                      keyword: "type",
                                                      params: { type: "array" },
                                                      message: "must be array",
                                                    },
                                                  ];
                                                  return false;
                                                }
                                              }
                                              var valid13 = _errs84 === errors;
                                            } else {
                                              var valid13 = true;
                                            }
                                          }
                                        }
                                      }
                                    }
                                  } else {
                                    validate21.errors = [
                                      {
                                        instancePath:
                                          instancePath + "/steps/" + i4,
                                        schemaPath:
                                          "#/properties/steps/items/type",
                                        keyword: "type",
                                        params: { type: "object" },
                                        message: "must be object",
                                      },
                                    ];
                                    return false;
                                  }
                                }
                                var valid12 = _errs77 === errors;
                                if (!valid12) {
                                  break;
                                }
                              }
                            }
                          } else {
                            validate21.errors = [
                              {
                                instancePath: instancePath + "/steps",
                                schemaPath: "#/properties/steps/type",
                                keyword: "type",
                                params: { type: "array" },
                                message: "must be array",
                              },
                            ];
                            return false;
                          }
                        }
                        var valid0 = _errs75 === errors;
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
