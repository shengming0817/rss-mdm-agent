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
  $ref: "#/$defs/OperationSnapshot",
};
const schema32 = {
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
};
const schema33 = {
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
};
const schema34 = {
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
const schema36 = {
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
};
const schema37 = {
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
        (data.scope === undefined && (missing0 = "scope")) ||
        (data.revision === undefined && (missing0 = "revision")) ||
        (data.baseline === undefined && (missing0 = "baseline")) ||
        (data.title === undefined && (missing0 = "title")) ||
        (data.source === undefined && (missing0 = "source")) ||
        (data.targetLabel === undefined && (missing0 = "targetLabel")) ||
        (data.impact === undefined && (missing0 = "impact")) ||
        (data.current === undefined && (missing0 = "current")) ||
        (data.proposal === undefined && (missing0 = "proposal")) ||
        (data.phase === undefined && (missing0 = "phase")) ||
        (data.allowed === undefined && (missing0 = "allowed")) ||
        (data.approval === undefined && (missing0 = "approval")) ||
        (data.objects === undefined && (missing0 = "objects")) ||
        (data.logs === undefined && (missing0 = "logs"))
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
          if (data.scope !== undefined) {
            let data0 = data.scope;
            const _errs2 = errors;
            const _errs3 = errors;
            if (errors === _errs3) {
              if (data0 && typeof data0 == "object" && !Array.isArray(data0)) {
                let missing1;
                if (
                  (data0.authority === undefined && (missing1 = "authority")) ||
                  (data0.tenant === undefined && (missing1 = "tenant")) ||
                  (data0.principal === undefined && (missing1 = "principal")) ||
                  (data0.session === undefined && (missing1 = "session")) ||
                  (data0.target === undefined && (missing1 = "target")) ||
                  (data0.instance === undefined && (missing1 = "instance"))
                ) {
                  validate21.errors = [
                    {
                      instancePath: instancePath + "/scope",
                      schemaPath: "#/$defs/OperationScope/required",
                      keyword: "required",
                      params: { missingProperty: missing1 },
                      message: "must have required property '" + missing1 + "'",
                    },
                  ];
                  return false;
                } else {
                  const _errs5 = errors;
                  for (const key1 in data0) {
                    if (
                      !(
                        key1 === "authority" ||
                        key1 === "tenant" ||
                        key1 === "principal" ||
                        key1 === "session" ||
                        key1 === "target" ||
                        key1 === "instance"
                      )
                    ) {
                      validate21.errors = [
                        {
                          instancePath: instancePath + "/scope",
                          schemaPath:
                            "#/$defs/OperationScope/additionalProperties",
                          keyword: "additionalProperties",
                          params: { additionalProperty: key1 },
                          message: "must NOT have additional properties",
                        },
                      ];
                      return false;
                      break;
                    }
                  }
                  if (_errs5 === errors) {
                    if (data0.authority !== undefined) {
                      let data1 = data0.authority;
                      const _errs6 = errors;
                      if (errors === _errs6) {
                        if (typeof data1 === "string") {
                          if (func2(data1) > 256) {
                            validate21.errors = [
                              {
                                instancePath: instancePath + "/scope/authority",
                                schemaPath:
                                  "#/$defs/OperationScope/properties/authority/maxLength",
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
                              instancePath: instancePath + "/scope/authority",
                              schemaPath:
                                "#/$defs/OperationScope/properties/authority/type",
                              keyword: "type",
                              params: { type: "string" },
                              message: "must be string",
                            },
                          ];
                          return false;
                        }
                      }
                      var valid2 = _errs6 === errors;
                    } else {
                      var valid2 = true;
                    }
                    if (valid2) {
                      if (data0.tenant !== undefined) {
                        let data2 = data0.tenant;
                        const _errs8 = errors;
                        if (errors === _errs8) {
                          if (typeof data2 === "string") {
                            if (func2(data2) > 256) {
                              validate21.errors = [
                                {
                                  instancePath: instancePath + "/scope/tenant",
                                  schemaPath:
                                    "#/$defs/OperationScope/properties/tenant/maxLength",
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
                                instancePath: instancePath + "/scope/tenant",
                                schemaPath:
                                  "#/$defs/OperationScope/properties/tenant/type",
                                keyword: "type",
                                params: { type: "string" },
                                message: "must be string",
                              },
                            ];
                            return false;
                          }
                        }
                        var valid2 = _errs8 === errors;
                      } else {
                        var valid2 = true;
                      }
                      if (valid2) {
                        if (data0.principal !== undefined) {
                          let data3 = data0.principal;
                          const _errs10 = errors;
                          if (errors === _errs10) {
                            if (typeof data3 === "string") {
                              if (func2(data3) > 256) {
                                validate21.errors = [
                                  {
                                    instancePath:
                                      instancePath + "/scope/principal",
                                    schemaPath:
                                      "#/$defs/OperationScope/properties/principal/maxLength",
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
                                    instancePath + "/scope/principal",
                                  schemaPath:
                                    "#/$defs/OperationScope/properties/principal/type",
                                  keyword: "type",
                                  params: { type: "string" },
                                  message: "must be string",
                                },
                              ];
                              return false;
                            }
                          }
                          var valid2 = _errs10 === errors;
                        } else {
                          var valid2 = true;
                        }
                        if (valid2) {
                          if (data0.session !== undefined) {
                            let data4 = data0.session;
                            const _errs12 = errors;
                            if (errors === _errs12) {
                              if (typeof data4 === "string") {
                                if (func2(data4) > 256) {
                                  validate21.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/scope/session",
                                      schemaPath:
                                        "#/$defs/OperationScope/properties/session/maxLength",
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
                                      instancePath + "/scope/session",
                                    schemaPath:
                                      "#/$defs/OperationScope/properties/session/type",
                                    keyword: "type",
                                    params: { type: "string" },
                                    message: "must be string",
                                  },
                                ];
                                return false;
                              }
                            }
                            var valid2 = _errs12 === errors;
                          } else {
                            var valid2 = true;
                          }
                          if (valid2) {
                            if (data0.target !== undefined) {
                              let data5 = data0.target;
                              const _errs14 = errors;
                              if (errors === _errs14) {
                                if (typeof data5 === "string") {
                                  if (func2(data5) > 256) {
                                    validate21.errors = [
                                      {
                                        instancePath:
                                          instancePath + "/scope/target",
                                        schemaPath:
                                          "#/$defs/OperationScope/properties/target/maxLength",
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
                                        instancePath + "/scope/target",
                                      schemaPath:
                                        "#/$defs/OperationScope/properties/target/type",
                                      keyword: "type",
                                      params: { type: "string" },
                                      message: "must be string",
                                    },
                                  ];
                                  return false;
                                }
                              }
                              var valid2 = _errs14 === errors;
                            } else {
                              var valid2 = true;
                            }
                            if (valid2) {
                              if (data0.instance !== undefined) {
                                let data6 = data0.instance;
                                const _errs16 = errors;
                                if (errors === _errs16) {
                                  if (typeof data6 === "string") {
                                    if (func2(data6) > 128) {
                                      validate21.errors = [
                                        {
                                          instancePath:
                                            instancePath + "/scope/instance",
                                          schemaPath:
                                            "#/$defs/OperationScope/properties/instance/maxLength",
                                          keyword: "maxLength",
                                          params: { limit: 128 },
                                          message:
                                            "must NOT have more than 128 characters",
                                        },
                                      ];
                                      return false;
                                    } else {
                                      if (func2(data6) < 1) {
                                        validate21.errors = [
                                          {
                                            instancePath:
                                              instancePath + "/scope/instance",
                                            schemaPath:
                                              "#/$defs/OperationScope/properties/instance/minLength",
                                            keyword: "minLength",
                                            params: { limit: 1 },
                                            message:
                                              "must NOT have fewer than 1 characters",
                                          },
                                        ];
                                        return false;
                                      } else {
                                        if (!pattern4.test(data6)) {
                                          validate21.errors = [
                                            {
                                              instancePath:
                                                instancePath +
                                                "/scope/instance",
                                              schemaPath:
                                                "#/$defs/OperationScope/properties/instance/pattern",
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
                                          instancePath + "/scope/instance",
                                        schemaPath:
                                          "#/$defs/OperationScope/properties/instance/type",
                                        keyword: "type",
                                        params: { type: "string" },
                                        message: "must be string",
                                      },
                                    ];
                                    return false;
                                  }
                                }
                                var valid2 = _errs16 === errors;
                              } else {
                                var valid2 = true;
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
                    instancePath: instancePath + "/scope",
                    schemaPath: "#/$defs/OperationScope/type",
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
            if (data.revision !== undefined) {
              let data7 = data.revision;
              const _errs18 = errors;
              if (
                !(
                  typeof data7 == "number" &&
                  !(data7 % 1) &&
                  !isNaN(data7) &&
                  isFinite(data7)
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
              if (errors === _errs18) {
                if (typeof data7 == "number" && isFinite(data7)) {
                  if (data7 < 0 || isNaN(data7)) {
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
              var valid0 = _errs18 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              if (data.baseline !== undefined) {
                let data8 = data.baseline;
                const _errs20 = errors;
                if (errors === _errs20) {
                  if (typeof data8 === "string") {
                    if (func2(data8) > 256) {
                      validate21.errors = [
                        {
                          instancePath: instancePath + "/baseline",
                          schemaPath: "#/properties/baseline/maxLength",
                          keyword: "maxLength",
                          params: { limit: 256 },
                          message: "must NOT have more than 256 characters",
                        },
                      ];
                      return false;
                    }
                  } else {
                    validate21.errors = [
                      {
                        instancePath: instancePath + "/baseline",
                        schemaPath: "#/properties/baseline/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      },
                    ];
                    return false;
                  }
                }
                var valid0 = _errs20 === errors;
              } else {
                var valid0 = true;
              }
              if (valid0) {
                if (data.title !== undefined) {
                  let data9 = data.title;
                  const _errs22 = errors;
                  if (errors === _errs22) {
                    if (typeof data9 === "string") {
                      if (func2(data9) > 256) {
                        validate21.errors = [
                          {
                            instancePath: instancePath + "/title",
                            schemaPath: "#/properties/title/maxLength",
                            keyword: "maxLength",
                            params: { limit: 256 },
                            message: "must NOT have more than 256 characters",
                          },
                        ];
                        return false;
                      }
                    } else {
                      validate21.errors = [
                        {
                          instancePath: instancePath + "/title",
                          schemaPath: "#/properties/title/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                  }
                  var valid0 = _errs22 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.source !== undefined) {
                    let data10 = data.source;
                    const _errs24 = errors;
                    if (errors === _errs24) {
                      if (typeof data10 === "string") {
                        if (func2(data10) > 256) {
                          validate21.errors = [
                            {
                              instancePath: instancePath + "/source",
                              schemaPath: "#/properties/source/maxLength",
                              keyword: "maxLength",
                              params: { limit: 256 },
                              message: "must NOT have more than 256 characters",
                            },
                          ];
                          return false;
                        }
                      } else {
                        validate21.errors = [
                          {
                            instancePath: instancePath + "/source",
                            schemaPath: "#/properties/source/type",
                            keyword: "type",
                            params: { type: "string" },
                            message: "must be string",
                          },
                        ];
                        return false;
                      }
                    }
                    var valid0 = _errs24 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.targetLabel !== undefined) {
                      let data11 = data.targetLabel;
                      const _errs26 = errors;
                      if (errors === _errs26) {
                        if (typeof data11 === "string") {
                          if (func2(data11) > 256) {
                            validate21.errors = [
                              {
                                instancePath: instancePath + "/targetLabel",
                                schemaPath:
                                  "#/properties/targetLabel/maxLength",
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
                              instancePath: instancePath + "/targetLabel",
                              schemaPath: "#/properties/targetLabel/type",
                              keyword: "type",
                              params: { type: "string" },
                              message: "must be string",
                            },
                          ];
                          return false;
                        }
                      }
                      var valid0 = _errs26 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.impact !== undefined) {
                        let data12 = data.impact;
                        const _errs28 = errors;
                        if (errors === _errs28) {
                          if (typeof data12 === "string") {
                            if (func2(data12) > 4096) {
                              validate21.errors = [
                                {
                                  instancePath: instancePath + "/impact",
                                  schemaPath: "#/properties/impact/maxLength",
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
                                instancePath: instancePath + "/impact",
                                schemaPath: "#/properties/impact/type",
                                keyword: "type",
                                params: { type: "string" },
                                message: "must be string",
                              },
                            ];
                            return false;
                          }
                        }
                        var valid0 = _errs28 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.current !== undefined) {
                          let data13 = data.current;
                          const _errs30 = errors;
                          if (errors === _errs30) {
                            if (
                              data13 &&
                              typeof data13 == "object" &&
                              !Array.isArray(data13)
                            ) {
                              if (Object.keys(data13).length > 128) {
                                validate21.errors = [
                                  {
                                    instancePath: instancePath + "/current",
                                    schemaPath:
                                      "#/properties/current/maxProperties",
                                    keyword: "maxProperties",
                                    params: { limit: 128 },
                                    message:
                                      "must NOT have more than 128 properties",
                                  },
                                ];
                                return false;
                              } else {
                                for (const key2 in data13) {
                                  const _errs32 = errors;
                                  if (errors === _errs32) {
                                    if (typeof key2 === "string") {
                                      if (func2(key2) > 128) {
                                        const err0 = {
                                          instancePath:
                                            instancePath + "/current",
                                          schemaPath:
                                            "#/properties/current/propertyNames/maxLength",
                                          keyword: "maxLength",
                                          params: { limit: 128 },
                                          message:
                                            "must NOT have more than 128 characters",
                                          propertyName: key2,
                                        };
                                        if (vErrors === null) {
                                          vErrors = [err0];
                                        } else {
                                          vErrors.push(err0);
                                        }
                                        errors++;
                                      } else {
                                        if (func2(key2) < 1) {
                                          const err1 = {
                                            instancePath:
                                              instancePath + "/current",
                                            schemaPath:
                                              "#/properties/current/propertyNames/minLength",
                                            keyword: "minLength",
                                            params: { limit: 1 },
                                            message:
                                              "must NOT have fewer than 1 characters",
                                            propertyName: key2,
                                          };
                                          if (vErrors === null) {
                                            vErrors = [err1];
                                          } else {
                                            vErrors.push(err1);
                                          }
                                          errors++;
                                        } else {
                                          if (!pattern4.test(key2)) {
                                            const err2 = {
                                              instancePath:
                                                instancePath + "/current",
                                              schemaPath:
                                                "#/properties/current/propertyNames/pattern",
                                              keyword: "pattern",
                                              params: {
                                                pattern: "^[a-zA-Z0-9_-]+$",
                                              },
                                              message:
                                                'must match pattern "' +
                                                "^[a-zA-Z0-9_-]+$" +
                                                '"',
                                              propertyName: key2,
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
                                        instancePath: instancePath + "/current",
                                        schemaPath:
                                          "#/properties/current/propertyNames/type",
                                        keyword: "type",
                                        params: { type: "string" },
                                        message: "must be string",
                                        propertyName: key2,
                                      };
                                      if (vErrors === null) {
                                        vErrors = [err3];
                                      } else {
                                        vErrors.push(err3);
                                      }
                                      errors++;
                                    }
                                  }
                                  var valid3 = _errs32 === errors;
                                  if (!valid3) {
                                    const err4 = {
                                      instancePath: instancePath + "/current",
                                      schemaPath:
                                        "#/properties/current/propertyNames",
                                      keyword: "propertyNames",
                                      params: { propertyName: key2 },
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
                                if (valid3) {
                                  for (const key3 in data13) {
                                    let data14 = data13[key3];
                                    const _errs35 = errors;
                                    const _errs37 = errors;
                                    let valid6 = false;
                                    const _errs38 = errors;
                                    const _errs39 = errors;
                                    let valid7 = false;
                                    const _errs40 = errors;
                                    if (typeof data14 !== "string") {
                                      const err5 = {
                                        instancePath:
                                          instancePath +
                                          "/current/" +
                                          key3
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
                                    var _valid1 = _errs40 === errors;
                                    valid7 = valid7 || _valid1;
                                    const _errs42 = errors;
                                    if (
                                      !(
                                        typeof data14 == "number" &&
                                        isFinite(data14)
                                      )
                                    ) {
                                      const err6 = {
                                        instancePath:
                                          instancePath +
                                          "/current/" +
                                          key3
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
                                    var _valid1 = _errs42 === errors;
                                    valid7 = valid7 || _valid1;
                                    const _errs44 = errors;
                                    if (typeof data14 !== "boolean") {
                                      const err7 = {
                                        instancePath:
                                          instancePath +
                                          "/current/" +
                                          key3
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
                                    var _valid1 = _errs44 === errors;
                                    valid7 = valid7 || _valid1;
                                    const _errs46 = errors;
                                    if (data14 !== null) {
                                      const err8 = {
                                        instancePath:
                                          instancePath +
                                          "/current/" +
                                          key3
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
                                    var _valid1 = _errs46 === errors;
                                    valid7 = valid7 || _valid1;
                                    if (!valid7) {
                                      const err9 = {
                                        instancePath:
                                          instancePath +
                                          "/current/" +
                                          key3
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
                                      errors = _errs39;
                                      if (vErrors !== null) {
                                        if (_errs39) {
                                          vErrors.length = _errs39;
                                        } else {
                                          vErrors = null;
                                        }
                                      }
                                    }
                                    var _valid0 = _errs38 === errors;
                                    valid6 = valid6 || _valid0;
                                    const _errs48 = errors;
                                    if (errors === _errs48) {
                                      if (Array.isArray(data14)) {
                                        if (data14.length > 128) {
                                          const err10 = {
                                            instancePath:
                                              instancePath +
                                              "/current/" +
                                              key3
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
                                          var valid8 = true;
                                          const len0 = data14.length;
                                          for (let i0 = 0; i0 < len0; i0++) {
                                            let data15 = data14[i0];
                                            const _errs50 = errors;
                                            if (errors === _errs50) {
                                              if (typeof data15 === "string") {
                                                if (func2(data15) > 4096) {
                                                  const err11 = {
                                                    instancePath:
                                                      instancePath +
                                                      "/current/" +
                                                      key3
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
                                                    "/current/" +
                                                    key3
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
                                            var valid8 = _errs50 === errors;
                                            if (!valid8) {
                                              break;
                                            }
                                          }
                                        }
                                      } else {
                                        const err13 = {
                                          instancePath:
                                            instancePath +
                                            "/current/" +
                                            key3
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
                                    var _valid0 = _errs48 === errors;
                                    valid6 = valid6 || _valid0;
                                    if (!valid6) {
                                      const err14 = {
                                        instancePath:
                                          instancePath +
                                          "/current/" +
                                          key3
                                            .replace(/~/g, "~0")
                                            .replace(/\//g, "~1"),
                                        schemaPath:
                                          "#/$defs/OperationValue/anyOf",
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
                                      errors = _errs37;
                                      if (vErrors !== null) {
                                        if (_errs37) {
                                          vErrors.length = _errs37;
                                        } else {
                                          vErrors = null;
                                        }
                                      }
                                    }
                                    var valid4 = _errs35 === errors;
                                    if (!valid4) {
                                      break;
                                    }
                                  }
                                }
                              }
                            } else {
                              validate21.errors = [
                                {
                                  instancePath: instancePath + "/current",
                                  schemaPath: "#/properties/current/type",
                                  keyword: "type",
                                  params: { type: "object" },
                                  message: "must be object",
                                },
                              ];
                              return false;
                            }
                          }
                          var valid0 = _errs30 === errors;
                        } else {
                          var valid0 = true;
                        }
                        if (valid0) {
                          if (data.proposal !== undefined) {
                            let data16 = data.proposal;
                            const _errs52 = errors;
                            if (errors === _errs52) {
                              if (
                                data16 &&
                                typeof data16 == "object" &&
                                !Array.isArray(data16)
                              ) {
                                if (Object.keys(data16).length > 128) {
                                  validate21.errors = [
                                    {
                                      instancePath: instancePath + "/proposal",
                                      schemaPath:
                                        "#/properties/proposal/maxProperties",
                                      keyword: "maxProperties",
                                      params: { limit: 128 },
                                      message:
                                        "must NOT have more than 128 properties",
                                    },
                                  ];
                                  return false;
                                } else {
                                  for (const key4 in data16) {
                                    const _errs54 = errors;
                                    if (errors === _errs54) {
                                      if (typeof key4 === "string") {
                                        if (func2(key4) > 128) {
                                          const err15 = {
                                            instancePath:
                                              instancePath + "/proposal",
                                            schemaPath:
                                              "#/properties/proposal/propertyNames/maxLength",
                                            keyword: "maxLength",
                                            params: { limit: 128 },
                                            message:
                                              "must NOT have more than 128 characters",
                                            propertyName: key4,
                                          };
                                          if (vErrors === null) {
                                            vErrors = [err15];
                                          } else {
                                            vErrors.push(err15);
                                          }
                                          errors++;
                                        } else {
                                          if (func2(key4) < 1) {
                                            const err16 = {
                                              instancePath:
                                                instancePath + "/proposal",
                                              schemaPath:
                                                "#/properties/proposal/propertyNames/minLength",
                                              keyword: "minLength",
                                              params: { limit: 1 },
                                              message:
                                                "must NOT have fewer than 1 characters",
                                              propertyName: key4,
                                            };
                                            if (vErrors === null) {
                                              vErrors = [err16];
                                            } else {
                                              vErrors.push(err16);
                                            }
                                            errors++;
                                          } else {
                                            if (!pattern4.test(key4)) {
                                              const err17 = {
                                                instancePath:
                                                  instancePath + "/proposal",
                                                schemaPath:
                                                  "#/properties/proposal/propertyNames/pattern",
                                                keyword: "pattern",
                                                params: {
                                                  pattern: "^[a-zA-Z0-9_-]+$",
                                                },
                                                message:
                                                  'must match pattern "' +
                                                  "^[a-zA-Z0-9_-]+$" +
                                                  '"',
                                                propertyName: key4,
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
                                            instancePath + "/proposal",
                                          schemaPath:
                                            "#/properties/proposal/propertyNames/type",
                                          keyword: "type",
                                          params: { type: "string" },
                                          message: "must be string",
                                          propertyName: key4,
                                        };
                                        if (vErrors === null) {
                                          vErrors = [err18];
                                        } else {
                                          vErrors.push(err18);
                                        }
                                        errors++;
                                      }
                                    }
                                    var valid9 = _errs54 === errors;
                                    if (!valid9) {
                                      const err19 = {
                                        instancePath:
                                          instancePath + "/proposal",
                                        schemaPath:
                                          "#/properties/proposal/propertyNames",
                                        keyword: "propertyNames",
                                        params: { propertyName: key4 },
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
                                  if (valid9) {
                                    for (const key5 in data16) {
                                      let data17 = data16[key5];
                                      const _errs57 = errors;
                                      const _errs59 = errors;
                                      let valid12 = false;
                                      const _errs60 = errors;
                                      const _errs61 = errors;
                                      let valid13 = false;
                                      const _errs62 = errors;
                                      if (typeof data17 !== "string") {
                                        const err20 = {
                                          instancePath:
                                            instancePath +
                                            "/proposal/" +
                                            key5
                                              .replace(/~/g, "~0")
                                              .replace(/\//g, "~1"),
                                          schemaPath:
                                            "#/$defs/OperationValue/anyOf/0/anyOf/0/type",
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
                                      var _valid3 = _errs62 === errors;
                                      valid13 = valid13 || _valid3;
                                      const _errs64 = errors;
                                      if (
                                        !(
                                          typeof data17 == "number" &&
                                          isFinite(data17)
                                        )
                                      ) {
                                        const err21 = {
                                          instancePath:
                                            instancePath +
                                            "/proposal/" +
                                            key5
                                              .replace(/~/g, "~0")
                                              .replace(/\//g, "~1"),
                                          schemaPath:
                                            "#/$defs/OperationValue/anyOf/0/anyOf/1/type",
                                          keyword: "type",
                                          params: { type: "number" },
                                          message: "must be number",
                                        };
                                        if (vErrors === null) {
                                          vErrors = [err21];
                                        } else {
                                          vErrors.push(err21);
                                        }
                                        errors++;
                                      }
                                      var _valid3 = _errs64 === errors;
                                      valid13 = valid13 || _valid3;
                                      const _errs66 = errors;
                                      if (typeof data17 !== "boolean") {
                                        const err22 = {
                                          instancePath:
                                            instancePath +
                                            "/proposal/" +
                                            key5
                                              .replace(/~/g, "~0")
                                              .replace(/\//g, "~1"),
                                          schemaPath:
                                            "#/$defs/OperationValue/anyOf/0/anyOf/2/type",
                                          keyword: "type",
                                          params: { type: "boolean" },
                                          message: "must be boolean",
                                        };
                                        if (vErrors === null) {
                                          vErrors = [err22];
                                        } else {
                                          vErrors.push(err22);
                                        }
                                        errors++;
                                      }
                                      var _valid3 = _errs66 === errors;
                                      valid13 = valid13 || _valid3;
                                      const _errs68 = errors;
                                      if (data17 !== null) {
                                        const err23 = {
                                          instancePath:
                                            instancePath +
                                            "/proposal/" +
                                            key5
                                              .replace(/~/g, "~0")
                                              .replace(/\//g, "~1"),
                                          schemaPath:
                                            "#/$defs/OperationValue/anyOf/0/anyOf/3/type",
                                          keyword: "type",
                                          params: { type: "null" },
                                          message: "must be null",
                                        };
                                        if (vErrors === null) {
                                          vErrors = [err23];
                                        } else {
                                          vErrors.push(err23);
                                        }
                                        errors++;
                                      }
                                      var _valid3 = _errs68 === errors;
                                      valid13 = valid13 || _valid3;
                                      if (!valid13) {
                                        const err24 = {
                                          instancePath:
                                            instancePath +
                                            "/proposal/" +
                                            key5
                                              .replace(/~/g, "~0")
                                              .replace(/\//g, "~1"),
                                          schemaPath:
                                            "#/$defs/OperationValue/anyOf/0/anyOf",
                                          keyword: "anyOf",
                                          params: {},
                                          message:
                                            "must match a schema in anyOf",
                                        };
                                        if (vErrors === null) {
                                          vErrors = [err24];
                                        } else {
                                          vErrors.push(err24);
                                        }
                                        errors++;
                                      } else {
                                        errors = _errs61;
                                        if (vErrors !== null) {
                                          if (_errs61) {
                                            vErrors.length = _errs61;
                                          } else {
                                            vErrors = null;
                                          }
                                        }
                                      }
                                      var _valid2 = _errs60 === errors;
                                      valid12 = valid12 || _valid2;
                                      const _errs70 = errors;
                                      if (errors === _errs70) {
                                        if (Array.isArray(data17)) {
                                          if (data17.length > 128) {
                                            const err25 = {
                                              instancePath:
                                                instancePath +
                                                "/proposal/" +
                                                key5
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
                                              vErrors = [err25];
                                            } else {
                                              vErrors.push(err25);
                                            }
                                            errors++;
                                          } else {
                                            var valid14 = true;
                                            const len1 = data17.length;
                                            for (let i1 = 0; i1 < len1; i1++) {
                                              let data18 = data17[i1];
                                              const _errs72 = errors;
                                              if (errors === _errs72) {
                                                if (
                                                  typeof data18 === "string"
                                                ) {
                                                  if (func2(data18) > 4096) {
                                                    const err26 = {
                                                      instancePath:
                                                        instancePath +
                                                        "/proposal/" +
                                                        key5
                                                          .replace(/~/g, "~0")
                                                          .replace(
                                                            /\//g,
                                                            "~1",
                                                          ) +
                                                        "/" +
                                                        i1,
                                                      schemaPath:
                                                        "#/$defs/OperationValue/anyOf/1/items/maxLength",
                                                      keyword: "maxLength",
                                                      params: { limit: 4096 },
                                                      message:
                                                        "must NOT have more than 4096 characters",
                                                    };
                                                    if (vErrors === null) {
                                                      vErrors = [err26];
                                                    } else {
                                                      vErrors.push(err26);
                                                    }
                                                    errors++;
                                                  }
                                                } else {
                                                  const err27 = {
                                                    instancePath:
                                                      instancePath +
                                                      "/proposal/" +
                                                      key5
                                                        .replace(/~/g, "~0")
                                                        .replace(/\//g, "~1") +
                                                      "/" +
                                                      i1,
                                                    schemaPath:
                                                      "#/$defs/OperationValue/anyOf/1/items/type",
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
                                              var valid14 = _errs72 === errors;
                                              if (!valid14) {
                                                break;
                                              }
                                            }
                                          }
                                        } else {
                                          const err28 = {
                                            instancePath:
                                              instancePath +
                                              "/proposal/" +
                                              key5
                                                .replace(/~/g, "~0")
                                                .replace(/\//g, "~1"),
                                            schemaPath:
                                              "#/$defs/OperationValue/anyOf/1/type",
                                            keyword: "type",
                                            params: { type: "array" },
                                            message: "must be array",
                                          };
                                          if (vErrors === null) {
                                            vErrors = [err28];
                                          } else {
                                            vErrors.push(err28);
                                          }
                                          errors++;
                                        }
                                      }
                                      var _valid2 = _errs70 === errors;
                                      valid12 = valid12 || _valid2;
                                      if (!valid12) {
                                        const err29 = {
                                          instancePath:
                                            instancePath +
                                            "/proposal/" +
                                            key5
                                              .replace(/~/g, "~0")
                                              .replace(/\//g, "~1"),
                                          schemaPath:
                                            "#/$defs/OperationValue/anyOf",
                                          keyword: "anyOf",
                                          params: {},
                                          message:
                                            "must match a schema in anyOf",
                                        };
                                        if (vErrors === null) {
                                          vErrors = [err29];
                                        } else {
                                          vErrors.push(err29);
                                        }
                                        errors++;
                                        validate21.errors = vErrors;
                                        return false;
                                      } else {
                                        errors = _errs59;
                                        if (vErrors !== null) {
                                          if (_errs59) {
                                            vErrors.length = _errs59;
                                          } else {
                                            vErrors = null;
                                          }
                                        }
                                      }
                                      var valid10 = _errs57 === errors;
                                      if (!valid10) {
                                        break;
                                      }
                                    }
                                  }
                                }
                              } else {
                                validate21.errors = [
                                  {
                                    instancePath: instancePath + "/proposal",
                                    schemaPath: "#/properties/proposal/type",
                                    keyword: "type",
                                    params: { type: "object" },
                                    message: "must be object",
                                  },
                                ];
                                return false;
                              }
                            }
                            var valid0 = _errs52 === errors;
                          } else {
                            var valid0 = true;
                          }
                          if (valid0) {
                            if (data.phase !== undefined) {
                              let data19 = data.phase;
                              const _errs74 = errors;
                              if (
                                !(
                                  data19 === "ready" ||
                                  data19 === "submitting" ||
                                  data19 === "waiting" ||
                                  data19 === "running" ||
                                  data19 === "failed" ||
                                  data19 === "unknown" ||
                                  data19 === "succeeded" ||
                                  data19 === "pending_effect" ||
                                  data19 === "cancelled" ||
                                  data19 === "denied"
                                )
                              ) {
                                validate21.errors = [
                                  {
                                    instancePath: instancePath + "/phase",
                                    schemaPath: "#/properties/phase/enum",
                                    keyword: "enum",
                                    params: {
                                      allowedValues:
                                        schema32.properties.phase.enum,
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
                              if (data.reason !== undefined) {
                                let data20 = data.reason;
                                const _errs75 = errors;
                                if (errors === _errs75) {
                                  if (typeof data20 === "string") {
                                    if (func2(data20) > 4096) {
                                      validate21.errors = [
                                        {
                                          instancePath:
                                            instancePath + "/reason",
                                          schemaPath:
                                            "#/properties/reason/maxLength",
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
                                        instancePath: instancePath + "/reason",
                                        schemaPath: "#/properties/reason/type",
                                        keyword: "type",
                                        params: { type: "string" },
                                        message: "must be string",
                                      },
                                    ];
                                    return false;
                                  }
                                }
                                var valid0 = _errs75 === errors;
                              } else {
                                var valid0 = true;
                              }
                              if (valid0) {
                                if (data.allowed !== undefined) {
                                  let data21 = data.allowed;
                                  const _errs77 = errors;
                                  if (errors === _errs77) {
                                    if (Array.isArray(data21)) {
                                      if (data21.length > 4) {
                                        validate21.errors = [
                                          {
                                            instancePath:
                                              instancePath + "/allowed",
                                            schemaPath:
                                              "#/properties/allowed/maxItems",
                                            keyword: "maxItems",
                                            params: { limit: 4 },
                                            message:
                                              "must NOT have more than 4 items",
                                          },
                                        ];
                                        return false;
                                      } else {
                                        var valid15 = true;
                                        const len2 = data21.length;
                                        for (let i2 = 0; i2 < len2; i2++) {
                                          let data22 = data21[i2];
                                          const _errs79 = errors;
                                          if (
                                            !(
                                              data22 === "submit" ||
                                              data22 === "cancel" ||
                                              data22 === "refresh" ||
                                              data22 === "answer"
                                            )
                                          ) {
                                            validate21.errors = [
                                              {
                                                instancePath:
                                                  instancePath +
                                                  "/allowed/" +
                                                  i2,
                                                schemaPath:
                                                  "#/properties/allowed/items/enum",
                                                keyword: "enum",
                                                params: {
                                                  allowedValues:
                                                    schema32.properties.allowed
                                                      .items.enum,
                                                },
                                                message:
                                                  "must be equal to one of the allowed values",
                                              },
                                            ];
                                            return false;
                                          }
                                          var valid15 = _errs79 === errors;
                                          if (!valid15) {
                                            break;
                                          }
                                        }
                                      }
                                    } else {
                                      validate21.errors = [
                                        {
                                          instancePath:
                                            instancePath + "/allowed",
                                          schemaPath:
                                            "#/properties/allowed/type",
                                          keyword: "type",
                                          params: { type: "array" },
                                          message: "must be array",
                                        },
                                      ];
                                      return false;
                                    }
                                  }
                                  var valid0 = _errs77 === errors;
                                } else {
                                  var valid0 = true;
                                }
                                if (valid0) {
                                  if (data.approval !== undefined) {
                                    let data23 = data.approval;
                                    const _errs80 = errors;
                                    if (errors === _errs80) {
                                      if (
                                        data23 &&
                                        typeof data23 == "object" &&
                                        !Array.isArray(data23)
                                      ) {
                                        let missing2;
                                        if (
                                          (data23.user === undefined &&
                                            (missing2 = "user")) ||
                                          (data23.enterprise === undefined &&
                                            (missing2 = "enterprise")) ||
                                          (data23.elevation === undefined &&
                                            (missing2 = "elevation"))
                                        ) {
                                          validate21.errors = [
                                            {
                                              instancePath:
                                                instancePath + "/approval",
                                              schemaPath:
                                                "#/properties/approval/required",
                                              keyword: "required",
                                              params: {
                                                missingProperty: missing2,
                                              },
                                              message:
                                                "must have required property '" +
                                                missing2 +
                                                "'",
                                            },
                                          ];
                                          return false;
                                        } else {
                                          const _errs82 = errors;
                                          for (const key6 in data23) {
                                            if (
                                              !(
                                                key6 === "user" ||
                                                key6 === "enterprise" ||
                                                key6 === "elevation"
                                              )
                                            ) {
                                              validate21.errors = [
                                                {
                                                  instancePath:
                                                    instancePath + "/approval",
                                                  schemaPath:
                                                    "#/properties/approval/additionalProperties",
                                                  keyword:
                                                    "additionalProperties",
                                                  params: {
                                                    additionalProperty: key6,
                                                  },
                                                  message:
                                                    "must NOT have additional properties",
                                                },
                                              ];
                                              return false;
                                              break;
                                            }
                                          }
                                          if (_errs82 === errors) {
                                            if (data23.user !== undefined) {
                                              let data24 = data23.user;
                                              const _errs83 = errors;
                                              if (errors === _errs83) {
                                                if (
                                                  typeof data24 === "string"
                                                ) {
                                                  if (func2(data24) > 256) {
                                                    validate21.errors = [
                                                      {
                                                        instancePath:
                                                          instancePath +
                                                          "/approval/user",
                                                        schemaPath:
                                                          "#/properties/approval/properties/user/maxLength",
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
                                                        "/approval/user",
                                                      schemaPath:
                                                        "#/properties/approval/properties/user/type",
                                                      keyword: "type",
                                                      params: {
                                                        type: "string",
                                                      },
                                                      message: "must be string",
                                                    },
                                                  ];
                                                  return false;
                                                }
                                              }
                                              var valid16 = _errs83 === errors;
                                            } else {
                                              var valid16 = true;
                                            }
                                            if (valid16) {
                                              if (
                                                data23.enterprise !== undefined
                                              ) {
                                                let data25 = data23.enterprise;
                                                const _errs85 = errors;
                                                if (errors === _errs85) {
                                                  if (
                                                    typeof data25 === "string"
                                                  ) {
                                                    if (func2(data25) > 256) {
                                                      validate21.errors = [
                                                        {
                                                          instancePath:
                                                            instancePath +
                                                            "/approval/enterprise",
                                                          schemaPath:
                                                            "#/properties/approval/properties/enterprise/maxLength",
                                                          keyword: "maxLength",
                                                          params: {
                                                            limit: 256,
                                                          },
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
                                                          "/approval/enterprise",
                                                        schemaPath:
                                                          "#/properties/approval/properties/enterprise/type",
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
                                                var valid16 =
                                                  _errs85 === errors;
                                              } else {
                                                var valid16 = true;
                                              }
                                              if (valid16) {
                                                if (
                                                  data23.elevation !== undefined
                                                ) {
                                                  let data26 = data23.elevation;
                                                  const _errs87 = errors;
                                                  if (errors === _errs87) {
                                                    if (
                                                      typeof data26 === "string"
                                                    ) {
                                                      if (func2(data26) > 256) {
                                                        validate21.errors = [
                                                          {
                                                            instancePath:
                                                              instancePath +
                                                              "/approval/elevation",
                                                            schemaPath:
                                                              "#/properties/approval/properties/elevation/maxLength",
                                                            keyword:
                                                              "maxLength",
                                                            params: {
                                                              limit: 256,
                                                            },
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
                                                            "/approval/elevation",
                                                          schemaPath:
                                                            "#/properties/approval/properties/elevation/type",
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
                                                  var valid16 =
                                                    _errs87 === errors;
                                                } else {
                                                  var valid16 = true;
                                                }
                                              }
                                            }
                                          }
                                        }
                                      } else {
                                        validate21.errors = [
                                          {
                                            instancePath:
                                              instancePath + "/approval",
                                            schemaPath:
                                              "#/properties/approval/type",
                                            keyword: "type",
                                            params: { type: "object" },
                                            message: "must be object",
                                          },
                                        ];
                                        return false;
                                      }
                                    }
                                    var valid0 = _errs80 === errors;
                                  } else {
                                    var valid0 = true;
                                  }
                                  if (valid0) {
                                    if (data.objects !== undefined) {
                                      let data27 = data.objects;
                                      const _errs89 = errors;
                                      if (errors === _errs89) {
                                        if (Array.isArray(data27)) {
                                          if (data27.length > 128) {
                                            validate21.errors = [
                                              {
                                                instancePath:
                                                  instancePath + "/objects",
                                                schemaPath:
                                                  "#/properties/objects/maxItems",
                                                keyword: "maxItems",
                                                params: { limit: 128 },
                                                message:
                                                  "must NOT have more than 128 items",
                                              },
                                            ];
                                            return false;
                                          } else {
                                            var valid17 = true;
                                            const len3 = data27.length;
                                            for (let i3 = 0; i3 < len3; i3++) {
                                              let data28 = data27[i3];
                                              const _errs91 = errors;
                                              const _errs92 = errors;
                                              if (errors === _errs92) {
                                                if (
                                                  data28 &&
                                                  typeof data28 == "object" &&
                                                  !Array.isArray(data28)
                                                ) {
                                                  let missing3;
                                                  if (
                                                    (data28.id === undefined &&
                                                      (missing3 = "id")) ||
                                                    (data28.label ===
                                                      undefined &&
                                                      (missing3 = "label")) ||
                                                    (data28.detail ===
                                                      undefined &&
                                                      (missing3 = "detail"))
                                                  ) {
                                                    validate21.errors = [
                                                      {
                                                        instancePath:
                                                          instancePath +
                                                          "/objects/" +
                                                          i3,
                                                        schemaPath:
                                                          "#/$defs/OperationObject/required",
                                                        keyword: "required",
                                                        params: {
                                                          missingProperty:
                                                            missing3,
                                                        },
                                                        message:
                                                          "must have required property '" +
                                                          missing3 +
                                                          "'",
                                                      },
                                                    ];
                                                    return false;
                                                  } else {
                                                    const _errs94 = errors;
                                                    for (const key7 in data28) {
                                                      if (
                                                        !(
                                                          key7 === "id" ||
                                                          key7 === "label" ||
                                                          key7 === "detail" ||
                                                          key7 ===
                                                            "disabledReason" ||
                                                          key7 === "status"
                                                        )
                                                      ) {
                                                        validate21.errors = [
                                                          {
                                                            instancePath:
                                                              instancePath +
                                                              "/objects/" +
                                                              i3,
                                                            schemaPath:
                                                              "#/$defs/OperationObject/additionalProperties",
                                                            keyword:
                                                              "additionalProperties",
                                                            params: {
                                                              additionalProperty:
                                                                key7,
                                                            },
                                                            message:
                                                              "must NOT have additional properties",
                                                          },
                                                        ];
                                                        return false;
                                                        break;
                                                      }
                                                    }
                                                    if (_errs94 === errors) {
                                                      if (
                                                        data28.id !== undefined
                                                      ) {
                                                        let data29 = data28.id;
                                                        const _errs95 = errors;
                                                        if (
                                                          errors === _errs95
                                                        ) {
                                                          if (
                                                            typeof data29 ===
                                                            "string"
                                                          ) {
                                                            if (
                                                              func2(data29) >
                                                              128
                                                            ) {
                                                              validate21.errors =
                                                                [
                                                                  {
                                                                    instancePath:
                                                                      instancePath +
                                                                      "/objects/" +
                                                                      i3 +
                                                                      "/id",
                                                                    schemaPath:
                                                                      "#/$defs/OperationObject/properties/id/maxLength",
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
                                                                func2(data29) <
                                                                1
                                                              ) {
                                                                validate21.errors =
                                                                  [
                                                                    {
                                                                      instancePath:
                                                                        instancePath +
                                                                        "/objects/" +
                                                                        i3 +
                                                                        "/id",
                                                                      schemaPath:
                                                                        "#/$defs/OperationObject/properties/id/minLength",
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
                                                                    data29,
                                                                  )
                                                                ) {
                                                                  validate21.errors =
                                                                    [
                                                                      {
                                                                        instancePath:
                                                                          instancePath +
                                                                          "/objects/" +
                                                                          i3 +
                                                                          "/id",
                                                                        schemaPath:
                                                                          "#/$defs/OperationObject/properties/id/pattern",
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
                                                                    "/objects/" +
                                                                    i3 +
                                                                    "/id",
                                                                  schemaPath:
                                                                    "#/$defs/OperationObject/properties/id/type",
                                                                  keyword:
                                                                    "type",
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
                                                        var valid19 =
                                                          _errs95 === errors;
                                                      } else {
                                                        var valid19 = true;
                                                      }
                                                      if (valid19) {
                                                        if (
                                                          data28.label !==
                                                          undefined
                                                        ) {
                                                          let data30 =
                                                            data28.label;
                                                          const _errs97 =
                                                            errors;
                                                          if (
                                                            errors === _errs97
                                                          ) {
                                                            if (
                                                              typeof data30 ===
                                                              "string"
                                                            ) {
                                                              if (
                                                                func2(data30) >
                                                                256
                                                              ) {
                                                                validate21.errors =
                                                                  [
                                                                    {
                                                                      instancePath:
                                                                        instancePath +
                                                                        "/objects/" +
                                                                        i3 +
                                                                        "/label",
                                                                      schemaPath:
                                                                        "#/$defs/OperationObject/properties/label/maxLength",
                                                                      keyword:
                                                                        "maxLength",
                                                                      params: {
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
                                                                      "/objects/" +
                                                                      i3 +
                                                                      "/label",
                                                                    schemaPath:
                                                                      "#/$defs/OperationObject/properties/label/type",
                                                                    keyword:
                                                                      "type",
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
                                                          var valid19 =
                                                            _errs97 === errors;
                                                        } else {
                                                          var valid19 = true;
                                                        }
                                                        if (valid19) {
                                                          if (
                                                            data28.detail !==
                                                            undefined
                                                          ) {
                                                            let data31 =
                                                              data28.detail;
                                                            const _errs99 =
                                                              errors;
                                                            if (
                                                              errors === _errs99
                                                            ) {
                                                              if (
                                                                typeof data31 ===
                                                                "string"
                                                              ) {
                                                                if (
                                                                  func2(
                                                                    data31,
                                                                  ) > 4096
                                                                ) {
                                                                  validate21.errors =
                                                                    [
                                                                      {
                                                                        instancePath:
                                                                          instancePath +
                                                                          "/objects/" +
                                                                          i3 +
                                                                          "/detail",
                                                                        schemaPath:
                                                                          "#/$defs/OperationObject/properties/detail/maxLength",
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
                                                                        "/objects/" +
                                                                        i3 +
                                                                        "/detail",
                                                                      schemaPath:
                                                                        "#/$defs/OperationObject/properties/detail/type",
                                                                      keyword:
                                                                        "type",
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
                                                            var valid19 =
                                                              _errs99 ===
                                                              errors;
                                                          } else {
                                                            var valid19 = true;
                                                          }
                                                          if (valid19) {
                                                            if (
                                                              data28.disabledReason !==
                                                              undefined
                                                            ) {
                                                              let data32 =
                                                                data28.disabledReason;
                                                              const _errs101 =
                                                                errors;
                                                              if (
                                                                errors ===
                                                                _errs101
                                                              ) {
                                                                if (
                                                                  typeof data32 ===
                                                                  "string"
                                                                ) {
                                                                  if (
                                                                    func2(
                                                                      data32,
                                                                    ) > 4096
                                                                  ) {
                                                                    validate21.errors =
                                                                      [
                                                                        {
                                                                          instancePath:
                                                                            instancePath +
                                                                            "/objects/" +
                                                                            i3 +
                                                                            "/disabledReason",
                                                                          schemaPath:
                                                                            "#/$defs/OperationObject/properties/disabledReason/maxLength",
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
                                                                          "/objects/" +
                                                                          i3 +
                                                                          "/disabledReason",
                                                                        schemaPath:
                                                                          "#/$defs/OperationObject/properties/disabledReason/type",
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
                                                              var valid19 =
                                                                _errs101 ===
                                                                errors;
                                                            } else {
                                                              var valid19 = true;
                                                            }
                                                            if (valid19) {
                                                              if (
                                                                data28.status !==
                                                                undefined
                                                              ) {
                                                                let data33 =
                                                                  data28.status;
                                                                const _errs103 =
                                                                  errors;
                                                                if (
                                                                  errors ===
                                                                  _errs103
                                                                ) {
                                                                  if (
                                                                    typeof data33 ===
                                                                    "string"
                                                                  ) {
                                                                    if (
                                                                      func2(
                                                                        data33,
                                                                      ) > 256
                                                                    ) {
                                                                      validate21.errors =
                                                                        [
                                                                          {
                                                                            instancePath:
                                                                              instancePath +
                                                                              "/objects/" +
                                                                              i3 +
                                                                              "/status",
                                                                            schemaPath:
                                                                              "#/$defs/OperationObject/properties/status/maxLength",
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
                                                                            "/objects/" +
                                                                            i3 +
                                                                            "/status",
                                                                          schemaPath:
                                                                            "#/$defs/OperationObject/properties/status/type",
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
                                                                var valid19 =
                                                                  _errs103 ===
                                                                  errors;
                                                              } else {
                                                                var valid19 = true;
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
                                                      instancePath:
                                                        instancePath +
                                                        "/objects/" +
                                                        i3,
                                                      schemaPath:
                                                        "#/$defs/OperationObject/type",
                                                      keyword: "type",
                                                      params: {
                                                        type: "object",
                                                      },
                                                      message: "must be object",
                                                    },
                                                  ];
                                                  return false;
                                                }
                                              }
                                              var valid17 = _errs91 === errors;
                                              if (!valid17) {
                                                break;
                                              }
                                            }
                                          }
                                        } else {
                                          validate21.errors = [
                                            {
                                              instancePath:
                                                instancePath + "/objects",
                                              schemaPath:
                                                "#/properties/objects/type",
                                              keyword: "type",
                                              params: { type: "array" },
                                              message: "must be array",
                                            },
                                          ];
                                          return false;
                                        }
                                      }
                                      var valid0 = _errs89 === errors;
                                    } else {
                                      var valid0 = true;
                                    }
                                    if (valid0) {
                                      if (data.logs !== undefined) {
                                        let data34 = data.logs;
                                        const _errs105 = errors;
                                        if (errors === _errs105) {
                                          if (Array.isArray(data34)) {
                                            if (data34.length > 200) {
                                              validate21.errors = [
                                                {
                                                  instancePath:
                                                    instancePath + "/logs",
                                                  schemaPath:
                                                    "#/properties/logs/maxItems",
                                                  keyword: "maxItems",
                                                  params: { limit: 200 },
                                                  message:
                                                    "must NOT have more than 200 items",
                                                },
                                              ];
                                              return false;
                                            } else {
                                              var valid20 = true;
                                              const len4 = data34.length;
                                              for (
                                                let i4 = 0;
                                                i4 < len4;
                                                i4++
                                              ) {
                                                let data35 = data34[i4];
                                                const _errs107 = errors;
                                                if (errors === _errs107) {
                                                  if (
                                                    typeof data35 === "string"
                                                  ) {
                                                    if (func2(data35) > 2048) {
                                                      validate21.errors = [
                                                        {
                                                          instancePath:
                                                            instancePath +
                                                            "/logs/" +
                                                            i4,
                                                          schemaPath:
                                                            "#/properties/logs/items/maxLength",
                                                          keyword: "maxLength",
                                                          params: {
                                                            limit: 2048,
                                                          },
                                                          message:
                                                            "must NOT have more than 2048 characters",
                                                        },
                                                      ];
                                                      return false;
                                                    }
                                                  } else {
                                                    validate21.errors = [
                                                      {
                                                        instancePath:
                                                          instancePath +
                                                          "/logs/" +
                                                          i4,
                                                        schemaPath:
                                                          "#/properties/logs/items/type",
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
                                                var valid20 =
                                                  _errs107 === errors;
                                                if (!valid20) {
                                                  break;
                                                }
                                              }
                                            }
                                          } else {
                                            validate21.errors = [
                                              {
                                                instancePath:
                                                  instancePath + "/logs",
                                                schemaPath:
                                                  "#/properties/logs/type",
                                                keyword: "type",
                                                params: { type: "array" },
                                                message: "must be array",
                                              },
                                            ];
                                            return false;
                                          }
                                        }
                                        var valid0 = _errs105 === errors;
                                      } else {
                                        var valid0 = true;
                                      }
                                      if (valid0) {
                                        if (data.progress !== undefined) {
                                          let data36 = data.progress;
                                          const _errs109 = errors;
                                          if (errors === _errs109) {
                                            if (
                                              data36 &&
                                              typeof data36 == "object" &&
                                              !Array.isArray(data36)
                                            ) {
                                              let missing4;
                                              if (
                                                (data36.completed ===
                                                  undefined &&
                                                  (missing4 = "completed")) ||
                                                (data36.total === undefined &&
                                                  (missing4 = "total"))
                                              ) {
                                                validate21.errors = [
                                                  {
                                                    instancePath:
                                                      instancePath +
                                                      "/progress",
                                                    schemaPath:
                                                      "#/properties/progress/required",
                                                    keyword: "required",
                                                    params: {
                                                      missingProperty: missing4,
                                                    },
                                                    message:
                                                      "must have required property '" +
                                                      missing4 +
                                                      "'",
                                                  },
                                                ];
                                                return false;
                                              } else {
                                                const _errs111 = errors;
                                                for (const key8 in data36) {
                                                  if (
                                                    !(
                                                      key8 === "completed" ||
                                                      key8 === "total"
                                                    )
                                                  ) {
                                                    validate21.errors = [
                                                      {
                                                        instancePath:
                                                          instancePath +
                                                          "/progress",
                                                        schemaPath:
                                                          "#/properties/progress/additionalProperties",
                                                        keyword:
                                                          "additionalProperties",
                                                        params: {
                                                          additionalProperty:
                                                            key8,
                                                        },
                                                        message:
                                                          "must NOT have additional properties",
                                                      },
                                                    ];
                                                    return false;
                                                    break;
                                                  }
                                                }
                                                if (_errs111 === errors) {
                                                  if (
                                                    data36.completed !==
                                                    undefined
                                                  ) {
                                                    let data37 =
                                                      data36.completed;
                                                    const _errs112 = errors;
                                                    if (errors === _errs112) {
                                                      if (
                                                        typeof data37 ==
                                                          "number" &&
                                                        isFinite(data37)
                                                      ) {
                                                        if (
                                                          data37 < 0 ||
                                                          isNaN(data37)
                                                        ) {
                                                          validate21.errors = [
                                                            {
                                                              instancePath:
                                                                instancePath +
                                                                "/progress/completed",
                                                              schemaPath:
                                                                "#/properties/progress/properties/completed/minimum",
                                                              keyword:
                                                                "minimum",
                                                              params: {
                                                                comparison:
                                                                  ">=",
                                                                limit: 0,
                                                              },
                                                              message:
                                                                "must be >= 0",
                                                            },
                                                          ];
                                                          return false;
                                                        }
                                                      } else {
                                                        validate21.errors = [
                                                          {
                                                            instancePath:
                                                              instancePath +
                                                              "/progress/completed",
                                                            schemaPath:
                                                              "#/properties/progress/properties/completed/type",
                                                            keyword: "type",
                                                            params: {
                                                              type: "number",
                                                            },
                                                            message:
                                                              "must be number",
                                                          },
                                                        ];
                                                        return false;
                                                      }
                                                    }
                                                    var valid21 =
                                                      _errs112 === errors;
                                                  } else {
                                                    var valid21 = true;
                                                  }
                                                  if (valid21) {
                                                    if (
                                                      data36.total !== undefined
                                                    ) {
                                                      let data38 = data36.total;
                                                      const _errs114 = errors;
                                                      if (errors === _errs114) {
                                                        if (
                                                          typeof data38 ==
                                                            "number" &&
                                                          isFinite(data38)
                                                        ) {
                                                          if (
                                                            data38 <= 0 ||
                                                            isNaN(data38)
                                                          ) {
                                                            validate21.errors =
                                                              [
                                                                {
                                                                  instancePath:
                                                                    instancePath +
                                                                    "/progress/total",
                                                                  schemaPath:
                                                                    "#/properties/progress/properties/total/exclusiveMinimum",
                                                                  keyword:
                                                                    "exclusiveMinimum",
                                                                  params: {
                                                                    comparison:
                                                                      ">",
                                                                    limit: 0,
                                                                  },
                                                                  message:
                                                                    "must be > 0",
                                                                },
                                                              ];
                                                            return false;
                                                          }
                                                        } else {
                                                          validate21.errors = [
                                                            {
                                                              instancePath:
                                                                instancePath +
                                                                "/progress/total",
                                                              schemaPath:
                                                                "#/properties/progress/properties/total/type",
                                                              keyword: "type",
                                                              params: {
                                                                type: "number",
                                                              },
                                                              message:
                                                                "must be number",
                                                            },
                                                          ];
                                                          return false;
                                                        }
                                                      }
                                                      var valid21 =
                                                        _errs114 === errors;
                                                    } else {
                                                      var valid21 = true;
                                                    }
                                                  }
                                                }
                                              }
                                            } else {
                                              validate21.errors = [
                                                {
                                                  instancePath:
                                                    instancePath + "/progress",
                                                  schemaPath:
                                                    "#/properties/progress/type",
                                                  keyword: "type",
                                                  params: { type: "object" },
                                                  message: "must be object",
                                                },
                                              ];
                                              return false;
                                            }
                                          }
                                          var valid0 = _errs109 === errors;
                                        } else {
                                          var valid0 = true;
                                        }
                                        if (valid0) {
                                          if (data.result !== undefined) {
                                            let data39 = data.result;
                                            const _errs116 = errors;
                                            if (errors === _errs116) {
                                              if (typeof data39 === "string") {
                                                if (func2(data39) > 4096) {
                                                  validate21.errors = [
                                                    {
                                                      instancePath:
                                                        instancePath +
                                                        "/result",
                                                      schemaPath:
                                                        "#/properties/result/maxLength",
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
                                                      instancePath + "/result",
                                                    schemaPath:
                                                      "#/properties/result/type",
                                                    keyword: "type",
                                                    params: { type: "string" },
                                                    message: "must be string",
                                                  },
                                                ];
                                                return false;
                                              }
                                            }
                                            var valid0 = _errs116 === errors;
                                          } else {
                                            var valid0 = true;
                                          }
                                          if (valid0) {
                                            if (
                                              data.interaction !== undefined
                                            ) {
                                              let data40 = data.interaction;
                                              const _errs118 = errors;
                                              const _errs119 = errors;
                                              if (errors === _errs119) {
                                                if (
                                                  data40 &&
                                                  typeof data40 == "object" &&
                                                  !Array.isArray(data40)
                                                ) {
                                                  let missing5;
                                                  if (
                                                    (data40.id === undefined &&
                                                      (missing5 = "id")) ||
                                                    (data40.question ===
                                                      undefined &&
                                                      (missing5 =
                                                        "question")) ||
                                                    (data40.options ===
                                                      undefined &&
                                                      (missing5 = "options")) ||
                                                    (data40.expiresAtMs ===
                                                      undefined &&
                                                      (missing5 =
                                                        "expiresAtMs"))
                                                  ) {
                                                    validate21.errors = [
                                                      {
                                                        instancePath:
                                                          instancePath +
                                                          "/interaction",
                                                        schemaPath:
                                                          "#/$defs/OperationInteraction/required",
                                                        keyword: "required",
                                                        params: {
                                                          missingProperty:
                                                            missing5,
                                                        },
                                                        message:
                                                          "must have required property '" +
                                                          missing5 +
                                                          "'",
                                                      },
                                                    ];
                                                    return false;
                                                  } else {
                                                    const _errs121 = errors;
                                                    for (const key9 in data40) {
                                                      if (
                                                        !(
                                                          key9 === "id" ||
                                                          key9 === "question" ||
                                                          key9 === "options" ||
                                                          key9 === "expiresAtMs"
                                                        )
                                                      ) {
                                                        validate21.errors = [
                                                          {
                                                            instancePath:
                                                              instancePath +
                                                              "/interaction",
                                                            schemaPath:
                                                              "#/$defs/OperationInteraction/additionalProperties",
                                                            keyword:
                                                              "additionalProperties",
                                                            params: {
                                                              additionalProperty:
                                                                key9,
                                                            },
                                                            message:
                                                              "must NOT have additional properties",
                                                          },
                                                        ];
                                                        return false;
                                                        break;
                                                      }
                                                    }
                                                    if (_errs121 === errors) {
                                                      if (
                                                        data40.id !== undefined
                                                      ) {
                                                        let data41 = data40.id;
                                                        const _errs122 = errors;
                                                        if (
                                                          errors === _errs122
                                                        ) {
                                                          if (
                                                            typeof data41 ===
                                                            "string"
                                                          ) {
                                                            if (
                                                              func2(data41) >
                                                              128
                                                            ) {
                                                              validate21.errors =
                                                                [
                                                                  {
                                                                    instancePath:
                                                                      instancePath +
                                                                      "/interaction/id",
                                                                    schemaPath:
                                                                      "#/$defs/OperationInteraction/properties/id/maxLength",
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
                                                                func2(data41) <
                                                                1
                                                              ) {
                                                                validate21.errors =
                                                                  [
                                                                    {
                                                                      instancePath:
                                                                        instancePath +
                                                                        "/interaction/id",
                                                                      schemaPath:
                                                                        "#/$defs/OperationInteraction/properties/id/minLength",
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
                                                                    data41,
                                                                  )
                                                                ) {
                                                                  validate21.errors =
                                                                    [
                                                                      {
                                                                        instancePath:
                                                                          instancePath +
                                                                          "/interaction/id",
                                                                        schemaPath:
                                                                          "#/$defs/OperationInteraction/properties/id/pattern",
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
                                                                    "/interaction/id",
                                                                  schemaPath:
                                                                    "#/$defs/OperationInteraction/properties/id/type",
                                                                  keyword:
                                                                    "type",
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
                                                        var valid23 =
                                                          _errs122 === errors;
                                                      } else {
                                                        var valid23 = true;
                                                      }
                                                      if (valid23) {
                                                        if (
                                                          data40.question !==
                                                          undefined
                                                        ) {
                                                          let data42 =
                                                            data40.question;
                                                          const _errs124 =
                                                            errors;
                                                          if (
                                                            errors === _errs124
                                                          ) {
                                                            if (
                                                              typeof data42 ===
                                                              "string"
                                                            ) {
                                                              if (
                                                                func2(data42) >
                                                                4096
                                                              ) {
                                                                validate21.errors =
                                                                  [
                                                                    {
                                                                      instancePath:
                                                                        instancePath +
                                                                        "/interaction/question",
                                                                      schemaPath:
                                                                        "#/$defs/OperationInteraction/properties/question/maxLength",
                                                                      keyword:
                                                                        "maxLength",
                                                                      params: {
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
                                                                      "/interaction/question",
                                                                    schemaPath:
                                                                      "#/$defs/OperationInteraction/properties/question/type",
                                                                    keyword:
                                                                      "type",
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
                                                          var valid23 =
                                                            _errs124 === errors;
                                                        } else {
                                                          var valid23 = true;
                                                        }
                                                        if (valid23) {
                                                          if (
                                                            data40.options !==
                                                            undefined
                                                          ) {
                                                            let data43 =
                                                              data40.options;
                                                            const _errs126 =
                                                              errors;
                                                            if (
                                                              errors ===
                                                              _errs126
                                                            ) {
                                                              if (
                                                                Array.isArray(
                                                                  data43,
                                                                )
                                                              ) {
                                                                if (
                                                                  data43.length >
                                                                  16
                                                                ) {
                                                                  validate21.errors =
                                                                    [
                                                                      {
                                                                        instancePath:
                                                                          instancePath +
                                                                          "/interaction/options",
                                                                        schemaPath:
                                                                          "#/$defs/OperationInteraction/properties/options/maxItems",
                                                                        keyword:
                                                                          "maxItems",
                                                                        params:
                                                                          {
                                                                            limit: 16,
                                                                          },
                                                                        message:
                                                                          "must NOT have more than 16 items",
                                                                      },
                                                                    ];
                                                                  return false;
                                                                } else {
                                                                  var valid24 = true;
                                                                  const len5 =
                                                                    data43.length;
                                                                  for (
                                                                    let i5 = 0;
                                                                    i5 < len5;
                                                                    i5++
                                                                  ) {
                                                                    let data44 =
                                                                      data43[
                                                                        i5
                                                                      ];
                                                                    const _errs128 =
                                                                      errors;
                                                                    if (
                                                                      errors ===
                                                                      _errs128
                                                                    ) {
                                                                      if (
                                                                        data44 &&
                                                                        typeof data44 ==
                                                                          "object" &&
                                                                        !Array.isArray(
                                                                          data44,
                                                                        )
                                                                      ) {
                                                                        let missing6;
                                                                        if (
                                                                          (data44.id ===
                                                                            undefined &&
                                                                            (missing6 =
                                                                              "id")) ||
                                                                          (data44.label ===
                                                                            undefined &&
                                                                            (missing6 =
                                                                              "label"))
                                                                        ) {
                                                                          validate21.errors =
                                                                            [
                                                                              {
                                                                                instancePath:
                                                                                  instancePath +
                                                                                  "/interaction/options/" +
                                                                                  i5,
                                                                                schemaPath:
                                                                                  "#/$defs/OperationInteraction/properties/options/items/required",
                                                                                keyword:
                                                                                  "required",
                                                                                params:
                                                                                  {
                                                                                    missingProperty:
                                                                                      missing6,
                                                                                  },
                                                                                message:
                                                                                  "must have required property '" +
                                                                                  missing6 +
                                                                                  "'",
                                                                              },
                                                                            ];
                                                                          return false;
                                                                        } else {
                                                                          const _errs130 =
                                                                            errors;
                                                                          for (const key10 in data44) {
                                                                            if (
                                                                              !(
                                                                                key10 ===
                                                                                  "id" ||
                                                                                key10 ===
                                                                                  "label"
                                                                              )
                                                                            ) {
                                                                              validate21.errors =
                                                                                [
                                                                                  {
                                                                                    instancePath:
                                                                                      instancePath +
                                                                                      "/interaction/options/" +
                                                                                      i5,
                                                                                    schemaPath:
                                                                                      "#/$defs/OperationInteraction/properties/options/items/additionalProperties",
                                                                                    keyword:
                                                                                      "additionalProperties",
                                                                                    params:
                                                                                      {
                                                                                        additionalProperty:
                                                                                          key10,
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
                                                                            _errs130 ===
                                                                            errors
                                                                          ) {
                                                                            if (
                                                                              data44.id !==
                                                                              undefined
                                                                            ) {
                                                                              let data45 =
                                                                                data44.id;
                                                                              const _errs131 =
                                                                                errors;
                                                                              if (
                                                                                errors ===
                                                                                _errs131
                                                                              ) {
                                                                                if (
                                                                                  typeof data45 ===
                                                                                  "string"
                                                                                ) {
                                                                                  if (
                                                                                    func2(
                                                                                      data45,
                                                                                    ) >
                                                                                    128
                                                                                  ) {
                                                                                    validate21.errors =
                                                                                      [
                                                                                        {
                                                                                          instancePath:
                                                                                            instancePath +
                                                                                            "/interaction/options/" +
                                                                                            i5 +
                                                                                            "/id",
                                                                                          schemaPath:
                                                                                            "#/$defs/OperationInteraction/properties/options/items/properties/id/maxLength",
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
                                                                                      func2(
                                                                                        data45,
                                                                                      ) <
                                                                                      1
                                                                                    ) {
                                                                                      validate21.errors =
                                                                                        [
                                                                                          {
                                                                                            instancePath:
                                                                                              instancePath +
                                                                                              "/interaction/options/" +
                                                                                              i5 +
                                                                                              "/id",
                                                                                            schemaPath:
                                                                                              "#/$defs/OperationInteraction/properties/options/items/properties/id/minLength",
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
                                                                                          data45,
                                                                                        )
                                                                                      ) {
                                                                                        validate21.errors =
                                                                                          [
                                                                                            {
                                                                                              instancePath:
                                                                                                instancePath +
                                                                                                "/interaction/options/" +
                                                                                                i5 +
                                                                                                "/id",
                                                                                              schemaPath:
                                                                                                "#/$defs/OperationInteraction/properties/options/items/properties/id/pattern",
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
                                                                                          "/interaction/options/" +
                                                                                          i5 +
                                                                                          "/id",
                                                                                        schemaPath:
                                                                                          "#/$defs/OperationInteraction/properties/options/items/properties/id/type",
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
                                                                              var valid25 =
                                                                                _errs131 ===
                                                                                errors;
                                                                            } else {
                                                                              var valid25 = true;
                                                                            }
                                                                            if (
                                                                              valid25
                                                                            ) {
                                                                              if (
                                                                                data44.label !==
                                                                                undefined
                                                                              ) {
                                                                                let data46 =
                                                                                  data44.label;
                                                                                const _errs133 =
                                                                                  errors;
                                                                                if (
                                                                                  errors ===
                                                                                  _errs133
                                                                                ) {
                                                                                  if (
                                                                                    typeof data46 ===
                                                                                    "string"
                                                                                  ) {
                                                                                    if (
                                                                                      func2(
                                                                                        data46,
                                                                                      ) >
                                                                                      256
                                                                                    ) {
                                                                                      validate21.errors =
                                                                                        [
                                                                                          {
                                                                                            instancePath:
                                                                                              instancePath +
                                                                                              "/interaction/options/" +
                                                                                              i5 +
                                                                                              "/label",
                                                                                            schemaPath:
                                                                                              "#/$defs/OperationInteraction/properties/options/items/properties/label/maxLength",
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
                                                                                            "/interaction/options/" +
                                                                                            i5 +
                                                                                            "/label",
                                                                                          schemaPath:
                                                                                            "#/$defs/OperationInteraction/properties/options/items/properties/label/type",
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
                                                                                var valid25 =
                                                                                  _errs133 ===
                                                                                  errors;
                                                                              } else {
                                                                                var valid25 = true;
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
                                                                                "/interaction/options/" +
                                                                                i5,
                                                                              schemaPath:
                                                                                "#/$defs/OperationInteraction/properties/options/items/type",
                                                                              keyword:
                                                                                "type",
                                                                              params:
                                                                                {
                                                                                  type: "object",
                                                                                },
                                                                              message:
                                                                                "must be object",
                                                                            },
                                                                          ];
                                                                        return false;
                                                                      }
                                                                    }
                                                                    var valid24 =
                                                                      _errs128 ===
                                                                      errors;
                                                                    if (
                                                                      !valid24
                                                                    ) {
                                                                      break;
                                                                    }
                                                                  }
                                                                }
                                                              } else {
                                                                validate21.errors =
                                                                  [
                                                                    {
                                                                      instancePath:
                                                                        instancePath +
                                                                        "/interaction/options",
                                                                      schemaPath:
                                                                        "#/$defs/OperationInteraction/properties/options/type",
                                                                      keyword:
                                                                        "type",
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
                                                            var valid23 =
                                                              _errs126 ===
                                                              errors;
                                                          } else {
                                                            var valid23 = true;
                                                          }
                                                          if (valid23) {
                                                            if (
                                                              data40.expiresAtMs !==
                                                              undefined
                                                            ) {
                                                              let data47 =
                                                                data40.expiresAtMs;
                                                              const _errs135 =
                                                                errors;
                                                              if (
                                                                !(
                                                                  typeof data47 ==
                                                                    "number" &&
                                                                  !(
                                                                    data47 % 1
                                                                  ) &&
                                                                  !isNaN(
                                                                    data47,
                                                                  ) &&
                                                                  isFinite(
                                                                    data47,
                                                                  )
                                                                )
                                                              ) {
                                                                validate21.errors =
                                                                  [
                                                                    {
                                                                      instancePath:
                                                                        instancePath +
                                                                        "/interaction/expiresAtMs",
                                                                      schemaPath:
                                                                        "#/$defs/OperationInteraction/properties/expiresAtMs/type",
                                                                      keyword:
                                                                        "type",
                                                                      params: {
                                                                        type: "integer",
                                                                      },
                                                                      message:
                                                                        "must be integer",
                                                                    },
                                                                  ];
                                                                return false;
                                                              }
                                                              if (
                                                                errors ===
                                                                _errs135
                                                              ) {
                                                                if (
                                                                  typeof data47 ==
                                                                    "number" &&
                                                                  isFinite(
                                                                    data47,
                                                                  )
                                                                ) {
                                                                  if (
                                                                    data47 <
                                                                      0 ||
                                                                    isNaN(
                                                                      data47,
                                                                    )
                                                                  ) {
                                                                    validate21.errors =
                                                                      [
                                                                        {
                                                                          instancePath:
                                                                            instancePath +
                                                                            "/interaction/expiresAtMs",
                                                                          schemaPath:
                                                                            "#/$defs/OperationInteraction/properties/expiresAtMs/minimum",
                                                                          keyword:
                                                                            "minimum",
                                                                          params:
                                                                            {
                                                                              comparison:
                                                                                ">=",
                                                                              limit: 0,
                                                                            },
                                                                          message:
                                                                            "must be >= 0",
                                                                        },
                                                                      ];
                                                                    return false;
                                                                  }
                                                                }
                                                              }
                                                              var valid23 =
                                                                _errs135 ===
                                                                errors;
                                                            } else {
                                                              var valid23 = true;
                                                            }
                                                          }
                                                        }
                                                      }
                                                    }
                                                  }
                                                } else {
                                                  validate21.errors = [
                                                    {
                                                      instancePath:
                                                        instancePath +
                                                        "/interaction",
                                                      schemaPath:
                                                        "#/$defs/OperationInteraction/type",
                                                      keyword: "type",
                                                      params: {
                                                        type: "object",
                                                      },
                                                      message: "must be object",
                                                    },
                                                  ];
                                                  return false;
                                                }
                                              }
                                              var valid0 = _errs118 === errors;
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
