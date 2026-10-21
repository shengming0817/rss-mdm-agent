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
  $ref: "#/$defs/OperationSnapshot",
};
const schema32 = {
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
  additionalProperties: true,
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
        (data.current === undefined && (missing0 = "current")) ||
        (data.proposal === undefined && (missing0 = "proposal")) ||
        (data.phase === undefined && (missing0 = "phase")) ||
        (data.allowed === undefined && (missing0 = "allowed")) ||
        (data.approval === undefined && (missing0 = "approval")) ||
        (data.objects === undefined && (missing0 = "objects")) ||
        (data.logs === undefined && (missing0 = "logs")) ||
        (data.presentation === undefined && (missing0 = "presentation"))
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
                if (data.current !== undefined) {
                  let data9 = data.current;
                  const _errs22 = errors;
                  if (errors === _errs22) {
                    if (
                      data9 &&
                      typeof data9 == "object" &&
                      !Array.isArray(data9)
                    ) {
                      if (Object.keys(data9).length > 128) {
                        validate21.errors = [
                          {
                            instancePath: instancePath + "/current",
                            schemaPath: "#/properties/current/maxProperties",
                            keyword: "maxProperties",
                            params: { limit: 128 },
                            message: "must NOT have more than 128 properties",
                          },
                        ];
                        return false;
                      } else {
                        for (const key2 in data9) {
                          const _errs24 = errors;
                          if (errors === _errs24) {
                            if (typeof key2 === "string") {
                              if (func2(key2) > 128) {
                                const err0 = {
                                  instancePath: instancePath + "/current",
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
                                    instancePath: instancePath + "/current",
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
                                      instancePath: instancePath + "/current",
                                      schemaPath:
                                        "#/properties/current/propertyNames/pattern",
                                      keyword: "pattern",
                                      params: { pattern: "^[a-zA-Z0-9_-]+$" },
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
                          var valid3 = _errs24 === errors;
                          if (!valid3) {
                            const err4 = {
                              instancePath: instancePath + "/current",
                              schemaPath: "#/properties/current/propertyNames",
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
                          for (const key3 in data9) {
                            let data10 = data9[key3];
                            const _errs27 = errors;
                            const _errs29 = errors;
                            let valid6 = false;
                            const _errs30 = errors;
                            const _errs31 = errors;
                            let valid7 = false;
                            const _errs32 = errors;
                            if (typeof data10 !== "string") {
                              const err5 = {
                                instancePath:
                                  instancePath +
                                  "/current/" +
                                  key3.replace(/~/g, "~0").replace(/\//g, "~1"),
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
                            var _valid1 = _errs32 === errors;
                            valid7 = valid7 || _valid1;
                            const _errs34 = errors;
                            if (
                              !(typeof data10 == "number" && isFinite(data10))
                            ) {
                              const err6 = {
                                instancePath:
                                  instancePath +
                                  "/current/" +
                                  key3.replace(/~/g, "~0").replace(/\//g, "~1"),
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
                            var _valid1 = _errs34 === errors;
                            valid7 = valid7 || _valid1;
                            const _errs36 = errors;
                            if (typeof data10 !== "boolean") {
                              const err7 = {
                                instancePath:
                                  instancePath +
                                  "/current/" +
                                  key3.replace(/~/g, "~0").replace(/\//g, "~1"),
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
                            var _valid1 = _errs36 === errors;
                            valid7 = valid7 || _valid1;
                            const _errs38 = errors;
                            if (data10 !== null) {
                              const err8 = {
                                instancePath:
                                  instancePath +
                                  "/current/" +
                                  key3.replace(/~/g, "~0").replace(/\//g, "~1"),
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
                            var _valid1 = _errs38 === errors;
                            valid7 = valid7 || _valid1;
                            if (!valid7) {
                              const err9 = {
                                instancePath:
                                  instancePath +
                                  "/current/" +
                                  key3.replace(/~/g, "~0").replace(/\//g, "~1"),
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
                              errors = _errs31;
                              if (vErrors !== null) {
                                if (_errs31) {
                                  vErrors.length = _errs31;
                                } else {
                                  vErrors = null;
                                }
                              }
                            }
                            var _valid0 = _errs30 === errors;
                            valid6 = valid6 || _valid0;
                            const _errs40 = errors;
                            if (errors === _errs40) {
                              if (Array.isArray(data10)) {
                                if (data10.length > 128) {
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
                                  const len0 = data10.length;
                                  for (let i0 = 0; i0 < len0; i0++) {
                                    let data11 = data10[i0];
                                    const _errs42 = errors;
                                    if (errors === _errs42) {
                                      if (typeof data11 === "string") {
                                        if (func2(data11) > 4096) {
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
                                    var valid8 = _errs42 === errors;
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
                            var _valid0 = _errs40 === errors;
                            valid6 = valid6 || _valid0;
                            if (!valid6) {
                              const err14 = {
                                instancePath:
                                  instancePath +
                                  "/current/" +
                                  key3.replace(/~/g, "~0").replace(/\//g, "~1"),
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
                              errors = _errs29;
                              if (vErrors !== null) {
                                if (_errs29) {
                                  vErrors.length = _errs29;
                                } else {
                                  vErrors = null;
                                }
                              }
                            }
                            var valid4 = _errs27 === errors;
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
                  var valid0 = _errs22 === errors;
                } else {
                  var valid0 = true;
                }
                if (valid0) {
                  if (data.proposal !== undefined) {
                    let data12 = data.proposal;
                    const _errs44 = errors;
                    if (errors === _errs44) {
                      if (
                        data12 &&
                        typeof data12 == "object" &&
                        !Array.isArray(data12)
                      ) {
                        if (Object.keys(data12).length > 128) {
                          validate21.errors = [
                            {
                              instancePath: instancePath + "/proposal",
                              schemaPath: "#/properties/proposal/maxProperties",
                              keyword: "maxProperties",
                              params: { limit: 128 },
                              message: "must NOT have more than 128 properties",
                            },
                          ];
                          return false;
                        } else {
                          for (const key4 in data12) {
                            const _errs46 = errors;
                            if (errors === _errs46) {
                              if (typeof key4 === "string") {
                                if (func2(key4) > 128) {
                                  const err15 = {
                                    instancePath: instancePath + "/proposal",
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
                                      instancePath: instancePath + "/proposal",
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
                                        params: { pattern: "^[a-zA-Z0-9_-]+$" },
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
                                  instancePath: instancePath + "/proposal",
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
                            var valid9 = _errs46 === errors;
                            if (!valid9) {
                              const err19 = {
                                instancePath: instancePath + "/proposal",
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
                            for (const key5 in data12) {
                              let data13 = data12[key5];
                              const _errs49 = errors;
                              const _errs51 = errors;
                              let valid12 = false;
                              const _errs52 = errors;
                              const _errs53 = errors;
                              let valid13 = false;
                              const _errs54 = errors;
                              if (typeof data13 !== "string") {
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
                              var _valid3 = _errs54 === errors;
                              valid13 = valid13 || _valid3;
                              const _errs56 = errors;
                              if (
                                !(typeof data13 == "number" && isFinite(data13))
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
                              var _valid3 = _errs56 === errors;
                              valid13 = valid13 || _valid3;
                              const _errs58 = errors;
                              if (typeof data13 !== "boolean") {
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
                              var _valid3 = _errs58 === errors;
                              valid13 = valid13 || _valid3;
                              const _errs60 = errors;
                              if (data13 !== null) {
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
                              var _valid3 = _errs60 === errors;
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
                                  message: "must match a schema in anyOf",
                                };
                                if (vErrors === null) {
                                  vErrors = [err24];
                                } else {
                                  vErrors.push(err24);
                                }
                                errors++;
                              } else {
                                errors = _errs53;
                                if (vErrors !== null) {
                                  if (_errs53) {
                                    vErrors.length = _errs53;
                                  } else {
                                    vErrors = null;
                                  }
                                }
                              }
                              var _valid2 = _errs52 === errors;
                              valid12 = valid12 || _valid2;
                              const _errs62 = errors;
                              if (errors === _errs62) {
                                if (Array.isArray(data13)) {
                                  if (data13.length > 128) {
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
                                    const len1 = data13.length;
                                    for (let i1 = 0; i1 < len1; i1++) {
                                      let data14 = data13[i1];
                                      const _errs64 = errors;
                                      if (errors === _errs64) {
                                        if (typeof data14 === "string") {
                                          if (func2(data14) > 4096) {
                                            const err26 = {
                                              instancePath:
                                                instancePath +
                                                "/proposal/" +
                                                key5
                                                  .replace(/~/g, "~0")
                                                  .replace(/\//g, "~1") +
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
                                      var valid14 = _errs64 === errors;
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
                              var _valid2 = _errs62 === errors;
                              valid12 = valid12 || _valid2;
                              if (!valid12) {
                                const err29 = {
                                  instancePath:
                                    instancePath +
                                    "/proposal/" +
                                    key5
                                      .replace(/~/g, "~0")
                                      .replace(/\//g, "~1"),
                                  schemaPath: "#/$defs/OperationValue/anyOf",
                                  keyword: "anyOf",
                                  params: {},
                                  message: "must match a schema in anyOf",
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
                                errors = _errs51;
                                if (vErrors !== null) {
                                  if (_errs51) {
                                    vErrors.length = _errs51;
                                  } else {
                                    vErrors = null;
                                  }
                                }
                              }
                              var valid10 = _errs49 === errors;
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
                    var valid0 = _errs44 === errors;
                  } else {
                    var valid0 = true;
                  }
                  if (valid0) {
                    if (data.phase !== undefined) {
                      let data15 = data.phase;
                      const _errs66 = errors;
                      if (
                        !(
                          data15 === "ready" ||
                          data15 === "submitting" ||
                          data15 === "waiting" ||
                          data15 === "running" ||
                          data15 === "execution_ended" ||
                          data15 === "failed" ||
                          data15 === "unknown" ||
                          data15 === "succeeded" ||
                          data15 === "pending_effect" ||
                          data15 === "cancelled" ||
                          data15 === "denied"
                        )
                      ) {
                        validate21.errors = [
                          {
                            instancePath: instancePath + "/phase",
                            schemaPath: "#/properties/phase/enum",
                            keyword: "enum",
                            params: {
                              allowedValues: schema32.properties.phase.enum,
                            },
                            message:
                              "must be equal to one of the allowed values",
                          },
                        ];
                        return false;
                      }
                      var valid0 = _errs66 === errors;
                    } else {
                      var valid0 = true;
                    }
                    if (valid0) {
                      if (data.reason !== undefined) {
                        let data16 = data.reason;
                        const _errs67 = errors;
                        if (errors === _errs67) {
                          if (typeof data16 === "string") {
                            if (func2(data16) > 4096) {
                              validate21.errors = [
                                {
                                  instancePath: instancePath + "/reason",
                                  schemaPath: "#/properties/reason/maxLength",
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
                        var valid0 = _errs67 === errors;
                      } else {
                        var valid0 = true;
                      }
                      if (valid0) {
                        if (data.allowed !== undefined) {
                          let data17 = data.allowed;
                          const _errs69 = errors;
                          if (errors === _errs69) {
                            if (Array.isArray(data17)) {
                              if (data17.length > 4) {
                                validate21.errors = [
                                  {
                                    instancePath: instancePath + "/allowed",
                                    schemaPath: "#/properties/allowed/maxItems",
                                    keyword: "maxItems",
                                    params: { limit: 4 },
                                    message: "must NOT have more than 4 items",
                                  },
                                ];
                                return false;
                              } else {
                                var valid15 = true;
                                const len2 = data17.length;
                                for (let i2 = 0; i2 < len2; i2++) {
                                  let data18 = data17[i2];
                                  const _errs71 = errors;
                                  if (
                                    !(
                                      data18 === "submit" ||
                                      data18 === "cancel" ||
                                      data18 === "refresh" ||
                                      data18 === "answer"
                                    )
                                  ) {
                                    validate21.errors = [
                                      {
                                        instancePath:
                                          instancePath + "/allowed/" + i2,
                                        schemaPath:
                                          "#/properties/allowed/items/enum",
                                        keyword: "enum",
                                        params: {
                                          allowedValues:
                                            schema32.properties.allowed.items
                                              .enum,
                                        },
                                        message:
                                          "must be equal to one of the allowed values",
                                      },
                                    ];
                                    return false;
                                  }
                                  var valid15 = _errs71 === errors;
                                  if (!valid15) {
                                    break;
                                  }
                                }
                              }
                            } else {
                              validate21.errors = [
                                {
                                  instancePath: instancePath + "/allowed",
                                  schemaPath: "#/properties/allowed/type",
                                  keyword: "type",
                                  params: { type: "array" },
                                  message: "must be array",
                                },
                              ];
                              return false;
                            }
                          }
                          var valid0 = _errs69 === errors;
                        } else {
                          var valid0 = true;
                        }
                        if (valid0) {
                          if (data.approval !== undefined) {
                            let data19 = data.approval;
                            const _errs72 = errors;
                            if (errors === _errs72) {
                              if (
                                data19 &&
                                typeof data19 == "object" &&
                                !Array.isArray(data19)
                              ) {
                                let missing2;
                                if (
                                  (data19.user === undefined &&
                                    (missing2 = "user")) ||
                                  (data19.enterprise === undefined &&
                                    (missing2 = "enterprise")) ||
                                  (data19.elevation === undefined &&
                                    (missing2 = "elevation"))
                                ) {
                                  validate21.errors = [
                                    {
                                      instancePath: instancePath + "/approval",
                                      schemaPath:
                                        "#/properties/approval/required",
                                      keyword: "required",
                                      params: { missingProperty: missing2 },
                                      message:
                                        "must have required property '" +
                                        missing2 +
                                        "'",
                                    },
                                  ];
                                  return false;
                                } else {
                                  const _errs74 = errors;
                                  for (const key6 in data19) {
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
                                          keyword: "additionalProperties",
                                          params: { additionalProperty: key6 },
                                          message:
                                            "must NOT have additional properties",
                                        },
                                      ];
                                      return false;
                                      break;
                                    }
                                  }
                                  if (_errs74 === errors) {
                                    if (data19.user !== undefined) {
                                      let data20 = data19.user;
                                      const _errs75 = errors;
                                      if (errors === _errs75) {
                                        if (typeof data20 === "string") {
                                          if (func2(data20) > 256) {
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
                                                instancePath + "/approval/user",
                                              schemaPath:
                                                "#/properties/approval/properties/user/type",
                                              keyword: "type",
                                              params: { type: "string" },
                                              message: "must be string",
                                            },
                                          ];
                                          return false;
                                        }
                                      }
                                      var valid16 = _errs75 === errors;
                                    } else {
                                      var valid16 = true;
                                    }
                                    if (valid16) {
                                      if (data19.enterprise !== undefined) {
                                        let data21 = data19.enterprise;
                                        const _errs77 = errors;
                                        if (errors === _errs77) {
                                          if (typeof data21 === "string") {
                                            if (func2(data21) > 256) {
                                              validate21.errors = [
                                                {
                                                  instancePath:
                                                    instancePath +
                                                    "/approval/enterprise",
                                                  schemaPath:
                                                    "#/properties/approval/properties/enterprise/maxLength",
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
                                                  "/approval/enterprise",
                                                schemaPath:
                                                  "#/properties/approval/properties/enterprise/type",
                                                keyword: "type",
                                                params: { type: "string" },
                                                message: "must be string",
                                              },
                                            ];
                                            return false;
                                          }
                                        }
                                        var valid16 = _errs77 === errors;
                                      } else {
                                        var valid16 = true;
                                      }
                                      if (valid16) {
                                        if (data19.elevation !== undefined) {
                                          let data22 = data19.elevation;
                                          const _errs79 = errors;
                                          if (errors === _errs79) {
                                            if (typeof data22 === "string") {
                                              if (func2(data22) > 256) {
                                                validate21.errors = [
                                                  {
                                                    instancePath:
                                                      instancePath +
                                                      "/approval/elevation",
                                                    schemaPath:
                                                      "#/properties/approval/properties/elevation/maxLength",
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
                                                    "/approval/elevation",
                                                  schemaPath:
                                                    "#/properties/approval/properties/elevation/type",
                                                  keyword: "type",
                                                  params: { type: "string" },
                                                  message: "must be string",
                                                },
                                              ];
                                              return false;
                                            }
                                          }
                                          var valid16 = _errs79 === errors;
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
                                    instancePath: instancePath + "/approval",
                                    schemaPath: "#/properties/approval/type",
                                    keyword: "type",
                                    params: { type: "object" },
                                    message: "must be object",
                                  },
                                ];
                                return false;
                              }
                            }
                            var valid0 = _errs72 === errors;
                          } else {
                            var valid0 = true;
                          }
                          if (valid0) {
                            if (data.objects !== undefined) {
                              let data23 = data.objects;
                              const _errs81 = errors;
                              if (errors === _errs81) {
                                if (Array.isArray(data23)) {
                                  if (data23.length > 128) {
                                    validate21.errors = [
                                      {
                                        instancePath: instancePath + "/objects",
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
                                    const len3 = data23.length;
                                    for (let i3 = 0; i3 < len3; i3++) {
                                      let data24 = data23[i3];
                                      const _errs83 = errors;
                                      const _errs84 = errors;
                                      if (errors === _errs84) {
                                        if (
                                          data24 &&
                                          typeof data24 == "object" &&
                                          !Array.isArray(data24)
                                        ) {
                                          let missing3;
                                          if (
                                            (data24.id === undefined &&
                                              (missing3 = "id")) ||
                                            (data24.label === undefined &&
                                              (missing3 = "label")) ||
                                            (data24.detail === undefined &&
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
                                                  missingProperty: missing3,
                                                },
                                                message:
                                                  "must have required property '" +
                                                  missing3 +
                                                  "'",
                                              },
                                            ];
                                            return false;
                                          } else {
                                            if (data24.id !== undefined) {
                                              let data25 = data24.id;
                                              const _errs87 = errors;
                                              if (errors === _errs87) {
                                                if (
                                                  typeof data25 === "string"
                                                ) {
                                                  if (func2(data25) > 128) {
                                                    validate21.errors = [
                                                      {
                                                        instancePath:
                                                          instancePath +
                                                          "/objects/" +
                                                          i3 +
                                                          "/id",
                                                        schemaPath:
                                                          "#/$defs/OperationObject/properties/id/maxLength",
                                                        keyword: "maxLength",
                                                        params: { limit: 128 },
                                                        message:
                                                          "must NOT have more than 128 characters",
                                                      },
                                                    ];
                                                    return false;
                                                  } else {
                                                    if (func2(data25) < 1) {
                                                      validate21.errors = [
                                                        {
                                                          instancePath:
                                                            instancePath +
                                                            "/objects/" +
                                                            i3 +
                                                            "/id",
                                                          schemaPath:
                                                            "#/$defs/OperationObject/properties/id/minLength",
                                                          keyword: "minLength",
                                                          params: { limit: 1 },
                                                          message:
                                                            "must NOT have fewer than 1 characters",
                                                        },
                                                      ];
                                                      return false;
                                                    } else {
                                                      if (
                                                        !pattern4.test(data25)
                                                      ) {
                                                        validate21.errors = [
                                                          {
                                                            instancePath:
                                                              instancePath +
                                                              "/objects/" +
                                                              i3 +
                                                              "/id",
                                                            schemaPath:
                                                              "#/$defs/OperationObject/properties/id/pattern",
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
                                                        "/objects/" +
                                                        i3 +
                                                        "/id",
                                                      schemaPath:
                                                        "#/$defs/OperationObject/properties/id/type",
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
                                              var valid19 = _errs87 === errors;
                                            } else {
                                              var valid19 = true;
                                            }
                                            if (valid19) {
                                              if (data24.label !== undefined) {
                                                let data26 = data24.label;
                                                const _errs89 = errors;
                                                if (errors === _errs89) {
                                                  if (
                                                    typeof data26 === "string"
                                                  ) {
                                                    if (func2(data26) > 256) {
                                                      validate21.errors = [
                                                        {
                                                          instancePath:
                                                            instancePath +
                                                            "/objects/" +
                                                            i3 +
                                                            "/label",
                                                          schemaPath:
                                                            "#/$defs/OperationObject/properties/label/maxLength",
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
                                                          "/objects/" +
                                                          i3 +
                                                          "/label",
                                                        schemaPath:
                                                          "#/$defs/OperationObject/properties/label/type",
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
                                                var valid19 =
                                                  _errs89 === errors;
                                              } else {
                                                var valid19 = true;
                                              }
                                              if (valid19) {
                                                if (
                                                  data24.detail !== undefined
                                                ) {
                                                  let data27 = data24.detail;
                                                  const _errs91 = errors;
                                                  if (errors === _errs91) {
                                                    if (
                                                      typeof data27 === "string"
                                                    ) {
                                                      if (
                                                        func2(data27) > 4096
                                                      ) {
                                                        validate21.errors = [
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
                                                      validate21.errors = [
                                                        {
                                                          instancePath:
                                                            instancePath +
                                                            "/objects/" +
                                                            i3 +
                                                            "/detail",
                                                          schemaPath:
                                                            "#/$defs/OperationObject/properties/detail/type",
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
                                                  var valid19 =
                                                    _errs91 === errors;
                                                } else {
                                                  var valid19 = true;
                                                }
                                                if (valid19) {
                                                  if (
                                                    data24.disabledReason !==
                                                    undefined
                                                  ) {
                                                    let data28 =
                                                      data24.disabledReason;
                                                    const _errs93 = errors;
                                                    if (errors === _errs93) {
                                                      if (
                                                        typeof data28 ===
                                                        "string"
                                                      ) {
                                                        if (
                                                          func2(data28) > 4096
                                                        ) {
                                                          validate21.errors = [
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
                                                        validate21.errors = [
                                                          {
                                                            instancePath:
                                                              instancePath +
                                                              "/objects/" +
                                                              i3 +
                                                              "/disabledReason",
                                                            schemaPath:
                                                              "#/$defs/OperationObject/properties/disabledReason/type",
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
                                                    var valid19 =
                                                      _errs93 === errors;
                                                  } else {
                                                    var valid19 = true;
                                                  }
                                                  if (valid19) {
                                                    if (
                                                      data24.status !==
                                                      undefined
                                                    ) {
                                                      let data29 =
                                                        data24.status;
                                                      const _errs95 = errors;
                                                      if (errors === _errs95) {
                                                        if (
                                                          typeof data29 ===
                                                          "string"
                                                        ) {
                                                          if (
                                                            func2(data29) > 256
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
                                                                "/objects/" +
                                                                i3 +
                                                                "/status",
                                                              schemaPath:
                                                                "#/$defs/OperationObject/properties/status/type",
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
                                                      var valid19 =
                                                        _errs95 === errors;
                                                    } else {
                                                      var valid19 = true;
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
                                                instancePath + "/objects/" + i3,
                                              schemaPath:
                                                "#/$defs/OperationObject/type",
                                              keyword: "type",
                                              params: { type: "object" },
                                              message: "must be object",
                                            },
                                          ];
                                          return false;
                                        }
                                      }
                                      var valid17 = _errs83 === errors;
                                      if (!valid17) {
                                        break;
                                      }
                                    }
                                  }
                                } else {
                                  validate21.errors = [
                                    {
                                      instancePath: instancePath + "/objects",
                                      schemaPath: "#/properties/objects/type",
                                      keyword: "type",
                                      params: { type: "array" },
                                      message: "must be array",
                                    },
                                  ];
                                  return false;
                                }
                              }
                              var valid0 = _errs81 === errors;
                            } else {
                              var valid0 = true;
                            }
                            if (valid0) {
                              if (data.logs !== undefined) {
                                let data30 = data.logs;
                                const _errs97 = errors;
                                if (errors === _errs97) {
                                  if (Array.isArray(data30)) {
                                    if (data30.length > 200) {
                                      validate21.errors = [
                                        {
                                          instancePath: instancePath + "/logs",
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
                                      const len4 = data30.length;
                                      for (let i4 = 0; i4 < len4; i4++) {
                                        let data31 = data30[i4];
                                        const _errs99 = errors;
                                        if (errors === _errs99) {
                                          if (typeof data31 === "string") {
                                            if (func2(data31) > 2048) {
                                              validate21.errors = [
                                                {
                                                  instancePath:
                                                    instancePath +
                                                    "/logs/" +
                                                    i4,
                                                  schemaPath:
                                                    "#/properties/logs/items/maxLength",
                                                  keyword: "maxLength",
                                                  params: { limit: 2048 },
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
                                                  instancePath + "/logs/" + i4,
                                                schemaPath:
                                                  "#/properties/logs/items/type",
                                                keyword: "type",
                                                params: { type: "string" },
                                                message: "must be string",
                                              },
                                            ];
                                            return false;
                                          }
                                        }
                                        var valid20 = _errs99 === errors;
                                        if (!valid20) {
                                          break;
                                        }
                                      }
                                    }
                                  } else {
                                    validate21.errors = [
                                      {
                                        instancePath: instancePath + "/logs",
                                        schemaPath: "#/properties/logs/type",
                                        keyword: "type",
                                        params: { type: "array" },
                                        message: "must be array",
                                      },
                                    ];
                                    return false;
                                  }
                                }
                                var valid0 = _errs97 === errors;
                              } else {
                                var valid0 = true;
                              }
                              if (valid0) {
                                if (data.progress !== undefined) {
                                  let data32 = data.progress;
                                  const _errs101 = errors;
                                  if (errors === _errs101) {
                                    if (
                                      data32 &&
                                      typeof data32 == "object" &&
                                      !Array.isArray(data32)
                                    ) {
                                      let missing4;
                                      if (
                                        (data32.completed === undefined &&
                                          (missing4 = "completed")) ||
                                        (data32.total === undefined &&
                                          (missing4 = "total"))
                                      ) {
                                        validate21.errors = [
                                          {
                                            instancePath:
                                              instancePath + "/progress",
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
                                        const _errs103 = errors;
                                        for (const key7 in data32) {
                                          if (
                                            !(
                                              key7 === "completed" ||
                                              key7 === "total"
                                            )
                                          ) {
                                            validate21.errors = [
                                              {
                                                instancePath:
                                                  instancePath + "/progress",
                                                schemaPath:
                                                  "#/properties/progress/additionalProperties",
                                                keyword: "additionalProperties",
                                                params: {
                                                  additionalProperty: key7,
                                                },
                                                message:
                                                  "must NOT have additional properties",
                                              },
                                            ];
                                            return false;
                                            break;
                                          }
                                        }
                                        if (_errs103 === errors) {
                                          if (data32.completed !== undefined) {
                                            let data33 = data32.completed;
                                            const _errs104 = errors;
                                            if (errors === _errs104) {
                                              if (
                                                typeof data33 == "number" &&
                                                isFinite(data33)
                                              ) {
                                                if (
                                                  data33 < 0 ||
                                                  isNaN(data33)
                                                ) {
                                                  validate21.errors = [
                                                    {
                                                      instancePath:
                                                        instancePath +
                                                        "/progress/completed",
                                                      schemaPath:
                                                        "#/properties/progress/properties/completed/minimum",
                                                      keyword: "minimum",
                                                      params: {
                                                        comparison: ">=",
                                                        limit: 0,
                                                      },
                                                      message: "must be >= 0",
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
                                                    params: { type: "number" },
                                                    message: "must be number",
                                                  },
                                                ];
                                                return false;
                                              }
                                            }
                                            var valid21 = _errs104 === errors;
                                          } else {
                                            var valid21 = true;
                                          }
                                          if (valid21) {
                                            if (data32.total !== undefined) {
                                              let data34 = data32.total;
                                              const _errs106 = errors;
                                              if (errors === _errs106) {
                                                if (
                                                  typeof data34 == "number" &&
                                                  isFinite(data34)
                                                ) {
                                                  if (
                                                    data34 <= 0 ||
                                                    isNaN(data34)
                                                  ) {
                                                    validate21.errors = [
                                                      {
                                                        instancePath:
                                                          instancePath +
                                                          "/progress/total",
                                                        schemaPath:
                                                          "#/properties/progress/properties/total/exclusiveMinimum",
                                                        keyword:
                                                          "exclusiveMinimum",
                                                        params: {
                                                          comparison: ">",
                                                          limit: 0,
                                                        },
                                                        message: "must be > 0",
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
                                                      message: "must be number",
                                                    },
                                                  ];
                                                  return false;
                                                }
                                              }
                                              var valid21 = _errs106 === errors;
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
                                  var valid0 = _errs101 === errors;
                                } else {
                                  var valid0 = true;
                                }
                                if (valid0) {
                                  if (data.result !== undefined) {
                                    let data35 = data.result;
                                    const _errs108 = errors;
                                    if (errors === _errs108) {
                                      if (typeof data35 === "string") {
                                        if (func2(data35) > 4096) {
                                          validate21.errors = [
                                            {
                                              instancePath:
                                                instancePath + "/result",
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
                                    var valid0 = _errs108 === errors;
                                  } else {
                                    var valid0 = true;
                                  }
                                  if (valid0) {
                                    if (data.interaction !== undefined) {
                                      let data36 = data.interaction;
                                      const _errs110 = errors;
                                      const _errs111 = errors;
                                      if (errors === _errs111) {
                                        if (
                                          data36 &&
                                          typeof data36 == "object" &&
                                          !Array.isArray(data36)
                                        ) {
                                          let missing5;
                                          if (
                                            (data36.id === undefined &&
                                              (missing5 = "id")) ||
                                            (data36.question === undefined &&
                                              (missing5 = "question")) ||
                                            (data36.options === undefined &&
                                              (missing5 = "options")) ||
                                            (data36.expiresAtMs === undefined &&
                                              (missing5 = "expiresAtMs"))
                                          ) {
                                            validate21.errors = [
                                              {
                                                instancePath:
                                                  instancePath + "/interaction",
                                                schemaPath:
                                                  "#/$defs/OperationInteraction/required",
                                                keyword: "required",
                                                params: {
                                                  missingProperty: missing5,
                                                },
                                                message:
                                                  "must have required property '" +
                                                  missing5 +
                                                  "'",
                                              },
                                            ];
                                            return false;
                                          } else {
                                            const _errs113 = errors;
                                            for (const key8 in data36) {
                                              if (
                                                !(
                                                  key8 === "id" ||
                                                  key8 === "question" ||
                                                  key8 === "options" ||
                                                  key8 === "expiresAtMs"
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
                                                      additionalProperty: key8,
                                                    },
                                                    message:
                                                      "must NOT have additional properties",
                                                  },
                                                ];
                                                return false;
                                                break;
                                              }
                                            }
                                            if (_errs113 === errors) {
                                              if (data36.id !== undefined) {
                                                let data37 = data36.id;
                                                const _errs114 = errors;
                                                if (errors === _errs114) {
                                                  if (
                                                    typeof data37 === "string"
                                                  ) {
                                                    if (func2(data37) > 128) {
                                                      validate21.errors = [
                                                        {
                                                          instancePath:
                                                            instancePath +
                                                            "/interaction/id",
                                                          schemaPath:
                                                            "#/$defs/OperationInteraction/properties/id/maxLength",
                                                          keyword: "maxLength",
                                                          params: {
                                                            limit: 128,
                                                          },
                                                          message:
                                                            "must NOT have more than 128 characters",
                                                        },
                                                      ];
                                                      return false;
                                                    } else {
                                                      if (func2(data37) < 1) {
                                                        validate21.errors = [
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
                                                          !pattern4.test(data37)
                                                        ) {
                                                          validate21.errors = [
                                                            {
                                                              instancePath:
                                                                instancePath +
                                                                "/interaction/id",
                                                              schemaPath:
                                                                "#/$defs/OperationInteraction/properties/id/pattern",
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
                                                          "/interaction/id",
                                                        schemaPath:
                                                          "#/$defs/OperationInteraction/properties/id/type",
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
                                                var valid23 =
                                                  _errs114 === errors;
                                              } else {
                                                var valid23 = true;
                                              }
                                              if (valid23) {
                                                if (
                                                  data36.question !== undefined
                                                ) {
                                                  let data38 = data36.question;
                                                  const _errs116 = errors;
                                                  if (errors === _errs116) {
                                                    if (
                                                      typeof data38 === "string"
                                                    ) {
                                                      if (
                                                        func2(data38) > 4096
                                                      ) {
                                                        validate21.errors = [
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
                                                      validate21.errors = [
                                                        {
                                                          instancePath:
                                                            instancePath +
                                                            "/interaction/question",
                                                          schemaPath:
                                                            "#/$defs/OperationInteraction/properties/question/type",
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
                                                  var valid23 =
                                                    _errs116 === errors;
                                                } else {
                                                  var valid23 = true;
                                                }
                                                if (valid23) {
                                                  if (
                                                    data36.options !== undefined
                                                  ) {
                                                    let data39 = data36.options;
                                                    const _errs118 = errors;
                                                    if (errors === _errs118) {
                                                      if (
                                                        Array.isArray(data39)
                                                      ) {
                                                        if (
                                                          data39.length > 16
                                                        ) {
                                                          validate21.errors = [
                                                            {
                                                              instancePath:
                                                                instancePath +
                                                                "/interaction/options",
                                                              schemaPath:
                                                                "#/$defs/OperationInteraction/properties/options/maxItems",
                                                              keyword:
                                                                "maxItems",
                                                              params: {
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
                                                            data39.length;
                                                          for (
                                                            let i5 = 0;
                                                            i5 < len5;
                                                            i5++
                                                          ) {
                                                            let data40 =
                                                              data39[i5];
                                                            const _errs120 =
                                                              errors;
                                                            if (
                                                              errors ===
                                                              _errs120
                                                            ) {
                                                              if (
                                                                data40 &&
                                                                typeof data40 ==
                                                                  "object" &&
                                                                !Array.isArray(
                                                                  data40,
                                                                )
                                                              ) {
                                                                let missing6;
                                                                if (
                                                                  (data40.id ===
                                                                    undefined &&
                                                                    (missing6 =
                                                                      "id")) ||
                                                                  (data40.label ===
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
                                                                  const _errs122 =
                                                                    errors;
                                                                  for (const key9 in data40) {
                                                                    if (
                                                                      !(
                                                                        key9 ===
                                                                          "id" ||
                                                                        key9 ===
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
                                                                  if (
                                                                    _errs122 ===
                                                                    errors
                                                                  ) {
                                                                    if (
                                                                      data40.id !==
                                                                      undefined
                                                                    ) {
                                                                      let data41 =
                                                                        data40.id;
                                                                      const _errs123 =
                                                                        errors;
                                                                      if (
                                                                        errors ===
                                                                        _errs123
                                                                      ) {
                                                                        if (
                                                                          typeof data41 ===
                                                                          "string"
                                                                        ) {
                                                                          if (
                                                                            func2(
                                                                              data41,
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
                                                                                data41,
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
                                                                                  data41,
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
                                                                        _errs123 ===
                                                                        errors;
                                                                    } else {
                                                                      var valid25 = true;
                                                                    }
                                                                    if (
                                                                      valid25
                                                                    ) {
                                                                      if (
                                                                        data40.label !==
                                                                        undefined
                                                                      ) {
                                                                        let data42 =
                                                                          data40.label;
                                                                        const _errs125 =
                                                                          errors;
                                                                        if (
                                                                          errors ===
                                                                          _errs125
                                                                        ) {
                                                                          if (
                                                                            typeof data42 ===
                                                                            "string"
                                                                          ) {
                                                                            if (
                                                                              func2(
                                                                                data42,
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
                                                                          _errs125 ===
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
                                                            var valid24 =
                                                              _errs120 ===
                                                              errors;
                                                            if (!valid24) {
                                                              break;
                                                            }
                                                          }
                                                        }
                                                      } else {
                                                        validate21.errors = [
                                                          {
                                                            instancePath:
                                                              instancePath +
                                                              "/interaction/options",
                                                            schemaPath:
                                                              "#/$defs/OperationInteraction/properties/options/type",
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
                                                    var valid23 =
                                                      _errs118 === errors;
                                                  } else {
                                                    var valid23 = true;
                                                  }
                                                  if (valid23) {
                                                    if (
                                                      data36.expiresAtMs !==
                                                      undefined
                                                    ) {
                                                      let data43 =
                                                        data36.expiresAtMs;
                                                      const _errs127 = errors;
                                                      if (
                                                        !(
                                                          typeof data43 ==
                                                            "number" &&
                                                          !(data43 % 1) &&
                                                          !isNaN(data43) &&
                                                          isFinite(data43)
                                                        )
                                                      ) {
                                                        validate21.errors = [
                                                          {
                                                            instancePath:
                                                              instancePath +
                                                              "/interaction/expiresAtMs",
                                                            schemaPath:
                                                              "#/$defs/OperationInteraction/properties/expiresAtMs/type",
                                                            keyword: "type",
                                                            params: {
                                                              type: "integer",
                                                            },
                                                            message:
                                                              "must be integer",
                                                          },
                                                        ];
                                                        return false;
                                                      }
                                                      if (errors === _errs127) {
                                                        if (
                                                          typeof data43 ==
                                                            "number" &&
                                                          isFinite(data43)
                                                        ) {
                                                          if (
                                                            data43 < 0 ||
                                                            isNaN(data43)
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
                                                        }
                                                      }
                                                      var valid23 =
                                                        _errs127 === errors;
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
                                                instancePath + "/interaction",
                                              schemaPath:
                                                "#/$defs/OperationInteraction/type",
                                              keyword: "type",
                                              params: { type: "object" },
                                              message: "must be object",
                                            },
                                          ];
                                          return false;
                                        }
                                      }
                                      var valid0 = _errs110 === errors;
                                    } else {
                                      var valid0 = true;
                                    }
                                    if (valid0) {
                                      if (data.expiresAtMs !== undefined) {
                                        let data44 = data.expiresAtMs;
                                        const _errs129 = errors;
                                        if (
                                          !(
                                            typeof data44 == "number" &&
                                            !(data44 % 1) &&
                                            !isNaN(data44) &&
                                            isFinite(data44)
                                          )
                                        ) {
                                          validate21.errors = [
                                            {
                                              instancePath:
                                                instancePath + "/expiresAtMs",
                                              schemaPath:
                                                "#/properties/expiresAtMs/type",
                                              keyword: "type",
                                              params: { type: "integer" },
                                              message: "must be integer",
                                            },
                                          ];
                                          return false;
                                        }
                                        if (errors === _errs129) {
                                          if (
                                            typeof data44 == "number" &&
                                            isFinite(data44)
                                          ) {
                                            if (
                                              data44 > 9007199254740991 ||
                                              isNaN(data44)
                                            ) {
                                              validate21.errors = [
                                                {
                                                  instancePath:
                                                    instancePath +
                                                    "/expiresAtMs",
                                                  schemaPath:
                                                    "#/properties/expiresAtMs/maximum",
                                                  keyword: "maximum",
                                                  params: {
                                                    comparison: "<=",
                                                    limit: 9007199254740991,
                                                  },
                                                  message:
                                                    "must be <= 9007199254740991",
                                                },
                                              ];
                                              return false;
                                            } else {
                                              if (data44 < 0 || isNaN(data44)) {
                                                validate21.errors = [
                                                  {
                                                    instancePath:
                                                      instancePath +
                                                      "/expiresAtMs",
                                                    schemaPath:
                                                      "#/properties/expiresAtMs/minimum",
                                                    keyword: "minimum",
                                                    params: {
                                                      comparison: ">=",
                                                      limit: 0,
                                                    },
                                                    message: "must be >= 0",
                                                  },
                                                ];
                                                return false;
                                              }
                                            }
                                          }
                                        }
                                        var valid0 = _errs129 === errors;
                                      } else {
                                        var valid0 = true;
                                      }
                                      if (valid0) {
                                        if (data.presentation !== undefined) {
                                          let data45 = data.presentation;
                                          const _errs131 = errors;
                                          if (errors === _errs131) {
                                            if (
                                              data45 &&
                                              typeof data45 == "object" &&
                                              !Array.isArray(data45)
                                            ) {
                                              let missing7;
                                              if (
                                                (data45.title === undefined &&
                                                  (missing7 = "title")) ||
                                                (data45.targetLabel ===
                                                  undefined &&
                                                  (missing7 = "targetLabel")) ||
                                                (data45.source === undefined &&
                                                  (missing7 = "source")) ||
                                                (data45.impact === undefined &&
                                                  (missing7 = "impact"))
                                              ) {
                                                validate21.errors = [
                                                  {
                                                    instancePath:
                                                      instancePath +
                                                      "/presentation",
                                                    schemaPath:
                                                      "#/properties/presentation/required",
                                                    keyword: "required",
                                                    params: {
                                                      missingProperty: missing7,
                                                    },
                                                    message:
                                                      "must have required property '" +
                                                      missing7 +
                                                      "'",
                                                  },
                                                ];
                                                return false;
                                              } else {
                                                if (
                                                  data45.title !== undefined
                                                ) {
                                                  let data46 = data45.title;
                                                  const _errs134 = errors;
                                                  if (errors === _errs134) {
                                                    if (
                                                      typeof data46 === "string"
                                                    ) {
                                                      if (func2(data46) > 256) {
                                                        validate21.errors = [
                                                          {
                                                            instancePath:
                                                              instancePath +
                                                              "/presentation/title",
                                                            schemaPath:
                                                              "#/properties/presentation/properties/title/maxLength",
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
                                                            "/presentation/title",
                                                          schemaPath:
                                                            "#/properties/presentation/properties/title/type",
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
                                                  var valid26 =
                                                    _errs134 === errors;
                                                } else {
                                                  var valid26 = true;
                                                }
                                                if (valid26) {
                                                  if (
                                                    data45.targetLabel !==
                                                    undefined
                                                  ) {
                                                    let data47 =
                                                      data45.targetLabel;
                                                    const _errs136 = errors;
                                                    if (errors === _errs136) {
                                                      if (
                                                        typeof data47 ===
                                                        "string"
                                                      ) {
                                                        if (
                                                          func2(data47) > 256
                                                        ) {
                                                          validate21.errors = [
                                                            {
                                                              instancePath:
                                                                instancePath +
                                                                "/presentation/targetLabel",
                                                              schemaPath:
                                                                "#/properties/presentation/properties/targetLabel/maxLength",
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
                                                              "/presentation/targetLabel",
                                                            schemaPath:
                                                              "#/properties/presentation/properties/targetLabel/type",
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
                                                    var valid26 =
                                                      _errs136 === errors;
                                                  } else {
                                                    var valid26 = true;
                                                  }
                                                  if (valid26) {
                                                    if (
                                                      data45.source !==
                                                      undefined
                                                    ) {
                                                      let data48 =
                                                        data45.source;
                                                      const _errs138 = errors;
                                                      if (errors === _errs138) {
                                                        if (
                                                          typeof data48 ===
                                                          "string"
                                                        ) {
                                                          if (
                                                            func2(data48) > 256
                                                          ) {
                                                            validate21.errors =
                                                              [
                                                                {
                                                                  instancePath:
                                                                    instancePath +
                                                                    "/presentation/source",
                                                                  schemaPath:
                                                                    "#/properties/presentation/properties/source/maxLength",
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
                                                                "/presentation/source",
                                                              schemaPath:
                                                                "#/properties/presentation/properties/source/type",
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
                                                      var valid26 =
                                                        _errs138 === errors;
                                                    } else {
                                                      var valid26 = true;
                                                    }
                                                    if (valid26) {
                                                      if (
                                                        data45.impact !==
                                                        undefined
                                                      ) {
                                                        let data49 =
                                                          data45.impact;
                                                        const _errs140 = errors;
                                                        if (
                                                          errors === _errs140
                                                        ) {
                                                          if (
                                                            typeof data49 ===
                                                            "string"
                                                          ) {
                                                            if (
                                                              func2(data49) >
                                                              4096
                                                            ) {
                                                              validate21.errors =
                                                                [
                                                                  {
                                                                    instancePath:
                                                                      instancePath +
                                                                      "/presentation/impact",
                                                                    schemaPath:
                                                                      "#/properties/presentation/properties/impact/maxLength",
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
                                                                    "/presentation/impact",
                                                                  schemaPath:
                                                                    "#/properties/presentation/properties/impact/type",
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
                                                        var valid26 =
                                                          _errs140 === errors;
                                                      } else {
                                                        var valid26 = true;
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
                                                    "/presentation",
                                                  schemaPath:
                                                    "#/properties/presentation/type",
                                                  keyword: "type",
                                                  params: { type: "object" },
                                                  message: "must be object",
                                                },
                                              ];
                                              return false;
                                            }
                                          }
                                          var valid0 = _errs131 === errors;
                                        } else {
                                          var valid0 = true;
                                        }
                                        if (valid0) {
                                          if (data.secretFields !== undefined) {
                                            let data50 = data.secretFields;
                                            const _errs142 = errors;
                                            if (errors === _errs142) {
                                              if (Array.isArray(data50)) {
                                                if (data50.length > 128) {
                                                  validate21.errors = [
                                                    {
                                                      instancePath:
                                                        instancePath +
                                                        "/secretFields",
                                                      schemaPath:
                                                        "#/properties/secretFields/maxItems",
                                                      keyword: "maxItems",
                                                      params: { limit: 128 },
                                                      message:
                                                        "must NOT have more than 128 items",
                                                    },
                                                  ];
                                                  return false;
                                                } else {
                                                  var valid27 = true;
                                                  const len6 = data50.length;
                                                  for (
                                                    let i6 = 0;
                                                    i6 < len6;
                                                    i6++
                                                  ) {
                                                    let data51 = data50[i6];
                                                    const _errs144 = errors;
                                                    if (errors === _errs144) {
                                                      if (
                                                        typeof data51 ===
                                                        "string"
                                                      ) {
                                                        if (
                                                          func2(data51) > 128
                                                        ) {
                                                          validate21.errors = [
                                                            {
                                                              instancePath:
                                                                instancePath +
                                                                "/secretFields/" +
                                                                i6,
                                                              schemaPath:
                                                                "#/properties/secretFields/items/maxLength",
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
                                                            func2(data51) < 1
                                                          ) {
                                                            validate21.errors =
                                                              [
                                                                {
                                                                  instancePath:
                                                                    instancePath +
                                                                    "/secretFields/" +
                                                                    i6,
                                                                  schemaPath:
                                                                    "#/properties/secretFields/items/minLength",
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
                                                          }
                                                        }
                                                      } else {
                                                        validate21.errors = [
                                                          {
                                                            instancePath:
                                                              instancePath +
                                                              "/secretFields/" +
                                                              i6,
                                                            schemaPath:
                                                              "#/properties/secretFields/items/type",
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
                                                    var valid27 =
                                                      _errs144 === errors;
                                                    if (!valid27) {
                                                      break;
                                                    }
                                                  }
                                                  if (valid27) {
                                                    let i7 = data50.length;
                                                    let j0;
                                                    if (i7 > 1) {
                                                      const indices0 = {};
                                                      for (; i7--; ) {
                                                        let item0 = data50[i7];
                                                        if (
                                                          typeof item0 !==
                                                          "string"
                                                        ) {
                                                          continue;
                                                        }
                                                        if (
                                                          typeof indices0[
                                                            item0
                                                          ] == "number"
                                                        ) {
                                                          j0 = indices0[item0];
                                                          validate21.errors = [
                                                            {
                                                              instancePath:
                                                                instancePath +
                                                                "/secretFields",
                                                              schemaPath:
                                                                "#/properties/secretFields/uniqueItems",
                                                              keyword:
                                                                "uniqueItems",
                                                              params: {
                                                                i: i7,
                                                                j: j0,
                                                              },
                                                              message:
                                                                "must NOT have duplicate items (items ## " +
                                                                j0 +
                                                                " and " +
                                                                i7 +
                                                                " are identical)",
                                                            },
                                                          ];
                                                          return false;
                                                          break;
                                                        }
                                                        indices0[item0] = i7;
                                                      }
                                                    }
                                                  }
                                                }
                                              } else {
                                                validate21.errors = [
                                                  {
                                                    instancePath:
                                                      instancePath +
                                                      "/secretFields",
                                                    schemaPath:
                                                      "#/properties/secretFields/type",
                                                    keyword: "type",
                                                    params: { type: "array" },
                                                    message: "must be array",
                                                  },
                                                ];
                                                return false;
                                              }
                                            }
                                            var valid0 = _errs142 === errors;
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
