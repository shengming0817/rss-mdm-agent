// @ts-nocheck
// @generated from canonical schemas; do not edit.

"use strict";
export const validate = validate20;
export default validate20;
const schema31 = {
  $schema: "https://json-schema.org/draft/2020-12/schema",
  $id: "https://a2ui.org/specification/v0_9/server_to_client.json",
  title: "A2UI Message Schema",
  description:
    "Describes a JSON payload for an A2UI (Agent to UI) message, which is used to dynamically construct and update user interfaces.",
  type: "object",
  oneOf: [
    { $ref: "#/$defs/CreateSurfaceMessage" },
    { $ref: "#/$defs/UpdateComponentsMessage" },
    { $ref: "#/$defs/UpdateDataModelMessage" },
    { $ref: "#/$defs/DeleteSurfaceMessage" },
  ],
  $defs: {
    CreateSurfaceMessage: {
      type: "object",
      properties: {
        version: { enum: ["v0.9", "v0.9.1"] },
        createSurface: {
          type: "object",
          description:
            "Signals the client to create a new surface and begin rendering it. It is an error to send 'createSurface' for a surfaceId that already exists without first deleting it. When this message is sent, the client will expect 'updateComponents' and/or 'updateDataModel' messages for the same surfaceId that define the component tree.",
          properties: {
            surfaceId: {
              type: "string",
              description:
                "The unique identifier for the UI surface to be rendered.",
            },
            catalogId: {
              description:
                "A string that uniquely identifies this catalog. It is recommended to prefix this with an internet domain that you own, to avoid conflicts e.g. mycompany.com:somecatalog'.",
              type: "string",
            },
            theme: {
              $ref: "catalog.json#/$defs/theme",
              description:
                "Theme parameters for the surface (e.g., {'primaryColor': '#FF0000'}). These must validate against the 'theme' schema defined in the catalog.",
            },
            sendDataModel: {
              type: "boolean",
              description:
                "If true, the client will send the full data model of this surface in the metadata of every A2A message sent to the server that created the surface. Defaults to false.",
            },
          },
          required: ["surfaceId", "catalogId"],
          additionalProperties: false,
        },
      },
      required: ["createSurface", "version"],
      additionalProperties: false,
    },
    UpdateComponentsMessage: {
      type: "object",
      properties: {
        version: { enum: ["v0.9", "v0.9.1"] },
        updateComponents: {
          type: "object",
          description:
            "Updates a surface with a new set of components. This message can be sent multiple times to update the component tree of an existing surface. One of the components in one of the components lists MUST have an 'id' of 'root' to serve as the root of the component tree. A createSurface message MUST have been previously sent for the 'surfaceId' in this message; the surface's catalog is the one specified by that createSurface.",
          properties: {
            surfaceId: {
              type: "string",
              description:
                "The unique identifier for the UI surface to be updated.",
            },
            components: {
              type: "array",
              description:
                "A list containing all UI components for the surface.",
              minItems: 1,
              items: { $ref: "catalog.json#/$defs/anyComponent" },
            },
          },
          required: ["surfaceId", "components"],
          additionalProperties: false,
        },
      },
      required: ["updateComponents", "version"],
      additionalProperties: false,
    },
    UpdateDataModelMessage: {
      type: "object",
      properties: {
        version: { enum: ["v0.9", "v0.9.1"] },
        updateDataModel: {
          type: "object",
          description:
            "Updates the data model for an existing surface. This message can be sent multiple times to update the data model. A createSurface message MUST have been previously sent for the 'surfaceId' in this message; the surface's catalog is the one specified by that createSurface.",
          properties: {
            surfaceId: {
              type: "string",
              description:
                "The unique identifier for the UI surface this data model update applies to.",
            },
            path: {
              type: "string",
              description:
                "An optional path to a location within the data model (e.g., '/user/name'). If omitted, or set to '/', refers to the entire data model.",
            },
            value: {
              description:
                "The data to be updated in the data model. If present, the value at 'path' is replaced (or created). If omitted, the key at 'path' is removed.",
              additionalProperties: true,
            },
          },
          required: ["surfaceId"],
          additionalProperties: false,
        },
      },
      required: ["updateDataModel", "version"],
      additionalProperties: false,
    },
    DeleteSurfaceMessage: {
      type: "object",
      properties: {
        version: { enum: ["v0.9", "v0.9.1"] },
        deleteSurface: {
          type: "object",
          description:
            "Signals the client to delete the surface identified by 'surfaceId'. A createSurface message MUST have been previously sent for the 'surfaceId' in this message; the surface's catalog is the one specified by that createSurface.",
          properties: {
            surfaceId: {
              type: "string",
              description:
                "The unique identifier for the UI surface to be deleted.",
            },
          },
          required: ["surfaceId"],
          additionalProperties: false,
        },
      },
      required: ["deleteSurface", "version"],
      additionalProperties: false,
    },
  },
};
const schema119 = {
  type: "object",
  properties: {
    version: { enum: ["v0.9", "v0.9.1"] },
    updateDataModel: {
      type: "object",
      description:
        "Updates the data model for an existing surface. This message can be sent multiple times to update the data model. A createSurface message MUST have been previously sent for the 'surfaceId' in this message; the surface's catalog is the one specified by that createSurface.",
      properties: {
        surfaceId: {
          type: "string",
          description:
            "The unique identifier for the UI surface this data model update applies to.",
        },
        path: {
          type: "string",
          description:
            "An optional path to a location within the data model (e.g., '/user/name'). If omitted, or set to '/', refers to the entire data model.",
        },
        value: {
          description:
            "The data to be updated in the data model. If present, the value at 'path' is replaced (or created). If omitted, the key at 'path' is removed.",
          additionalProperties: true,
        },
      },
      required: ["surfaceId"],
      additionalProperties: false,
    },
  },
  required: ["updateDataModel", "version"],
  additionalProperties: false,
};
const schema120 = {
  type: "object",
  properties: {
    version: { enum: ["v0.9", "v0.9.1"] },
    deleteSurface: {
      type: "object",
      description:
        "Signals the client to delete the surface identified by 'surfaceId'. A createSurface message MUST have been previously sent for the 'surfaceId' in this message; the surface's catalog is the one specified by that createSurface.",
      properties: {
        surfaceId: {
          type: "string",
          description:
            "The unique identifier for the UI surface to be deleted.",
        },
      },
      required: ["surfaceId"],
      additionalProperties: false,
    },
  },
  required: ["deleteSurface", "version"],
  additionalProperties: false,
};
const schema32 = {
  type: "object",
  properties: {
    version: { enum: ["v0.9", "v0.9.1"] },
    createSurface: {
      type: "object",
      description:
        "Signals the client to create a new surface and begin rendering it. It is an error to send 'createSurface' for a surfaceId that already exists without first deleting it. When this message is sent, the client will expect 'updateComponents' and/or 'updateDataModel' messages for the same surfaceId that define the component tree.",
      properties: {
        surfaceId: {
          type: "string",
          description:
            "The unique identifier for the UI surface to be rendered.",
        },
        catalogId: {
          description:
            "A string that uniquely identifies this catalog. It is recommended to prefix this with an internet domain that you own, to avoid conflicts e.g. mycompany.com:somecatalog'.",
          type: "string",
        },
        theme: {
          $ref: "catalog.json#/$defs/theme",
          description:
            "Theme parameters for the surface (e.g., {'primaryColor': '#FF0000'}). These must validate against the 'theme' schema defined in the catalog.",
        },
        sendDataModel: {
          type: "boolean",
          description:
            "If true, the client will send the full data model of this surface in the metadata of every A2A message sent to the server that created the surface. Defaults to false.",
        },
      },
      required: ["surfaceId", "catalogId"],
      additionalProperties: false,
    },
  },
  required: ["createSurface", "version"],
  additionalProperties: false,
};
const schema34 = {
  type: "object",
  properties: {
    primaryColor: {
      type: "string",
      description:
        "The primary brand color used for highlights (e.g., primary buttons, active borders). Renderers may generate variants of this color for different contexts. Format: Hexadecimal code (e.g., '#00BFFF').",
      pattern: "^#[0-9a-fA-F]{6}$",
    },
    iconUrl: {
      type: "string",
      format: "uri",
      description:
        "A URL for an image that identifies the agent or tool associated with the surface.",
    },
    agentDisplayName: {
      type: "string",
      description:
        "Text to be displayed next to the surface to identify the agent or tool that created it.",
    },
  },
  additionalProperties: true,
};
const pattern4 = new RegExp("^#[0-9a-fA-F]{6}$", "u");
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
        (data.createSurface === undefined && (missing0 = "createSurface")) ||
        (data.version === undefined && (missing0 = "version"))
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
          if (!(key0 === "version" || key0 === "createSurface")) {
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
          if (data.version !== undefined) {
            let data0 = data.version;
            const _errs2 = errors;
            if (!(data0 === "v0.9" || data0 === "v0.9.1")) {
              validate21.errors = [
                {
                  instancePath: instancePath + "/version",
                  schemaPath: "#/properties/version/enum",
                  keyword: "enum",
                  params: { allowedValues: schema32.properties.version.enum },
                  message: "must be equal to one of the allowed values",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.createSurface !== undefined) {
              let data1 = data.createSurface;
              const _errs3 = errors;
              if (errors === _errs3) {
                if (
                  data1 &&
                  typeof data1 == "object" &&
                  !Array.isArray(data1)
                ) {
                  let missing1;
                  if (
                    (data1.surfaceId === undefined &&
                      (missing1 = "surfaceId")) ||
                    (data1.catalogId === undefined && (missing1 = "catalogId"))
                  ) {
                    validate21.errors = [
                      {
                        instancePath: instancePath + "/createSurface",
                        schemaPath: "#/properties/createSurface/required",
                        keyword: "required",
                        params: { missingProperty: missing1 },
                        message:
                          "must have required property '" + missing1 + "'",
                      },
                    ];
                    return false;
                  } else {
                    const _errs5 = errors;
                    for (const key1 in data1) {
                      if (
                        !(
                          key1 === "surfaceId" ||
                          key1 === "catalogId" ||
                          key1 === "theme" ||
                          key1 === "sendDataModel"
                        )
                      ) {
                        validate21.errors = [
                          {
                            instancePath: instancePath + "/createSurface",
                            schemaPath:
                              "#/properties/createSurface/additionalProperties",
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
                      if (data1.surfaceId !== undefined) {
                        const _errs6 = errors;
                        if (typeof data1.surfaceId !== "string") {
                          validate21.errors = [
                            {
                              instancePath:
                                instancePath + "/createSurface/surfaceId",
                              schemaPath:
                                "#/properties/createSurface/properties/surfaceId/type",
                              keyword: "type",
                              params: { type: "string" },
                              message: "must be string",
                            },
                          ];
                          return false;
                        }
                        var valid1 = _errs6 === errors;
                      } else {
                        var valid1 = true;
                      }
                      if (valid1) {
                        if (data1.catalogId !== undefined) {
                          const _errs8 = errors;
                          if (typeof data1.catalogId !== "string") {
                            validate21.errors = [
                              {
                                instancePath:
                                  instancePath + "/createSurface/catalogId",
                                schemaPath:
                                  "#/properties/createSurface/properties/catalogId/type",
                                keyword: "type",
                                params: { type: "string" },
                                message: "must be string",
                              },
                            ];
                            return false;
                          }
                          var valid1 = _errs8 === errors;
                        } else {
                          var valid1 = true;
                        }
                        if (valid1) {
                          if (data1.theme !== undefined) {
                            let data4 = data1.theme;
                            const _errs10 = errors;
                            const _errs11 = errors;
                            if (errors === _errs11) {
                              if (
                                data4 &&
                                typeof data4 == "object" &&
                                !Array.isArray(data4)
                              ) {
                                if (data4.primaryColor !== undefined) {
                                  let data5 = data4.primaryColor;
                                  const _errs14 = errors;
                                  if (errors === _errs14) {
                                    if (typeof data5 === "string") {
                                      if (!pattern4.test(data5)) {
                                        validate21.errors = [
                                          {
                                            instancePath:
                                              instancePath +
                                              "/createSurface/theme/primaryColor",
                                            schemaPath:
                                              "catalog.json#/$defs/theme/properties/primaryColor/pattern",
                                            keyword: "pattern",
                                            params: {
                                              pattern: "^#[0-9a-fA-F]{6}$",
                                            },
                                            message:
                                              'must match pattern "' +
                                              "^#[0-9a-fA-F]{6}$" +
                                              '"',
                                          },
                                        ];
                                        return false;
                                      }
                                    } else {
                                      validate21.errors = [
                                        {
                                          instancePath:
                                            instancePath +
                                            "/createSurface/theme/primaryColor",
                                          schemaPath:
                                            "catalog.json#/$defs/theme/properties/primaryColor/type",
                                          keyword: "type",
                                          params: { type: "string" },
                                          message: "must be string",
                                        },
                                      ];
                                      return false;
                                    }
                                  }
                                  var valid3 = _errs14 === errors;
                                } else {
                                  var valid3 = true;
                                }
                                if (valid3) {
                                  if (data4.iconUrl !== undefined) {
                                    const _errs16 = errors;
                                    if (errors === _errs16) {
                                      if (errors === _errs16) {
                                        if (
                                          !(typeof data4.iconUrl === "string")
                                        ) {
                                          validate21.errors = [
                                            {
                                              instancePath:
                                                instancePath +
                                                "/createSurface/theme/iconUrl",
                                              schemaPath:
                                                "catalog.json#/$defs/theme/properties/iconUrl/type",
                                              keyword: "type",
                                              params: { type: "string" },
                                              message: "must be string",
                                            },
                                          ];
                                          return false;
                                        }
                                      }
                                    }
                                    var valid3 = _errs16 === errors;
                                  } else {
                                    var valid3 = true;
                                  }
                                  if (valid3) {
                                    if (data4.agentDisplayName !== undefined) {
                                      const _errs18 = errors;
                                      if (
                                        typeof data4.agentDisplayName !==
                                        "string"
                                      ) {
                                        validate21.errors = [
                                          {
                                            instancePath:
                                              instancePath +
                                              "/createSurface/theme/agentDisplayName",
                                            schemaPath:
                                              "catalog.json#/$defs/theme/properties/agentDisplayName/type",
                                            keyword: "type",
                                            params: { type: "string" },
                                            message: "must be string",
                                          },
                                        ];
                                        return false;
                                      }
                                      var valid3 = _errs18 === errors;
                                    } else {
                                      var valid3 = true;
                                    }
                                  }
                                }
                              } else {
                                validate21.errors = [
                                  {
                                    instancePath:
                                      instancePath + "/createSurface/theme",
                                    schemaPath:
                                      "catalog.json#/$defs/theme/type",
                                    keyword: "type",
                                    params: { type: "object" },
                                    message: "must be object",
                                  },
                                ];
                                return false;
                              }
                            }
                            var valid1 = _errs10 === errors;
                          } else {
                            var valid1 = true;
                          }
                          if (valid1) {
                            if (data1.sendDataModel !== undefined) {
                              const _errs20 = errors;
                              if (typeof data1.sendDataModel !== "boolean") {
                                validate21.errors = [
                                  {
                                    instancePath:
                                      instancePath +
                                      "/createSurface/sendDataModel",
                                    schemaPath:
                                      "#/properties/createSurface/properties/sendDataModel/type",
                                    keyword: "type",
                                    params: { type: "boolean" },
                                    message: "must be boolean",
                                  },
                                ];
                                return false;
                              }
                              var valid1 = _errs20 === errors;
                            } else {
                              var valid1 = true;
                            }
                          }
                        }
                      }
                    }
                  }
                } else {
                  validate21.errors = [
                    {
                      instancePath: instancePath + "/createSurface",
                      schemaPath: "#/properties/createSurface/type",
                      keyword: "type",
                      params: { type: "object" },
                      message: "must be object",
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
const schema35 = {
  type: "object",
  properties: {
    version: { enum: ["v0.9", "v0.9.1"] },
    updateComponents: {
      type: "object",
      description:
        "Updates a surface with a new set of components. This message can be sent multiple times to update the component tree of an existing surface. One of the components in one of the components lists MUST have an 'id' of 'root' to serve as the root of the component tree. A createSurface message MUST have been previously sent for the 'surfaceId' in this message; the surface's catalog is the one specified by that createSurface.",
      properties: {
        surfaceId: {
          type: "string",
          description:
            "The unique identifier for the UI surface to be updated.",
        },
        components: {
          type: "array",
          description: "A list containing all UI components for the surface.",
          minItems: 1,
          items: { $ref: "catalog.json#/$defs/anyComponent" },
        },
      },
      required: ["surfaceId", "components"],
      additionalProperties: false,
    },
  },
  required: ["updateComponents", "version"],
  additionalProperties: false,
};
const schema36 = {
  oneOf: [
    { $ref: "#/components/Text" },
    { $ref: "#/components/Image" },
    { $ref: "#/components/Icon" },
    { $ref: "#/components/Video" },
    { $ref: "#/components/AudioPlayer" },
    { $ref: "#/components/Row" },
    { $ref: "#/components/Column" },
    { $ref: "#/components/List" },
    { $ref: "#/components/Card" },
    { $ref: "#/components/Tabs" },
    { $ref: "#/components/Modal" },
    { $ref: "#/components/Divider" },
    { $ref: "#/components/Button" },
    { $ref: "#/components/TextField" },
    { $ref: "#/components/CheckBox" },
    { $ref: "#/components/ChoicePicker" },
    { $ref: "#/components/Slider" },
    { $ref: "#/components/DateTimeInput" },
  ],
  discriminator: { propertyName: "component" },
};
const schema37 = {
  type: "object",
  allOf: [
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentCommon",
    },
    { $ref: "#/$defs/CatalogComponentCommon" },
    {
      type: "object",
      properties: {
        component: { const: "Text" },
        text: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
          description:
            "The text content to display. While simple Markdown formatting is supported (i.e. without HTML, images, or links), utilizing dedicated UI components is generally preferred for a richer and more structured presentation.",
        },
        variant: {
          type: "string",
          description: "A hint for the base text style.",
          enum: ["h1", "h2", "h3", "h4", "h5", "caption", "body"],
          default: "body",
        },
      },
      required: ["component", "text"],
    },
  ],
  unevaluatedProperties: false,
};
const schema68 = {
  type: "object",
  properties: {
    weight: {
      type: "number",
      description:
        "The relative weight of this component within a Row or Column. This is similar to the CSS 'flex-grow' property. Note: this may ONLY be set when the component is a direct descendant of a Row or Column.",
    },
  },
};
const schema39 = {
  type: "object",
  properties: {
    id: { $ref: "#/$defs/ComponentId" },
    accessibility: { $ref: "#/$defs/AccessibilityAttributes" },
  },
  required: ["id"],
};
const schema40 = {
  type: "string",
  description:
    "The unique identifier for a component, used for both definitions and references within the same surface.",
};
const schema41 = {
  type: "object",
  description:
    "Attributes to enhance accessibility when using assistive technologies like screen readers.",
  properties: {
    label: {
      $ref: "#/$defs/DynamicString",
      description:
        "A short string, typically 1 to 3 words, used by assistive technologies to convey the purpose or intent of an element. For example, an input field might have an accessible label of 'User ID' or a button might be labeled 'Submit'.",
    },
    description: {
      $ref: "#/$defs/DynamicString",
      description:
        "Additional information provided by assistive technologies about an element such as instructions, format requirements, or result of an action. For example, a mute button might have a label of 'Mute' and a description of 'Silences notifications about this conversation'.",
    },
  },
};
const schema42 = {
  description: "Represents a string",
  oneOf: [
    { type: "string" },
    { $ref: "#/$defs/DataBinding" },
    {
      allOf: [
        { $ref: "#/$defs/FunctionCall" },
        { properties: { returnType: { const: "string" } } },
      ],
    },
  ],
};
const schema43 = {
  type: "object",
  properties: {
    path: {
      type: "string",
      description: "A JSON Pointer path to a value in the data model.",
    },
  },
  required: ["path"],
  additionalProperties: false,
};
const schema44 = {
  type: "object",
  description: "Invokes a named function on the client.",
  properties: {
    call: { type: "string", description: "The name of the function to call." },
    args: {
      type: "object",
      description: "Arguments passed to the function.",
      additionalProperties: {
        anyOf: [
          { $ref: "#/$defs/DynamicValue" },
          {
            type: "object",
            description: "A literal object argument (e.g. configuration).",
          },
        ],
      },
    },
    returnType: {
      type: "string",
      description: "The expected return type of the function call.",
      enum: ["string", "number", "boolean", "array", "object", "any", "void"],
      default: "boolean",
    },
  },
  required: ["call"],
  oneOf: [{ $ref: "catalog.json#/$defs/anyFunction" }],
};
const schema45 = {
  oneOf: [
    { $ref: "#/functions/required" },
    { $ref: "#/functions/regex" },
    { $ref: "#/functions/length" },
    { $ref: "#/functions/numeric" },
    { $ref: "#/functions/email" },
    { $ref: "#/functions/formatString" },
    { $ref: "#/functions/formatNumber" },
    { $ref: "#/functions/formatCurrency" },
    { $ref: "#/functions/formatDate" },
    { $ref: "#/functions/pluralize" },
    { $ref: "#/functions/openUrl" },
    { $ref: "#/functions/and" },
    { $ref: "#/functions/or" },
    { $ref: "#/functions/not" },
  ],
};
const schema46 = {
  type: "object",
  description: "Checks that the value is not null, undefined, or empty.",
  properties: {
    call: { const: "required" },
    args: {
      type: "object",
      properties: { value: { description: "The value to check." } },
      required: ["value"],
      additionalProperties: false,
    },
    returnType: { const: "boolean" },
  },
  required: ["call", "args"],
  unevaluatedProperties: false,
};
const schema62 = {
  type: "object",
  description:
    "Opens the specified URL in a browser or handler. This function has no return value.",
  properties: {
    call: { const: "openUrl" },
    args: {
      type: "object",
      properties: {
        url: { type: "string", format: "uri", description: "The URL to open." },
      },
      required: ["url"],
      additionalProperties: false,
    },
    returnType: { const: "void" },
  },
  required: ["call", "args"],
  unevaluatedProperties: false,
};
const schema47 = {
  type: "object",
  description: "Checks that the value matches a regular expression string.",
  properties: {
    call: { const: "regex" },
    args: {
      type: "object",
      properties: {
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
        },
        pattern: {
          type: "string",
          description: "The regex pattern to match against.",
        },
      },
      required: ["value", "pattern"],
      unevaluatedProperties: false,
    },
    returnType: { const: "boolean" },
  },
  required: ["call", "args"],
  unevaluatedProperties: false,
};
const wrapper0 = { validate: validate30 };
function validate33(
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
  const evaluated0 = validate33.evaluated;
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
        (data.call === undefined && (missing0 = "call")) ||
        (data.args === undefined && (missing0 = "args"))
      ) {
        validate33.errors = [
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
        if (data.call !== undefined) {
          const _errs1 = errors;
          if ("regex" !== data.call) {
            validate33.errors = [
              {
                instancePath: instancePath + "/call",
                schemaPath: "#/properties/call/const",
                keyword: "const",
                params: { allowedValue: "regex" },
                message: "must be equal to constant",
              },
            ];
            return false;
          }
          var valid0 = _errs1 === errors;
        } else {
          var valid0 = true;
        }
        if (valid0) {
          if (data.args !== undefined) {
            let data1 = data.args;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (data1 && typeof data1 == "object" && !Array.isArray(data1)) {
                let missing1;
                if (
                  (data1.value === undefined && (missing1 = "value")) ||
                  (data1.pattern === undefined && (missing1 = "pattern"))
                ) {
                  validate33.errors = [
                    {
                      instancePath: instancePath + "/args",
                      schemaPath: "#/properties/args/required",
                      keyword: "required",
                      params: { missingProperty: missing1 },
                      message: "must have required property '" + missing1 + "'",
                    },
                  ];
                  return false;
                } else {
                  if (data1.value !== undefined) {
                    const _errs4 = errors;
                    if (
                      !wrapper0.validate(data1.value, {
                        instancePath: instancePath + "/args/value",
                        parentData: data1,
                        parentDataProperty: "value",
                        rootData,
                        dynamicAnchors,
                      })
                    ) {
                      vErrors =
                        vErrors === null
                          ? wrapper0.validate.errors
                          : vErrors.concat(wrapper0.validate.errors);
                      errors = vErrors.length;
                    }
                    var valid1 = _errs4 === errors;
                  } else {
                    var valid1 = true;
                  }
                  if (valid1) {
                    if (data1.pattern !== undefined) {
                      const _errs5 = errors;
                      if (typeof data1.pattern !== "string") {
                        validate33.errors = [
                          {
                            instancePath: instancePath + "/args/pattern",
                            schemaPath:
                              "#/properties/args/properties/pattern/type",
                            keyword: "type",
                            params: { type: "string" },
                            message: "must be string",
                          },
                        ];
                        return false;
                      }
                      var valid1 = _errs5 === errors;
                    } else {
                      var valid1 = true;
                    }
                    if (valid1) {
                      for (const key0 in data1) {
                        if (key0 !== "value" && key0 !== "pattern") {
                          validate33.errors = [
                            {
                              instancePath: instancePath + "/args",
                              schemaPath:
                                "#/properties/args/unevaluatedProperties",
                              keyword: "unevaluatedProperties",
                              params: { unevaluatedProperty: key0 },
                              message: "must NOT have unevaluated properties",
                            },
                          ];
                          return false;
                          break;
                        }
                      }
                    }
                  }
                }
              } else {
                validate33.errors = [
                  {
                    instancePath: instancePath + "/args",
                    schemaPath: "#/properties/args/type",
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
            if (data.returnType !== undefined) {
              const _errs8 = errors;
              if ("boolean" !== data.returnType) {
                validate33.errors = [
                  {
                    instancePath: instancePath + "/returnType",
                    schemaPath: "#/properties/returnType/const",
                    keyword: "const",
                    params: { allowedValue: "boolean" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs8 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              for (const key1 in data) {
                if (
                  key1 !== "call" &&
                  key1 !== "args" &&
                  key1 !== "returnType"
                ) {
                  validate33.errors = [
                    {
                      instancePath,
                      schemaPath: "#/unevaluatedProperties",
                      keyword: "unevaluatedProperties",
                      params: { unevaluatedProperty: key1 },
                      message: "must NOT have unevaluated properties",
                    },
                  ];
                  return false;
                  break;
                }
              }
            }
          }
        }
      }
    } else {
      validate33.errors = [
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
  validate33.errors = vErrors;
  return errors === 0;
}
validate33.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema48 = {
  type: "object",
  description: "Checks string length constraints.",
  properties: {
    call: { const: "length" },
    args: {
      type: "object",
      properties: {
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
        },
        min: {
          type: "integer",
          minimum: 0,
          description: "The minimum allowed length.",
        },
        max: {
          type: "integer",
          minimum: 0,
          description: "The maximum allowed length.",
        },
      },
      required: ["value"],
      anyOf: [{ required: ["min"] }, { required: ["max"] }],
      unevaluatedProperties: false,
    },
    returnType: { const: "boolean" },
  },
  required: ["call", "args"],
  unevaluatedProperties: false,
};
function validate35(
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
  const evaluated0 = validate35.evaluated;
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
        (data.call === undefined && (missing0 = "call")) ||
        (data.args === undefined && (missing0 = "args"))
      ) {
        validate35.errors = [
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
        if (data.call !== undefined) {
          const _errs1 = errors;
          if ("length" !== data.call) {
            validate35.errors = [
              {
                instancePath: instancePath + "/call",
                schemaPath: "#/properties/call/const",
                keyword: "const",
                params: { allowedValue: "length" },
                message: "must be equal to constant",
              },
            ];
            return false;
          }
          var valid0 = _errs1 === errors;
        } else {
          var valid0 = true;
        }
        if (valid0) {
          if (data.args !== undefined) {
            let data1 = data.args;
            const _errs2 = errors;
            const _errs4 = errors;
            let valid1 = false;
            const _errs5 = errors;
            if (data1 && typeof data1 == "object" && !Array.isArray(data1)) {
              let missing1;
              if (data1.min === undefined && (missing1 = "min")) {
                const err0 = {
                  instancePath: instancePath + "/args",
                  schemaPath: "#/properties/args/anyOf/0/required",
                  keyword: "required",
                  params: { missingProperty: missing1 },
                  message: "must have required property '" + missing1 + "'",
                };
                if (vErrors === null) {
                  vErrors = [err0];
                } else {
                  vErrors.push(err0);
                }
                errors++;
              }
            }
            var _valid0 = _errs5 === errors;
            valid1 = valid1 || _valid0;
            const _errs6 = errors;
            if (data1 && typeof data1 == "object" && !Array.isArray(data1)) {
              let missing2;
              if (data1.max === undefined && (missing2 = "max")) {
                const err1 = {
                  instancePath: instancePath + "/args",
                  schemaPath: "#/properties/args/anyOf/1/required",
                  keyword: "required",
                  params: { missingProperty: missing2 },
                  message: "must have required property '" + missing2 + "'",
                };
                if (vErrors === null) {
                  vErrors = [err1];
                } else {
                  vErrors.push(err1);
                }
                errors++;
              }
            }
            var _valid0 = _errs6 === errors;
            valid1 = valid1 || _valid0;
            if (!valid1) {
              const err2 = {
                instancePath: instancePath + "/args",
                schemaPath: "#/properties/args/anyOf",
                keyword: "anyOf",
                params: {},
                message: "must match a schema in anyOf",
              };
              if (vErrors === null) {
                vErrors = [err2];
              } else {
                vErrors.push(err2);
              }
              errors++;
              validate35.errors = vErrors;
              return false;
            } else {
              errors = _errs4;
              if (vErrors !== null) {
                if (_errs4) {
                  vErrors.length = _errs4;
                } else {
                  vErrors = null;
                }
              }
            }
            if (errors === _errs2) {
              if (data1 && typeof data1 == "object" && !Array.isArray(data1)) {
                let missing3;
                if (data1.value === undefined && (missing3 = "value")) {
                  validate35.errors = [
                    {
                      instancePath: instancePath + "/args",
                      schemaPath: "#/properties/args/required",
                      keyword: "required",
                      params: { missingProperty: missing3 },
                      message: "must have required property '" + missing3 + "'",
                    },
                  ];
                  return false;
                } else {
                  if (data1.value !== undefined) {
                    const _errs7 = errors;
                    if (
                      !wrapper0.validate(data1.value, {
                        instancePath: instancePath + "/args/value",
                        parentData: data1,
                        parentDataProperty: "value",
                        rootData,
                        dynamicAnchors,
                      })
                    ) {
                      vErrors =
                        vErrors === null
                          ? wrapper0.validate.errors
                          : vErrors.concat(wrapper0.validate.errors);
                      errors = vErrors.length;
                    }
                    var valid2 = _errs7 === errors;
                  } else {
                    var valid2 = true;
                  }
                  if (valid2) {
                    if (data1.min !== undefined) {
                      let data3 = data1.min;
                      const _errs8 = errors;
                      if (
                        !(
                          typeof data3 == "number" &&
                          !(data3 % 1) &&
                          !isNaN(data3)
                        )
                      ) {
                        validate35.errors = [
                          {
                            instancePath: instancePath + "/args/min",
                            schemaPath: "#/properties/args/properties/min/type",
                            keyword: "type",
                            params: { type: "integer" },
                            message: "must be integer",
                          },
                        ];
                        return false;
                      }
                      if (errors === _errs8) {
                        if (typeof data3 == "number") {
                          if (data3 < 0 || isNaN(data3)) {
                            validate35.errors = [
                              {
                                instancePath: instancePath + "/args/min",
                                schemaPath:
                                  "#/properties/args/properties/min/minimum",
                                keyword: "minimum",
                                params: { comparison: ">=", limit: 0 },
                                message: "must be >= 0",
                              },
                            ];
                            return false;
                          }
                        }
                      }
                      var valid2 = _errs8 === errors;
                    } else {
                      var valid2 = true;
                    }
                    if (valid2) {
                      if (data1.max !== undefined) {
                        let data4 = data1.max;
                        const _errs10 = errors;
                        if (
                          !(
                            typeof data4 == "number" &&
                            !(data4 % 1) &&
                            !isNaN(data4)
                          )
                        ) {
                          validate35.errors = [
                            {
                              instancePath: instancePath + "/args/max",
                              schemaPath:
                                "#/properties/args/properties/max/type",
                              keyword: "type",
                              params: { type: "integer" },
                              message: "must be integer",
                            },
                          ];
                          return false;
                        }
                        if (errors === _errs10) {
                          if (typeof data4 == "number") {
                            if (data4 < 0 || isNaN(data4)) {
                              validate35.errors = [
                                {
                                  instancePath: instancePath + "/args/max",
                                  schemaPath:
                                    "#/properties/args/properties/max/minimum",
                                  keyword: "minimum",
                                  params: { comparison: ">=", limit: 0 },
                                  message: "must be >= 0",
                                },
                              ];
                              return false;
                            }
                          }
                        }
                        var valid2 = _errs10 === errors;
                      } else {
                        var valid2 = true;
                      }
                      if (valid2) {
                        for (const key0 in data1) {
                          if (
                            key0 !== "value" &&
                            key0 !== "min" &&
                            key0 !== "max"
                          ) {
                            validate35.errors = [
                              {
                                instancePath: instancePath + "/args",
                                schemaPath:
                                  "#/properties/args/unevaluatedProperties",
                                keyword: "unevaluatedProperties",
                                params: { unevaluatedProperty: key0 },
                                message: "must NOT have unevaluated properties",
                              },
                            ];
                            return false;
                            break;
                          }
                        }
                      }
                    }
                  }
                }
              } else {
                validate35.errors = [
                  {
                    instancePath: instancePath + "/args",
                    schemaPath: "#/properties/args/type",
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
            if (data.returnType !== undefined) {
              const _errs13 = errors;
              if ("boolean" !== data.returnType) {
                validate35.errors = [
                  {
                    instancePath: instancePath + "/returnType",
                    schemaPath: "#/properties/returnType/const",
                    keyword: "const",
                    params: { allowedValue: "boolean" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs13 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              for (const key1 in data) {
                if (
                  key1 !== "call" &&
                  key1 !== "args" &&
                  key1 !== "returnType"
                ) {
                  validate35.errors = [
                    {
                      instancePath,
                      schemaPath: "#/unevaluatedProperties",
                      keyword: "unevaluatedProperties",
                      params: { unevaluatedProperty: key1 },
                      message: "must NOT have unevaluated properties",
                    },
                  ];
                  return false;
                  break;
                }
              }
            }
          }
        }
      }
    } else {
      validate35.errors = [
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
  validate35.errors = vErrors;
  return errors === 0;
}
validate35.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema49 = {
  type: "object",
  description: "Checks numeric range constraints.",
  properties: {
    call: { const: "numeric" },
    args: {
      type: "object",
      properties: {
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicNumber",
        },
        min: { type: "number", description: "The minimum allowed value." },
        max: { type: "number", description: "The maximum allowed value." },
      },
      required: ["value"],
      anyOf: [{ required: ["min"] }, { required: ["max"] }],
      unevaluatedProperties: false,
    },
    returnType: { const: "boolean" },
  },
  required: ["call", "args"],
  unevaluatedProperties: false,
};
const schema50 = {
  description:
    "Represents a value that can be either a literal number, a path to a number in the data model, or a function call returning a number.",
  oneOf: [
    { type: "number" },
    { $ref: "#/$defs/DataBinding" },
    {
      allOf: [
        { $ref: "#/$defs/FunctionCall" },
        { properties: { returnType: { const: "number" } } },
      ],
    },
  ],
};
const wrapper2 = { validate: validate31 };
function validate38(
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
  const evaluated0 = validate38.evaluated;
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
  if (!(typeof data == "number")) {
    const err0 = {
      instancePath,
      schemaPath: "#/oneOf/0/type",
      keyword: "type",
      params: { type: "number" },
      message: "must be number",
    };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  const _errs4 = errors;
  if (errors === _errs4) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.path === undefined && (missing0 = "path")) {
        const err1 = {
          instancePath,
          schemaPath: "#/$defs/DataBinding/required",
          keyword: "required",
          params: { missingProperty: missing0 },
          message: "must have required property '" + missing0 + "'",
        };
        if (vErrors === null) {
          vErrors = [err1];
        } else {
          vErrors.push(err1);
        }
        errors++;
      } else {
        const _errs6 = errors;
        for (const key0 in data) {
          if (!(key0 === "path")) {
            const err2 = {
              instancePath,
              schemaPath: "#/$defs/DataBinding/additionalProperties",
              keyword: "additionalProperties",
              params: { additionalProperty: key0 },
              message: "must NOT have additional properties",
            };
            if (vErrors === null) {
              vErrors = [err2];
            } else {
              vErrors.push(err2);
            }
            errors++;
            break;
          }
        }
        if (_errs6 === errors) {
          if (data.path !== undefined) {
            if (typeof data.path !== "string") {
              const err3 = {
                instancePath: instancePath + "/path",
                schemaPath: "#/$defs/DataBinding/properties/path/type",
                keyword: "type",
                params: { type: "string" },
                message: "must be string",
              };
              if (vErrors === null) {
                vErrors = [err3];
              } else {
                vErrors.push(err3);
              }
              errors++;
            }
          }
        }
      }
    } else {
      const err4 = {
        instancePath,
        schemaPath: "#/$defs/DataBinding/type",
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
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      var props0 = true;
    }
    const _errs9 = errors;
    const _errs10 = errors;
    if (
      !wrapper2.validate(data, {
        instancePath,
        parentData,
        parentDataProperty,
        rootData,
        dynamicAnchors,
      })
    ) {
      vErrors =
        vErrors === null
          ? wrapper2.validate.errors
          : vErrors.concat(wrapper2.validate.errors);
      errors = vErrors.length;
    } else {
      var props1 = wrapper2.validate.evaluated.props;
      var items0 = wrapper2.validate.evaluated.items;
    }
    var valid3 = _errs10 === errors;
    if (valid3) {
      const _errs11 = errors;
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.returnType !== undefined) {
          if ("number" !== data.returnType) {
            const err5 = {
              instancePath: instancePath + "/returnType",
              schemaPath: "#/oneOf/2/allOf/1/properties/returnType/const",
              keyword: "const",
              params: { allowedValue: "number" },
              message: "must be equal to constant",
            };
            if (vErrors === null) {
              vErrors = [err5];
            } else {
              vErrors.push(err5);
            }
            errors++;
          }
        }
      }
      var valid3 = _errs11 === errors;
      if (valid3) {
        if (props1 !== true) {
          props1 = props1 || {};
          props1.returnType = true;
        }
      }
    }
    var _valid0 = _errs9 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
        if (props0 !== true && props1 !== undefined) {
          if (props1 === true) {
            props0 = true;
          } else {
            props0 = props0 || {};
            Object.assign(props0, props1);
          }
        }
      }
    }
  }
  if (!valid0) {
    const err6 = {
      instancePath,
      schemaPath: "#/oneOf",
      keyword: "oneOf",
      params: { passingSchemas: passing0 },
      message: "must match exactly one schema in oneOf",
    };
    if (vErrors === null) {
      vErrors = [err6];
    } else {
      vErrors.push(err6);
    }
    errors++;
    validate38.errors = vErrors;
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
  validate38.errors = vErrors;
  evaluated0.props = props0;
  evaluated0.items = items0;
  return errors === 0;
}
validate38.evaluated = { dynamicProps: true, dynamicItems: true };
function validate37(
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
  const evaluated0 = validate37.evaluated;
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
        (data.call === undefined && (missing0 = "call")) ||
        (data.args === undefined && (missing0 = "args"))
      ) {
        validate37.errors = [
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
        if (data.call !== undefined) {
          const _errs1 = errors;
          if ("numeric" !== data.call) {
            validate37.errors = [
              {
                instancePath: instancePath + "/call",
                schemaPath: "#/properties/call/const",
                keyword: "const",
                params: { allowedValue: "numeric" },
                message: "must be equal to constant",
              },
            ];
            return false;
          }
          var valid0 = _errs1 === errors;
        } else {
          var valid0 = true;
        }
        if (valid0) {
          if (data.args !== undefined) {
            let data1 = data.args;
            const _errs2 = errors;
            const _errs4 = errors;
            let valid1 = false;
            const _errs5 = errors;
            if (data1 && typeof data1 == "object" && !Array.isArray(data1)) {
              let missing1;
              if (data1.min === undefined && (missing1 = "min")) {
                const err0 = {
                  instancePath: instancePath + "/args",
                  schemaPath: "#/properties/args/anyOf/0/required",
                  keyword: "required",
                  params: { missingProperty: missing1 },
                  message: "must have required property '" + missing1 + "'",
                };
                if (vErrors === null) {
                  vErrors = [err0];
                } else {
                  vErrors.push(err0);
                }
                errors++;
              }
            }
            var _valid0 = _errs5 === errors;
            valid1 = valid1 || _valid0;
            const _errs6 = errors;
            if (data1 && typeof data1 == "object" && !Array.isArray(data1)) {
              let missing2;
              if (data1.max === undefined && (missing2 = "max")) {
                const err1 = {
                  instancePath: instancePath + "/args",
                  schemaPath: "#/properties/args/anyOf/1/required",
                  keyword: "required",
                  params: { missingProperty: missing2 },
                  message: "must have required property '" + missing2 + "'",
                };
                if (vErrors === null) {
                  vErrors = [err1];
                } else {
                  vErrors.push(err1);
                }
                errors++;
              }
            }
            var _valid0 = _errs6 === errors;
            valid1 = valid1 || _valid0;
            if (!valid1) {
              const err2 = {
                instancePath: instancePath + "/args",
                schemaPath: "#/properties/args/anyOf",
                keyword: "anyOf",
                params: {},
                message: "must match a schema in anyOf",
              };
              if (vErrors === null) {
                vErrors = [err2];
              } else {
                vErrors.push(err2);
              }
              errors++;
              validate37.errors = vErrors;
              return false;
            } else {
              errors = _errs4;
              if (vErrors !== null) {
                if (_errs4) {
                  vErrors.length = _errs4;
                } else {
                  vErrors = null;
                }
              }
            }
            if (errors === _errs2) {
              if (data1 && typeof data1 == "object" && !Array.isArray(data1)) {
                let missing3;
                if (data1.value === undefined && (missing3 = "value")) {
                  validate37.errors = [
                    {
                      instancePath: instancePath + "/args",
                      schemaPath: "#/properties/args/required",
                      keyword: "required",
                      params: { missingProperty: missing3 },
                      message: "must have required property '" + missing3 + "'",
                    },
                  ];
                  return false;
                } else {
                  if (data1.value !== undefined) {
                    const _errs7 = errors;
                    if (
                      !validate38(data1.value, {
                        instancePath: instancePath + "/args/value",
                        parentData: data1,
                        parentDataProperty: "value",
                        rootData,
                        dynamicAnchors,
                      })
                    ) {
                      vErrors =
                        vErrors === null
                          ? validate38.errors
                          : vErrors.concat(validate38.errors);
                      errors = vErrors.length;
                    }
                    var valid2 = _errs7 === errors;
                  } else {
                    var valid2 = true;
                  }
                  if (valid2) {
                    if (data1.min !== undefined) {
                      const _errs8 = errors;
                      if (!(typeof data1.min == "number")) {
                        validate37.errors = [
                          {
                            instancePath: instancePath + "/args/min",
                            schemaPath: "#/properties/args/properties/min/type",
                            keyword: "type",
                            params: { type: "number" },
                            message: "must be number",
                          },
                        ];
                        return false;
                      }
                      var valid2 = _errs8 === errors;
                    } else {
                      var valid2 = true;
                    }
                    if (valid2) {
                      if (data1.max !== undefined) {
                        const _errs10 = errors;
                        if (!(typeof data1.max == "number")) {
                          validate37.errors = [
                            {
                              instancePath: instancePath + "/args/max",
                              schemaPath:
                                "#/properties/args/properties/max/type",
                              keyword: "type",
                              params: { type: "number" },
                              message: "must be number",
                            },
                          ];
                          return false;
                        }
                        var valid2 = _errs10 === errors;
                      } else {
                        var valid2 = true;
                      }
                      if (valid2) {
                        for (const key0 in data1) {
                          if (
                            key0 !== "value" &&
                            key0 !== "min" &&
                            key0 !== "max"
                          ) {
                            validate37.errors = [
                              {
                                instancePath: instancePath + "/args",
                                schemaPath:
                                  "#/properties/args/unevaluatedProperties",
                                keyword: "unevaluatedProperties",
                                params: { unevaluatedProperty: key0 },
                                message: "must NOT have unevaluated properties",
                              },
                            ];
                            return false;
                            break;
                          }
                        }
                      }
                    }
                  }
                }
              } else {
                validate37.errors = [
                  {
                    instancePath: instancePath + "/args",
                    schemaPath: "#/properties/args/type",
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
            if (data.returnType !== undefined) {
              const _errs13 = errors;
              if ("boolean" !== data.returnType) {
                validate37.errors = [
                  {
                    instancePath: instancePath + "/returnType",
                    schemaPath: "#/properties/returnType/const",
                    keyword: "const",
                    params: { allowedValue: "boolean" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs13 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              for (const key1 in data) {
                if (
                  key1 !== "call" &&
                  key1 !== "args" &&
                  key1 !== "returnType"
                ) {
                  validate37.errors = [
                    {
                      instancePath,
                      schemaPath: "#/unevaluatedProperties",
                      keyword: "unevaluatedProperties",
                      params: { unevaluatedProperty: key1 },
                      message: "must NOT have unevaluated properties",
                    },
                  ];
                  return false;
                  break;
                }
              }
            }
          }
        }
      }
    } else {
      validate37.errors = [
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
  validate37.errors = vErrors;
  return errors === 0;
}
validate37.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema52 = {
  type: "object",
  description: "Checks that the value is a valid email address.",
  properties: {
    call: { const: "email" },
    args: {
      type: "object",
      properties: {
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
        },
      },
      required: ["value"],
      unevaluatedProperties: false,
    },
    returnType: { const: "boolean" },
  },
  required: ["call", "args"],
  unevaluatedProperties: false,
};
function validate41(
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
  const evaluated0 = validate41.evaluated;
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
        (data.call === undefined && (missing0 = "call")) ||
        (data.args === undefined && (missing0 = "args"))
      ) {
        validate41.errors = [
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
        if (data.call !== undefined) {
          const _errs1 = errors;
          if ("email" !== data.call) {
            validate41.errors = [
              {
                instancePath: instancePath + "/call",
                schemaPath: "#/properties/call/const",
                keyword: "const",
                params: { allowedValue: "email" },
                message: "must be equal to constant",
              },
            ];
            return false;
          }
          var valid0 = _errs1 === errors;
        } else {
          var valid0 = true;
        }
        if (valid0) {
          if (data.args !== undefined) {
            let data1 = data.args;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (data1 && typeof data1 == "object" && !Array.isArray(data1)) {
                let missing1;
                if (data1.value === undefined && (missing1 = "value")) {
                  validate41.errors = [
                    {
                      instancePath: instancePath + "/args",
                      schemaPath: "#/properties/args/required",
                      keyword: "required",
                      params: { missingProperty: missing1 },
                      message: "must have required property '" + missing1 + "'",
                    },
                  ];
                  return false;
                } else {
                  if (data1.value !== undefined) {
                    const _errs4 = errors;
                    if (
                      !wrapper0.validate(data1.value, {
                        instancePath: instancePath + "/args/value",
                        parentData: data1,
                        parentDataProperty: "value",
                        rootData,
                        dynamicAnchors,
                      })
                    ) {
                      vErrors =
                        vErrors === null
                          ? wrapper0.validate.errors
                          : vErrors.concat(wrapper0.validate.errors);
                      errors = vErrors.length;
                    }
                    var valid1 = _errs4 === errors;
                  } else {
                    var valid1 = true;
                  }
                  if (valid1) {
                    for (const key0 in data1) {
                      if (key0 !== "value") {
                        validate41.errors = [
                          {
                            instancePath: instancePath + "/args",
                            schemaPath:
                              "#/properties/args/unevaluatedProperties",
                            keyword: "unevaluatedProperties",
                            params: { unevaluatedProperty: key0 },
                            message: "must NOT have unevaluated properties",
                          },
                        ];
                        return false;
                        break;
                      }
                    }
                  }
                }
              } else {
                validate41.errors = [
                  {
                    instancePath: instancePath + "/args",
                    schemaPath: "#/properties/args/type",
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
            if (data.returnType !== undefined) {
              const _errs6 = errors;
              if ("boolean" !== data.returnType) {
                validate41.errors = [
                  {
                    instancePath: instancePath + "/returnType",
                    schemaPath: "#/properties/returnType/const",
                    keyword: "const",
                    params: { allowedValue: "boolean" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs6 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              for (const key1 in data) {
                if (
                  key1 !== "call" &&
                  key1 !== "args" &&
                  key1 !== "returnType"
                ) {
                  validate41.errors = [
                    {
                      instancePath,
                      schemaPath: "#/unevaluatedProperties",
                      keyword: "unevaluatedProperties",
                      params: { unevaluatedProperty: key1 },
                      message: "must NOT have unevaluated properties",
                    },
                  ];
                  return false;
                  break;
                }
              }
            }
          }
        }
      }
    } else {
      validate41.errors = [
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
  validate41.errors = vErrors;
  return errors === 0;
}
validate41.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema53 = {
  type: "object",
  description:
    "Performs string interpolation of data model values and other functions in the catalog functions list and returns the resulting string. The value string can contain interpolated expressions in the `${expression}` format. Supported expression types include: JSON Pointer paths to the data model (e.g., `${/absolute/path}` or `${relative/path}`), and client-side function calls (e.g., `${now()}`). Function arguments must be named (e.g., `${formatDate(value:${/currentDate}, format:'MM-dd')}`). To include a literal `${` sequence, escape it as `\\${`.",
  properties: {
    call: { const: "formatString" },
    args: {
      type: "object",
      properties: {
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
        },
      },
      required: ["value"],
      unevaluatedProperties: false,
    },
    returnType: { const: "string" },
  },
  required: ["call", "args"],
  unevaluatedProperties: false,
};
function validate43(
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
  const evaluated0 = validate43.evaluated;
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
        (data.call === undefined && (missing0 = "call")) ||
        (data.args === undefined && (missing0 = "args"))
      ) {
        validate43.errors = [
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
        if (data.call !== undefined) {
          const _errs1 = errors;
          if ("formatString" !== data.call) {
            validate43.errors = [
              {
                instancePath: instancePath + "/call",
                schemaPath: "#/properties/call/const",
                keyword: "const",
                params: { allowedValue: "formatString" },
                message: "must be equal to constant",
              },
            ];
            return false;
          }
          var valid0 = _errs1 === errors;
        } else {
          var valid0 = true;
        }
        if (valid0) {
          if (data.args !== undefined) {
            let data1 = data.args;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (data1 && typeof data1 == "object" && !Array.isArray(data1)) {
                let missing1;
                if (data1.value === undefined && (missing1 = "value")) {
                  validate43.errors = [
                    {
                      instancePath: instancePath + "/args",
                      schemaPath: "#/properties/args/required",
                      keyword: "required",
                      params: { missingProperty: missing1 },
                      message: "must have required property '" + missing1 + "'",
                    },
                  ];
                  return false;
                } else {
                  if (data1.value !== undefined) {
                    const _errs4 = errors;
                    if (
                      !wrapper0.validate(data1.value, {
                        instancePath: instancePath + "/args/value",
                        parentData: data1,
                        parentDataProperty: "value",
                        rootData,
                        dynamicAnchors,
                      })
                    ) {
                      vErrors =
                        vErrors === null
                          ? wrapper0.validate.errors
                          : vErrors.concat(wrapper0.validate.errors);
                      errors = vErrors.length;
                    }
                    var valid1 = _errs4 === errors;
                  } else {
                    var valid1 = true;
                  }
                  if (valid1) {
                    for (const key0 in data1) {
                      if (key0 !== "value") {
                        validate43.errors = [
                          {
                            instancePath: instancePath + "/args",
                            schemaPath:
                              "#/properties/args/unevaluatedProperties",
                            keyword: "unevaluatedProperties",
                            params: { unevaluatedProperty: key0 },
                            message: "must NOT have unevaluated properties",
                          },
                        ];
                        return false;
                        break;
                      }
                    }
                  }
                }
              } else {
                validate43.errors = [
                  {
                    instancePath: instancePath + "/args",
                    schemaPath: "#/properties/args/type",
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
            if (data.returnType !== undefined) {
              const _errs6 = errors;
              if ("string" !== data.returnType) {
                validate43.errors = [
                  {
                    instancePath: instancePath + "/returnType",
                    schemaPath: "#/properties/returnType/const",
                    keyword: "const",
                    params: { allowedValue: "string" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs6 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              for (const key1 in data) {
                if (
                  key1 !== "call" &&
                  key1 !== "args" &&
                  key1 !== "returnType"
                ) {
                  validate43.errors = [
                    {
                      instancePath,
                      schemaPath: "#/unevaluatedProperties",
                      keyword: "unevaluatedProperties",
                      params: { unevaluatedProperty: key1 },
                      message: "must NOT have unevaluated properties",
                    },
                  ];
                  return false;
                  break;
                }
              }
            }
          }
        }
      }
    } else {
      validate43.errors = [
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
  validate43.errors = vErrors;
  return errors === 0;
}
validate43.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema54 = {
  type: "object",
  description:
    "Formats a number with the specified grouping and decimal precision.",
  properties: {
    call: { const: "formatNumber" },
    args: {
      type: "object",
      properties: {
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicNumber",
          description: "The number to format.",
        },
        decimals: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicNumber",
          description:
            "Optional. The number of decimal places to show. Defaults to 0 or 2 depending on locale.",
        },
        grouping: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicBoolean",
          description:
            "Optional. If true, uses locale-specific grouping separators (e.g. '1,000'). If false, returns raw digits (e.g. '1000'). Defaults to true.",
        },
      },
      required: ["value"],
      unevaluatedProperties: false,
    },
    returnType: { const: "string" },
  },
  required: ["call", "args"],
  unevaluatedProperties: false,
};
const schema55 = {
  description:
    "A boolean value that can be a literal, a path, or a function call returning a boolean.",
  oneOf: [
    { type: "boolean" },
    { $ref: "#/$defs/DataBinding" },
    {
      allOf: [
        { $ref: "#/$defs/FunctionCall" },
        { properties: { returnType: { const: "boolean" } } },
      ],
    },
  ],
};
function validate48(
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
  const evaluated0 = validate48.evaluated;
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
  if (typeof data !== "boolean") {
    const err0 = {
      instancePath,
      schemaPath: "#/oneOf/0/type",
      keyword: "type",
      params: { type: "boolean" },
      message: "must be boolean",
    };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  const _errs4 = errors;
  if (errors === _errs4) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.path === undefined && (missing0 = "path")) {
        const err1 = {
          instancePath,
          schemaPath: "#/$defs/DataBinding/required",
          keyword: "required",
          params: { missingProperty: missing0 },
          message: "must have required property '" + missing0 + "'",
        };
        if (vErrors === null) {
          vErrors = [err1];
        } else {
          vErrors.push(err1);
        }
        errors++;
      } else {
        const _errs6 = errors;
        for (const key0 in data) {
          if (!(key0 === "path")) {
            const err2 = {
              instancePath,
              schemaPath: "#/$defs/DataBinding/additionalProperties",
              keyword: "additionalProperties",
              params: { additionalProperty: key0 },
              message: "must NOT have additional properties",
            };
            if (vErrors === null) {
              vErrors = [err2];
            } else {
              vErrors.push(err2);
            }
            errors++;
            break;
          }
        }
        if (_errs6 === errors) {
          if (data.path !== undefined) {
            if (typeof data.path !== "string") {
              const err3 = {
                instancePath: instancePath + "/path",
                schemaPath: "#/$defs/DataBinding/properties/path/type",
                keyword: "type",
                params: { type: "string" },
                message: "must be string",
              };
              if (vErrors === null) {
                vErrors = [err3];
              } else {
                vErrors.push(err3);
              }
              errors++;
            }
          }
        }
      }
    } else {
      const err4 = {
        instancePath,
        schemaPath: "#/$defs/DataBinding/type",
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
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      var props0 = true;
    }
    const _errs9 = errors;
    const _errs10 = errors;
    if (
      !wrapper2.validate(data, {
        instancePath,
        parentData,
        parentDataProperty,
        rootData,
        dynamicAnchors,
      })
    ) {
      vErrors =
        vErrors === null
          ? wrapper2.validate.errors
          : vErrors.concat(wrapper2.validate.errors);
      errors = vErrors.length;
    } else {
      var props1 = wrapper2.validate.evaluated.props;
      var items0 = wrapper2.validate.evaluated.items;
    }
    var valid3 = _errs10 === errors;
    if (valid3) {
      const _errs11 = errors;
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.returnType !== undefined) {
          if ("boolean" !== data.returnType) {
            const err5 = {
              instancePath: instancePath + "/returnType",
              schemaPath: "#/oneOf/2/allOf/1/properties/returnType/const",
              keyword: "const",
              params: { allowedValue: "boolean" },
              message: "must be equal to constant",
            };
            if (vErrors === null) {
              vErrors = [err5];
            } else {
              vErrors.push(err5);
            }
            errors++;
          }
        }
      }
      var valid3 = _errs11 === errors;
      if (valid3) {
        if (props1 !== true) {
          props1 = props1 || {};
          props1.returnType = true;
        }
      }
    }
    var _valid0 = _errs9 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
        if (props0 !== true && props1 !== undefined) {
          if (props1 === true) {
            props0 = true;
          } else {
            props0 = props0 || {};
            Object.assign(props0, props1);
          }
        }
      }
    }
  }
  if (!valid0) {
    const err6 = {
      instancePath,
      schemaPath: "#/oneOf",
      keyword: "oneOf",
      params: { passingSchemas: passing0 },
      message: "must match exactly one schema in oneOf",
    };
    if (vErrors === null) {
      vErrors = [err6];
    } else {
      vErrors.push(err6);
    }
    errors++;
    validate48.errors = vErrors;
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
  validate48.errors = vErrors;
  evaluated0.props = props0;
  evaluated0.items = items0;
  return errors === 0;
}
validate48.evaluated = { dynamicProps: true, dynamicItems: true };
function validate45(
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
  const evaluated0 = validate45.evaluated;
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
        (data.call === undefined && (missing0 = "call")) ||
        (data.args === undefined && (missing0 = "args"))
      ) {
        validate45.errors = [
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
        if (data.call !== undefined) {
          const _errs1 = errors;
          if ("formatNumber" !== data.call) {
            validate45.errors = [
              {
                instancePath: instancePath + "/call",
                schemaPath: "#/properties/call/const",
                keyword: "const",
                params: { allowedValue: "formatNumber" },
                message: "must be equal to constant",
              },
            ];
            return false;
          }
          var valid0 = _errs1 === errors;
        } else {
          var valid0 = true;
        }
        if (valid0) {
          if (data.args !== undefined) {
            let data1 = data.args;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (data1 && typeof data1 == "object" && !Array.isArray(data1)) {
                let missing1;
                if (data1.value === undefined && (missing1 = "value")) {
                  validate45.errors = [
                    {
                      instancePath: instancePath + "/args",
                      schemaPath: "#/properties/args/required",
                      keyword: "required",
                      params: { missingProperty: missing1 },
                      message: "must have required property '" + missing1 + "'",
                    },
                  ];
                  return false;
                } else {
                  if (data1.value !== undefined) {
                    const _errs4 = errors;
                    if (
                      !validate38(data1.value, {
                        instancePath: instancePath + "/args/value",
                        parentData: data1,
                        parentDataProperty: "value",
                        rootData,
                        dynamicAnchors,
                      })
                    ) {
                      vErrors =
                        vErrors === null
                          ? validate38.errors
                          : vErrors.concat(validate38.errors);
                      errors = vErrors.length;
                    }
                    var valid1 = _errs4 === errors;
                  } else {
                    var valid1 = true;
                  }
                  if (valid1) {
                    if (data1.decimals !== undefined) {
                      const _errs5 = errors;
                      if (
                        !validate38(data1.decimals, {
                          instancePath: instancePath + "/args/decimals",
                          parentData: data1,
                          parentDataProperty: "decimals",
                          rootData,
                          dynamicAnchors,
                        })
                      ) {
                        vErrors =
                          vErrors === null
                            ? validate38.errors
                            : vErrors.concat(validate38.errors);
                        errors = vErrors.length;
                      }
                      var valid1 = _errs5 === errors;
                    } else {
                      var valid1 = true;
                    }
                    if (valid1) {
                      if (data1.grouping !== undefined) {
                        const _errs6 = errors;
                        if (
                          !validate48(data1.grouping, {
                            instancePath: instancePath + "/args/grouping",
                            parentData: data1,
                            parentDataProperty: "grouping",
                            rootData,
                            dynamicAnchors,
                          })
                        ) {
                          vErrors =
                            vErrors === null
                              ? validate48.errors
                              : vErrors.concat(validate48.errors);
                          errors = vErrors.length;
                        }
                        var valid1 = _errs6 === errors;
                      } else {
                        var valid1 = true;
                      }
                      if (valid1) {
                        for (const key0 in data1) {
                          if (
                            key0 !== "value" &&
                            key0 !== "decimals" &&
                            key0 !== "grouping"
                          ) {
                            validate45.errors = [
                              {
                                instancePath: instancePath + "/args",
                                schemaPath:
                                  "#/properties/args/unevaluatedProperties",
                                keyword: "unevaluatedProperties",
                                params: { unevaluatedProperty: key0 },
                                message: "must NOT have unevaluated properties",
                              },
                            ];
                            return false;
                            break;
                          }
                        }
                      }
                    }
                  }
                }
              } else {
                validate45.errors = [
                  {
                    instancePath: instancePath + "/args",
                    schemaPath: "#/properties/args/type",
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
            if (data.returnType !== undefined) {
              const _errs8 = errors;
              if ("string" !== data.returnType) {
                validate45.errors = [
                  {
                    instancePath: instancePath + "/returnType",
                    schemaPath: "#/properties/returnType/const",
                    keyword: "const",
                    params: { allowedValue: "string" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs8 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              for (const key1 in data) {
                if (
                  key1 !== "call" &&
                  key1 !== "args" &&
                  key1 !== "returnType"
                ) {
                  validate45.errors = [
                    {
                      instancePath,
                      schemaPath: "#/unevaluatedProperties",
                      keyword: "unevaluatedProperties",
                      params: { unevaluatedProperty: key1 },
                      message: "must NOT have unevaluated properties",
                    },
                  ];
                  return false;
                  break;
                }
              }
            }
          }
        }
      }
    } else {
      validate45.errors = [
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
  validate45.errors = vErrors;
  return errors === 0;
}
validate45.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema57 = {
  type: "object",
  description: "Formats a number as a currency string.",
  properties: {
    call: { const: "formatCurrency" },
    args: {
      type: "object",
      properties: {
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicNumber",
          description: "The monetary amount.",
        },
        currency: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
          description: "The ISO 4217 currency code (e.g., 'USD', 'EUR').",
        },
        decimals: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicNumber",
          description:
            "Optional. The number of decimal places to show. Defaults to 0 or 2 depending on locale.",
        },
        grouping: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicBoolean",
          description:
            "Optional. If true, uses locale-specific grouping separators (e.g. '1,000'). If false, returns raw digits (e.g. '1000'). Defaults to true.",
        },
      },
      required: ["currency", "value"],
      unevaluatedProperties: false,
    },
    returnType: { const: "string" },
  },
  required: ["call", "args"],
  unevaluatedProperties: false,
};
function validate51(
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
  const evaluated0 = validate51.evaluated;
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
        (data.call === undefined && (missing0 = "call")) ||
        (data.args === undefined && (missing0 = "args"))
      ) {
        validate51.errors = [
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
        if (data.call !== undefined) {
          const _errs1 = errors;
          if ("formatCurrency" !== data.call) {
            validate51.errors = [
              {
                instancePath: instancePath + "/call",
                schemaPath: "#/properties/call/const",
                keyword: "const",
                params: { allowedValue: "formatCurrency" },
                message: "must be equal to constant",
              },
            ];
            return false;
          }
          var valid0 = _errs1 === errors;
        } else {
          var valid0 = true;
        }
        if (valid0) {
          if (data.args !== undefined) {
            let data1 = data.args;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (data1 && typeof data1 == "object" && !Array.isArray(data1)) {
                let missing1;
                if (
                  (data1.currency === undefined && (missing1 = "currency")) ||
                  (data1.value === undefined && (missing1 = "value"))
                ) {
                  validate51.errors = [
                    {
                      instancePath: instancePath + "/args",
                      schemaPath: "#/properties/args/required",
                      keyword: "required",
                      params: { missingProperty: missing1 },
                      message: "must have required property '" + missing1 + "'",
                    },
                  ];
                  return false;
                } else {
                  if (data1.value !== undefined) {
                    const _errs4 = errors;
                    if (
                      !validate38(data1.value, {
                        instancePath: instancePath + "/args/value",
                        parentData: data1,
                        parentDataProperty: "value",
                        rootData,
                        dynamicAnchors,
                      })
                    ) {
                      vErrors =
                        vErrors === null
                          ? validate38.errors
                          : vErrors.concat(validate38.errors);
                      errors = vErrors.length;
                    }
                    var valid1 = _errs4 === errors;
                  } else {
                    var valid1 = true;
                  }
                  if (valid1) {
                    if (data1.currency !== undefined) {
                      const _errs5 = errors;
                      if (
                        !wrapper0.validate(data1.currency, {
                          instancePath: instancePath + "/args/currency",
                          parentData: data1,
                          parentDataProperty: "currency",
                          rootData,
                          dynamicAnchors,
                        })
                      ) {
                        vErrors =
                          vErrors === null
                            ? wrapper0.validate.errors
                            : vErrors.concat(wrapper0.validate.errors);
                        errors = vErrors.length;
                      }
                      var valid1 = _errs5 === errors;
                    } else {
                      var valid1 = true;
                    }
                    if (valid1) {
                      if (data1.decimals !== undefined) {
                        const _errs6 = errors;
                        if (
                          !validate38(data1.decimals, {
                            instancePath: instancePath + "/args/decimals",
                            parentData: data1,
                            parentDataProperty: "decimals",
                            rootData,
                            dynamicAnchors,
                          })
                        ) {
                          vErrors =
                            vErrors === null
                              ? validate38.errors
                              : vErrors.concat(validate38.errors);
                          errors = vErrors.length;
                        }
                        var valid1 = _errs6 === errors;
                      } else {
                        var valid1 = true;
                      }
                      if (valid1) {
                        if (data1.grouping !== undefined) {
                          const _errs7 = errors;
                          if (
                            !validate48(data1.grouping, {
                              instancePath: instancePath + "/args/grouping",
                              parentData: data1,
                              parentDataProperty: "grouping",
                              rootData,
                              dynamicAnchors,
                            })
                          ) {
                            vErrors =
                              vErrors === null
                                ? validate48.errors
                                : vErrors.concat(validate48.errors);
                            errors = vErrors.length;
                          }
                          var valid1 = _errs7 === errors;
                        } else {
                          var valid1 = true;
                        }
                        if (valid1) {
                          for (const key0 in data1) {
                            if (
                              key0 !== "value" &&
                              key0 !== "currency" &&
                              key0 !== "decimals" &&
                              key0 !== "grouping"
                            ) {
                              validate51.errors = [
                                {
                                  instancePath: instancePath + "/args",
                                  schemaPath:
                                    "#/properties/args/unevaluatedProperties",
                                  keyword: "unevaluatedProperties",
                                  params: { unevaluatedProperty: key0 },
                                  message:
                                    "must NOT have unevaluated properties",
                                },
                              ];
                              return false;
                              break;
                            }
                          }
                        }
                      }
                    }
                  }
                }
              } else {
                validate51.errors = [
                  {
                    instancePath: instancePath + "/args",
                    schemaPath: "#/properties/args/type",
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
            if (data.returnType !== undefined) {
              const _errs9 = errors;
              if ("string" !== data.returnType) {
                validate51.errors = [
                  {
                    instancePath: instancePath + "/returnType",
                    schemaPath: "#/properties/returnType/const",
                    keyword: "const",
                    params: { allowedValue: "string" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs9 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              for (const key1 in data) {
                if (
                  key1 !== "call" &&
                  key1 !== "args" &&
                  key1 !== "returnType"
                ) {
                  validate51.errors = [
                    {
                      instancePath,
                      schemaPath: "#/unevaluatedProperties",
                      keyword: "unevaluatedProperties",
                      params: { unevaluatedProperty: key1 },
                      message: "must NOT have unevaluated properties",
                    },
                  ];
                  return false;
                  break;
                }
              }
            }
          }
        }
      }
    } else {
      validate51.errors = [
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
  validate51.errors = vErrors;
  return errors === 0;
}
validate51.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema58 = {
  type: "object",
  description: "Formats a timestamp into a string using a pattern.",
  properties: {
    call: { const: "formatDate" },
    args: {
      type: "object",
      properties: {
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
          description: "The date to format.",
        },
        format: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
          description:
            "A Unicode TR35 date pattern string.\n\nToken Reference:\n- Year: 'yy' (26), 'yyyy' (2026)\n- Month: 'M' (1), 'MM' (01), 'MMM' (Jan), 'MMMM' (January)\n- Day: 'd' (1), 'dd' (01), 'E' (Tue), 'EEEE' (Tuesday)\n- Hour (12h): 'h' (1-12), 'hh' (01-12) - requires 'a' for AM/PM\n- Hour (24h): 'H' (0-23), 'HH' (00-23) - Military Time\n- Minute: 'mm' (00-59)\n- Second: 'ss' (00-59)\n- Period: 'a' (AM/PM)\n\nExamples:\n- 'MMM dd, yyyy' -> 'Jan 16, 2026'\n- 'HH:mm' -> '14:30' (Military)\n- 'h:mm a' -> '2:30 PM'\n- 'EEEE, d MMMM' -> 'Friday, 16 January'",
        },
      },
      required: ["format", "value"],
      unevaluatedProperties: false,
    },
    returnType: { const: "string" },
  },
  required: ["call", "args"],
  unevaluatedProperties: false,
};
const schema59 = {
  description:
    "A value that can be a literal, a path, or a function call returning any type.",
  oneOf: [
    { type: "string" },
    { type: "number" },
    { type: "boolean" },
    { type: "array" },
    { $ref: "#/$defs/DataBinding" },
    { $ref: "#/$defs/FunctionCall" },
  ],
};
function validate57(
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
  const evaluated0 = validate57.evaluated;
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
  if (typeof data !== "string") {
    const err0 = {
      instancePath,
      schemaPath: "#/oneOf/0/type",
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
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (!(typeof data == "number")) {
    const err1 = {
      instancePath,
      schemaPath: "#/oneOf/1/type",
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
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "boolean") {
      const err2 = {
        instancePath,
        schemaPath: "#/oneOf/2/type",
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
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
      const _errs7 = errors;
      if (!Array.isArray(data)) {
        const err3 = {
          instancePath,
          schemaPath: "#/oneOf/3/type",
          keyword: "type",
          params: { type: "array" },
          message: "must be array",
        };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
      var _valid0 = _errs7 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
        }
        const _errs9 = errors;
        const _errs10 = errors;
        if (errors === _errs10) {
          if (data && typeof data == "object" && !Array.isArray(data)) {
            let missing0;
            if (data.path === undefined && (missing0 = "path")) {
              const err4 = {
                instancePath,
                schemaPath: "#/$defs/DataBinding/required",
                keyword: "required",
                params: { missingProperty: missing0 },
                message: "must have required property '" + missing0 + "'",
              };
              if (vErrors === null) {
                vErrors = [err4];
              } else {
                vErrors.push(err4);
              }
              errors++;
            } else {
              const _errs12 = errors;
              for (const key0 in data) {
                if (!(key0 === "path")) {
                  const err5 = {
                    instancePath,
                    schemaPath: "#/$defs/DataBinding/additionalProperties",
                    keyword: "additionalProperties",
                    params: { additionalProperty: key0 },
                    message: "must NOT have additional properties",
                  };
                  if (vErrors === null) {
                    vErrors = [err5];
                  } else {
                    vErrors.push(err5);
                  }
                  errors++;
                  break;
                }
              }
              if (_errs12 === errors) {
                if (data.path !== undefined) {
                  if (typeof data.path !== "string") {
                    const err6 = {
                      instancePath: instancePath + "/path",
                      schemaPath: "#/$defs/DataBinding/properties/path/type",
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
                }
              }
            }
          } else {
            const err7 = {
              instancePath,
              schemaPath: "#/$defs/DataBinding/type",
              keyword: "type",
              params: { type: "object" },
              message: "must be object",
            };
            if (vErrors === null) {
              vErrors = [err7];
            } else {
              vErrors.push(err7);
            }
            errors++;
          }
        }
        var _valid0 = _errs9 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
            var props0 = true;
          }
          const _errs15 = errors;
          if (
            !wrapper2.validate(data, {
              instancePath,
              parentData,
              parentDataProperty,
              rootData,
              dynamicAnchors,
            })
          ) {
            vErrors =
              vErrors === null
                ? wrapper2.validate.errors
                : vErrors.concat(wrapper2.validate.errors);
            errors = vErrors.length;
          } else {
            var props1 = wrapper2.validate.evaluated.props;
            var items0 = wrapper2.validate.evaluated.items;
          }
          var _valid0 = _errs15 === errors;
          if (_valid0 && valid0) {
            valid0 = false;
            passing0 = [passing0, 5];
          } else {
            if (_valid0) {
              valid0 = true;
              passing0 = 5;
              if (props0 !== true && props1 !== undefined) {
                if (props1 === true) {
                  props0 = true;
                } else {
                  props0 = props0 || {};
                  Object.assign(props0, props1);
                }
              }
            }
          }
        }
      }
    }
  }
  if (!valid0) {
    const err8 = {
      instancePath,
      schemaPath: "#/oneOf",
      keyword: "oneOf",
      params: { passingSchemas: passing0 },
      message: "must match exactly one schema in oneOf",
    };
    if (vErrors === null) {
      vErrors = [err8];
    } else {
      vErrors.push(err8);
    }
    errors++;
    validate57.errors = vErrors;
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
  validate57.errors = vErrors;
  evaluated0.props = props0;
  evaluated0.items = items0;
  return errors === 0;
}
validate57.evaluated = { dynamicProps: true, dynamicItems: true };
function validate56(
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
  const evaluated0 = validate56.evaluated;
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
        (data.call === undefined && (missing0 = "call")) ||
        (data.args === undefined && (missing0 = "args"))
      ) {
        validate56.errors = [
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
        if (data.call !== undefined) {
          const _errs1 = errors;
          if ("formatDate" !== data.call) {
            validate56.errors = [
              {
                instancePath: instancePath + "/call",
                schemaPath: "#/properties/call/const",
                keyword: "const",
                params: { allowedValue: "formatDate" },
                message: "must be equal to constant",
              },
            ];
            return false;
          }
          var valid0 = _errs1 === errors;
        } else {
          var valid0 = true;
        }
        if (valid0) {
          if (data.args !== undefined) {
            let data1 = data.args;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (data1 && typeof data1 == "object" && !Array.isArray(data1)) {
                let missing1;
                if (
                  (data1.format === undefined && (missing1 = "format")) ||
                  (data1.value === undefined && (missing1 = "value"))
                ) {
                  validate56.errors = [
                    {
                      instancePath: instancePath + "/args",
                      schemaPath: "#/properties/args/required",
                      keyword: "required",
                      params: { missingProperty: missing1 },
                      message: "must have required property '" + missing1 + "'",
                    },
                  ];
                  return false;
                } else {
                  if (data1.value !== undefined) {
                    const _errs4 = errors;
                    if (
                      !validate57(data1.value, {
                        instancePath: instancePath + "/args/value",
                        parentData: data1,
                        parentDataProperty: "value",
                        rootData,
                        dynamicAnchors,
                      })
                    ) {
                      vErrors =
                        vErrors === null
                          ? validate57.errors
                          : vErrors.concat(validate57.errors);
                      errors = vErrors.length;
                    }
                    var valid1 = _errs4 === errors;
                  } else {
                    var valid1 = true;
                  }
                  if (valid1) {
                    if (data1.format !== undefined) {
                      const _errs5 = errors;
                      if (
                        !wrapper0.validate(data1.format, {
                          instancePath: instancePath + "/args/format",
                          parentData: data1,
                          parentDataProperty: "format",
                          rootData,
                          dynamicAnchors,
                        })
                      ) {
                        vErrors =
                          vErrors === null
                            ? wrapper0.validate.errors
                            : vErrors.concat(wrapper0.validate.errors);
                        errors = vErrors.length;
                      }
                      var valid1 = _errs5 === errors;
                    } else {
                      var valid1 = true;
                    }
                    if (valid1) {
                      for (const key0 in data1) {
                        if (key0 !== "value" && key0 !== "format") {
                          validate56.errors = [
                            {
                              instancePath: instancePath + "/args",
                              schemaPath:
                                "#/properties/args/unevaluatedProperties",
                              keyword: "unevaluatedProperties",
                              params: { unevaluatedProperty: key0 },
                              message: "must NOT have unevaluated properties",
                            },
                          ];
                          return false;
                          break;
                        }
                      }
                    }
                  }
                }
              } else {
                validate56.errors = [
                  {
                    instancePath: instancePath + "/args",
                    schemaPath: "#/properties/args/type",
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
            if (data.returnType !== undefined) {
              const _errs7 = errors;
              if ("string" !== data.returnType) {
                validate56.errors = [
                  {
                    instancePath: instancePath + "/returnType",
                    schemaPath: "#/properties/returnType/const",
                    keyword: "const",
                    params: { allowedValue: "string" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs7 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              for (const key1 in data) {
                if (
                  key1 !== "call" &&
                  key1 !== "args" &&
                  key1 !== "returnType"
                ) {
                  validate56.errors = [
                    {
                      instancePath,
                      schemaPath: "#/unevaluatedProperties",
                      keyword: "unevaluatedProperties",
                      params: { unevaluatedProperty: key1 },
                      message: "must NOT have unevaluated properties",
                    },
                  ];
                  return false;
                  break;
                }
              }
            }
          }
        }
      }
    } else {
      validate56.errors = [
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
  validate56.errors = vErrors;
  return errors === 0;
}
validate56.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema61 = {
  type: "object",
  description:
    "Returns a localized string based on the Common Locale Data Repository (CLDR) plural category of the count (zero, one, two, few, many, other). Requires an 'other' fallback. For English, just use 'one' and 'other'.",
  properties: {
    call: { const: "pluralize" },
    args: {
      type: "object",
      properties: {
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicNumber",
          description:
            "The numeric value used to determine the plural category.",
        },
        zero: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
          description: "String for the 'zero' category (e.g., 0 items).",
        },
        one: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
          description: "String for the 'one' category (e.g., 1 item).",
        },
        two: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
          description:
            "String for the 'two' category (used in Arabic, Welsh, etc.).",
        },
        few: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
          description:
            "String for the 'few' category (e.g., small groups in Slavic languages).",
        },
        many: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
          description:
            "String for the 'many' category (e.g., large groups in various languages).",
        },
        other: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
          description:
            "The default/fallback string (used for general plural cases).",
        },
      },
      required: ["value", "other"],
      unevaluatedProperties: false,
    },
    returnType: { const: "string" },
  },
  required: ["call", "args"],
  unevaluatedProperties: false,
};
function validate60(
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
  const evaluated0 = validate60.evaluated;
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
        (data.call === undefined && (missing0 = "call")) ||
        (data.args === undefined && (missing0 = "args"))
      ) {
        validate60.errors = [
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
        if (data.call !== undefined) {
          const _errs1 = errors;
          if ("pluralize" !== data.call) {
            validate60.errors = [
              {
                instancePath: instancePath + "/call",
                schemaPath: "#/properties/call/const",
                keyword: "const",
                params: { allowedValue: "pluralize" },
                message: "must be equal to constant",
              },
            ];
            return false;
          }
          var valid0 = _errs1 === errors;
        } else {
          var valid0 = true;
        }
        if (valid0) {
          if (data.args !== undefined) {
            let data1 = data.args;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (data1 && typeof data1 == "object" && !Array.isArray(data1)) {
                let missing1;
                if (
                  (data1.value === undefined && (missing1 = "value")) ||
                  (data1.other === undefined && (missing1 = "other"))
                ) {
                  validate60.errors = [
                    {
                      instancePath: instancePath + "/args",
                      schemaPath: "#/properties/args/required",
                      keyword: "required",
                      params: { missingProperty: missing1 },
                      message: "must have required property '" + missing1 + "'",
                    },
                  ];
                  return false;
                } else {
                  if (data1.value !== undefined) {
                    const _errs4 = errors;
                    if (
                      !validate38(data1.value, {
                        instancePath: instancePath + "/args/value",
                        parentData: data1,
                        parentDataProperty: "value",
                        rootData,
                        dynamicAnchors,
                      })
                    ) {
                      vErrors =
                        vErrors === null
                          ? validate38.errors
                          : vErrors.concat(validate38.errors);
                      errors = vErrors.length;
                    }
                    var valid1 = _errs4 === errors;
                  } else {
                    var valid1 = true;
                  }
                  if (valid1) {
                    if (data1.zero !== undefined) {
                      const _errs5 = errors;
                      if (
                        !wrapper0.validate(data1.zero, {
                          instancePath: instancePath + "/args/zero",
                          parentData: data1,
                          parentDataProperty: "zero",
                          rootData,
                          dynamicAnchors,
                        })
                      ) {
                        vErrors =
                          vErrors === null
                            ? wrapper0.validate.errors
                            : vErrors.concat(wrapper0.validate.errors);
                        errors = vErrors.length;
                      }
                      var valid1 = _errs5 === errors;
                    } else {
                      var valid1 = true;
                    }
                    if (valid1) {
                      if (data1.one !== undefined) {
                        const _errs6 = errors;
                        if (
                          !wrapper0.validate(data1.one, {
                            instancePath: instancePath + "/args/one",
                            parentData: data1,
                            parentDataProperty: "one",
                            rootData,
                            dynamicAnchors,
                          })
                        ) {
                          vErrors =
                            vErrors === null
                              ? wrapper0.validate.errors
                              : vErrors.concat(wrapper0.validate.errors);
                          errors = vErrors.length;
                        }
                        var valid1 = _errs6 === errors;
                      } else {
                        var valid1 = true;
                      }
                      if (valid1) {
                        if (data1.two !== undefined) {
                          const _errs7 = errors;
                          if (
                            !wrapper0.validate(data1.two, {
                              instancePath: instancePath + "/args/two",
                              parentData: data1,
                              parentDataProperty: "two",
                              rootData,
                              dynamicAnchors,
                            })
                          ) {
                            vErrors =
                              vErrors === null
                                ? wrapper0.validate.errors
                                : vErrors.concat(wrapper0.validate.errors);
                            errors = vErrors.length;
                          }
                          var valid1 = _errs7 === errors;
                        } else {
                          var valid1 = true;
                        }
                        if (valid1) {
                          if (data1.few !== undefined) {
                            const _errs8 = errors;
                            if (
                              !wrapper0.validate(data1.few, {
                                instancePath: instancePath + "/args/few",
                                parentData: data1,
                                parentDataProperty: "few",
                                rootData,
                                dynamicAnchors,
                              })
                            ) {
                              vErrors =
                                vErrors === null
                                  ? wrapper0.validate.errors
                                  : vErrors.concat(wrapper0.validate.errors);
                              errors = vErrors.length;
                            }
                            var valid1 = _errs8 === errors;
                          } else {
                            var valid1 = true;
                          }
                          if (valid1) {
                            if (data1.many !== undefined) {
                              const _errs9 = errors;
                              if (
                                !wrapper0.validate(data1.many, {
                                  instancePath: instancePath + "/args/many",
                                  parentData: data1,
                                  parentDataProperty: "many",
                                  rootData,
                                  dynamicAnchors,
                                })
                              ) {
                                vErrors =
                                  vErrors === null
                                    ? wrapper0.validate.errors
                                    : vErrors.concat(wrapper0.validate.errors);
                                errors = vErrors.length;
                              }
                              var valid1 = _errs9 === errors;
                            } else {
                              var valid1 = true;
                            }
                            if (valid1) {
                              if (data1.other !== undefined) {
                                const _errs10 = errors;
                                if (
                                  !wrapper0.validate(data1.other, {
                                    instancePath: instancePath + "/args/other",
                                    parentData: data1,
                                    parentDataProperty: "other",
                                    rootData,
                                    dynamicAnchors,
                                  })
                                ) {
                                  vErrors =
                                    vErrors === null
                                      ? wrapper0.validate.errors
                                      : vErrors.concat(
                                          wrapper0.validate.errors,
                                        );
                                  errors = vErrors.length;
                                }
                                var valid1 = _errs10 === errors;
                              } else {
                                var valid1 = true;
                              }
                              if (valid1) {
                                for (const key0 in data1) {
                                  if (
                                    key0 !== "value" &&
                                    key0 !== "zero" &&
                                    key0 !== "one" &&
                                    key0 !== "two" &&
                                    key0 !== "few" &&
                                    key0 !== "many" &&
                                    key0 !== "other"
                                  ) {
                                    validate60.errors = [
                                      {
                                        instancePath: instancePath + "/args",
                                        schemaPath:
                                          "#/properties/args/unevaluatedProperties",
                                        keyword: "unevaluatedProperties",
                                        params: { unevaluatedProperty: key0 },
                                        message:
                                          "must NOT have unevaluated properties",
                                      },
                                    ];
                                    return false;
                                    break;
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
                validate60.errors = [
                  {
                    instancePath: instancePath + "/args",
                    schemaPath: "#/properties/args/type",
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
            if (data.returnType !== undefined) {
              const _errs12 = errors;
              if ("string" !== data.returnType) {
                validate60.errors = [
                  {
                    instancePath: instancePath + "/returnType",
                    schemaPath: "#/properties/returnType/const",
                    keyword: "const",
                    params: { allowedValue: "string" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs12 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              for (const key1 in data) {
                if (
                  key1 !== "call" &&
                  key1 !== "args" &&
                  key1 !== "returnType"
                ) {
                  validate60.errors = [
                    {
                      instancePath,
                      schemaPath: "#/unevaluatedProperties",
                      keyword: "unevaluatedProperties",
                      params: { unevaluatedProperty: key1 },
                      message: "must NOT have unevaluated properties",
                    },
                  ];
                  return false;
                  break;
                }
              }
            }
          }
        }
      }
    } else {
      validate60.errors = [
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
  validate60.errors = vErrors;
  return errors === 0;
}
validate60.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema63 = {
  type: "object",
  description: "Performs a logical AND operation on a list of boolean values.",
  properties: {
    call: { const: "and" },
    args: {
      type: "object",
      properties: {
        values: {
          type: "array",
          description: "The list of boolean values to evaluate.",
          items: {
            $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicBoolean",
          },
          minItems: 2,
        },
      },
      required: ["values"],
      unevaluatedProperties: false,
    },
    returnType: { const: "boolean" },
  },
  required: ["call", "args"],
  unevaluatedProperties: false,
};
function validate63(
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
  const evaluated0 = validate63.evaluated;
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
        (data.call === undefined && (missing0 = "call")) ||
        (data.args === undefined && (missing0 = "args"))
      ) {
        validate63.errors = [
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
        if (data.call !== undefined) {
          const _errs1 = errors;
          if ("and" !== data.call) {
            validate63.errors = [
              {
                instancePath: instancePath + "/call",
                schemaPath: "#/properties/call/const",
                keyword: "const",
                params: { allowedValue: "and" },
                message: "must be equal to constant",
              },
            ];
            return false;
          }
          var valid0 = _errs1 === errors;
        } else {
          var valid0 = true;
        }
        if (valid0) {
          if (data.args !== undefined) {
            let data1 = data.args;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (data1 && typeof data1 == "object" && !Array.isArray(data1)) {
                let missing1;
                if (data1.values === undefined && (missing1 = "values")) {
                  validate63.errors = [
                    {
                      instancePath: instancePath + "/args",
                      schemaPath: "#/properties/args/required",
                      keyword: "required",
                      params: { missingProperty: missing1 },
                      message: "must have required property '" + missing1 + "'",
                    },
                  ];
                  return false;
                } else {
                  if (data1.values !== undefined) {
                    let data2 = data1.values;
                    const _errs4 = errors;
                    if (errors === _errs4) {
                      if (Array.isArray(data2)) {
                        if (data2.length < 2) {
                          validate63.errors = [
                            {
                              instancePath: instancePath + "/args/values",
                              schemaPath:
                                "#/properties/args/properties/values/minItems",
                              keyword: "minItems",
                              params: { limit: 2 },
                              message: "must NOT have fewer than 2 items",
                            },
                          ];
                          return false;
                        } else {
                          var valid2 = true;
                          const len0 = data2.length;
                          for (let i0 = 0; i0 < len0; i0++) {
                            const _errs6 = errors;
                            if (
                              !validate48(data2[i0], {
                                instancePath:
                                  instancePath + "/args/values/" + i0,
                                parentData: data2,
                                parentDataProperty: i0,
                                rootData,
                                dynamicAnchors,
                              })
                            ) {
                              vErrors =
                                vErrors === null
                                  ? validate48.errors
                                  : vErrors.concat(validate48.errors);
                              errors = vErrors.length;
                            }
                            var valid2 = _errs6 === errors;
                            if (!valid2) {
                              break;
                            }
                          }
                        }
                      } else {
                        validate63.errors = [
                          {
                            instancePath: instancePath + "/args/values",
                            schemaPath:
                              "#/properties/args/properties/values/type",
                            keyword: "type",
                            params: { type: "array" },
                            message: "must be array",
                          },
                        ];
                        return false;
                      }
                    }
                    var valid1 = _errs4 === errors;
                  } else {
                    var valid1 = true;
                  }
                  if (valid1) {
                    for (const key0 in data1) {
                      if (key0 !== "values") {
                        validate63.errors = [
                          {
                            instancePath: instancePath + "/args",
                            schemaPath:
                              "#/properties/args/unevaluatedProperties",
                            keyword: "unevaluatedProperties",
                            params: { unevaluatedProperty: key0 },
                            message: "must NOT have unevaluated properties",
                          },
                        ];
                        return false;
                        break;
                      }
                    }
                  }
                }
              } else {
                validate63.errors = [
                  {
                    instancePath: instancePath + "/args",
                    schemaPath: "#/properties/args/type",
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
            if (data.returnType !== undefined) {
              const _errs8 = errors;
              if ("boolean" !== data.returnType) {
                validate63.errors = [
                  {
                    instancePath: instancePath + "/returnType",
                    schemaPath: "#/properties/returnType/const",
                    keyword: "const",
                    params: { allowedValue: "boolean" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs8 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              for (const key1 in data) {
                if (
                  key1 !== "call" &&
                  key1 !== "args" &&
                  key1 !== "returnType"
                ) {
                  validate63.errors = [
                    {
                      instancePath,
                      schemaPath: "#/unevaluatedProperties",
                      keyword: "unevaluatedProperties",
                      params: { unevaluatedProperty: key1 },
                      message: "must NOT have unevaluated properties",
                    },
                  ];
                  return false;
                  break;
                }
              }
            }
          }
        }
      }
    } else {
      validate63.errors = [
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
  validate63.errors = vErrors;
  return errors === 0;
}
validate63.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema64 = {
  type: "object",
  description: "Performs a logical OR operation on a list of boolean values.",
  properties: {
    call: { const: "or" },
    args: {
      type: "object",
      properties: {
        values: {
          type: "array",
          description: "The list of boolean values to evaluate.",
          items: {
            $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicBoolean",
          },
          minItems: 2,
        },
      },
      required: ["values"],
      unevaluatedProperties: false,
    },
    returnType: { const: "boolean" },
  },
  required: ["call", "args"],
  unevaluatedProperties: false,
};
function validate66(
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
  const evaluated0 = validate66.evaluated;
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
        (data.call === undefined && (missing0 = "call")) ||
        (data.args === undefined && (missing0 = "args"))
      ) {
        validate66.errors = [
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
        if (data.call !== undefined) {
          const _errs1 = errors;
          if ("or" !== data.call) {
            validate66.errors = [
              {
                instancePath: instancePath + "/call",
                schemaPath: "#/properties/call/const",
                keyword: "const",
                params: { allowedValue: "or" },
                message: "must be equal to constant",
              },
            ];
            return false;
          }
          var valid0 = _errs1 === errors;
        } else {
          var valid0 = true;
        }
        if (valid0) {
          if (data.args !== undefined) {
            let data1 = data.args;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (data1 && typeof data1 == "object" && !Array.isArray(data1)) {
                let missing1;
                if (data1.values === undefined && (missing1 = "values")) {
                  validate66.errors = [
                    {
                      instancePath: instancePath + "/args",
                      schemaPath: "#/properties/args/required",
                      keyword: "required",
                      params: { missingProperty: missing1 },
                      message: "must have required property '" + missing1 + "'",
                    },
                  ];
                  return false;
                } else {
                  if (data1.values !== undefined) {
                    let data2 = data1.values;
                    const _errs4 = errors;
                    if (errors === _errs4) {
                      if (Array.isArray(data2)) {
                        if (data2.length < 2) {
                          validate66.errors = [
                            {
                              instancePath: instancePath + "/args/values",
                              schemaPath:
                                "#/properties/args/properties/values/minItems",
                              keyword: "minItems",
                              params: { limit: 2 },
                              message: "must NOT have fewer than 2 items",
                            },
                          ];
                          return false;
                        } else {
                          var valid2 = true;
                          const len0 = data2.length;
                          for (let i0 = 0; i0 < len0; i0++) {
                            const _errs6 = errors;
                            if (
                              !validate48(data2[i0], {
                                instancePath:
                                  instancePath + "/args/values/" + i0,
                                parentData: data2,
                                parentDataProperty: i0,
                                rootData,
                                dynamicAnchors,
                              })
                            ) {
                              vErrors =
                                vErrors === null
                                  ? validate48.errors
                                  : vErrors.concat(validate48.errors);
                              errors = vErrors.length;
                            }
                            var valid2 = _errs6 === errors;
                            if (!valid2) {
                              break;
                            }
                          }
                        }
                      } else {
                        validate66.errors = [
                          {
                            instancePath: instancePath + "/args/values",
                            schemaPath:
                              "#/properties/args/properties/values/type",
                            keyword: "type",
                            params: { type: "array" },
                            message: "must be array",
                          },
                        ];
                        return false;
                      }
                    }
                    var valid1 = _errs4 === errors;
                  } else {
                    var valid1 = true;
                  }
                  if (valid1) {
                    for (const key0 in data1) {
                      if (key0 !== "values") {
                        validate66.errors = [
                          {
                            instancePath: instancePath + "/args",
                            schemaPath:
                              "#/properties/args/unevaluatedProperties",
                            keyword: "unevaluatedProperties",
                            params: { unevaluatedProperty: key0 },
                            message: "must NOT have unevaluated properties",
                          },
                        ];
                        return false;
                        break;
                      }
                    }
                  }
                }
              } else {
                validate66.errors = [
                  {
                    instancePath: instancePath + "/args",
                    schemaPath: "#/properties/args/type",
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
            if (data.returnType !== undefined) {
              const _errs8 = errors;
              if ("boolean" !== data.returnType) {
                validate66.errors = [
                  {
                    instancePath: instancePath + "/returnType",
                    schemaPath: "#/properties/returnType/const",
                    keyword: "const",
                    params: { allowedValue: "boolean" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs8 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              for (const key1 in data) {
                if (
                  key1 !== "call" &&
                  key1 !== "args" &&
                  key1 !== "returnType"
                ) {
                  validate66.errors = [
                    {
                      instancePath,
                      schemaPath: "#/unevaluatedProperties",
                      keyword: "unevaluatedProperties",
                      params: { unevaluatedProperty: key1 },
                      message: "must NOT have unevaluated properties",
                    },
                  ];
                  return false;
                  break;
                }
              }
            }
          }
        }
      }
    } else {
      validate66.errors = [
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
  validate66.errors = vErrors;
  return errors === 0;
}
validate66.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema65 = {
  type: "object",
  description: "Performs a logical NOT operation on a boolean value.",
  properties: {
    call: { const: "not" },
    args: {
      type: "object",
      properties: {
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicBoolean",
          description: "The boolean value to negate.",
        },
      },
      required: ["value"],
      unevaluatedProperties: false,
    },
    returnType: { const: "boolean" },
  },
  required: ["call", "args"],
  unevaluatedProperties: false,
};
function validate69(
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
  const evaluated0 = validate69.evaluated;
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
        (data.call === undefined && (missing0 = "call")) ||
        (data.args === undefined && (missing0 = "args"))
      ) {
        validate69.errors = [
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
        if (data.call !== undefined) {
          const _errs1 = errors;
          if ("not" !== data.call) {
            validate69.errors = [
              {
                instancePath: instancePath + "/call",
                schemaPath: "#/properties/call/const",
                keyword: "const",
                params: { allowedValue: "not" },
                message: "must be equal to constant",
              },
            ];
            return false;
          }
          var valid0 = _errs1 === errors;
        } else {
          var valid0 = true;
        }
        if (valid0) {
          if (data.args !== undefined) {
            let data1 = data.args;
            const _errs2 = errors;
            if (errors === _errs2) {
              if (data1 && typeof data1 == "object" && !Array.isArray(data1)) {
                let missing1;
                if (data1.value === undefined && (missing1 = "value")) {
                  validate69.errors = [
                    {
                      instancePath: instancePath + "/args",
                      schemaPath: "#/properties/args/required",
                      keyword: "required",
                      params: { missingProperty: missing1 },
                      message: "must have required property '" + missing1 + "'",
                    },
                  ];
                  return false;
                } else {
                  if (data1.value !== undefined) {
                    const _errs4 = errors;
                    if (
                      !validate48(data1.value, {
                        instancePath: instancePath + "/args/value",
                        parentData: data1,
                        parentDataProperty: "value",
                        rootData,
                        dynamicAnchors,
                      })
                    ) {
                      vErrors =
                        vErrors === null
                          ? validate48.errors
                          : vErrors.concat(validate48.errors);
                      errors = vErrors.length;
                    }
                    var valid1 = _errs4 === errors;
                  } else {
                    var valid1 = true;
                  }
                  if (valid1) {
                    for (const key0 in data1) {
                      if (key0 !== "value") {
                        validate69.errors = [
                          {
                            instancePath: instancePath + "/args",
                            schemaPath:
                              "#/properties/args/unevaluatedProperties",
                            keyword: "unevaluatedProperties",
                            params: { unevaluatedProperty: key0 },
                            message: "must NOT have unevaluated properties",
                          },
                        ];
                        return false;
                        break;
                      }
                    }
                  }
                }
              } else {
                validate69.errors = [
                  {
                    instancePath: instancePath + "/args",
                    schemaPath: "#/properties/args/type",
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
            if (data.returnType !== undefined) {
              const _errs6 = errors;
              if ("boolean" !== data.returnType) {
                validate69.errors = [
                  {
                    instancePath: instancePath + "/returnType",
                    schemaPath: "#/properties/returnType/const",
                    keyword: "const",
                    params: { allowedValue: "boolean" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid0 = _errs6 === errors;
            } else {
              var valid0 = true;
            }
            if (valid0) {
              for (const key1 in data) {
                if (
                  key1 !== "call" &&
                  key1 !== "args" &&
                  key1 !== "returnType"
                ) {
                  validate69.errors = [
                    {
                      instancePath,
                      schemaPath: "#/unevaluatedProperties",
                      keyword: "unevaluatedProperties",
                      params: { unevaluatedProperty: key1 },
                      message: "must NOT have unevaluated properties",
                    },
                  ];
                  return false;
                  break;
                }
              }
            }
          }
        }
      }
    } else {
      validate69.errors = [
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
  validate69.errors = vErrors;
  return errors === 0;
}
validate69.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
function validate32(
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
  const evaluated0 = validate32.evaluated;
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
  const _errs2 = errors;
  if (errors === _errs2) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.call === undefined && (missing0 = "call")) ||
        (data.args === undefined && (missing0 = "args"))
      ) {
        const err0 = {
          instancePath,
          schemaPath: "#/functions/required/required",
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
        if (data.call !== undefined) {
          const _errs4 = errors;
          if ("required" !== data.call) {
            const err1 = {
              instancePath: instancePath + "/call",
              schemaPath: "#/functions/required/properties/call/const",
              keyword: "const",
              params: { allowedValue: "required" },
              message: "must be equal to constant",
            };
            if (vErrors === null) {
              vErrors = [err1];
            } else {
              vErrors.push(err1);
            }
            errors++;
          }
          var valid2 = _errs4 === errors;
        } else {
          var valid2 = true;
        }
        if (valid2) {
          if (data.args !== undefined) {
            let data1 = data.args;
            const _errs5 = errors;
            if (errors === _errs5) {
              if (data1 && typeof data1 == "object" && !Array.isArray(data1)) {
                let missing1;
                if (data1.value === undefined && (missing1 = "value")) {
                  const err2 = {
                    instancePath: instancePath + "/args",
                    schemaPath: "#/functions/required/properties/args/required",
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
                  for (const key0 in data1) {
                    if (!(key0 === "value")) {
                      const err3 = {
                        instancePath: instancePath + "/args",
                        schemaPath:
                          "#/functions/required/properties/args/additionalProperties",
                        keyword: "additionalProperties",
                        params: { additionalProperty: key0 },
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
                }
              } else {
                const err4 = {
                  instancePath: instancePath + "/args",
                  schemaPath: "#/functions/required/properties/args/type",
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
            var valid2 = _errs5 === errors;
          } else {
            var valid2 = true;
          }
          if (valid2) {
            if (data.returnType !== undefined) {
              const _errs8 = errors;
              if ("boolean" !== data.returnType) {
                const err5 = {
                  instancePath: instancePath + "/returnType",
                  schemaPath:
                    "#/functions/required/properties/returnType/const",
                  keyword: "const",
                  params: { allowedValue: "boolean" },
                  message: "must be equal to constant",
                };
                if (vErrors === null) {
                  vErrors = [err5];
                } else {
                  vErrors.push(err5);
                }
                errors++;
              }
              var valid2 = _errs8 === errors;
            } else {
              var valid2 = true;
            }
            if (valid2) {
              for (const key1 in data) {
                if (
                  key1 !== "call" &&
                  key1 !== "args" &&
                  key1 !== "returnType"
                ) {
                  const err6 = {
                    instancePath,
                    schemaPath: "#/functions/required/unevaluatedProperties",
                    keyword: "unevaluatedProperties",
                    params: { unevaluatedProperty: key1 },
                    message: "must NOT have unevaluated properties",
                  };
                  if (vErrors === null) {
                    vErrors = [err6];
                  } else {
                    vErrors.push(err6);
                  }
                  errors++;
                  break;
                }
              }
            }
          }
        }
      }
    } else {
      const err7 = {
        instancePath,
        schemaPath: "#/functions/required/type",
        keyword: "type",
        params: { type: "object" },
        message: "must be object",
      };
      if (vErrors === null) {
        vErrors = [err7];
      } else {
        vErrors.push(err7);
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
  const _errs10 = errors;
  if (
    !validate33(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate33.errors : vErrors.concat(validate33.errors);
    errors = vErrors.length;
  }
  var _valid0 = _errs10 === errors;
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
    const _errs11 = errors;
    if (
      !validate35(data, {
        instancePath,
        parentData,
        parentDataProperty,
        rootData,
        dynamicAnchors,
      })
    ) {
      vErrors =
        vErrors === null
          ? validate35.errors
          : vErrors.concat(validate35.errors);
      errors = vErrors.length;
    }
    var _valid0 = _errs11 === errors;
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
      const _errs12 = errors;
      if (
        !validate37(data, {
          instancePath,
          parentData,
          parentDataProperty,
          rootData,
          dynamicAnchors,
        })
      ) {
        vErrors =
          vErrors === null
            ? validate37.errors
            : vErrors.concat(validate37.errors);
        errors = vErrors.length;
      }
      var _valid0 = _errs12 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
          if (props0 !== true) {
            props0 = true;
          }
        }
        const _errs13 = errors;
        if (
          !validate41(data, {
            instancePath,
            parentData,
            parentDataProperty,
            rootData,
            dynamicAnchors,
          })
        ) {
          vErrors =
            vErrors === null
              ? validate41.errors
              : vErrors.concat(validate41.errors);
          errors = vErrors.length;
        }
        var _valid0 = _errs13 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
            if (props0 !== true) {
              props0 = true;
            }
          }
          const _errs14 = errors;
          if (
            !validate43(data, {
              instancePath,
              parentData,
              parentDataProperty,
              rootData,
              dynamicAnchors,
            })
          ) {
            vErrors =
              vErrors === null
                ? validate43.errors
                : vErrors.concat(validate43.errors);
            errors = vErrors.length;
          }
          var _valid0 = _errs14 === errors;
          if (_valid0 && valid0) {
            valid0 = false;
            passing0 = [passing0, 5];
          } else {
            if (_valid0) {
              valid0 = true;
              passing0 = 5;
              if (props0 !== true) {
                props0 = true;
              }
            }
            const _errs15 = errors;
            if (
              !validate45(data, {
                instancePath,
                parentData,
                parentDataProperty,
                rootData,
                dynamicAnchors,
              })
            ) {
              vErrors =
                vErrors === null
                  ? validate45.errors
                  : vErrors.concat(validate45.errors);
              errors = vErrors.length;
            }
            var _valid0 = _errs15 === errors;
            if (_valid0 && valid0) {
              valid0 = false;
              passing0 = [passing0, 6];
            } else {
              if (_valid0) {
                valid0 = true;
                passing0 = 6;
                if (props0 !== true) {
                  props0 = true;
                }
              }
              const _errs16 = errors;
              if (
                !validate51(data, {
                  instancePath,
                  parentData,
                  parentDataProperty,
                  rootData,
                  dynamicAnchors,
                })
              ) {
                vErrors =
                  vErrors === null
                    ? validate51.errors
                    : vErrors.concat(validate51.errors);
                errors = vErrors.length;
              }
              var _valid0 = _errs16 === errors;
              if (_valid0 && valid0) {
                valid0 = false;
                passing0 = [passing0, 7];
              } else {
                if (_valid0) {
                  valid0 = true;
                  passing0 = 7;
                  if (props0 !== true) {
                    props0 = true;
                  }
                }
                const _errs17 = errors;
                if (
                  !validate56(data, {
                    instancePath,
                    parentData,
                    parentDataProperty,
                    rootData,
                    dynamicAnchors,
                  })
                ) {
                  vErrors =
                    vErrors === null
                      ? validate56.errors
                      : vErrors.concat(validate56.errors);
                  errors = vErrors.length;
                }
                var _valid0 = _errs17 === errors;
                if (_valid0 && valid0) {
                  valid0 = false;
                  passing0 = [passing0, 8];
                } else {
                  if (_valid0) {
                    valid0 = true;
                    passing0 = 8;
                    if (props0 !== true) {
                      props0 = true;
                    }
                  }
                  const _errs18 = errors;
                  if (
                    !validate60(data, {
                      instancePath,
                      parentData,
                      parentDataProperty,
                      rootData,
                      dynamicAnchors,
                    })
                  ) {
                    vErrors =
                      vErrors === null
                        ? validate60.errors
                        : vErrors.concat(validate60.errors);
                    errors = vErrors.length;
                  }
                  var _valid0 = _errs18 === errors;
                  if (_valid0 && valid0) {
                    valid0 = false;
                    passing0 = [passing0, 9];
                  } else {
                    if (_valid0) {
                      valid0 = true;
                      passing0 = 9;
                      if (props0 !== true) {
                        props0 = true;
                      }
                    }
                    const _errs19 = errors;
                    const _errs20 = errors;
                    if (errors === _errs20) {
                      if (
                        data &&
                        typeof data == "object" &&
                        !Array.isArray(data)
                      ) {
                        let missing2;
                        if (
                          (data.call === undefined && (missing2 = "call")) ||
                          (data.args === undefined && (missing2 = "args"))
                        ) {
                          const err8 = {
                            instancePath,
                            schemaPath: "#/functions/openUrl/required",
                            keyword: "required",
                            params: { missingProperty: missing2 },
                            message:
                              "must have required property '" + missing2 + "'",
                          };
                          if (vErrors === null) {
                            vErrors = [err8];
                          } else {
                            vErrors.push(err8);
                          }
                          errors++;
                        } else {
                          if (data.call !== undefined) {
                            const _errs22 = errors;
                            if ("openUrl" !== data.call) {
                              const err9 = {
                                instancePath: instancePath + "/call",
                                schemaPath:
                                  "#/functions/openUrl/properties/call/const",
                                keyword: "const",
                                params: { allowedValue: "openUrl" },
                                message: "must be equal to constant",
                              };
                              if (vErrors === null) {
                                vErrors = [err9];
                              } else {
                                vErrors.push(err9);
                              }
                              errors++;
                            }
                            var valid4 = _errs22 === errors;
                          } else {
                            var valid4 = true;
                          }
                          if (valid4) {
                            if (data.args !== undefined) {
                              let data4 = data.args;
                              const _errs23 = errors;
                              if (errors === _errs23) {
                                if (
                                  data4 &&
                                  typeof data4 == "object" &&
                                  !Array.isArray(data4)
                                ) {
                                  let missing3;
                                  if (
                                    data4.url === undefined &&
                                    (missing3 = "url")
                                  ) {
                                    const err10 = {
                                      instancePath: instancePath + "/args",
                                      schemaPath:
                                        "#/functions/openUrl/properties/args/required",
                                      keyword: "required",
                                      params: { missingProperty: missing3 },
                                      message:
                                        "must have required property '" +
                                        missing3 +
                                        "'",
                                    };
                                    if (vErrors === null) {
                                      vErrors = [err10];
                                    } else {
                                      vErrors.push(err10);
                                    }
                                    errors++;
                                  } else {
                                    const _errs25 = errors;
                                    for (const key2 in data4) {
                                      if (!(key2 === "url")) {
                                        const err11 = {
                                          instancePath: instancePath + "/args",
                                          schemaPath:
                                            "#/functions/openUrl/properties/args/additionalProperties",
                                          keyword: "additionalProperties",
                                          params: { additionalProperty: key2 },
                                          message:
                                            "must NOT have additional properties",
                                        };
                                        if (vErrors === null) {
                                          vErrors = [err11];
                                        } else {
                                          vErrors.push(err11);
                                        }
                                        errors++;
                                        break;
                                      }
                                    }
                                    if (_errs25 === errors) {
                                      if (data4.url !== undefined) {
                                        const _errs26 = errors;
                                        if (errors === _errs26) {
                                          if (errors === _errs26) {
                                            if (
                                              !(typeof data4.url === "string")
                                            ) {
                                              const err12 = {
                                                instancePath:
                                                  instancePath + "/args/url",
                                                schemaPath:
                                                  "#/functions/openUrl/properties/args/properties/url/type",
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
                                        }
                                      }
                                    }
                                  }
                                } else {
                                  const err13 = {
                                    instancePath: instancePath + "/args",
                                    schemaPath:
                                      "#/functions/openUrl/properties/args/type",
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
                              var valid4 = _errs23 === errors;
                            } else {
                              var valid4 = true;
                            }
                            if (valid4) {
                              if (data.returnType !== undefined) {
                                const _errs28 = errors;
                                if ("void" !== data.returnType) {
                                  const err14 = {
                                    instancePath: instancePath + "/returnType",
                                    schemaPath:
                                      "#/functions/openUrl/properties/returnType/const",
                                    keyword: "const",
                                    params: { allowedValue: "void" },
                                    message: "must be equal to constant",
                                  };
                                  if (vErrors === null) {
                                    vErrors = [err14];
                                  } else {
                                    vErrors.push(err14);
                                  }
                                  errors++;
                                }
                                var valid4 = _errs28 === errors;
                              } else {
                                var valid4 = true;
                              }
                              if (valid4) {
                                for (const key3 in data) {
                                  if (
                                    key3 !== "call" &&
                                    key3 !== "args" &&
                                    key3 !== "returnType"
                                  ) {
                                    const err15 = {
                                      instancePath,
                                      schemaPath:
                                        "#/functions/openUrl/unevaluatedProperties",
                                      keyword: "unevaluatedProperties",
                                      params: { unevaluatedProperty: key3 },
                                      message:
                                        "must NOT have unevaluated properties",
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
                              }
                            }
                          }
                        }
                      } else {
                        const err16 = {
                          instancePath,
                          schemaPath: "#/functions/openUrl/type",
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
                    var _valid0 = _errs19 === errors;
                    if (_valid0 && valid0) {
                      valid0 = false;
                      passing0 = [passing0, 10];
                    } else {
                      if (_valid0) {
                        valid0 = true;
                        passing0 = 10;
                        if (props0 !== true) {
                          props0 = true;
                        }
                      }
                      const _errs30 = errors;
                      if (
                        !validate63(data, {
                          instancePath,
                          parentData,
                          parentDataProperty,
                          rootData,
                          dynamicAnchors,
                        })
                      ) {
                        vErrors =
                          vErrors === null
                            ? validate63.errors
                            : vErrors.concat(validate63.errors);
                        errors = vErrors.length;
                      }
                      var _valid0 = _errs30 === errors;
                      if (_valid0 && valid0) {
                        valid0 = false;
                        passing0 = [passing0, 11];
                      } else {
                        if (_valid0) {
                          valid0 = true;
                          passing0 = 11;
                          if (props0 !== true) {
                            props0 = true;
                          }
                        }
                        const _errs31 = errors;
                        if (
                          !validate66(data, {
                            instancePath,
                            parentData,
                            parentDataProperty,
                            rootData,
                            dynamicAnchors,
                          })
                        ) {
                          vErrors =
                            vErrors === null
                              ? validate66.errors
                              : vErrors.concat(validate66.errors);
                          errors = vErrors.length;
                        }
                        var _valid0 = _errs31 === errors;
                        if (_valid0 && valid0) {
                          valid0 = false;
                          passing0 = [passing0, 12];
                        } else {
                          if (_valid0) {
                            valid0 = true;
                            passing0 = 12;
                            if (props0 !== true) {
                              props0 = true;
                            }
                          }
                          const _errs32 = errors;
                          if (
                            !validate69(data, {
                              instancePath,
                              parentData,
                              parentDataProperty,
                              rootData,
                              dynamicAnchors,
                            })
                          ) {
                            vErrors =
                              vErrors === null
                                ? validate69.errors
                                : vErrors.concat(validate69.errors);
                            errors = vErrors.length;
                          }
                          var _valid0 = _errs32 === errors;
                          if (_valid0 && valid0) {
                            valid0 = false;
                            passing0 = [passing0, 13];
                          } else {
                            if (_valid0) {
                              valid0 = true;
                              passing0 = 13;
                              if (props0 !== true) {
                                props0 = true;
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
    validate32.errors = vErrors;
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
  validate32.errors = vErrors;
  evaluated0.props = props0;
  return errors === 0;
}
validate32.evaluated = { dynamicProps: true, dynamicItems: false };
function validate73(
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
  const evaluated0 = validate73.evaluated;
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
  if (typeof data !== "string") {
    const err0 = {
      instancePath,
      schemaPath: "#/oneOf/0/type",
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
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (!(typeof data == "number")) {
    const err1 = {
      instancePath,
      schemaPath: "#/oneOf/1/type",
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
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "boolean") {
      const err2 = {
        instancePath,
        schemaPath: "#/oneOf/2/type",
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
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
      const _errs7 = errors;
      if (!Array.isArray(data)) {
        const err3 = {
          instancePath,
          schemaPath: "#/oneOf/3/type",
          keyword: "type",
          params: { type: "array" },
          message: "must be array",
        };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
      var _valid0 = _errs7 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
        }
        const _errs9 = errors;
        const _errs10 = errors;
        if (errors === _errs10) {
          if (data && typeof data == "object" && !Array.isArray(data)) {
            let missing0;
            if (data.path === undefined && (missing0 = "path")) {
              const err4 = {
                instancePath,
                schemaPath: "#/$defs/DataBinding/required",
                keyword: "required",
                params: { missingProperty: missing0 },
                message: "must have required property '" + missing0 + "'",
              };
              if (vErrors === null) {
                vErrors = [err4];
              } else {
                vErrors.push(err4);
              }
              errors++;
            } else {
              const _errs12 = errors;
              for (const key0 in data) {
                if (!(key0 === "path")) {
                  const err5 = {
                    instancePath,
                    schemaPath: "#/$defs/DataBinding/additionalProperties",
                    keyword: "additionalProperties",
                    params: { additionalProperty: key0 },
                    message: "must NOT have additional properties",
                  };
                  if (vErrors === null) {
                    vErrors = [err5];
                  } else {
                    vErrors.push(err5);
                  }
                  errors++;
                  break;
                }
              }
              if (_errs12 === errors) {
                if (data.path !== undefined) {
                  if (typeof data.path !== "string") {
                    const err6 = {
                      instancePath: instancePath + "/path",
                      schemaPath: "#/$defs/DataBinding/properties/path/type",
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
                }
              }
            }
          } else {
            const err7 = {
              instancePath,
              schemaPath: "#/$defs/DataBinding/type",
              keyword: "type",
              params: { type: "object" },
              message: "must be object",
            };
            if (vErrors === null) {
              vErrors = [err7];
            } else {
              vErrors.push(err7);
            }
            errors++;
          }
        }
        var _valid0 = _errs9 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
            var props0 = true;
          }
          const _errs15 = errors;
          if (
            !wrapper2.validate(data, {
              instancePath,
              parentData,
              parentDataProperty,
              rootData,
              dynamicAnchors,
            })
          ) {
            vErrors =
              vErrors === null
                ? wrapper2.validate.errors
                : vErrors.concat(wrapper2.validate.errors);
            errors = vErrors.length;
          } else {
            var props1 = wrapper2.validate.evaluated.props;
            var items0 = wrapper2.validate.evaluated.items;
          }
          var _valid0 = _errs15 === errors;
          if (_valid0 && valid0) {
            valid0 = false;
            passing0 = [passing0, 5];
          } else {
            if (_valid0) {
              valid0 = true;
              passing0 = 5;
              if (props0 !== true && props1 !== undefined) {
                if (props1 === true) {
                  props0 = true;
                } else {
                  props0 = props0 || {};
                  Object.assign(props0, props1);
                }
              }
            }
          }
        }
      }
    }
  }
  if (!valid0) {
    const err8 = {
      instancePath,
      schemaPath: "#/oneOf",
      keyword: "oneOf",
      params: { passingSchemas: passing0 },
      message: "must match exactly one schema in oneOf",
    };
    if (vErrors === null) {
      vErrors = [err8];
    } else {
      vErrors.push(err8);
    }
    errors++;
    validate73.errors = vErrors;
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
  validate73.errors = vErrors;
  evaluated0.props = props0;
  evaluated0.items = items0;
  return errors === 0;
}
validate73.evaluated = { dynamicProps: true, dynamicItems: true };
function validate31(
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
  const evaluated0 = validate31.evaluated;
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
  if (
    !validate32(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate32.errors : vErrors.concat(validate32.errors);
    errors = vErrors.length;
  } else {
    var props0 = validate32.evaluated.props;
  }
  var _valid0 = _errs2 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  if (!valid0) {
    const err0 = {
      instancePath,
      schemaPath: "#/oneOf",
      keyword: "oneOf",
      params: { passingSchemas: passing0 },
      message: "must match exactly one schema in oneOf",
    };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
    validate31.errors = vErrors;
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
      let missing0;
      if (data.call === undefined && (missing0 = "call")) {
        validate31.errors = [
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
        if (props0 !== true) {
          props0 = props0 || {};
          props0.call = true;
          props0.args = true;
          props0.returnType = true;
        }
        if (data.call !== undefined) {
          const _errs3 = errors;
          if (typeof data.call !== "string") {
            validate31.errors = [
              {
                instancePath: instancePath + "/call",
                schemaPath: "#/properties/call/type",
                keyword: "type",
                params: { type: "string" },
                message: "must be string",
              },
            ];
            return false;
          }
          var valid1 = _errs3 === errors;
        } else {
          var valid1 = true;
        }
        if (valid1) {
          if (data.args !== undefined) {
            let data1 = data.args;
            const _errs5 = errors;
            if (errors === _errs5) {
              if (data1 && typeof data1 == "object" && !Array.isArray(data1)) {
                for (const key0 in data1) {
                  let data2 = data1[key0];
                  const _errs8 = errors;
                  const _errs9 = errors;
                  let valid3 = false;
                  const _errs10 = errors;
                  if (
                    !validate73(data2, {
                      instancePath:
                        instancePath +
                        "/args/" +
                        key0.replace(/~/g, "~0").replace(/\//g, "~1"),
                      parentData: data1,
                      parentDataProperty: key0,
                      rootData,
                      dynamicAnchors,
                    })
                  ) {
                    vErrors =
                      vErrors === null
                        ? validate73.errors
                        : vErrors.concat(validate73.errors);
                    errors = vErrors.length;
                  }
                  var _valid1 = _errs10 === errors;
                  valid3 = valid3 || _valid1;
                  const _errs11 = errors;
                  if (
                    !(
                      data2 &&
                      typeof data2 == "object" &&
                      !Array.isArray(data2)
                    )
                  ) {
                    const err1 = {
                      instancePath:
                        instancePath +
                        "/args/" +
                        key0.replace(/~/g, "~0").replace(/\//g, "~1"),
                      schemaPath:
                        "#/properties/args/additionalProperties/anyOf/1/type",
                      keyword: "type",
                      params: { type: "object" },
                      message: "must be object",
                    };
                    if (vErrors === null) {
                      vErrors = [err1];
                    } else {
                      vErrors.push(err1);
                    }
                    errors++;
                  }
                  var _valid1 = _errs11 === errors;
                  valid3 = valid3 || _valid1;
                  if (!valid3) {
                    const err2 = {
                      instancePath:
                        instancePath +
                        "/args/" +
                        key0.replace(/~/g, "~0").replace(/\//g, "~1"),
                      schemaPath:
                        "#/properties/args/additionalProperties/anyOf",
                      keyword: "anyOf",
                      params: {},
                      message: "must match a schema in anyOf",
                    };
                    if (vErrors === null) {
                      vErrors = [err2];
                    } else {
                      vErrors.push(err2);
                    }
                    errors++;
                    validate31.errors = vErrors;
                    return false;
                  } else {
                    errors = _errs9;
                    if (vErrors !== null) {
                      if (_errs9) {
                        vErrors.length = _errs9;
                      } else {
                        vErrors = null;
                      }
                    }
                  }
                  var valid2 = _errs8 === errors;
                  if (!valid2) {
                    break;
                  }
                }
              } else {
                validate31.errors = [
                  {
                    instancePath: instancePath + "/args",
                    schemaPath: "#/properties/args/type",
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
            if (data.returnType !== undefined) {
              let data3 = data.returnType;
              const _errs13 = errors;
              if (typeof data3 !== "string") {
                validate31.errors = [
                  {
                    instancePath: instancePath + "/returnType",
                    schemaPath: "#/properties/returnType/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              if (
                !(
                  data3 === "string" ||
                  data3 === "number" ||
                  data3 === "boolean" ||
                  data3 === "array" ||
                  data3 === "object" ||
                  data3 === "any" ||
                  data3 === "void"
                )
              ) {
                validate31.errors = [
                  {
                    instancePath: instancePath + "/returnType",
                    schemaPath: "#/properties/returnType/enum",
                    keyword: "enum",
                    params: {
                      allowedValues: schema44.properties.returnType.enum,
                    },
                    message: "must be equal to one of the allowed values",
                  },
                ];
                return false;
              }
              var valid1 = _errs13 === errors;
            } else {
              var valid1 = true;
            }
          }
        }
      }
    } else {
      validate31.errors = [
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
  validate31.errors = vErrors;
  evaluated0.props = props0;
  return errors === 0;
}
validate31.evaluated = { dynamicProps: true, dynamicItems: false };
function validate30(
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
  const evaluated0 = validate30.evaluated;
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
  if (typeof data !== "string") {
    const err0 = {
      instancePath,
      schemaPath: "#/oneOf/0/type",
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
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  const _errs4 = errors;
  if (errors === _errs4) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.path === undefined && (missing0 = "path")) {
        const err1 = {
          instancePath,
          schemaPath: "#/$defs/DataBinding/required",
          keyword: "required",
          params: { missingProperty: missing0 },
          message: "must have required property '" + missing0 + "'",
        };
        if (vErrors === null) {
          vErrors = [err1];
        } else {
          vErrors.push(err1);
        }
        errors++;
      } else {
        const _errs6 = errors;
        for (const key0 in data) {
          if (!(key0 === "path")) {
            const err2 = {
              instancePath,
              schemaPath: "#/$defs/DataBinding/additionalProperties",
              keyword: "additionalProperties",
              params: { additionalProperty: key0 },
              message: "must NOT have additional properties",
            };
            if (vErrors === null) {
              vErrors = [err2];
            } else {
              vErrors.push(err2);
            }
            errors++;
            break;
          }
        }
        if (_errs6 === errors) {
          if (data.path !== undefined) {
            if (typeof data.path !== "string") {
              const err3 = {
                instancePath: instancePath + "/path",
                schemaPath: "#/$defs/DataBinding/properties/path/type",
                keyword: "type",
                params: { type: "string" },
                message: "must be string",
              };
              if (vErrors === null) {
                vErrors = [err3];
              } else {
                vErrors.push(err3);
              }
              errors++;
            }
          }
        }
      }
    } else {
      const err4 = {
        instancePath,
        schemaPath: "#/$defs/DataBinding/type",
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
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      var props0 = true;
    }
    const _errs9 = errors;
    const _errs10 = errors;
    if (
      !validate31(data, {
        instancePath,
        parentData,
        parentDataProperty,
        rootData,
        dynamicAnchors,
      })
    ) {
      vErrors =
        vErrors === null
          ? validate31.errors
          : vErrors.concat(validate31.errors);
      errors = vErrors.length;
    } else {
      var props1 = validate31.evaluated.props;
    }
    var valid3 = _errs10 === errors;
    if (valid3) {
      const _errs11 = errors;
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.returnType !== undefined) {
          if ("string" !== data.returnType) {
            const err5 = {
              instancePath: instancePath + "/returnType",
              schemaPath: "#/oneOf/2/allOf/1/properties/returnType/const",
              keyword: "const",
              params: { allowedValue: "string" },
              message: "must be equal to constant",
            };
            if (vErrors === null) {
              vErrors = [err5];
            } else {
              vErrors.push(err5);
            }
            errors++;
          }
        }
      }
      var valid3 = _errs11 === errors;
      if (valid3) {
        if (props1 !== true) {
          props1 = props1 || {};
          props1.returnType = true;
        }
      }
    }
    var _valid0 = _errs9 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
        if (props0 !== true && props1 !== undefined) {
          if (props1 === true) {
            props0 = true;
          } else {
            props0 = props0 || {};
            Object.assign(props0, props1);
          }
        }
      }
    }
  }
  if (!valid0) {
    const err6 = {
      instancePath,
      schemaPath: "#/oneOf",
      keyword: "oneOf",
      params: { passingSchemas: passing0 },
      message: "must match exactly one schema in oneOf",
    };
    if (vErrors === null) {
      vErrors = [err6];
    } else {
      vErrors.push(err6);
    }
    errors++;
    validate30.errors = vErrors;
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
  validate30.errors = vErrors;
  evaluated0.props = props0;
  return errors === 0;
}
validate30.evaluated = { dynamicProps: true, dynamicItems: false };
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
      if (data.label !== undefined) {
        const _errs1 = errors;
        if (
          !validate30(data.label, {
            instancePath: instancePath + "/label",
            parentData: data,
            parentDataProperty: "label",
            rootData,
            dynamicAnchors,
          })
        ) {
          vErrors =
            vErrors === null
              ? validate30.errors
              : vErrors.concat(validate30.errors);
          errors = vErrors.length;
        }
        var valid0 = _errs1 === errors;
      } else {
        var valid0 = true;
      }
      if (valid0) {
        if (data.description !== undefined) {
          const _errs2 = errors;
          if (
            !validate30(data.description, {
              instancePath: instancePath + "/description",
              parentData: data,
              parentDataProperty: "description",
              rootData,
              dynamicAnchors,
            })
          ) {
            vErrors =
              vErrors === null
                ? validate30.errors
                : vErrors.concat(validate30.errors);
            errors = vErrors.length;
          }
          var valid0 = _errs2 === errors;
        } else {
          var valid0 = true;
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
  props: { label: true, description: true },
  dynamicProps: false,
  dynamicItems: false,
};
function validate28(
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
  const evaluated0 = validate28.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.id === undefined && (missing0 = "id")) {
        validate28.errors = [
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
        if (data.id !== undefined) {
          const _errs1 = errors;
          if (typeof data.id !== "string") {
            validate28.errors = [
              {
                instancePath: instancePath + "/id",
                schemaPath: "#/$defs/ComponentId/type",
                keyword: "type",
                params: { type: "string" },
                message: "must be string",
              },
            ];
            return false;
          }
          var valid0 = _errs1 === errors;
        } else {
          var valid0 = true;
        }
        if (valid0) {
          if (data.accessibility !== undefined) {
            const _errs4 = errors;
            if (
              !validate29(data.accessibility, {
                instancePath: instancePath + "/accessibility",
                parentData: data,
                parentDataProperty: "accessibility",
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
            var valid0 = _errs4 === errors;
          } else {
            var valid0 = true;
          }
        }
      }
    } else {
      validate28.errors = [
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
  validate28.errors = vErrors;
  return errors === 0;
}
validate28.evaluated = {
  props: { id: true, accessibility: true },
  dynamicProps: false,
  dynamicItems: false,
};
function validate26(
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
  const evaluated0 = validate26.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs1 = errors;
  if (
    !validate28(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate28.errors : vErrors.concat(validate28.errors);
    errors = vErrors.length;
  }
  var valid0 = _errs1 === errors;
  if (valid0) {
    const _errs2 = errors;
    const _errs3 = errors;
    if (errors === _errs3) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.weight !== undefined) {
          if (!(typeof data.weight == "number")) {
            validate26.errors = [
              {
                instancePath: instancePath + "/weight",
                schemaPath:
                  "#/$defs/CatalogComponentCommon/properties/weight/type",
                keyword: "type",
                params: { type: "number" },
                message: "must be number",
              },
            ];
            return false;
          }
        }
      } else {
        validate26.errors = [
          {
            instancePath,
            schemaPath: "#/$defs/CatalogComponentCommon/type",
            keyword: "type",
            params: { type: "object" },
            message: "must be object",
          },
        ];
        return false;
      }
    }
    var valid0 = _errs2 === errors;
    if (valid0) {
      const _errs7 = errors;
      if (errors === _errs7) {
        if (data && typeof data == "object" && !Array.isArray(data)) {
          let missing0;
          if (
            (data.component === undefined && (missing0 = "component")) ||
            (data.text === undefined && (missing0 = "text"))
          ) {
            validate26.errors = [
              {
                instancePath,
                schemaPath: "#/allOf/2/required",
                keyword: "required",
                params: { missingProperty: missing0 },
                message: "must have required property '" + missing0 + "'",
              },
            ];
            return false;
          } else {
            if (data.component !== undefined) {
              const _errs9 = errors;
              if ("Text" !== data.component) {
                validate26.errors = [
                  {
                    instancePath: instancePath + "/component",
                    schemaPath: "#/allOf/2/properties/component/const",
                    keyword: "const",
                    params: { allowedValue: "Text" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid3 = _errs9 === errors;
            } else {
              var valid3 = true;
            }
            if (valid3) {
              if (data.text !== undefined) {
                const _errs10 = errors;
                if (
                  !validate30(data.text, {
                    instancePath: instancePath + "/text",
                    parentData: data,
                    parentDataProperty: "text",
                    rootData,
                    dynamicAnchors,
                  })
                ) {
                  vErrors =
                    vErrors === null
                      ? validate30.errors
                      : vErrors.concat(validate30.errors);
                  errors = vErrors.length;
                }
                var valid3 = _errs10 === errors;
              } else {
                var valid3 = true;
              }
              if (valid3) {
                if (data.variant !== undefined) {
                  let data3 = data.variant;
                  const _errs11 = errors;
                  if (typeof data3 !== "string") {
                    validate26.errors = [
                      {
                        instancePath: instancePath + "/variant",
                        schemaPath: "#/allOf/2/properties/variant/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      },
                    ];
                    return false;
                  }
                  if (
                    !(
                      data3 === "h1" ||
                      data3 === "h2" ||
                      data3 === "h3" ||
                      data3 === "h4" ||
                      data3 === "h5" ||
                      data3 === "caption" ||
                      data3 === "body"
                    )
                  ) {
                    validate26.errors = [
                      {
                        instancePath: instancePath + "/variant",
                        schemaPath: "#/allOf/2/properties/variant/enum",
                        keyword: "enum",
                        params: {
                          allowedValues:
                            schema37.allOf[2].properties.variant.enum,
                        },
                        message: "must be equal to one of the allowed values",
                      },
                    ];
                    return false;
                  }
                  var valid3 = _errs11 === errors;
                } else {
                  var valid3 = true;
                }
              }
            }
          }
        } else {
          validate26.errors = [
            {
              instancePath,
              schemaPath: "#/allOf/2/type",
              keyword: "type",
              params: { type: "object" },
              message: "must be object",
            },
          ];
          return false;
        }
      }
      var valid0 = _errs7 === errors;
    }
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      for (const key0 in data) {
        if (
          key0 !== "component" &&
          key0 !== "text" &&
          key0 !== "variant" &&
          key0 !== "weight" &&
          key0 !== "id" &&
          key0 !== "accessibility"
        ) {
          validate26.errors = [
            {
              instancePath,
              schemaPath: "#/unevaluatedProperties",
              keyword: "unevaluatedProperties",
              params: { unevaluatedProperty: key0 },
              message: "must NOT have unevaluated properties",
            },
          ];
          return false;
          break;
        }
      }
    } else {
      validate26.errors = [
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
  validate26.errors = vErrors;
  return errors === 0;
}
validate26.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema69 = {
  type: "object",
  allOf: [
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentCommon",
    },
    { $ref: "#/$defs/CatalogComponentCommon" },
    {
      type: "object",
      properties: {
        component: { const: "Image" },
        url: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
          description: "The URL of the image to display.",
        },
        description: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
          description: "Accessibility text for the image.",
        },
        fit: {
          type: "string",
          description:
            "Specifies how the image should be resized to fit its container. This corresponds to the CSS 'object-fit' property.",
          enum: ["contain", "cover", "fill", "none", "scaleDown"],
          default: "fill",
        },
        variant: {
          type: "string",
          description: "A hint for the image size and style.",
          enum: [
            "icon",
            "avatar",
            "smallFeature",
            "mediumFeature",
            "largeFeature",
            "header",
          ],
          default: "mediumFeature",
        },
      },
      required: ["component", "url"],
    },
  ],
  unevaluatedProperties: false,
};
function validate82(
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
  const evaluated0 = validate82.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs1 = errors;
  if (
    !validate28(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate28.errors : vErrors.concat(validate28.errors);
    errors = vErrors.length;
  }
  var valid0 = _errs1 === errors;
  if (valid0) {
    const _errs2 = errors;
    const _errs3 = errors;
    if (errors === _errs3) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.weight !== undefined) {
          if (!(typeof data.weight == "number")) {
            validate82.errors = [
              {
                instancePath: instancePath + "/weight",
                schemaPath:
                  "#/$defs/CatalogComponentCommon/properties/weight/type",
                keyword: "type",
                params: { type: "number" },
                message: "must be number",
              },
            ];
            return false;
          }
        }
      } else {
        validate82.errors = [
          {
            instancePath,
            schemaPath: "#/$defs/CatalogComponentCommon/type",
            keyword: "type",
            params: { type: "object" },
            message: "must be object",
          },
        ];
        return false;
      }
    }
    var valid0 = _errs2 === errors;
    if (valid0) {
      const _errs7 = errors;
      if (errors === _errs7) {
        if (data && typeof data == "object" && !Array.isArray(data)) {
          let missing0;
          if (
            (data.component === undefined && (missing0 = "component")) ||
            (data.url === undefined && (missing0 = "url"))
          ) {
            validate82.errors = [
              {
                instancePath,
                schemaPath: "#/allOf/2/required",
                keyword: "required",
                params: { missingProperty: missing0 },
                message: "must have required property '" + missing0 + "'",
              },
            ];
            return false;
          } else {
            if (data.component !== undefined) {
              const _errs9 = errors;
              if ("Image" !== data.component) {
                validate82.errors = [
                  {
                    instancePath: instancePath + "/component",
                    schemaPath: "#/allOf/2/properties/component/const",
                    keyword: "const",
                    params: { allowedValue: "Image" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid3 = _errs9 === errors;
            } else {
              var valid3 = true;
            }
            if (valid3) {
              if (data.url !== undefined) {
                const _errs10 = errors;
                if (
                  !validate30(data.url, {
                    instancePath: instancePath + "/url",
                    parentData: data,
                    parentDataProperty: "url",
                    rootData,
                    dynamicAnchors,
                  })
                ) {
                  vErrors =
                    vErrors === null
                      ? validate30.errors
                      : vErrors.concat(validate30.errors);
                  errors = vErrors.length;
                }
                var valid3 = _errs10 === errors;
              } else {
                var valid3 = true;
              }
              if (valid3) {
                if (data.description !== undefined) {
                  const _errs11 = errors;
                  if (
                    !validate30(data.description, {
                      instancePath: instancePath + "/description",
                      parentData: data,
                      parentDataProperty: "description",
                      rootData,
                      dynamicAnchors,
                    })
                  ) {
                    vErrors =
                      vErrors === null
                        ? validate30.errors
                        : vErrors.concat(validate30.errors);
                    errors = vErrors.length;
                  }
                  var valid3 = _errs11 === errors;
                } else {
                  var valid3 = true;
                }
                if (valid3) {
                  if (data.fit !== undefined) {
                    let data4 = data.fit;
                    const _errs12 = errors;
                    if (typeof data4 !== "string") {
                      validate82.errors = [
                        {
                          instancePath: instancePath + "/fit",
                          schemaPath: "#/allOf/2/properties/fit/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                    if (
                      !(
                        data4 === "contain" ||
                        data4 === "cover" ||
                        data4 === "fill" ||
                        data4 === "none" ||
                        data4 === "scaleDown"
                      )
                    ) {
                      validate82.errors = [
                        {
                          instancePath: instancePath + "/fit",
                          schemaPath: "#/allOf/2/properties/fit/enum",
                          keyword: "enum",
                          params: {
                            allowedValues:
                              schema69.allOf[2].properties.fit.enum,
                          },
                          message: "must be equal to one of the allowed values",
                        },
                      ];
                      return false;
                    }
                    var valid3 = _errs12 === errors;
                  } else {
                    var valid3 = true;
                  }
                  if (valid3) {
                    if (data.variant !== undefined) {
                      let data5 = data.variant;
                      const _errs14 = errors;
                      if (typeof data5 !== "string") {
                        validate82.errors = [
                          {
                            instancePath: instancePath + "/variant",
                            schemaPath: "#/allOf/2/properties/variant/type",
                            keyword: "type",
                            params: { type: "string" },
                            message: "must be string",
                          },
                        ];
                        return false;
                      }
                      if (
                        !(
                          data5 === "icon" ||
                          data5 === "avatar" ||
                          data5 === "smallFeature" ||
                          data5 === "mediumFeature" ||
                          data5 === "largeFeature" ||
                          data5 === "header"
                        )
                      ) {
                        validate82.errors = [
                          {
                            instancePath: instancePath + "/variant",
                            schemaPath: "#/allOf/2/properties/variant/enum",
                            keyword: "enum",
                            params: {
                              allowedValues:
                                schema69.allOf[2].properties.variant.enum,
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
                  }
                }
              }
            }
          }
        } else {
          validate82.errors = [
            {
              instancePath,
              schemaPath: "#/allOf/2/type",
              keyword: "type",
              params: { type: "object" },
              message: "must be object",
            },
          ];
          return false;
        }
      }
      var valid0 = _errs7 === errors;
    }
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      for (const key0 in data) {
        if (
          key0 !== "component" &&
          key0 !== "url" &&
          key0 !== "description" &&
          key0 !== "fit" &&
          key0 !== "variant" &&
          key0 !== "weight" &&
          key0 !== "id" &&
          key0 !== "accessibility"
        ) {
          validate82.errors = [
            {
              instancePath,
              schemaPath: "#/unevaluatedProperties",
              keyword: "unevaluatedProperties",
              params: { unevaluatedProperty: key0 },
              message: "must NOT have unevaluated properties",
            },
          ];
          return false;
          break;
        }
      }
    } else {
      validate82.errors = [
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
  validate82.errors = vErrors;
  return errors === 0;
}
validate82.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema71 = {
  type: "object",
  allOf: [
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentCommon",
    },
    { $ref: "#/$defs/CatalogComponentCommon" },
    {
      type: "object",
      properties: {
        component: { const: "Icon" },
        name: {
          description: "The name of the icon to display.",
          oneOf: [
            {
              type: "string",
              enum: [
                "accountCircle",
                "add",
                "arrowBack",
                "arrowForward",
                "attachFile",
                "calendarToday",
                "call",
                "camera",
                "check",
                "close",
                "delete",
                "download",
                "edit",
                "event",
                "error",
                "fastForward",
                "favorite",
                "favoriteOff",
                "folder",
                "help",
                "home",
                "info",
                "locationOn",
                "lock",
                "lockOpen",
                "mail",
                "menu",
                "moreVert",
                "moreHoriz",
                "notificationsOff",
                "notifications",
                "pause",
                "payment",
                "person",
                "phone",
                "photo",
                "play",
                "print",
                "refresh",
                "rewind",
                "search",
                "send",
                "settings",
                "share",
                "shoppingCart",
                "skipNext",
                "skipPrevious",
                "star",
                "starHalf",
                "starOff",
                "stop",
                "upload",
                "visibility",
                "visibilityOff",
                "volumeDown",
                "volumeMute",
                "volumeOff",
                "volumeUp",
                "warning",
              ],
            },
            {
              type: "object",
              properties: { svgPath: { type: "string" } },
              required: ["svgPath"],
              additionalProperties: false,
            },
            {
              $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DataBinding",
            },
          ],
        },
      },
      required: ["component", "name"],
    },
  ],
  unevaluatedProperties: false,
};
function validate87(
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
  const evaluated0 = validate87.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs1 = errors;
  if (
    !validate28(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate28.errors : vErrors.concat(validate28.errors);
    errors = vErrors.length;
  }
  var valid0 = _errs1 === errors;
  if (valid0) {
    const _errs2 = errors;
    const _errs3 = errors;
    if (errors === _errs3) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.weight !== undefined) {
          if (!(typeof data.weight == "number")) {
            validate87.errors = [
              {
                instancePath: instancePath + "/weight",
                schemaPath:
                  "#/$defs/CatalogComponentCommon/properties/weight/type",
                keyword: "type",
                params: { type: "number" },
                message: "must be number",
              },
            ];
            return false;
          }
        }
      } else {
        validate87.errors = [
          {
            instancePath,
            schemaPath: "#/$defs/CatalogComponentCommon/type",
            keyword: "type",
            params: { type: "object" },
            message: "must be object",
          },
        ];
        return false;
      }
    }
    var valid0 = _errs2 === errors;
    if (valid0) {
      const _errs7 = errors;
      if (errors === _errs7) {
        if (data && typeof data == "object" && !Array.isArray(data)) {
          let missing0;
          if (
            (data.component === undefined && (missing0 = "component")) ||
            (data.name === undefined && (missing0 = "name"))
          ) {
            validate87.errors = [
              {
                instancePath,
                schemaPath: "#/allOf/2/required",
                keyword: "required",
                params: { missingProperty: missing0 },
                message: "must have required property '" + missing0 + "'",
              },
            ];
            return false;
          } else {
            if (data.component !== undefined) {
              const _errs9 = errors;
              if ("Icon" !== data.component) {
                validate87.errors = [
                  {
                    instancePath: instancePath + "/component",
                    schemaPath: "#/allOf/2/properties/component/const",
                    keyword: "const",
                    params: { allowedValue: "Icon" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid3 = _errs9 === errors;
            } else {
              var valid3 = true;
            }
            if (valid3) {
              if (data.name !== undefined) {
                let data2 = data.name;
                const _errs10 = errors;
                const _errs11 = errors;
                let valid4 = false;
                let passing0 = null;
                const _errs12 = errors;
                if (typeof data2 !== "string") {
                  const err0 = {
                    instancePath: instancePath + "/name",
                    schemaPath: "#/allOf/2/properties/name/oneOf/0/type",
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
                if (
                  !(
                    data2 === "accountCircle" ||
                    data2 === "add" ||
                    data2 === "arrowBack" ||
                    data2 === "arrowForward" ||
                    data2 === "attachFile" ||
                    data2 === "calendarToday" ||
                    data2 === "call" ||
                    data2 === "camera" ||
                    data2 === "check" ||
                    data2 === "close" ||
                    data2 === "delete" ||
                    data2 === "download" ||
                    data2 === "edit" ||
                    data2 === "event" ||
                    data2 === "error" ||
                    data2 === "fastForward" ||
                    data2 === "favorite" ||
                    data2 === "favoriteOff" ||
                    data2 === "folder" ||
                    data2 === "help" ||
                    data2 === "home" ||
                    data2 === "info" ||
                    data2 === "locationOn" ||
                    data2 === "lock" ||
                    data2 === "lockOpen" ||
                    data2 === "mail" ||
                    data2 === "menu" ||
                    data2 === "moreVert" ||
                    data2 === "moreHoriz" ||
                    data2 === "notificationsOff" ||
                    data2 === "notifications" ||
                    data2 === "pause" ||
                    data2 === "payment" ||
                    data2 === "person" ||
                    data2 === "phone" ||
                    data2 === "photo" ||
                    data2 === "play" ||
                    data2 === "print" ||
                    data2 === "refresh" ||
                    data2 === "rewind" ||
                    data2 === "search" ||
                    data2 === "send" ||
                    data2 === "settings" ||
                    data2 === "share" ||
                    data2 === "shoppingCart" ||
                    data2 === "skipNext" ||
                    data2 === "skipPrevious" ||
                    data2 === "star" ||
                    data2 === "starHalf" ||
                    data2 === "starOff" ||
                    data2 === "stop" ||
                    data2 === "upload" ||
                    data2 === "visibility" ||
                    data2 === "visibilityOff" ||
                    data2 === "volumeDown" ||
                    data2 === "volumeMute" ||
                    data2 === "volumeOff" ||
                    data2 === "volumeUp" ||
                    data2 === "warning"
                  )
                ) {
                  const err1 = {
                    instancePath: instancePath + "/name",
                    schemaPath: "#/allOf/2/properties/name/oneOf/0/enum",
                    keyword: "enum",
                    params: {
                      allowedValues:
                        schema71.allOf[2].properties.name.oneOf[0].enum,
                    },
                    message: "must be equal to one of the allowed values",
                  };
                  if (vErrors === null) {
                    vErrors = [err1];
                  } else {
                    vErrors.push(err1);
                  }
                  errors++;
                }
                var _valid0 = _errs12 === errors;
                if (_valid0) {
                  valid4 = true;
                  passing0 = 0;
                }
                const _errs14 = errors;
                if (errors === _errs14) {
                  if (
                    data2 &&
                    typeof data2 == "object" &&
                    !Array.isArray(data2)
                  ) {
                    let missing1;
                    if (data2.svgPath === undefined && (missing1 = "svgPath")) {
                      const err2 = {
                        instancePath: instancePath + "/name",
                        schemaPath:
                          "#/allOf/2/properties/name/oneOf/1/required",
                        keyword: "required",
                        params: { missingProperty: missing1 },
                        message:
                          "must have required property '" + missing1 + "'",
                      };
                      if (vErrors === null) {
                        vErrors = [err2];
                      } else {
                        vErrors.push(err2);
                      }
                      errors++;
                    } else {
                      const _errs16 = errors;
                      for (const key0 in data2) {
                        if (!(key0 === "svgPath")) {
                          const err3 = {
                            instancePath: instancePath + "/name",
                            schemaPath:
                              "#/allOf/2/properties/name/oneOf/1/additionalProperties",
                            keyword: "additionalProperties",
                            params: { additionalProperty: key0 },
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
                      if (_errs16 === errors) {
                        if (data2.svgPath !== undefined) {
                          if (typeof data2.svgPath !== "string") {
                            const err4 = {
                              instancePath: instancePath + "/name/svgPath",
                              schemaPath:
                                "#/allOf/2/properties/name/oneOf/1/properties/svgPath/type",
                              keyword: "type",
                              params: { type: "string" },
                              message: "must be string",
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
                  } else {
                    const err5 = {
                      instancePath: instancePath + "/name",
                      schemaPath: "#/allOf/2/properties/name/oneOf/1/type",
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
                var _valid0 = _errs14 === errors;
                if (_valid0 && valid4) {
                  valid4 = false;
                  passing0 = [passing0, 1];
                } else {
                  if (_valid0) {
                    valid4 = true;
                    passing0 = 1;
                    var props0 = true;
                  }
                  const _errs19 = errors;
                  const _errs20 = errors;
                  if (errors === _errs20) {
                    if (
                      data2 &&
                      typeof data2 == "object" &&
                      !Array.isArray(data2)
                    ) {
                      let missing2;
                      if (data2.path === undefined && (missing2 = "path")) {
                        const err6 = {
                          instancePath: instancePath + "/name",
                          schemaPath:
                            "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DataBinding/required",
                          keyword: "required",
                          params: { missingProperty: missing2 },
                          message:
                            "must have required property '" + missing2 + "'",
                        };
                        if (vErrors === null) {
                          vErrors = [err6];
                        } else {
                          vErrors.push(err6);
                        }
                        errors++;
                      } else {
                        const _errs22 = errors;
                        for (const key1 in data2) {
                          if (!(key1 === "path")) {
                            const err7 = {
                              instancePath: instancePath + "/name",
                              schemaPath:
                                "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DataBinding/additionalProperties",
                              keyword: "additionalProperties",
                              params: { additionalProperty: key1 },
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
                        if (_errs22 === errors) {
                          if (data2.path !== undefined) {
                            if (typeof data2.path !== "string") {
                              const err8 = {
                                instancePath: instancePath + "/name/path",
                                schemaPath:
                                  "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DataBinding/properties/path/type",
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
                          }
                        }
                      }
                    } else {
                      const err9 = {
                        instancePath: instancePath + "/name",
                        schemaPath:
                          "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DataBinding/type",
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
                  var _valid0 = _errs19 === errors;
                  if (_valid0 && valid4) {
                    valid4 = false;
                    passing0 = [passing0, 2];
                  } else {
                    if (_valid0) {
                      valid4 = true;
                      passing0 = 2;
                      if (props0 !== true) {
                        props0 = true;
                      }
                    }
                  }
                }
                if (!valid4) {
                  const err10 = {
                    instancePath: instancePath + "/name",
                    schemaPath: "#/allOf/2/properties/name/oneOf",
                    keyword: "oneOf",
                    params: { passingSchemas: passing0 },
                    message: "must match exactly one schema in oneOf",
                  };
                  if (vErrors === null) {
                    vErrors = [err10];
                  } else {
                    vErrors.push(err10);
                  }
                  errors++;
                  validate87.errors = vErrors;
                  return false;
                } else {
                  errors = _errs11;
                  if (vErrors !== null) {
                    if (_errs11) {
                      vErrors.length = _errs11;
                    } else {
                      vErrors = null;
                    }
                  }
                }
                var valid3 = _errs10 === errors;
              } else {
                var valid3 = true;
              }
            }
          }
        } else {
          validate87.errors = [
            {
              instancePath,
              schemaPath: "#/allOf/2/type",
              keyword: "type",
              params: { type: "object" },
              message: "must be object",
            },
          ];
          return false;
        }
      }
      var valid0 = _errs7 === errors;
    }
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      for (const key2 in data) {
        if (
          key2 !== "component" &&
          key2 !== "name" &&
          key2 !== "weight" &&
          key2 !== "id" &&
          key2 !== "accessibility"
        ) {
          validate87.errors = [
            {
              instancePath,
              schemaPath: "#/unevaluatedProperties",
              keyword: "unevaluatedProperties",
              params: { unevaluatedProperty: key2 },
              message: "must NOT have unevaluated properties",
            },
          ];
          return false;
          break;
        }
      }
    } else {
      validate87.errors = [
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
  validate87.errors = vErrors;
  return errors === 0;
}
validate87.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema74 = {
  type: "object",
  allOf: [
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentCommon",
    },
    { $ref: "#/$defs/CatalogComponentCommon" },
    {
      type: "object",
      properties: {
        component: { const: "Video" },
        url: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
          description: "The URL of the video to display.",
        },
      },
      required: ["component", "url"],
    },
  ],
  unevaluatedProperties: false,
};
function validate90(
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
  const evaluated0 = validate90.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs1 = errors;
  if (
    !validate28(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate28.errors : vErrors.concat(validate28.errors);
    errors = vErrors.length;
  }
  var valid0 = _errs1 === errors;
  if (valid0) {
    const _errs2 = errors;
    const _errs3 = errors;
    if (errors === _errs3) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.weight !== undefined) {
          if (!(typeof data.weight == "number")) {
            validate90.errors = [
              {
                instancePath: instancePath + "/weight",
                schemaPath:
                  "#/$defs/CatalogComponentCommon/properties/weight/type",
                keyword: "type",
                params: { type: "number" },
                message: "must be number",
              },
            ];
            return false;
          }
        }
      } else {
        validate90.errors = [
          {
            instancePath,
            schemaPath: "#/$defs/CatalogComponentCommon/type",
            keyword: "type",
            params: { type: "object" },
            message: "must be object",
          },
        ];
        return false;
      }
    }
    var valid0 = _errs2 === errors;
    if (valid0) {
      const _errs7 = errors;
      if (errors === _errs7) {
        if (data && typeof data == "object" && !Array.isArray(data)) {
          let missing0;
          if (
            (data.component === undefined && (missing0 = "component")) ||
            (data.url === undefined && (missing0 = "url"))
          ) {
            validate90.errors = [
              {
                instancePath,
                schemaPath: "#/allOf/2/required",
                keyword: "required",
                params: { missingProperty: missing0 },
                message: "must have required property '" + missing0 + "'",
              },
            ];
            return false;
          } else {
            if (data.component !== undefined) {
              const _errs9 = errors;
              if ("Video" !== data.component) {
                validate90.errors = [
                  {
                    instancePath: instancePath + "/component",
                    schemaPath: "#/allOf/2/properties/component/const",
                    keyword: "const",
                    params: { allowedValue: "Video" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid3 = _errs9 === errors;
            } else {
              var valid3 = true;
            }
            if (valid3) {
              if (data.url !== undefined) {
                const _errs10 = errors;
                if (
                  !validate30(data.url, {
                    instancePath: instancePath + "/url",
                    parentData: data,
                    parentDataProperty: "url",
                    rootData,
                    dynamicAnchors,
                  })
                ) {
                  vErrors =
                    vErrors === null
                      ? validate30.errors
                      : vErrors.concat(validate30.errors);
                  errors = vErrors.length;
                }
                var valid3 = _errs10 === errors;
              } else {
                var valid3 = true;
              }
            }
          }
        } else {
          validate90.errors = [
            {
              instancePath,
              schemaPath: "#/allOf/2/type",
              keyword: "type",
              params: { type: "object" },
              message: "must be object",
            },
          ];
          return false;
        }
      }
      var valid0 = _errs7 === errors;
    }
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      for (const key0 in data) {
        if (
          key0 !== "component" &&
          key0 !== "url" &&
          key0 !== "weight" &&
          key0 !== "id" &&
          key0 !== "accessibility"
        ) {
          validate90.errors = [
            {
              instancePath,
              schemaPath: "#/unevaluatedProperties",
              keyword: "unevaluatedProperties",
              params: { unevaluatedProperty: key0 },
              message: "must NOT have unevaluated properties",
            },
          ];
          return false;
          break;
        }
      }
    } else {
      validate90.errors = [
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
  validate90.errors = vErrors;
  return errors === 0;
}
validate90.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema76 = {
  type: "object",
  allOf: [
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentCommon",
    },
    { $ref: "#/$defs/CatalogComponentCommon" },
    {
      type: "object",
      properties: {
        component: { const: "AudioPlayer" },
        url: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
          description: "The URL of the audio to be played.",
        },
        description: {
          description:
            "A description of the audio, such as a title or summary.",
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
        },
      },
      required: ["component", "url"],
    },
  ],
  unevaluatedProperties: false,
};
function validate94(
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
  const evaluated0 = validate94.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs1 = errors;
  if (
    !validate28(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate28.errors : vErrors.concat(validate28.errors);
    errors = vErrors.length;
  }
  var valid0 = _errs1 === errors;
  if (valid0) {
    const _errs2 = errors;
    const _errs3 = errors;
    if (errors === _errs3) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.weight !== undefined) {
          if (!(typeof data.weight == "number")) {
            validate94.errors = [
              {
                instancePath: instancePath + "/weight",
                schemaPath:
                  "#/$defs/CatalogComponentCommon/properties/weight/type",
                keyword: "type",
                params: { type: "number" },
                message: "must be number",
              },
            ];
            return false;
          }
        }
      } else {
        validate94.errors = [
          {
            instancePath,
            schemaPath: "#/$defs/CatalogComponentCommon/type",
            keyword: "type",
            params: { type: "object" },
            message: "must be object",
          },
        ];
        return false;
      }
    }
    var valid0 = _errs2 === errors;
    if (valid0) {
      const _errs7 = errors;
      if (errors === _errs7) {
        if (data && typeof data == "object" && !Array.isArray(data)) {
          let missing0;
          if (
            (data.component === undefined && (missing0 = "component")) ||
            (data.url === undefined && (missing0 = "url"))
          ) {
            validate94.errors = [
              {
                instancePath,
                schemaPath: "#/allOf/2/required",
                keyword: "required",
                params: { missingProperty: missing0 },
                message: "must have required property '" + missing0 + "'",
              },
            ];
            return false;
          } else {
            if (data.component !== undefined) {
              const _errs9 = errors;
              if ("AudioPlayer" !== data.component) {
                validate94.errors = [
                  {
                    instancePath: instancePath + "/component",
                    schemaPath: "#/allOf/2/properties/component/const",
                    keyword: "const",
                    params: { allowedValue: "AudioPlayer" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid3 = _errs9 === errors;
            } else {
              var valid3 = true;
            }
            if (valid3) {
              if (data.url !== undefined) {
                const _errs10 = errors;
                if (
                  !validate30(data.url, {
                    instancePath: instancePath + "/url",
                    parentData: data,
                    parentDataProperty: "url",
                    rootData,
                    dynamicAnchors,
                  })
                ) {
                  vErrors =
                    vErrors === null
                      ? validate30.errors
                      : vErrors.concat(validate30.errors);
                  errors = vErrors.length;
                }
                var valid3 = _errs10 === errors;
              } else {
                var valid3 = true;
              }
              if (valid3) {
                if (data.description !== undefined) {
                  const _errs11 = errors;
                  if (
                    !validate30(data.description, {
                      instancePath: instancePath + "/description",
                      parentData: data,
                      parentDataProperty: "description",
                      rootData,
                      dynamicAnchors,
                    })
                  ) {
                    vErrors =
                      vErrors === null
                        ? validate30.errors
                        : vErrors.concat(validate30.errors);
                    errors = vErrors.length;
                  }
                  var valid3 = _errs11 === errors;
                } else {
                  var valid3 = true;
                }
              }
            }
          }
        } else {
          validate94.errors = [
            {
              instancePath,
              schemaPath: "#/allOf/2/type",
              keyword: "type",
              params: { type: "object" },
              message: "must be object",
            },
          ];
          return false;
        }
      }
      var valid0 = _errs7 === errors;
    }
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      for (const key0 in data) {
        if (
          key0 !== "component" &&
          key0 !== "url" &&
          key0 !== "description" &&
          key0 !== "weight" &&
          key0 !== "id" &&
          key0 !== "accessibility"
        ) {
          validate94.errors = [
            {
              instancePath,
              schemaPath: "#/unevaluatedProperties",
              keyword: "unevaluatedProperties",
              params: { unevaluatedProperty: key0 },
              message: "must NOT have unevaluated properties",
            },
          ];
          return false;
          break;
        }
      }
    } else {
      validate94.errors = [
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
  validate94.errors = vErrors;
  return errors === 0;
}
validate94.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema78 = {
  type: "object",
  allOf: [
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentCommon",
    },
    { $ref: "#/$defs/CatalogComponentCommon" },
    {
      type: "object",
      description:
        "A layout component that arranges its children horizontally. To create a grid layout, nest Columns within this Row.",
      properties: {
        component: { const: "Row" },
        children: {
          description:
            "Defines the children. Use an array of strings for a fixed set of children, or a template object to generate children from a data list. Children cannot be defined inline, they must be referred to by ID.",
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        justify: {
          type: "string",
          description:
            "Defines the arrangement of children along the main axis (horizontally). Use 'spaceBetween' to push items to the edges, or 'start'/'end'/'center' to pack them together.",
          enum: [
            "center",
            "end",
            "spaceAround",
            "spaceBetween",
            "spaceEvenly",
            "start",
            "stretch",
          ],
          default: "start",
        },
        align: {
          type: "string",
          description:
            "Defines the alignment of children along the cross axis (vertically). This is similar to the CSS 'align-items' property, but uses camelCase values (e.g., 'start').",
          enum: ["start", "center", "end", "stretch"],
          default: "stretch",
        },
      },
      required: ["component", "children"],
    },
  ],
  unevaluatedProperties: false,
};
const schema80 = {
  oneOf: [
    {
      type: "array",
      items: { $ref: "#/$defs/ComponentId" },
      description: "A static list of child component IDs.",
    },
    {
      type: "object",
      description:
        "A template for generating a dynamic list of children from a data model list. The `componentId` is the component to use as a template.",
      properties: {
        componentId: { $ref: "#/$defs/ComponentId" },
        path: {
          type: "string",
          description:
            "The path to the list of component property objects in the data model.",
        },
      },
      required: ["componentId", "path"],
      additionalProperties: false,
    },
  ],
};
function validate101(
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
  const evaluated0 = validate101.evaluated;
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
    if (Array.isArray(data)) {
      var valid1 = true;
      const len0 = data.length;
      for (let i0 = 0; i0 < len0; i0++) {
        const _errs3 = errors;
        if (typeof data[i0] !== "string") {
          const err0 = {
            instancePath: instancePath + "/" + i0,
            schemaPath: "#/$defs/ComponentId/type",
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
        var valid1 = _errs3 === errors;
        if (!valid1) {
          break;
        }
      }
    } else {
      const err1 = {
        instancePath,
        schemaPath: "#/oneOf/0/type",
        keyword: "type",
        params: { type: "array" },
        message: "must be array",
      };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
    var items0 = true;
  }
  const _errs6 = errors;
  if (errors === _errs6) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (
        (data.componentId === undefined && (missing0 = "componentId")) ||
        (data.path === undefined && (missing0 = "path"))
      ) {
        const err2 = {
          instancePath,
          schemaPath: "#/oneOf/1/required",
          keyword: "required",
          params: { missingProperty: missing0 },
          message: "must have required property '" + missing0 + "'",
        };
        if (vErrors === null) {
          vErrors = [err2];
        } else {
          vErrors.push(err2);
        }
        errors++;
      } else {
        const _errs8 = errors;
        for (const key0 in data) {
          if (!(key0 === "componentId" || key0 === "path")) {
            const err3 = {
              instancePath,
              schemaPath: "#/oneOf/1/additionalProperties",
              keyword: "additionalProperties",
              params: { additionalProperty: key0 },
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
        if (_errs8 === errors) {
          if (data.componentId !== undefined) {
            const _errs9 = errors;
            if (typeof data.componentId !== "string") {
              const err4 = {
                instancePath: instancePath + "/componentId",
                schemaPath: "#/$defs/ComponentId/type",
                keyword: "type",
                params: { type: "string" },
                message: "must be string",
              };
              if (vErrors === null) {
                vErrors = [err4];
              } else {
                vErrors.push(err4);
              }
              errors++;
            }
            var valid3 = _errs9 === errors;
          } else {
            var valid3 = true;
          }
          if (valid3) {
            if (data.path !== undefined) {
              const _errs12 = errors;
              if (typeof data.path !== "string") {
                const err5 = {
                  instancePath: instancePath + "/path",
                  schemaPath: "#/oneOf/1/properties/path/type",
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
              var valid3 = _errs12 === errors;
            } else {
              var valid3 = true;
            }
          }
        }
      }
    } else {
      const err6 = {
        instancePath,
        schemaPath: "#/oneOf/1/type",
        keyword: "type",
        params: { type: "object" },
        message: "must be object",
      };
      if (vErrors === null) {
        vErrors = [err6];
      } else {
        vErrors.push(err6);
      }
      errors++;
    }
  }
  var _valid0 = _errs6 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      var props0 = true;
    }
  }
  if (!valid0) {
    const err7 = {
      instancePath,
      schemaPath: "#/oneOf",
      keyword: "oneOf",
      params: { passingSchemas: passing0 },
      message: "must match exactly one schema in oneOf",
    };
    if (vErrors === null) {
      vErrors = [err7];
    } else {
      vErrors.push(err7);
    }
    errors++;
    validate101.errors = vErrors;
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
  validate101.errors = vErrors;
  evaluated0.props = props0;
  evaluated0.items = items0;
  return errors === 0;
}
validate101.evaluated = { dynamicProps: true, dynamicItems: true };
function validate99(
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
  const evaluated0 = validate99.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs1 = errors;
  if (
    !validate28(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate28.errors : vErrors.concat(validate28.errors);
    errors = vErrors.length;
  }
  var valid0 = _errs1 === errors;
  if (valid0) {
    const _errs2 = errors;
    const _errs3 = errors;
    if (errors === _errs3) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.weight !== undefined) {
          if (!(typeof data.weight == "number")) {
            validate99.errors = [
              {
                instancePath: instancePath + "/weight",
                schemaPath:
                  "#/$defs/CatalogComponentCommon/properties/weight/type",
                keyword: "type",
                params: { type: "number" },
                message: "must be number",
              },
            ];
            return false;
          }
        }
      } else {
        validate99.errors = [
          {
            instancePath,
            schemaPath: "#/$defs/CatalogComponentCommon/type",
            keyword: "type",
            params: { type: "object" },
            message: "must be object",
          },
        ];
        return false;
      }
    }
    var valid0 = _errs2 === errors;
    if (valid0) {
      const _errs7 = errors;
      if (errors === _errs7) {
        if (data && typeof data == "object" && !Array.isArray(data)) {
          let missing0;
          if (
            (data.component === undefined && (missing0 = "component")) ||
            (data.children === undefined && (missing0 = "children"))
          ) {
            validate99.errors = [
              {
                instancePath,
                schemaPath: "#/allOf/2/required",
                keyword: "required",
                params: { missingProperty: missing0 },
                message: "must have required property '" + missing0 + "'",
              },
            ];
            return false;
          } else {
            if (data.component !== undefined) {
              const _errs9 = errors;
              if ("Row" !== data.component) {
                validate99.errors = [
                  {
                    instancePath: instancePath + "/component",
                    schemaPath: "#/allOf/2/properties/component/const",
                    keyword: "const",
                    params: { allowedValue: "Row" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid3 = _errs9 === errors;
            } else {
              var valid3 = true;
            }
            if (valid3) {
              if (data.children !== undefined) {
                const _errs10 = errors;
                if (
                  !validate101(data.children, {
                    instancePath: instancePath + "/children",
                    parentData: data,
                    parentDataProperty: "children",
                    rootData,
                    dynamicAnchors,
                  })
                ) {
                  vErrors =
                    vErrors === null
                      ? validate101.errors
                      : vErrors.concat(validate101.errors);
                  errors = vErrors.length;
                }
                var valid3 = _errs10 === errors;
              } else {
                var valid3 = true;
              }
              if (valid3) {
                if (data.justify !== undefined) {
                  let data3 = data.justify;
                  const _errs11 = errors;
                  if (typeof data3 !== "string") {
                    validate99.errors = [
                      {
                        instancePath: instancePath + "/justify",
                        schemaPath: "#/allOf/2/properties/justify/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      },
                    ];
                    return false;
                  }
                  if (
                    !(
                      data3 === "center" ||
                      data3 === "end" ||
                      data3 === "spaceAround" ||
                      data3 === "spaceBetween" ||
                      data3 === "spaceEvenly" ||
                      data3 === "start" ||
                      data3 === "stretch"
                    )
                  ) {
                    validate99.errors = [
                      {
                        instancePath: instancePath + "/justify",
                        schemaPath: "#/allOf/2/properties/justify/enum",
                        keyword: "enum",
                        params: {
                          allowedValues:
                            schema78.allOf[2].properties.justify.enum,
                        },
                        message: "must be equal to one of the allowed values",
                      },
                    ];
                    return false;
                  }
                  var valid3 = _errs11 === errors;
                } else {
                  var valid3 = true;
                }
                if (valid3) {
                  if (data.align !== undefined) {
                    let data4 = data.align;
                    const _errs13 = errors;
                    if (typeof data4 !== "string") {
                      validate99.errors = [
                        {
                          instancePath: instancePath + "/align",
                          schemaPath: "#/allOf/2/properties/align/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                    if (
                      !(
                        data4 === "start" ||
                        data4 === "center" ||
                        data4 === "end" ||
                        data4 === "stretch"
                      )
                    ) {
                      validate99.errors = [
                        {
                          instancePath: instancePath + "/align",
                          schemaPath: "#/allOf/2/properties/align/enum",
                          keyword: "enum",
                          params: {
                            allowedValues:
                              schema78.allOf[2].properties.align.enum,
                          },
                          message: "must be equal to one of the allowed values",
                        },
                      ];
                      return false;
                    }
                    var valid3 = _errs13 === errors;
                  } else {
                    var valid3 = true;
                  }
                }
              }
            }
          }
        } else {
          validate99.errors = [
            {
              instancePath,
              schemaPath: "#/allOf/2/type",
              keyword: "type",
              params: { type: "object" },
              message: "must be object",
            },
          ];
          return false;
        }
      }
      var valid0 = _errs7 === errors;
    }
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      for (const key0 in data) {
        if (
          key0 !== "component" &&
          key0 !== "children" &&
          key0 !== "justify" &&
          key0 !== "align" &&
          key0 !== "weight" &&
          key0 !== "id" &&
          key0 !== "accessibility"
        ) {
          validate99.errors = [
            {
              instancePath,
              schemaPath: "#/unevaluatedProperties",
              keyword: "unevaluatedProperties",
              params: { unevaluatedProperty: key0 },
              message: "must NOT have unevaluated properties",
            },
          ];
          return false;
          break;
        }
      }
    } else {
      validate99.errors = [
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
  validate99.errors = vErrors;
  return errors === 0;
}
validate99.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema83 = {
  type: "object",
  allOf: [
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentCommon",
    },
    { $ref: "#/$defs/CatalogComponentCommon" },
    {
      type: "object",
      description:
        "A layout component that arranges its children vertically. To create a grid layout, nest Rows within this Column.",
      properties: {
        component: { const: "Column" },
        children: {
          description:
            "Defines the children. Use an array of strings for a fixed set of children, or a template object to generate children from a data list. Children cannot be defined inline, they must be referred to by ID.",
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        justify: {
          type: "string",
          description:
            "Defines the arrangement of children along the main axis (vertically). Use 'spaceBetween' to push items to the edges (e.g. header at top, footer at bottom), or 'start'/'end'/'center' to pack them together.",
          enum: [
            "start",
            "center",
            "end",
            "spaceBetween",
            "spaceAround",
            "spaceEvenly",
            "stretch",
          ],
          default: "start",
        },
        align: {
          type: "string",
          description:
            "Defines the alignment of children along the cross axis (horizontally). This is similar to the CSS 'align-items' property.",
          enum: ["center", "end", "start", "stretch"],
          default: "stretch",
        },
      },
      required: ["component", "children"],
    },
  ],
  unevaluatedProperties: false,
};
function validate104(
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
  const evaluated0 = validate104.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs1 = errors;
  if (
    !validate28(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate28.errors : vErrors.concat(validate28.errors);
    errors = vErrors.length;
  }
  var valid0 = _errs1 === errors;
  if (valid0) {
    const _errs2 = errors;
    const _errs3 = errors;
    if (errors === _errs3) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.weight !== undefined) {
          if (!(typeof data.weight == "number")) {
            validate104.errors = [
              {
                instancePath: instancePath + "/weight",
                schemaPath:
                  "#/$defs/CatalogComponentCommon/properties/weight/type",
                keyword: "type",
                params: { type: "number" },
                message: "must be number",
              },
            ];
            return false;
          }
        }
      } else {
        validate104.errors = [
          {
            instancePath,
            schemaPath: "#/$defs/CatalogComponentCommon/type",
            keyword: "type",
            params: { type: "object" },
            message: "must be object",
          },
        ];
        return false;
      }
    }
    var valid0 = _errs2 === errors;
    if (valid0) {
      const _errs7 = errors;
      if (errors === _errs7) {
        if (data && typeof data == "object" && !Array.isArray(data)) {
          let missing0;
          if (
            (data.component === undefined && (missing0 = "component")) ||
            (data.children === undefined && (missing0 = "children"))
          ) {
            validate104.errors = [
              {
                instancePath,
                schemaPath: "#/allOf/2/required",
                keyword: "required",
                params: { missingProperty: missing0 },
                message: "must have required property '" + missing0 + "'",
              },
            ];
            return false;
          } else {
            if (data.component !== undefined) {
              const _errs9 = errors;
              if ("Column" !== data.component) {
                validate104.errors = [
                  {
                    instancePath: instancePath + "/component",
                    schemaPath: "#/allOf/2/properties/component/const",
                    keyword: "const",
                    params: { allowedValue: "Column" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid3 = _errs9 === errors;
            } else {
              var valid3 = true;
            }
            if (valid3) {
              if (data.children !== undefined) {
                const _errs10 = errors;
                if (
                  !validate101(data.children, {
                    instancePath: instancePath + "/children",
                    parentData: data,
                    parentDataProperty: "children",
                    rootData,
                    dynamicAnchors,
                  })
                ) {
                  vErrors =
                    vErrors === null
                      ? validate101.errors
                      : vErrors.concat(validate101.errors);
                  errors = vErrors.length;
                }
                var valid3 = _errs10 === errors;
              } else {
                var valid3 = true;
              }
              if (valid3) {
                if (data.justify !== undefined) {
                  let data3 = data.justify;
                  const _errs11 = errors;
                  if (typeof data3 !== "string") {
                    validate104.errors = [
                      {
                        instancePath: instancePath + "/justify",
                        schemaPath: "#/allOf/2/properties/justify/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      },
                    ];
                    return false;
                  }
                  if (
                    !(
                      data3 === "start" ||
                      data3 === "center" ||
                      data3 === "end" ||
                      data3 === "spaceBetween" ||
                      data3 === "spaceAround" ||
                      data3 === "spaceEvenly" ||
                      data3 === "stretch"
                    )
                  ) {
                    validate104.errors = [
                      {
                        instancePath: instancePath + "/justify",
                        schemaPath: "#/allOf/2/properties/justify/enum",
                        keyword: "enum",
                        params: {
                          allowedValues:
                            schema83.allOf[2].properties.justify.enum,
                        },
                        message: "must be equal to one of the allowed values",
                      },
                    ];
                    return false;
                  }
                  var valid3 = _errs11 === errors;
                } else {
                  var valid3 = true;
                }
                if (valid3) {
                  if (data.align !== undefined) {
                    let data4 = data.align;
                    const _errs13 = errors;
                    if (typeof data4 !== "string") {
                      validate104.errors = [
                        {
                          instancePath: instancePath + "/align",
                          schemaPath: "#/allOf/2/properties/align/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                    if (
                      !(
                        data4 === "center" ||
                        data4 === "end" ||
                        data4 === "start" ||
                        data4 === "stretch"
                      )
                    ) {
                      validate104.errors = [
                        {
                          instancePath: instancePath + "/align",
                          schemaPath: "#/allOf/2/properties/align/enum",
                          keyword: "enum",
                          params: {
                            allowedValues:
                              schema83.allOf[2].properties.align.enum,
                          },
                          message: "must be equal to one of the allowed values",
                        },
                      ];
                      return false;
                    }
                    var valid3 = _errs13 === errors;
                  } else {
                    var valid3 = true;
                  }
                }
              }
            }
          }
        } else {
          validate104.errors = [
            {
              instancePath,
              schemaPath: "#/allOf/2/type",
              keyword: "type",
              params: { type: "object" },
              message: "must be object",
            },
          ];
          return false;
        }
      }
      var valid0 = _errs7 === errors;
    }
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      for (const key0 in data) {
        if (
          key0 !== "component" &&
          key0 !== "children" &&
          key0 !== "justify" &&
          key0 !== "align" &&
          key0 !== "weight" &&
          key0 !== "id" &&
          key0 !== "accessibility"
        ) {
          validate104.errors = [
            {
              instancePath,
              schemaPath: "#/unevaluatedProperties",
              keyword: "unevaluatedProperties",
              params: { unevaluatedProperty: key0 },
              message: "must NOT have unevaluated properties",
            },
          ];
          return false;
          break;
        }
      }
    } else {
      validate104.errors = [
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
  validate104.errors = vErrors;
  return errors === 0;
}
validate104.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema85 = {
  type: "object",
  allOf: [
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentCommon",
    },
    { $ref: "#/$defs/CatalogComponentCommon" },
    {
      type: "object",
      properties: {
        component: { const: "List" },
        children: {
          description:
            "Defines the children. Use an array of strings for a fixed set of children, or a template object to generate children from a data list.",
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        direction: {
          type: "string",
          description: "The direction in which the list items are laid out.",
          enum: ["vertical", "horizontal"],
          default: "vertical",
        },
        align: {
          type: "string",
          description:
            "Defines the alignment of children along the cross axis.",
          enum: ["start", "center", "end", "stretch"],
          default: "stretch",
        },
      },
      required: ["component", "children"],
    },
  ],
  unevaluatedProperties: false,
};
function validate108(
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
  const evaluated0 = validate108.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs1 = errors;
  if (
    !validate28(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate28.errors : vErrors.concat(validate28.errors);
    errors = vErrors.length;
  }
  var valid0 = _errs1 === errors;
  if (valid0) {
    const _errs2 = errors;
    const _errs3 = errors;
    if (errors === _errs3) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.weight !== undefined) {
          if (!(typeof data.weight == "number")) {
            validate108.errors = [
              {
                instancePath: instancePath + "/weight",
                schemaPath:
                  "#/$defs/CatalogComponentCommon/properties/weight/type",
                keyword: "type",
                params: { type: "number" },
                message: "must be number",
              },
            ];
            return false;
          }
        }
      } else {
        validate108.errors = [
          {
            instancePath,
            schemaPath: "#/$defs/CatalogComponentCommon/type",
            keyword: "type",
            params: { type: "object" },
            message: "must be object",
          },
        ];
        return false;
      }
    }
    var valid0 = _errs2 === errors;
    if (valid0) {
      const _errs7 = errors;
      if (errors === _errs7) {
        if (data && typeof data == "object" && !Array.isArray(data)) {
          let missing0;
          if (
            (data.component === undefined && (missing0 = "component")) ||
            (data.children === undefined && (missing0 = "children"))
          ) {
            validate108.errors = [
              {
                instancePath,
                schemaPath: "#/allOf/2/required",
                keyword: "required",
                params: { missingProperty: missing0 },
                message: "must have required property '" + missing0 + "'",
              },
            ];
            return false;
          } else {
            if (data.component !== undefined) {
              const _errs9 = errors;
              if ("List" !== data.component) {
                validate108.errors = [
                  {
                    instancePath: instancePath + "/component",
                    schemaPath: "#/allOf/2/properties/component/const",
                    keyword: "const",
                    params: { allowedValue: "List" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid3 = _errs9 === errors;
            } else {
              var valid3 = true;
            }
            if (valid3) {
              if (data.children !== undefined) {
                const _errs10 = errors;
                if (
                  !validate101(data.children, {
                    instancePath: instancePath + "/children",
                    parentData: data,
                    parentDataProperty: "children",
                    rootData,
                    dynamicAnchors,
                  })
                ) {
                  vErrors =
                    vErrors === null
                      ? validate101.errors
                      : vErrors.concat(validate101.errors);
                  errors = vErrors.length;
                }
                var valid3 = _errs10 === errors;
              } else {
                var valid3 = true;
              }
              if (valid3) {
                if (data.direction !== undefined) {
                  let data3 = data.direction;
                  const _errs11 = errors;
                  if (typeof data3 !== "string") {
                    validate108.errors = [
                      {
                        instancePath: instancePath + "/direction",
                        schemaPath: "#/allOf/2/properties/direction/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      },
                    ];
                    return false;
                  }
                  if (!(data3 === "vertical" || data3 === "horizontal")) {
                    validate108.errors = [
                      {
                        instancePath: instancePath + "/direction",
                        schemaPath: "#/allOf/2/properties/direction/enum",
                        keyword: "enum",
                        params: {
                          allowedValues:
                            schema85.allOf[2].properties.direction.enum,
                        },
                        message: "must be equal to one of the allowed values",
                      },
                    ];
                    return false;
                  }
                  var valid3 = _errs11 === errors;
                } else {
                  var valid3 = true;
                }
                if (valid3) {
                  if (data.align !== undefined) {
                    let data4 = data.align;
                    const _errs13 = errors;
                    if (typeof data4 !== "string") {
                      validate108.errors = [
                        {
                          instancePath: instancePath + "/align",
                          schemaPath: "#/allOf/2/properties/align/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                    if (
                      !(
                        data4 === "start" ||
                        data4 === "center" ||
                        data4 === "end" ||
                        data4 === "stretch"
                      )
                    ) {
                      validate108.errors = [
                        {
                          instancePath: instancePath + "/align",
                          schemaPath: "#/allOf/2/properties/align/enum",
                          keyword: "enum",
                          params: {
                            allowedValues:
                              schema85.allOf[2].properties.align.enum,
                          },
                          message: "must be equal to one of the allowed values",
                        },
                      ];
                      return false;
                    }
                    var valid3 = _errs13 === errors;
                  } else {
                    var valid3 = true;
                  }
                }
              }
            }
          }
        } else {
          validate108.errors = [
            {
              instancePath,
              schemaPath: "#/allOf/2/type",
              keyword: "type",
              params: { type: "object" },
              message: "must be object",
            },
          ];
          return false;
        }
      }
      var valid0 = _errs7 === errors;
    }
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      for (const key0 in data) {
        if (
          key0 !== "component" &&
          key0 !== "children" &&
          key0 !== "direction" &&
          key0 !== "align" &&
          key0 !== "weight" &&
          key0 !== "id" &&
          key0 !== "accessibility"
        ) {
          validate108.errors = [
            {
              instancePath,
              schemaPath: "#/unevaluatedProperties",
              keyword: "unevaluatedProperties",
              params: { unevaluatedProperty: key0 },
              message: "must NOT have unevaluated properties",
            },
          ];
          return false;
          break;
        }
      }
    } else {
      validate108.errors = [
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
  validate108.errors = vErrors;
  return errors === 0;
}
validate108.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema87 = {
  type: "object",
  allOf: [
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentCommon",
    },
    { $ref: "#/$defs/CatalogComponentCommon" },
    {
      type: "object",
      properties: {
        component: { const: "Card" },
        child: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentId",
          description:
            "The ID of the single child component to be rendered inside the card. To display multiple elements, you MUST wrap them in a layout component (like Column or Row) and pass that container's ID here. Do NOT pass multiple IDs or a non-existent ID.",
        },
      },
      required: ["component", "child"],
    },
  ],
  unevaluatedProperties: false,
};
function validate112(
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
  const evaluated0 = validate112.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs1 = errors;
  if (
    !validate28(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate28.errors : vErrors.concat(validate28.errors);
    errors = vErrors.length;
  }
  var valid0 = _errs1 === errors;
  if (valid0) {
    const _errs2 = errors;
    const _errs3 = errors;
    if (errors === _errs3) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.weight !== undefined) {
          if (!(typeof data.weight == "number")) {
            validate112.errors = [
              {
                instancePath: instancePath + "/weight",
                schemaPath:
                  "#/$defs/CatalogComponentCommon/properties/weight/type",
                keyword: "type",
                params: { type: "number" },
                message: "must be number",
              },
            ];
            return false;
          }
        }
      } else {
        validate112.errors = [
          {
            instancePath,
            schemaPath: "#/$defs/CatalogComponentCommon/type",
            keyword: "type",
            params: { type: "object" },
            message: "must be object",
          },
        ];
        return false;
      }
    }
    var valid0 = _errs2 === errors;
    if (valid0) {
      const _errs7 = errors;
      if (errors === _errs7) {
        if (data && typeof data == "object" && !Array.isArray(data)) {
          let missing0;
          if (
            (data.component === undefined && (missing0 = "component")) ||
            (data.child === undefined && (missing0 = "child"))
          ) {
            validate112.errors = [
              {
                instancePath,
                schemaPath: "#/allOf/2/required",
                keyword: "required",
                params: { missingProperty: missing0 },
                message: "must have required property '" + missing0 + "'",
              },
            ];
            return false;
          } else {
            if (data.component !== undefined) {
              const _errs9 = errors;
              if ("Card" !== data.component) {
                validate112.errors = [
                  {
                    instancePath: instancePath + "/component",
                    schemaPath: "#/allOf/2/properties/component/const",
                    keyword: "const",
                    params: { allowedValue: "Card" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid3 = _errs9 === errors;
            } else {
              var valid3 = true;
            }
            if (valid3) {
              if (data.child !== undefined) {
                const _errs10 = errors;
                if (typeof data.child !== "string") {
                  validate112.errors = [
                    {
                      instancePath: instancePath + "/child",
                      schemaPath:
                        "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentId/type",
                      keyword: "type",
                      params: { type: "string" },
                      message: "must be string",
                    },
                  ];
                  return false;
                }
                var valid3 = _errs10 === errors;
              } else {
                var valid3 = true;
              }
            }
          }
        } else {
          validate112.errors = [
            {
              instancePath,
              schemaPath: "#/allOf/2/type",
              keyword: "type",
              params: { type: "object" },
              message: "must be object",
            },
          ];
          return false;
        }
      }
      var valid0 = _errs7 === errors;
    }
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      for (const key0 in data) {
        if (
          key0 !== "component" &&
          key0 !== "child" &&
          key0 !== "weight" &&
          key0 !== "id" &&
          key0 !== "accessibility"
        ) {
          validate112.errors = [
            {
              instancePath,
              schemaPath: "#/unevaluatedProperties",
              keyword: "unevaluatedProperties",
              params: { unevaluatedProperty: key0 },
              message: "must NOT have unevaluated properties",
            },
          ];
          return false;
          break;
        }
      }
    } else {
      validate112.errors = [
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
  validate112.errors = vErrors;
  return errors === 0;
}
validate112.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema90 = {
  type: "object",
  allOf: [
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentCommon",
    },
    { $ref: "#/$defs/CatalogComponentCommon" },
    {
      type: "object",
      properties: {
        component: { const: "Tabs" },
        tabs: {
          type: "array",
          description:
            "An array of objects, where each object defines a tab with a title and a child component.",
          minItems: 1,
          items: {
            type: "object",
            properties: {
              title: {
                description: "The tab title.",
                $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
              },
              child: {
                $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentId",
                description: "The ID of the child component.",
              },
            },
            required: ["title", "child"],
            additionalProperties: false,
          },
        },
      },
      required: ["component", "tabs"],
    },
  ],
  unevaluatedProperties: false,
};
function validate115(
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
  const evaluated0 = validate115.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs1 = errors;
  if (
    !validate28(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate28.errors : vErrors.concat(validate28.errors);
    errors = vErrors.length;
  }
  var valid0 = _errs1 === errors;
  if (valid0) {
    const _errs2 = errors;
    const _errs3 = errors;
    if (errors === _errs3) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.weight !== undefined) {
          if (!(typeof data.weight == "number")) {
            validate115.errors = [
              {
                instancePath: instancePath + "/weight",
                schemaPath:
                  "#/$defs/CatalogComponentCommon/properties/weight/type",
                keyword: "type",
                params: { type: "number" },
                message: "must be number",
              },
            ];
            return false;
          }
        }
      } else {
        validate115.errors = [
          {
            instancePath,
            schemaPath: "#/$defs/CatalogComponentCommon/type",
            keyword: "type",
            params: { type: "object" },
            message: "must be object",
          },
        ];
        return false;
      }
    }
    var valid0 = _errs2 === errors;
    if (valid0) {
      const _errs7 = errors;
      if (errors === _errs7) {
        if (data && typeof data == "object" && !Array.isArray(data)) {
          let missing0;
          if (
            (data.component === undefined && (missing0 = "component")) ||
            (data.tabs === undefined && (missing0 = "tabs"))
          ) {
            validate115.errors = [
              {
                instancePath,
                schemaPath: "#/allOf/2/required",
                keyword: "required",
                params: { missingProperty: missing0 },
                message: "must have required property '" + missing0 + "'",
              },
            ];
            return false;
          } else {
            if (data.component !== undefined) {
              const _errs9 = errors;
              if ("Tabs" !== data.component) {
                validate115.errors = [
                  {
                    instancePath: instancePath + "/component",
                    schemaPath: "#/allOf/2/properties/component/const",
                    keyword: "const",
                    params: { allowedValue: "Tabs" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid3 = _errs9 === errors;
            } else {
              var valid3 = true;
            }
            if (valid3) {
              if (data.tabs !== undefined) {
                let data2 = data.tabs;
                const _errs10 = errors;
                if (errors === _errs10) {
                  if (Array.isArray(data2)) {
                    if (data2.length < 1) {
                      validate115.errors = [
                        {
                          instancePath: instancePath + "/tabs",
                          schemaPath: "#/allOf/2/properties/tabs/minItems",
                          keyword: "minItems",
                          params: { limit: 1 },
                          message: "must NOT have fewer than 1 items",
                        },
                      ];
                      return false;
                    } else {
                      var valid4 = true;
                      const len0 = data2.length;
                      for (let i0 = 0; i0 < len0; i0++) {
                        let data3 = data2[i0];
                        const _errs12 = errors;
                        if (errors === _errs12) {
                          if (
                            data3 &&
                            typeof data3 == "object" &&
                            !Array.isArray(data3)
                          ) {
                            let missing1;
                            if (
                              (data3.title === undefined &&
                                (missing1 = "title")) ||
                              (data3.child === undefined &&
                                (missing1 = "child"))
                            ) {
                              validate115.errors = [
                                {
                                  instancePath: instancePath + "/tabs/" + i0,
                                  schemaPath:
                                    "#/allOf/2/properties/tabs/items/required",
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
                              const _errs14 = errors;
                              for (const key0 in data3) {
                                if (!(key0 === "title" || key0 === "child")) {
                                  validate115.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/tabs/" + i0,
                                      schemaPath:
                                        "#/allOf/2/properties/tabs/items/additionalProperties",
                                      keyword: "additionalProperties",
                                      params: { additionalProperty: key0 },
                                      message:
                                        "must NOT have additional properties",
                                    },
                                  ];
                                  return false;
                                  break;
                                }
                              }
                              if (_errs14 === errors) {
                                if (data3.title !== undefined) {
                                  const _errs15 = errors;
                                  if (
                                    !validate30(data3.title, {
                                      instancePath:
                                        instancePath + "/tabs/" + i0 + "/title",
                                      parentData: data3,
                                      parentDataProperty: "title",
                                      rootData,
                                      dynamicAnchors,
                                    })
                                  ) {
                                    vErrors =
                                      vErrors === null
                                        ? validate30.errors
                                        : vErrors.concat(validate30.errors);
                                    errors = vErrors.length;
                                  }
                                  var valid5 = _errs15 === errors;
                                } else {
                                  var valid5 = true;
                                }
                                if (valid5) {
                                  if (data3.child !== undefined) {
                                    const _errs16 = errors;
                                    if (typeof data3.child !== "string") {
                                      validate115.errors = [
                                        {
                                          instancePath:
                                            instancePath +
                                            "/tabs/" +
                                            i0 +
                                            "/child",
                                          schemaPath:
                                            "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentId/type",
                                          keyword: "type",
                                          params: { type: "string" },
                                          message: "must be string",
                                        },
                                      ];
                                      return false;
                                    }
                                    var valid5 = _errs16 === errors;
                                  } else {
                                    var valid5 = true;
                                  }
                                }
                              }
                            }
                          } else {
                            validate115.errors = [
                              {
                                instancePath: instancePath + "/tabs/" + i0,
                                schemaPath:
                                  "#/allOf/2/properties/tabs/items/type",
                                keyword: "type",
                                params: { type: "object" },
                                message: "must be object",
                              },
                            ];
                            return false;
                          }
                        }
                        var valid4 = _errs12 === errors;
                        if (!valid4) {
                          break;
                        }
                      }
                    }
                  } else {
                    validate115.errors = [
                      {
                        instancePath: instancePath + "/tabs",
                        schemaPath: "#/allOf/2/properties/tabs/type",
                        keyword: "type",
                        params: { type: "array" },
                        message: "must be array",
                      },
                    ];
                    return false;
                  }
                }
                var valid3 = _errs10 === errors;
              } else {
                var valid3 = true;
              }
            }
          }
        } else {
          validate115.errors = [
            {
              instancePath,
              schemaPath: "#/allOf/2/type",
              keyword: "type",
              params: { type: "object" },
              message: "must be object",
            },
          ];
          return false;
        }
      }
      var valid0 = _errs7 === errors;
    }
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      for (const key1 in data) {
        if (
          key1 !== "component" &&
          key1 !== "tabs" &&
          key1 !== "weight" &&
          key1 !== "id" &&
          key1 !== "accessibility"
        ) {
          validate115.errors = [
            {
              instancePath,
              schemaPath: "#/unevaluatedProperties",
              keyword: "unevaluatedProperties",
              params: { unevaluatedProperty: key1 },
              message: "must NOT have unevaluated properties",
            },
          ];
          return false;
          break;
        }
      }
    } else {
      validate115.errors = [
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
  validate115.errors = vErrors;
  return errors === 0;
}
validate115.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema93 = {
  type: "object",
  allOf: [
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentCommon",
    },
    { $ref: "#/$defs/CatalogComponentCommon" },
    {
      type: "object",
      properties: {
        component: { const: "Modal" },
        trigger: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentId",
          description:
            "The ID of the component that opens the modal when interacted with (e.g., a button).",
        },
        content: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentId",
          description:
            "The ID of the component to be displayed inside the modal.",
        },
      },
      required: ["component", "trigger", "content"],
    },
  ],
  unevaluatedProperties: false,
};
function validate119(
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
  const evaluated0 = validate119.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs1 = errors;
  if (
    !validate28(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate28.errors : vErrors.concat(validate28.errors);
    errors = vErrors.length;
  }
  var valid0 = _errs1 === errors;
  if (valid0) {
    const _errs2 = errors;
    const _errs3 = errors;
    if (errors === _errs3) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.weight !== undefined) {
          if (!(typeof data.weight == "number")) {
            validate119.errors = [
              {
                instancePath: instancePath + "/weight",
                schemaPath:
                  "#/$defs/CatalogComponentCommon/properties/weight/type",
                keyword: "type",
                params: { type: "number" },
                message: "must be number",
              },
            ];
            return false;
          }
        }
      } else {
        validate119.errors = [
          {
            instancePath,
            schemaPath: "#/$defs/CatalogComponentCommon/type",
            keyword: "type",
            params: { type: "object" },
            message: "must be object",
          },
        ];
        return false;
      }
    }
    var valid0 = _errs2 === errors;
    if (valid0) {
      const _errs7 = errors;
      if (errors === _errs7) {
        if (data && typeof data == "object" && !Array.isArray(data)) {
          let missing0;
          if (
            (data.component === undefined && (missing0 = "component")) ||
            (data.trigger === undefined && (missing0 = "trigger")) ||
            (data.content === undefined && (missing0 = "content"))
          ) {
            validate119.errors = [
              {
                instancePath,
                schemaPath: "#/allOf/2/required",
                keyword: "required",
                params: { missingProperty: missing0 },
                message: "must have required property '" + missing0 + "'",
              },
            ];
            return false;
          } else {
            if (data.component !== undefined) {
              const _errs9 = errors;
              if ("Modal" !== data.component) {
                validate119.errors = [
                  {
                    instancePath: instancePath + "/component",
                    schemaPath: "#/allOf/2/properties/component/const",
                    keyword: "const",
                    params: { allowedValue: "Modal" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid3 = _errs9 === errors;
            } else {
              var valid3 = true;
            }
            if (valid3) {
              if (data.trigger !== undefined) {
                const _errs10 = errors;
                if (typeof data.trigger !== "string") {
                  validate119.errors = [
                    {
                      instancePath: instancePath + "/trigger",
                      schemaPath:
                        "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentId/type",
                      keyword: "type",
                      params: { type: "string" },
                      message: "must be string",
                    },
                  ];
                  return false;
                }
                var valid3 = _errs10 === errors;
              } else {
                var valid3 = true;
              }
              if (valid3) {
                if (data.content !== undefined) {
                  const _errs13 = errors;
                  if (typeof data.content !== "string") {
                    validate119.errors = [
                      {
                        instancePath: instancePath + "/content",
                        schemaPath:
                          "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentId/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      },
                    ];
                    return false;
                  }
                  var valid3 = _errs13 === errors;
                } else {
                  var valid3 = true;
                }
              }
            }
          }
        } else {
          validate119.errors = [
            {
              instancePath,
              schemaPath: "#/allOf/2/type",
              keyword: "type",
              params: { type: "object" },
              message: "must be object",
            },
          ];
          return false;
        }
      }
      var valid0 = _errs7 === errors;
    }
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      for (const key0 in data) {
        if (
          key0 !== "component" &&
          key0 !== "trigger" &&
          key0 !== "content" &&
          key0 !== "weight" &&
          key0 !== "id" &&
          key0 !== "accessibility"
        ) {
          validate119.errors = [
            {
              instancePath,
              schemaPath: "#/unevaluatedProperties",
              keyword: "unevaluatedProperties",
              params: { unevaluatedProperty: key0 },
              message: "must NOT have unevaluated properties",
            },
          ];
          return false;
          break;
        }
      }
    } else {
      validate119.errors = [
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
  validate119.errors = vErrors;
  return errors === 0;
}
validate119.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema97 = {
  type: "object",
  allOf: [
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentCommon",
    },
    { $ref: "#/$defs/CatalogComponentCommon" },
    {
      type: "object",
      properties: {
        component: { const: "Divider" },
        axis: {
          type: "string",
          description: "The orientation of the divider.",
          enum: ["horizontal", "vertical"],
          default: "horizontal",
        },
      },
      required: ["component"],
    },
  ],
  unevaluatedProperties: false,
};
function validate122(
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
  const evaluated0 = validate122.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs1 = errors;
  if (
    !validate28(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate28.errors : vErrors.concat(validate28.errors);
    errors = vErrors.length;
  }
  var valid0 = _errs1 === errors;
  if (valid0) {
    const _errs2 = errors;
    const _errs3 = errors;
    if (errors === _errs3) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.weight !== undefined) {
          if (!(typeof data.weight == "number")) {
            validate122.errors = [
              {
                instancePath: instancePath + "/weight",
                schemaPath:
                  "#/$defs/CatalogComponentCommon/properties/weight/type",
                keyword: "type",
                params: { type: "number" },
                message: "must be number",
              },
            ];
            return false;
          }
        }
      } else {
        validate122.errors = [
          {
            instancePath,
            schemaPath: "#/$defs/CatalogComponentCommon/type",
            keyword: "type",
            params: { type: "object" },
            message: "must be object",
          },
        ];
        return false;
      }
    }
    var valid0 = _errs2 === errors;
    if (valid0) {
      const _errs7 = errors;
      if (errors === _errs7) {
        if (data && typeof data == "object" && !Array.isArray(data)) {
          let missing0;
          if (data.component === undefined && (missing0 = "component")) {
            validate122.errors = [
              {
                instancePath,
                schemaPath: "#/allOf/2/required",
                keyword: "required",
                params: { missingProperty: missing0 },
                message: "must have required property '" + missing0 + "'",
              },
            ];
            return false;
          } else {
            if (data.component !== undefined) {
              const _errs9 = errors;
              if ("Divider" !== data.component) {
                validate122.errors = [
                  {
                    instancePath: instancePath + "/component",
                    schemaPath: "#/allOf/2/properties/component/const",
                    keyword: "const",
                    params: { allowedValue: "Divider" },
                    message: "must be equal to constant",
                  },
                ];
                return false;
              }
              var valid3 = _errs9 === errors;
            } else {
              var valid3 = true;
            }
            if (valid3) {
              if (data.axis !== undefined) {
                let data2 = data.axis;
                const _errs10 = errors;
                if (typeof data2 !== "string") {
                  validate122.errors = [
                    {
                      instancePath: instancePath + "/axis",
                      schemaPath: "#/allOf/2/properties/axis/type",
                      keyword: "type",
                      params: { type: "string" },
                      message: "must be string",
                    },
                  ];
                  return false;
                }
                if (!(data2 === "horizontal" || data2 === "vertical")) {
                  validate122.errors = [
                    {
                      instancePath: instancePath + "/axis",
                      schemaPath: "#/allOf/2/properties/axis/enum",
                      keyword: "enum",
                      params: {
                        allowedValues: schema97.allOf[2].properties.axis.enum,
                      },
                      message: "must be equal to one of the allowed values",
                    },
                  ];
                  return false;
                }
                var valid3 = _errs10 === errors;
              } else {
                var valid3 = true;
              }
            }
          }
        } else {
          validate122.errors = [
            {
              instancePath,
              schemaPath: "#/allOf/2/type",
              keyword: "type",
              params: { type: "object" },
              message: "must be object",
            },
          ];
          return false;
        }
      }
      var valid0 = _errs7 === errors;
    }
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      for (const key0 in data) {
        if (
          key0 !== "component" &&
          key0 !== "axis" &&
          key0 !== "weight" &&
          key0 !== "id" &&
          key0 !== "accessibility"
        ) {
          validate122.errors = [
            {
              instancePath,
              schemaPath: "#/unevaluatedProperties",
              keyword: "unevaluatedProperties",
              params: { unevaluatedProperty: key0 },
              message: "must NOT have unevaluated properties",
            },
          ];
          return false;
          break;
        }
      }
    } else {
      validate122.errors = [
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
  validate122.errors = vErrors;
  return errors === 0;
}
validate122.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema99 = {
  type: "object",
  allOf: [
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentCommon",
    },
    { $ref: "#/$defs/CatalogComponentCommon" },
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/Checkable",
    },
    {
      type: "object",
      properties: {
        component: { const: "Button" },
        child: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentId",
          description:
            "The ID of the child component. Use a 'Text' component for a labeled button. Only use an 'Icon' if the requirements explicitly ask for an icon-only button.",
        },
        variant: {
          type: "string",
          description:
            "A hint for the button style. If omitted, a default button style is used. 'primary' indicates this is the main call-to-action button. 'borderless' means the button has no visual border or background, making its child content appear like a clickable link.",
          enum: ["default", "primary", "borderless"],
          default: "default",
        },
        action: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/Action",
        },
      },
      required: ["component", "child", "action"],
    },
  ],
  unevaluatedProperties: false,
};
const schema101 = {
  description: "Properties for components that support client-side checks.",
  type: "object",
  properties: {
    checks: {
      type: "array",
      description:
        "A list of checks to perform. These are function calls that must return a boolean indicating validity.",
      items: { $ref: "#/$defs/CheckRule" },
    },
  },
};
const schema102 = {
  type: "object",
  description: "A single validation rule applied to an input component.",
  properties: {
    condition: { $ref: "#/$defs/DynamicBoolean" },
    message: {
      type: "string",
      description: "The error message to display if the check fails.",
    },
  },
  required: ["condition", "message"],
  additionalProperties: false,
};
function validate129(
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
  const evaluated0 = validate129.evaluated;
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
  if (typeof data !== "boolean") {
    const err0 = {
      instancePath,
      schemaPath: "#/oneOf/0/type",
      keyword: "type",
      params: { type: "boolean" },
      message: "must be boolean",
    };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  const _errs4 = errors;
  if (errors === _errs4) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.path === undefined && (missing0 = "path")) {
        const err1 = {
          instancePath,
          schemaPath: "#/$defs/DataBinding/required",
          keyword: "required",
          params: { missingProperty: missing0 },
          message: "must have required property '" + missing0 + "'",
        };
        if (vErrors === null) {
          vErrors = [err1];
        } else {
          vErrors.push(err1);
        }
        errors++;
      } else {
        const _errs6 = errors;
        for (const key0 in data) {
          if (!(key0 === "path")) {
            const err2 = {
              instancePath,
              schemaPath: "#/$defs/DataBinding/additionalProperties",
              keyword: "additionalProperties",
              params: { additionalProperty: key0 },
              message: "must NOT have additional properties",
            };
            if (vErrors === null) {
              vErrors = [err2];
            } else {
              vErrors.push(err2);
            }
            errors++;
            break;
          }
        }
        if (_errs6 === errors) {
          if (data.path !== undefined) {
            if (typeof data.path !== "string") {
              const err3 = {
                instancePath: instancePath + "/path",
                schemaPath: "#/$defs/DataBinding/properties/path/type",
                keyword: "type",
                params: { type: "string" },
                message: "must be string",
              };
              if (vErrors === null) {
                vErrors = [err3];
              } else {
                vErrors.push(err3);
              }
              errors++;
            }
          }
        }
      }
    } else {
      const err4 = {
        instancePath,
        schemaPath: "#/$defs/DataBinding/type",
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
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      var props0 = true;
    }
    const _errs9 = errors;
    const _errs10 = errors;
    if (
      !validate31(data, {
        instancePath,
        parentData,
        parentDataProperty,
        rootData,
        dynamicAnchors,
      })
    ) {
      vErrors =
        vErrors === null
          ? validate31.errors
          : vErrors.concat(validate31.errors);
      errors = vErrors.length;
    } else {
      var props1 = validate31.evaluated.props;
    }
    var valid3 = _errs10 === errors;
    if (valid3) {
      const _errs11 = errors;
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.returnType !== undefined) {
          if ("boolean" !== data.returnType) {
            const err5 = {
              instancePath: instancePath + "/returnType",
              schemaPath: "#/oneOf/2/allOf/1/properties/returnType/const",
              keyword: "const",
              params: { allowedValue: "boolean" },
              message: "must be equal to constant",
            };
            if (vErrors === null) {
              vErrors = [err5];
            } else {
              vErrors.push(err5);
            }
            errors++;
          }
        }
      }
      var valid3 = _errs11 === errors;
      if (valid3) {
        if (props1 !== true) {
          props1 = props1 || {};
          props1.returnType = true;
        }
      }
    }
    var _valid0 = _errs9 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
        if (props0 !== true && props1 !== undefined) {
          if (props1 === true) {
            props0 = true;
          } else {
            props0 = props0 || {};
            Object.assign(props0, props1);
          }
        }
      }
    }
  }
  if (!valid0) {
    const err6 = {
      instancePath,
      schemaPath: "#/oneOf",
      keyword: "oneOf",
      params: { passingSchemas: passing0 },
      message: "must match exactly one schema in oneOf",
    };
    if (vErrors === null) {
      vErrors = [err6];
    } else {
      vErrors.push(err6);
    }
    errors++;
    validate129.errors = vErrors;
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
  validate129.errors = vErrors;
  evaluated0.props = props0;
  return errors === 0;
}
validate129.evaluated = { dynamicProps: true, dynamicItems: false };
function validate128(
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
  const evaluated0 = validate128.evaluated;
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
        (data.condition === undefined && (missing0 = "condition")) ||
        (data.message === undefined && (missing0 = "message"))
      ) {
        validate128.errors = [
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
          if (!(key0 === "condition" || key0 === "message")) {
            validate128.errors = [
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
          if (data.condition !== undefined) {
            const _errs2 = errors;
            if (
              !validate129(data.condition, {
                instancePath: instancePath + "/condition",
                parentData: data,
                parentDataProperty: "condition",
                rootData,
                dynamicAnchors,
              })
            ) {
              vErrors =
                vErrors === null
                  ? validate129.errors
                  : vErrors.concat(validate129.errors);
              errors = vErrors.length;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.message !== undefined) {
              const _errs3 = errors;
              if (typeof data.message !== "string") {
                validate128.errors = [
                  {
                    instancePath: instancePath + "/message",
                    schemaPath: "#/properties/message/type",
                    keyword: "type",
                    params: { type: "string" },
                    message: "must be string",
                  },
                ];
                return false;
              }
              var valid0 = _errs3 === errors;
            } else {
              var valid0 = true;
            }
          }
        }
      }
    } else {
      validate128.errors = [
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
  validate128.errors = vErrors;
  return errors === 0;
}
validate128.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
function validate127(
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
  const evaluated0 = validate127.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      if (data.checks !== undefined) {
        let data0 = data.checks;
        const _errs1 = errors;
        if (errors === _errs1) {
          if (Array.isArray(data0)) {
            var valid1 = true;
            const len0 = data0.length;
            for (let i0 = 0; i0 < len0; i0++) {
              const _errs3 = errors;
              if (
                !validate128(data0[i0], {
                  instancePath: instancePath + "/checks/" + i0,
                  parentData: data0,
                  parentDataProperty: i0,
                  rootData,
                  dynamicAnchors,
                })
              ) {
                vErrors =
                  vErrors === null
                    ? validate128.errors
                    : vErrors.concat(validate128.errors);
                errors = vErrors.length;
              }
              var valid1 = _errs3 === errors;
              if (!valid1) {
                break;
              }
            }
          } else {
            validate127.errors = [
              {
                instancePath: instancePath + "/checks",
                schemaPath: "#/properties/checks/type",
                keyword: "type",
                params: { type: "array" },
                message: "must be array",
              },
            ];
            return false;
          }
        }
      }
    } else {
      validate127.errors = [
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
  validate127.errors = vErrors;
  return errors === 0;
}
validate127.evaluated = {
  props: { checks: true },
  dynamicProps: false,
  dynamicItems: false,
};
const schema106 = {
  description:
    "Defines an interaction handler that can either trigger a server-side event or execute a local client-side function.",
  oneOf: [
    {
      type: "object",
      description: "Triggers a server-side event.",
      properties: {
        event: {
          type: "object",
          description: "The event to dispatch to the server.",
          properties: {
            name: {
              type: "string",
              description:
                "The name of the action to be dispatched to the server.",
            },
            context: {
              type: "object",
              description:
                "A JSON object containing the key-value pairs for the action context. Values can be literals or paths. Use literal values unless the value must be dynamically bound to the data model. Do NOT use paths for static IDs.",
              additionalProperties: { $ref: "#/$defs/DynamicValue" },
            },
          },
          required: ["name"],
          additionalProperties: false,
        },
      },
      required: ["event"],
      additionalProperties: false,
    },
    {
      type: "object",
      description: "Executes a local client-side function.",
      properties: { functionCall: { $ref: "#/$defs/FunctionCall" } },
      required: ["functionCall"],
      additionalProperties: false,
    },
  ],
};
function validate134(
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
  const evaluated0 = validate134.evaluated;
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
      if (data.event === undefined && (missing0 = "event")) {
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
          if (!(key0 === "event")) {
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
          if (data.event !== undefined) {
            let data0 = data.event;
            const _errs4 = errors;
            if (errors === _errs4) {
              if (data0 && typeof data0 == "object" && !Array.isArray(data0)) {
                let missing1;
                if (data0.name === undefined && (missing1 = "name")) {
                  const err2 = {
                    instancePath: instancePath + "/event",
                    schemaPath: "#/oneOf/0/properties/event/required",
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
                  const _errs6 = errors;
                  for (const key1 in data0) {
                    if (!(key1 === "name" || key1 === "context")) {
                      const err3 = {
                        instancePath: instancePath + "/event",
                        schemaPath:
                          "#/oneOf/0/properties/event/additionalProperties",
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
                  if (_errs6 === errors) {
                    if (data0.name !== undefined) {
                      const _errs7 = errors;
                      if (typeof data0.name !== "string") {
                        const err4 = {
                          instancePath: instancePath + "/event/name",
                          schemaPath:
                            "#/oneOf/0/properties/event/properties/name/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        };
                        if (vErrors === null) {
                          vErrors = [err4];
                        } else {
                          vErrors.push(err4);
                        }
                        errors++;
                      }
                      var valid2 = _errs7 === errors;
                    } else {
                      var valid2 = true;
                    }
                    if (valid2) {
                      if (data0.context !== undefined) {
                        let data2 = data0.context;
                        const _errs9 = errors;
                        if (errors === _errs9) {
                          if (
                            data2 &&
                            typeof data2 == "object" &&
                            !Array.isArray(data2)
                          ) {
                            for (const key2 in data2) {
                              const _errs12 = errors;
                              if (
                                !validate73(data2[key2], {
                                  instancePath:
                                    instancePath +
                                    "/event/context/" +
                                    key2
                                      .replace(/~/g, "~0")
                                      .replace(/\//g, "~1"),
                                  parentData: data2,
                                  parentDataProperty: key2,
                                  rootData,
                                  dynamicAnchors,
                                })
                              ) {
                                vErrors =
                                  vErrors === null
                                    ? validate73.errors
                                    : vErrors.concat(validate73.errors);
                                errors = vErrors.length;
                              }
                              var valid3 = _errs12 === errors;
                              if (!valid3) {
                                break;
                              }
                            }
                          } else {
                            const err5 = {
                              instancePath: instancePath + "/event/context",
                              schemaPath:
                                "#/oneOf/0/properties/event/properties/context/type",
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
                        var valid2 = _errs9 === errors;
                      } else {
                        var valid2 = true;
                      }
                    }
                  }
                }
              } else {
                const err6 = {
                  instancePath: instancePath + "/event",
                  schemaPath: "#/oneOf/0/properties/event/type",
                  keyword: "type",
                  params: { type: "object" },
                  message: "must be object",
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
        }
      }
    } else {
      const err7 = {
        instancePath,
        schemaPath: "#/oneOf/0/type",
        keyword: "type",
        params: { type: "object" },
        message: "must be object",
      };
      if (vErrors === null) {
        vErrors = [err7];
      } else {
        vErrors.push(err7);
      }
      errors++;
    }
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
    var props1 = true;
  }
  const _errs13 = errors;
  if (errors === _errs13) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing2;
      if (data.functionCall === undefined && (missing2 = "functionCall")) {
        const err8 = {
          instancePath,
          schemaPath: "#/oneOf/1/required",
          keyword: "required",
          params: { missingProperty: missing2 },
          message: "must have required property '" + missing2 + "'",
        };
        if (vErrors === null) {
          vErrors = [err8];
        } else {
          vErrors.push(err8);
        }
        errors++;
      } else {
        const _errs15 = errors;
        for (const key3 in data) {
          if (!(key3 === "functionCall")) {
            const err9 = {
              instancePath,
              schemaPath: "#/oneOf/1/additionalProperties",
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
        if (_errs15 === errors) {
          if (data.functionCall !== undefined) {
            if (
              !validate31(data.functionCall, {
                instancePath: instancePath + "/functionCall",
                parentData: data,
                parentDataProperty: "functionCall",
                rootData,
                dynamicAnchors,
              })
            ) {
              vErrors =
                vErrors === null
                  ? validate31.errors
                  : vErrors.concat(validate31.errors);
              errors = vErrors.length;
            }
          }
        }
      }
    } else {
      const err10 = {
        instancePath,
        schemaPath: "#/oneOf/1/type",
        keyword: "type",
        params: { type: "object" },
        message: "must be object",
      };
      if (vErrors === null) {
        vErrors = [err10];
      } else {
        vErrors.push(err10);
      }
      errors++;
    }
  }
  var _valid0 = _errs13 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      if (props1 !== true) {
        props1 = true;
      }
    }
  }
  if (!valid0) {
    const err11 = {
      instancePath,
      schemaPath: "#/oneOf",
      keyword: "oneOf",
      params: { passingSchemas: passing0 },
      message: "must match exactly one schema in oneOf",
    };
    if (vErrors === null) {
      vErrors = [err11];
    } else {
      vErrors.push(err11);
    }
    errors++;
    validate134.errors = vErrors;
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
  validate134.errors = vErrors;
  evaluated0.props = props1;
  return errors === 0;
}
validate134.evaluated = { dynamicProps: true, dynamicItems: false };
function validate125(
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
  const evaluated0 = validate125.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs1 = errors;
  if (
    !validate28(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate28.errors : vErrors.concat(validate28.errors);
    errors = vErrors.length;
  }
  var valid0 = _errs1 === errors;
  if (valid0) {
    const _errs2 = errors;
    const _errs3 = errors;
    if (errors === _errs3) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.weight !== undefined) {
          if (!(typeof data.weight == "number")) {
            validate125.errors = [
              {
                instancePath: instancePath + "/weight",
                schemaPath:
                  "#/$defs/CatalogComponentCommon/properties/weight/type",
                keyword: "type",
                params: { type: "number" },
                message: "must be number",
              },
            ];
            return false;
          }
        }
      } else {
        validate125.errors = [
          {
            instancePath,
            schemaPath: "#/$defs/CatalogComponentCommon/type",
            keyword: "type",
            params: { type: "object" },
            message: "must be object",
          },
        ];
        return false;
      }
    }
    var valid0 = _errs2 === errors;
    if (valid0) {
      const _errs7 = errors;
      if (
        !validate127(data, {
          instancePath,
          parentData,
          parentDataProperty,
          rootData,
          dynamicAnchors,
        })
      ) {
        vErrors =
          vErrors === null
            ? validate127.errors
            : vErrors.concat(validate127.errors);
        errors = vErrors.length;
      }
      var valid0 = _errs7 === errors;
      if (valid0) {
        const _errs8 = errors;
        if (errors === _errs8) {
          if (data && typeof data == "object" && !Array.isArray(data)) {
            let missing0;
            if (
              (data.component === undefined && (missing0 = "component")) ||
              (data.child === undefined && (missing0 = "child")) ||
              (data.action === undefined && (missing0 = "action"))
            ) {
              validate125.errors = [
                {
                  instancePath,
                  schemaPath: "#/allOf/3/required",
                  keyword: "required",
                  params: { missingProperty: missing0 },
                  message: "must have required property '" + missing0 + "'",
                },
              ];
              return false;
            } else {
              if (data.component !== undefined) {
                const _errs10 = errors;
                if ("Button" !== data.component) {
                  validate125.errors = [
                    {
                      instancePath: instancePath + "/component",
                      schemaPath: "#/allOf/3/properties/component/const",
                      keyword: "const",
                      params: { allowedValue: "Button" },
                      message: "must be equal to constant",
                    },
                  ];
                  return false;
                }
                var valid3 = _errs10 === errors;
              } else {
                var valid3 = true;
              }
              if (valid3) {
                if (data.child !== undefined) {
                  const _errs11 = errors;
                  if (typeof data.child !== "string") {
                    validate125.errors = [
                      {
                        instancePath: instancePath + "/child",
                        schemaPath:
                          "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentId/type",
                        keyword: "type",
                        params: { type: "string" },
                        message: "must be string",
                      },
                    ];
                    return false;
                  }
                  var valid3 = _errs11 === errors;
                } else {
                  var valid3 = true;
                }
                if (valid3) {
                  if (data.variant !== undefined) {
                    let data3 = data.variant;
                    const _errs14 = errors;
                    if (typeof data3 !== "string") {
                      validate125.errors = [
                        {
                          instancePath: instancePath + "/variant",
                          schemaPath: "#/allOf/3/properties/variant/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                    if (
                      !(
                        data3 === "default" ||
                        data3 === "primary" ||
                        data3 === "borderless"
                      )
                    ) {
                      validate125.errors = [
                        {
                          instancePath: instancePath + "/variant",
                          schemaPath: "#/allOf/3/properties/variant/enum",
                          keyword: "enum",
                          params: {
                            allowedValues:
                              schema99.allOf[3].properties.variant.enum,
                          },
                          message: "must be equal to one of the allowed values",
                        },
                      ];
                      return false;
                    }
                    var valid3 = _errs14 === errors;
                  } else {
                    var valid3 = true;
                  }
                  if (valid3) {
                    if (data.action !== undefined) {
                      const _errs16 = errors;
                      if (
                        !validate134(data.action, {
                          instancePath: instancePath + "/action",
                          parentData: data,
                          parentDataProperty: "action",
                          rootData,
                          dynamicAnchors,
                        })
                      ) {
                        vErrors =
                          vErrors === null
                            ? validate134.errors
                            : vErrors.concat(validate134.errors);
                        errors = vErrors.length;
                      }
                      var valid3 = _errs16 === errors;
                    } else {
                      var valid3 = true;
                    }
                  }
                }
              }
            }
          } else {
            validate125.errors = [
              {
                instancePath,
                schemaPath: "#/allOf/3/type",
                keyword: "type",
                params: { type: "object" },
                message: "must be object",
              },
            ];
            return false;
          }
        }
        var valid0 = _errs8 === errors;
      }
    }
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      for (const key0 in data) {
        if (
          key0 !== "component" &&
          key0 !== "child" &&
          key0 !== "variant" &&
          key0 !== "action" &&
          key0 !== "checks" &&
          key0 !== "weight" &&
          key0 !== "id" &&
          key0 !== "accessibility"
        ) {
          validate125.errors = [
            {
              instancePath,
              schemaPath: "#/unevaluatedProperties",
              keyword: "unevaluatedProperties",
              params: { unevaluatedProperty: key0 },
              message: "must NOT have unevaluated properties",
            },
          ];
          return false;
          break;
        }
      }
    } else {
      validate125.errors = [
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
  validate125.errors = vErrors;
  return errors === 0;
}
validate125.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema107 = {
  type: "object",
  allOf: [
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentCommon",
    },
    { $ref: "#/$defs/CatalogComponentCommon" },
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/Checkable",
    },
    {
      type: "object",
      properties: {
        component: { const: "TextField" },
        label: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
          description: "The text label for the input field.",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
          description: "The value of the text field.",
        },
        variant: {
          type: "string",
          description: "The type of input field to display.",
          enum: ["longText", "number", "shortText", "obscured"],
          default: "shortText",
        },
        validationRegexp: {
          type: "string",
          description:
            "A regular expression used for client-side validation of the input.",
        },
      },
      required: ["component", "label"],
    },
  ],
  unevaluatedProperties: false,
};
function validate139(
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
  const evaluated0 = validate139.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs1 = errors;
  if (
    !validate28(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate28.errors : vErrors.concat(validate28.errors);
    errors = vErrors.length;
  }
  var valid0 = _errs1 === errors;
  if (valid0) {
    const _errs2 = errors;
    const _errs3 = errors;
    if (errors === _errs3) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.weight !== undefined) {
          if (!(typeof data.weight == "number")) {
            validate139.errors = [
              {
                instancePath: instancePath + "/weight",
                schemaPath:
                  "#/$defs/CatalogComponentCommon/properties/weight/type",
                keyword: "type",
                params: { type: "number" },
                message: "must be number",
              },
            ];
            return false;
          }
        }
      } else {
        validate139.errors = [
          {
            instancePath,
            schemaPath: "#/$defs/CatalogComponentCommon/type",
            keyword: "type",
            params: { type: "object" },
            message: "must be object",
          },
        ];
        return false;
      }
    }
    var valid0 = _errs2 === errors;
    if (valid0) {
      const _errs7 = errors;
      if (
        !validate127(data, {
          instancePath,
          parentData,
          parentDataProperty,
          rootData,
          dynamicAnchors,
        })
      ) {
        vErrors =
          vErrors === null
            ? validate127.errors
            : vErrors.concat(validate127.errors);
        errors = vErrors.length;
      }
      var valid0 = _errs7 === errors;
      if (valid0) {
        const _errs8 = errors;
        if (errors === _errs8) {
          if (data && typeof data == "object" && !Array.isArray(data)) {
            let missing0;
            if (
              (data.component === undefined && (missing0 = "component")) ||
              (data.label === undefined && (missing0 = "label"))
            ) {
              validate139.errors = [
                {
                  instancePath,
                  schemaPath: "#/allOf/3/required",
                  keyword: "required",
                  params: { missingProperty: missing0 },
                  message: "must have required property '" + missing0 + "'",
                },
              ];
              return false;
            } else {
              if (data.component !== undefined) {
                const _errs10 = errors;
                if ("TextField" !== data.component) {
                  validate139.errors = [
                    {
                      instancePath: instancePath + "/component",
                      schemaPath: "#/allOf/3/properties/component/const",
                      keyword: "const",
                      params: { allowedValue: "TextField" },
                      message: "must be equal to constant",
                    },
                  ];
                  return false;
                }
                var valid3 = _errs10 === errors;
              } else {
                var valid3 = true;
              }
              if (valid3) {
                if (data.label !== undefined) {
                  const _errs11 = errors;
                  if (
                    !validate30(data.label, {
                      instancePath: instancePath + "/label",
                      parentData: data,
                      parentDataProperty: "label",
                      rootData,
                      dynamicAnchors,
                    })
                  ) {
                    vErrors =
                      vErrors === null
                        ? validate30.errors
                        : vErrors.concat(validate30.errors);
                    errors = vErrors.length;
                  }
                  var valid3 = _errs11 === errors;
                } else {
                  var valid3 = true;
                }
                if (valid3) {
                  if (data.value !== undefined) {
                    const _errs12 = errors;
                    if (
                      !validate30(data.value, {
                        instancePath: instancePath + "/value",
                        parentData: data,
                        parentDataProperty: "value",
                        rootData,
                        dynamicAnchors,
                      })
                    ) {
                      vErrors =
                        vErrors === null
                          ? validate30.errors
                          : vErrors.concat(validate30.errors);
                      errors = vErrors.length;
                    }
                    var valid3 = _errs12 === errors;
                  } else {
                    var valid3 = true;
                  }
                  if (valid3) {
                    if (data.variant !== undefined) {
                      let data4 = data.variant;
                      const _errs13 = errors;
                      if (typeof data4 !== "string") {
                        validate139.errors = [
                          {
                            instancePath: instancePath + "/variant",
                            schemaPath: "#/allOf/3/properties/variant/type",
                            keyword: "type",
                            params: { type: "string" },
                            message: "must be string",
                          },
                        ];
                        return false;
                      }
                      if (
                        !(
                          data4 === "longText" ||
                          data4 === "number" ||
                          data4 === "shortText" ||
                          data4 === "obscured"
                        )
                      ) {
                        validate139.errors = [
                          {
                            instancePath: instancePath + "/variant",
                            schemaPath: "#/allOf/3/properties/variant/enum",
                            keyword: "enum",
                            params: {
                              allowedValues:
                                schema107.allOf[3].properties.variant.enum,
                            },
                            message:
                              "must be equal to one of the allowed values",
                          },
                        ];
                        return false;
                      }
                      var valid3 = _errs13 === errors;
                    } else {
                      var valid3 = true;
                    }
                    if (valid3) {
                      if (data.validationRegexp !== undefined) {
                        const _errs15 = errors;
                        if (typeof data.validationRegexp !== "string") {
                          validate139.errors = [
                            {
                              instancePath: instancePath + "/validationRegexp",
                              schemaPath:
                                "#/allOf/3/properties/validationRegexp/type",
                              keyword: "type",
                              params: { type: "string" },
                              message: "must be string",
                            },
                          ];
                          return false;
                        }
                        var valid3 = _errs15 === errors;
                      } else {
                        var valid3 = true;
                      }
                    }
                  }
                }
              }
            }
          } else {
            validate139.errors = [
              {
                instancePath,
                schemaPath: "#/allOf/3/type",
                keyword: "type",
                params: { type: "object" },
                message: "must be object",
              },
            ];
            return false;
          }
        }
        var valid0 = _errs8 === errors;
      }
    }
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      for (const key0 in data) {
        if (
          key0 !== "component" &&
          key0 !== "label" &&
          key0 !== "value" &&
          key0 !== "variant" &&
          key0 !== "validationRegexp" &&
          key0 !== "checks" &&
          key0 !== "weight" &&
          key0 !== "id" &&
          key0 !== "accessibility"
        ) {
          validate139.errors = [
            {
              instancePath,
              schemaPath: "#/unevaluatedProperties",
              keyword: "unevaluatedProperties",
              params: { unevaluatedProperty: key0 },
              message: "must NOT have unevaluated properties",
            },
          ];
          return false;
          break;
        }
      }
    } else {
      validate139.errors = [
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
  validate139.errors = vErrors;
  return errors === 0;
}
validate139.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema109 = {
  type: "object",
  allOf: [
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentCommon",
    },
    { $ref: "#/$defs/CatalogComponentCommon" },
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/Checkable",
    },
    {
      type: "object",
      properties: {
        component: { const: "CheckBox" },
        label: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
          description: "The text to display next to the checkbox.",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicBoolean",
          description:
            "The current state of the checkbox (true for checked, false for unchecked).",
        },
      },
      required: ["component", "label", "value"],
    },
  ],
  unevaluatedProperties: false,
};
function validate145(
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
  const evaluated0 = validate145.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs1 = errors;
  if (
    !validate28(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate28.errors : vErrors.concat(validate28.errors);
    errors = vErrors.length;
  }
  var valid0 = _errs1 === errors;
  if (valid0) {
    const _errs2 = errors;
    const _errs3 = errors;
    if (errors === _errs3) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.weight !== undefined) {
          if (!(typeof data.weight == "number")) {
            validate145.errors = [
              {
                instancePath: instancePath + "/weight",
                schemaPath:
                  "#/$defs/CatalogComponentCommon/properties/weight/type",
                keyword: "type",
                params: { type: "number" },
                message: "must be number",
              },
            ];
            return false;
          }
        }
      } else {
        validate145.errors = [
          {
            instancePath,
            schemaPath: "#/$defs/CatalogComponentCommon/type",
            keyword: "type",
            params: { type: "object" },
            message: "must be object",
          },
        ];
        return false;
      }
    }
    var valid0 = _errs2 === errors;
    if (valid0) {
      const _errs7 = errors;
      if (
        !validate127(data, {
          instancePath,
          parentData,
          parentDataProperty,
          rootData,
          dynamicAnchors,
        })
      ) {
        vErrors =
          vErrors === null
            ? validate127.errors
            : vErrors.concat(validate127.errors);
        errors = vErrors.length;
      }
      var valid0 = _errs7 === errors;
      if (valid0) {
        const _errs8 = errors;
        if (errors === _errs8) {
          if (data && typeof data == "object" && !Array.isArray(data)) {
            let missing0;
            if (
              (data.component === undefined && (missing0 = "component")) ||
              (data.label === undefined && (missing0 = "label")) ||
              (data.value === undefined && (missing0 = "value"))
            ) {
              validate145.errors = [
                {
                  instancePath,
                  schemaPath: "#/allOf/3/required",
                  keyword: "required",
                  params: { missingProperty: missing0 },
                  message: "must have required property '" + missing0 + "'",
                },
              ];
              return false;
            } else {
              if (data.component !== undefined) {
                const _errs10 = errors;
                if ("CheckBox" !== data.component) {
                  validate145.errors = [
                    {
                      instancePath: instancePath + "/component",
                      schemaPath: "#/allOf/3/properties/component/const",
                      keyword: "const",
                      params: { allowedValue: "CheckBox" },
                      message: "must be equal to constant",
                    },
                  ];
                  return false;
                }
                var valid3 = _errs10 === errors;
              } else {
                var valid3 = true;
              }
              if (valid3) {
                if (data.label !== undefined) {
                  const _errs11 = errors;
                  if (
                    !validate30(data.label, {
                      instancePath: instancePath + "/label",
                      parentData: data,
                      parentDataProperty: "label",
                      rootData,
                      dynamicAnchors,
                    })
                  ) {
                    vErrors =
                      vErrors === null
                        ? validate30.errors
                        : vErrors.concat(validate30.errors);
                    errors = vErrors.length;
                  }
                  var valid3 = _errs11 === errors;
                } else {
                  var valid3 = true;
                }
                if (valid3) {
                  if (data.value !== undefined) {
                    const _errs12 = errors;
                    if (
                      !validate48(data.value, {
                        instancePath: instancePath + "/value",
                        parentData: data,
                        parentDataProperty: "value",
                        rootData,
                        dynamicAnchors,
                      })
                    ) {
                      vErrors =
                        vErrors === null
                          ? validate48.errors
                          : vErrors.concat(validate48.errors);
                      errors = vErrors.length;
                    }
                    var valid3 = _errs12 === errors;
                  } else {
                    var valid3 = true;
                  }
                }
              }
            }
          } else {
            validate145.errors = [
              {
                instancePath,
                schemaPath: "#/allOf/3/type",
                keyword: "type",
                params: { type: "object" },
                message: "must be object",
              },
            ];
            return false;
          }
        }
        var valid0 = _errs8 === errors;
      }
    }
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      for (const key0 in data) {
        if (
          key0 !== "component" &&
          key0 !== "label" &&
          key0 !== "value" &&
          key0 !== "checks" &&
          key0 !== "weight" &&
          key0 !== "id" &&
          key0 !== "accessibility"
        ) {
          validate145.errors = [
            {
              instancePath,
              schemaPath: "#/unevaluatedProperties",
              keyword: "unevaluatedProperties",
              params: { unevaluatedProperty: key0 },
              message: "must NOT have unevaluated properties",
            },
          ];
          return false;
          break;
        }
      }
    } else {
      validate145.errors = [
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
  validate145.errors = vErrors;
  return errors === 0;
}
validate145.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema111 = {
  type: "object",
  allOf: [
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentCommon",
    },
    { $ref: "#/$defs/CatalogComponentCommon" },
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/Checkable",
    },
    {
      type: "object",
      description:
        "A component that allows selecting one or more options from a list.",
      properties: {
        component: { const: "ChoicePicker" },
        label: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
          description: "The label for the group of options.",
        },
        variant: {
          type: "string",
          description:
            "A hint for how the choice picker should be displayed and behave.",
          enum: ["multipleSelection", "mutuallyExclusive"],
          default: "mutuallyExclusive",
        },
        options: {
          type: "array",
          description: "The list of available options to choose from.",
          items: {
            type: "object",
            properties: {
              label: {
                description: "The text to display for this option.",
                $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
              },
              value: {
                type: "string",
                description: "The stable value associated with this option.",
              },
            },
            required: ["label", "value"],
            additionalProperties: false,
          },
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicStringList",
          description:
            "The list of currently selected values. This should be bound to a string array in the data model.",
        },
        displayStyle: {
          type: "string",
          description: "The display style of the component.",
          enum: ["checkbox", "chips"],
          default: "checkbox",
        },
        filterable: {
          type: "boolean",
          description:
            "If true, displays a search input to filter the options.",
          default: false,
        },
      },
      required: ["component", "options", "value"],
    },
  ],
  unevaluatedProperties: false,
};
const schema113 = {
  description:
    "Represents a value that can be either a literal array of strings, a path to a string array in the data model, or a function call returning a string array.",
  oneOf: [
    { type: "array", items: { type: "string" } },
    { $ref: "#/$defs/DataBinding" },
    {
      allOf: [
        { $ref: "#/$defs/FunctionCall" },
        { properties: { returnType: { const: "array" } } },
      ],
    },
  ],
};
function validate156(
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
  const evaluated0 = validate156.evaluated;
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
    if (Array.isArray(data)) {
      var valid1 = true;
      const len0 = data.length;
      for (let i0 = 0; i0 < len0; i0++) {
        const _errs3 = errors;
        if (typeof data[i0] !== "string") {
          const err0 = {
            instancePath: instancePath + "/" + i0,
            schemaPath: "#/oneOf/0/items/type",
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
        var valid1 = _errs3 === errors;
        if (!valid1) {
          break;
        }
      }
    } else {
      const err1 = {
        instancePath,
        schemaPath: "#/oneOf/0/type",
        keyword: "type",
        params: { type: "array" },
        message: "must be array",
      };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
    var items0 = true;
  }
  const _errs5 = errors;
  const _errs6 = errors;
  if (errors === _errs6) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      let missing0;
      if (data.path === undefined && (missing0 = "path")) {
        const err2 = {
          instancePath,
          schemaPath: "#/$defs/DataBinding/required",
          keyword: "required",
          params: { missingProperty: missing0 },
          message: "must have required property '" + missing0 + "'",
        };
        if (vErrors === null) {
          vErrors = [err2];
        } else {
          vErrors.push(err2);
        }
        errors++;
      } else {
        const _errs8 = errors;
        for (const key0 in data) {
          if (!(key0 === "path")) {
            const err3 = {
              instancePath,
              schemaPath: "#/$defs/DataBinding/additionalProperties",
              keyword: "additionalProperties",
              params: { additionalProperty: key0 },
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
        if (_errs8 === errors) {
          if (data.path !== undefined) {
            if (typeof data.path !== "string") {
              const err4 = {
                instancePath: instancePath + "/path",
                schemaPath: "#/$defs/DataBinding/properties/path/type",
                keyword: "type",
                params: { type: "string" },
                message: "must be string",
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
    } else {
      const err5 = {
        instancePath,
        schemaPath: "#/$defs/DataBinding/type",
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
  var _valid0 = _errs5 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      var props0 = true;
    }
    const _errs11 = errors;
    const _errs12 = errors;
    if (
      !validate31(data, {
        instancePath,
        parentData,
        parentDataProperty,
        rootData,
        dynamicAnchors,
      })
    ) {
      vErrors =
        vErrors === null
          ? validate31.errors
          : vErrors.concat(validate31.errors);
      errors = vErrors.length;
    } else {
      var props1 = validate31.evaluated.props;
    }
    var valid4 = _errs12 === errors;
    if (valid4) {
      const _errs13 = errors;
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.returnType !== undefined) {
          if ("array" !== data.returnType) {
            const err6 = {
              instancePath: instancePath + "/returnType",
              schemaPath: "#/oneOf/2/allOf/1/properties/returnType/const",
              keyword: "const",
              params: { allowedValue: "array" },
              message: "must be equal to constant",
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
      var valid4 = _errs13 === errors;
      if (valid4) {
        if (props1 !== true) {
          props1 = props1 || {};
          props1.returnType = true;
        }
      }
    }
    var _valid0 = _errs11 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
        if (props0 !== true && props1 !== undefined) {
          if (props1 === true) {
            props0 = true;
          } else {
            props0 = props0 || {};
            Object.assign(props0, props1);
          }
        }
      }
    }
  }
  if (!valid0) {
    const err7 = {
      instancePath,
      schemaPath: "#/oneOf",
      keyword: "oneOf",
      params: { passingSchemas: passing0 },
      message: "must match exactly one schema in oneOf",
    };
    if (vErrors === null) {
      vErrors = [err7];
    } else {
      vErrors.push(err7);
    }
    errors++;
    validate156.errors = vErrors;
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
  validate156.errors = vErrors;
  evaluated0.props = props0;
  evaluated0.items = items0;
  return errors === 0;
}
validate156.evaluated = { dynamicProps: true, dynamicItems: true };
function validate151(
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
  const evaluated0 = validate151.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs1 = errors;
  if (
    !validate28(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate28.errors : vErrors.concat(validate28.errors);
    errors = vErrors.length;
  }
  var valid0 = _errs1 === errors;
  if (valid0) {
    const _errs2 = errors;
    const _errs3 = errors;
    if (errors === _errs3) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.weight !== undefined) {
          if (!(typeof data.weight == "number")) {
            validate151.errors = [
              {
                instancePath: instancePath + "/weight",
                schemaPath:
                  "#/$defs/CatalogComponentCommon/properties/weight/type",
                keyword: "type",
                params: { type: "number" },
                message: "must be number",
              },
            ];
            return false;
          }
        }
      } else {
        validate151.errors = [
          {
            instancePath,
            schemaPath: "#/$defs/CatalogComponentCommon/type",
            keyword: "type",
            params: { type: "object" },
            message: "must be object",
          },
        ];
        return false;
      }
    }
    var valid0 = _errs2 === errors;
    if (valid0) {
      const _errs7 = errors;
      if (
        !validate127(data, {
          instancePath,
          parentData,
          parentDataProperty,
          rootData,
          dynamicAnchors,
        })
      ) {
        vErrors =
          vErrors === null
            ? validate127.errors
            : vErrors.concat(validate127.errors);
        errors = vErrors.length;
      }
      var valid0 = _errs7 === errors;
      if (valid0) {
        const _errs8 = errors;
        if (errors === _errs8) {
          if (data && typeof data == "object" && !Array.isArray(data)) {
            let missing0;
            if (
              (data.component === undefined && (missing0 = "component")) ||
              (data.options === undefined && (missing0 = "options")) ||
              (data.value === undefined && (missing0 = "value"))
            ) {
              validate151.errors = [
                {
                  instancePath,
                  schemaPath: "#/allOf/3/required",
                  keyword: "required",
                  params: { missingProperty: missing0 },
                  message: "must have required property '" + missing0 + "'",
                },
              ];
              return false;
            } else {
              if (data.component !== undefined) {
                const _errs10 = errors;
                if ("ChoicePicker" !== data.component) {
                  validate151.errors = [
                    {
                      instancePath: instancePath + "/component",
                      schemaPath: "#/allOf/3/properties/component/const",
                      keyword: "const",
                      params: { allowedValue: "ChoicePicker" },
                      message: "must be equal to constant",
                    },
                  ];
                  return false;
                }
                var valid3 = _errs10 === errors;
              } else {
                var valid3 = true;
              }
              if (valid3) {
                if (data.label !== undefined) {
                  const _errs11 = errors;
                  if (
                    !validate30(data.label, {
                      instancePath: instancePath + "/label",
                      parentData: data,
                      parentDataProperty: "label",
                      rootData,
                      dynamicAnchors,
                    })
                  ) {
                    vErrors =
                      vErrors === null
                        ? validate30.errors
                        : vErrors.concat(validate30.errors);
                    errors = vErrors.length;
                  }
                  var valid3 = _errs11 === errors;
                } else {
                  var valid3 = true;
                }
                if (valid3) {
                  if (data.variant !== undefined) {
                    let data3 = data.variant;
                    const _errs12 = errors;
                    if (typeof data3 !== "string") {
                      validate151.errors = [
                        {
                          instancePath: instancePath + "/variant",
                          schemaPath: "#/allOf/3/properties/variant/type",
                          keyword: "type",
                          params: { type: "string" },
                          message: "must be string",
                        },
                      ];
                      return false;
                    }
                    if (
                      !(
                        data3 === "multipleSelection" ||
                        data3 === "mutuallyExclusive"
                      )
                    ) {
                      validate151.errors = [
                        {
                          instancePath: instancePath + "/variant",
                          schemaPath: "#/allOf/3/properties/variant/enum",
                          keyword: "enum",
                          params: {
                            allowedValues:
                              schema111.allOf[3].properties.variant.enum,
                          },
                          message: "must be equal to one of the allowed values",
                        },
                      ];
                      return false;
                    }
                    var valid3 = _errs12 === errors;
                  } else {
                    var valid3 = true;
                  }
                  if (valid3) {
                    if (data.options !== undefined) {
                      let data4 = data.options;
                      const _errs14 = errors;
                      if (errors === _errs14) {
                        if (Array.isArray(data4)) {
                          var valid4 = true;
                          const len0 = data4.length;
                          for (let i0 = 0; i0 < len0; i0++) {
                            let data5 = data4[i0];
                            const _errs16 = errors;
                            if (errors === _errs16) {
                              if (
                                data5 &&
                                typeof data5 == "object" &&
                                !Array.isArray(data5)
                              ) {
                                let missing1;
                                if (
                                  (data5.label === undefined &&
                                    (missing1 = "label")) ||
                                  (data5.value === undefined &&
                                    (missing1 = "value"))
                                ) {
                                  validate151.errors = [
                                    {
                                      instancePath:
                                        instancePath + "/options/" + i0,
                                      schemaPath:
                                        "#/allOf/3/properties/options/items/required",
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
                                  const _errs18 = errors;
                                  for (const key0 in data5) {
                                    if (
                                      !(key0 === "label" || key0 === "value")
                                    ) {
                                      validate151.errors = [
                                        {
                                          instancePath:
                                            instancePath + "/options/" + i0,
                                          schemaPath:
                                            "#/allOf/3/properties/options/items/additionalProperties",
                                          keyword: "additionalProperties",
                                          params: { additionalProperty: key0 },
                                          message:
                                            "must NOT have additional properties",
                                        },
                                      ];
                                      return false;
                                      break;
                                    }
                                  }
                                  if (_errs18 === errors) {
                                    if (data5.label !== undefined) {
                                      const _errs19 = errors;
                                      if (
                                        !validate30(data5.label, {
                                          instancePath:
                                            instancePath +
                                            "/options/" +
                                            i0 +
                                            "/label",
                                          parentData: data5,
                                          parentDataProperty: "label",
                                          rootData,
                                          dynamicAnchors,
                                        })
                                      ) {
                                        vErrors =
                                          vErrors === null
                                            ? validate30.errors
                                            : vErrors.concat(validate30.errors);
                                        errors = vErrors.length;
                                      }
                                      var valid5 = _errs19 === errors;
                                    } else {
                                      var valid5 = true;
                                    }
                                    if (valid5) {
                                      if (data5.value !== undefined) {
                                        const _errs20 = errors;
                                        if (typeof data5.value !== "string") {
                                          validate151.errors = [
                                            {
                                              instancePath:
                                                instancePath +
                                                "/options/" +
                                                i0 +
                                                "/value",
                                              schemaPath:
                                                "#/allOf/3/properties/options/items/properties/value/type",
                                              keyword: "type",
                                              params: { type: "string" },
                                              message: "must be string",
                                            },
                                          ];
                                          return false;
                                        }
                                        var valid5 = _errs20 === errors;
                                      } else {
                                        var valid5 = true;
                                      }
                                    }
                                  }
                                }
                              } else {
                                validate151.errors = [
                                  {
                                    instancePath:
                                      instancePath + "/options/" + i0,
                                    schemaPath:
                                      "#/allOf/3/properties/options/items/type",
                                    keyword: "type",
                                    params: { type: "object" },
                                    message: "must be object",
                                  },
                                ];
                                return false;
                              }
                            }
                            var valid4 = _errs16 === errors;
                            if (!valid4) {
                              break;
                            }
                          }
                        } else {
                          validate151.errors = [
                            {
                              instancePath: instancePath + "/options",
                              schemaPath: "#/allOf/3/properties/options/type",
                              keyword: "type",
                              params: { type: "array" },
                              message: "must be array",
                            },
                          ];
                          return false;
                        }
                      }
                      var valid3 = _errs14 === errors;
                    } else {
                      var valid3 = true;
                    }
                    if (valid3) {
                      if (data.value !== undefined) {
                        const _errs22 = errors;
                        if (
                          !validate156(data.value, {
                            instancePath: instancePath + "/value",
                            parentData: data,
                            parentDataProperty: "value",
                            rootData,
                            dynamicAnchors,
                          })
                        ) {
                          vErrors =
                            vErrors === null
                              ? validate156.errors
                              : vErrors.concat(validate156.errors);
                          errors = vErrors.length;
                        }
                        var valid3 = _errs22 === errors;
                      } else {
                        var valid3 = true;
                      }
                      if (valid3) {
                        if (data.displayStyle !== undefined) {
                          let data9 = data.displayStyle;
                          const _errs23 = errors;
                          if (typeof data9 !== "string") {
                            validate151.errors = [
                              {
                                instancePath: instancePath + "/displayStyle",
                                schemaPath:
                                  "#/allOf/3/properties/displayStyle/type",
                                keyword: "type",
                                params: { type: "string" },
                                message: "must be string",
                              },
                            ];
                            return false;
                          }
                          if (!(data9 === "checkbox" || data9 === "chips")) {
                            validate151.errors = [
                              {
                                instancePath: instancePath + "/displayStyle",
                                schemaPath:
                                  "#/allOf/3/properties/displayStyle/enum",
                                keyword: "enum",
                                params: {
                                  allowedValues:
                                    schema111.allOf[3].properties.displayStyle
                                      .enum,
                                },
                                message:
                                  "must be equal to one of the allowed values",
                              },
                            ];
                            return false;
                          }
                          var valid3 = _errs23 === errors;
                        } else {
                          var valid3 = true;
                        }
                        if (valid3) {
                          if (data.filterable !== undefined) {
                            const _errs25 = errors;
                            if (typeof data.filterable !== "boolean") {
                              validate151.errors = [
                                {
                                  instancePath: instancePath + "/filterable",
                                  schemaPath:
                                    "#/allOf/3/properties/filterable/type",
                                  keyword: "type",
                                  params: { type: "boolean" },
                                  message: "must be boolean",
                                },
                              ];
                              return false;
                            }
                            var valid3 = _errs25 === errors;
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
          } else {
            validate151.errors = [
              {
                instancePath,
                schemaPath: "#/allOf/3/type",
                keyword: "type",
                params: { type: "object" },
                message: "must be object",
              },
            ];
            return false;
          }
        }
        var valid0 = _errs8 === errors;
      }
    }
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      for (const key1 in data) {
        if (
          key1 !== "component" &&
          key1 !== "label" &&
          key1 !== "variant" &&
          key1 !== "options" &&
          key1 !== "value" &&
          key1 !== "displayStyle" &&
          key1 !== "filterable" &&
          key1 !== "checks" &&
          key1 !== "weight" &&
          key1 !== "id" &&
          key1 !== "accessibility"
        ) {
          validate151.errors = [
            {
              instancePath,
              schemaPath: "#/unevaluatedProperties",
              keyword: "unevaluatedProperties",
              params: { unevaluatedProperty: key1 },
              message: "must NOT have unevaluated properties",
            },
          ];
          return false;
          break;
        }
      }
    } else {
      validate151.errors = [
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
  validate151.errors = vErrors;
  return errors === 0;
}
validate151.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema115 = {
  type: "object",
  allOf: [
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentCommon",
    },
    { $ref: "#/$defs/CatalogComponentCommon" },
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/Checkable",
    },
    {
      type: "object",
      properties: {
        component: { const: "Slider" },
        label: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
          description: "The label for the slider.",
        },
        min: {
          type: "number",
          description: "The minimum value of the slider.",
          default: 0,
        },
        max: {
          type: "number",
          description: "The maximum value of the slider.",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicNumber",
          description: "The current value of the slider.",
        },
      },
      required: ["component", "value", "max"],
    },
  ],
  unevaluatedProperties: false,
};
function validate160(
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
  const evaluated0 = validate160.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs1 = errors;
  if (
    !validate28(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate28.errors : vErrors.concat(validate28.errors);
    errors = vErrors.length;
  }
  var valid0 = _errs1 === errors;
  if (valid0) {
    const _errs2 = errors;
    const _errs3 = errors;
    if (errors === _errs3) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.weight !== undefined) {
          if (!(typeof data.weight == "number")) {
            validate160.errors = [
              {
                instancePath: instancePath + "/weight",
                schemaPath:
                  "#/$defs/CatalogComponentCommon/properties/weight/type",
                keyword: "type",
                params: { type: "number" },
                message: "must be number",
              },
            ];
            return false;
          }
        }
      } else {
        validate160.errors = [
          {
            instancePath,
            schemaPath: "#/$defs/CatalogComponentCommon/type",
            keyword: "type",
            params: { type: "object" },
            message: "must be object",
          },
        ];
        return false;
      }
    }
    var valid0 = _errs2 === errors;
    if (valid0) {
      const _errs7 = errors;
      if (
        !validate127(data, {
          instancePath,
          parentData,
          parentDataProperty,
          rootData,
          dynamicAnchors,
        })
      ) {
        vErrors =
          vErrors === null
            ? validate127.errors
            : vErrors.concat(validate127.errors);
        errors = vErrors.length;
      }
      var valid0 = _errs7 === errors;
      if (valid0) {
        const _errs8 = errors;
        if (errors === _errs8) {
          if (data && typeof data == "object" && !Array.isArray(data)) {
            let missing0;
            if (
              (data.component === undefined && (missing0 = "component")) ||
              (data.value === undefined && (missing0 = "value")) ||
              (data.max === undefined && (missing0 = "max"))
            ) {
              validate160.errors = [
                {
                  instancePath,
                  schemaPath: "#/allOf/3/required",
                  keyword: "required",
                  params: { missingProperty: missing0 },
                  message: "must have required property '" + missing0 + "'",
                },
              ];
              return false;
            } else {
              if (data.component !== undefined) {
                const _errs10 = errors;
                if ("Slider" !== data.component) {
                  validate160.errors = [
                    {
                      instancePath: instancePath + "/component",
                      schemaPath: "#/allOf/3/properties/component/const",
                      keyword: "const",
                      params: { allowedValue: "Slider" },
                      message: "must be equal to constant",
                    },
                  ];
                  return false;
                }
                var valid3 = _errs10 === errors;
              } else {
                var valid3 = true;
              }
              if (valid3) {
                if (data.label !== undefined) {
                  const _errs11 = errors;
                  if (
                    !validate30(data.label, {
                      instancePath: instancePath + "/label",
                      parentData: data,
                      parentDataProperty: "label",
                      rootData,
                      dynamicAnchors,
                    })
                  ) {
                    vErrors =
                      vErrors === null
                        ? validate30.errors
                        : vErrors.concat(validate30.errors);
                    errors = vErrors.length;
                  }
                  var valid3 = _errs11 === errors;
                } else {
                  var valid3 = true;
                }
                if (valid3) {
                  if (data.min !== undefined) {
                    const _errs12 = errors;
                    if (!(typeof data.min == "number")) {
                      validate160.errors = [
                        {
                          instancePath: instancePath + "/min",
                          schemaPath: "#/allOf/3/properties/min/type",
                          keyword: "type",
                          params: { type: "number" },
                          message: "must be number",
                        },
                      ];
                      return false;
                    }
                    var valid3 = _errs12 === errors;
                  } else {
                    var valid3 = true;
                  }
                  if (valid3) {
                    if (data.max !== undefined) {
                      const _errs14 = errors;
                      if (!(typeof data.max == "number")) {
                        validate160.errors = [
                          {
                            instancePath: instancePath + "/max",
                            schemaPath: "#/allOf/3/properties/max/type",
                            keyword: "type",
                            params: { type: "number" },
                            message: "must be number",
                          },
                        ];
                        return false;
                      }
                      var valid3 = _errs14 === errors;
                    } else {
                      var valid3 = true;
                    }
                    if (valid3) {
                      if (data.value !== undefined) {
                        const _errs16 = errors;
                        if (
                          !validate38(data.value, {
                            instancePath: instancePath + "/value",
                            parentData: data,
                            parentDataProperty: "value",
                            rootData,
                            dynamicAnchors,
                          })
                        ) {
                          vErrors =
                            vErrors === null
                              ? validate38.errors
                              : vErrors.concat(validate38.errors);
                          errors = vErrors.length;
                        }
                        var valid3 = _errs16 === errors;
                      } else {
                        var valid3 = true;
                      }
                    }
                  }
                }
              }
            }
          } else {
            validate160.errors = [
              {
                instancePath,
                schemaPath: "#/allOf/3/type",
                keyword: "type",
                params: { type: "object" },
                message: "must be object",
              },
            ];
            return false;
          }
        }
        var valid0 = _errs8 === errors;
      }
    }
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      for (const key0 in data) {
        if (
          key0 !== "component" &&
          key0 !== "label" &&
          key0 !== "min" &&
          key0 !== "max" &&
          key0 !== "value" &&
          key0 !== "checks" &&
          key0 !== "weight" &&
          key0 !== "id" &&
          key0 !== "accessibility"
        ) {
          validate160.errors = [
            {
              instancePath,
              schemaPath: "#/unevaluatedProperties",
              keyword: "unevaluatedProperties",
              params: { unevaluatedProperty: key0 },
              message: "must NOT have unevaluated properties",
            },
          ];
          return false;
          break;
        }
      }
    } else {
      validate160.errors = [
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
  validate160.errors = vErrors;
  return errors === 0;
}
validate160.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
};
const schema117 = {
  type: "object",
  allOf: [
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ComponentCommon",
    },
    { $ref: "#/$defs/CatalogComponentCommon" },
    {
      $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/Checkable",
    },
    {
      type: "object",
      properties: {
        component: { const: "DateTimeInput" },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
          description:
            "The selected date and/or time value in ISO 8601 format. If not yet set, initialize with an empty string.",
        },
        enableDate: {
          type: "boolean",
          description: "If true, allows the user to select a date.",
          default: false,
        },
        enableTime: {
          type: "boolean",
          description: "If true, allows the user to select a time.",
          default: false,
        },
        min: {
          allOf: [
            {
              $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
            },
            {
              if: { type: "string" },
              then: {
                oneOf: [
                  { format: "date" },
                  { format: "time" },
                  { format: "date-time" },
                ],
              },
            },
          ],
          description: "The minimum allowed date/time in ISO 8601 format.",
        },
        max: {
          allOf: [
            {
              $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
            },
            {
              if: { type: "string" },
              then: {
                oneOf: [
                  { format: "date" },
                  { format: "time" },
                  { format: "date-time" },
                ],
              },
            },
          ],
          description: "The maximum allowed date/time in ISO 8601 format.",
        },
        label: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicString",
          description: "The text label for the input field.",
        },
      },
      required: ["component", "value"],
    },
  ],
  unevaluatedProperties: false,
};
function validate166(
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
  const evaluated0 = validate166.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = undefined;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = undefined;
  }
  const _errs1 = errors;
  if (
    !validate28(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate28.errors : vErrors.concat(validate28.errors);
    errors = vErrors.length;
  }
  var valid0 = _errs1 === errors;
  if (valid0) {
    const _errs2 = errors;
    const _errs3 = errors;
    if (errors === _errs3) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.weight !== undefined) {
          if (!(typeof data.weight == "number")) {
            validate166.errors = [
              {
                instancePath: instancePath + "/weight",
                schemaPath:
                  "#/$defs/CatalogComponentCommon/properties/weight/type",
                keyword: "type",
                params: { type: "number" },
                message: "must be number",
              },
            ];
            return false;
          }
        }
      } else {
        validate166.errors = [
          {
            instancePath,
            schemaPath: "#/$defs/CatalogComponentCommon/type",
            keyword: "type",
            params: { type: "object" },
            message: "must be object",
          },
        ];
        return false;
      }
    }
    var valid0 = _errs2 === errors;
    if (valid0) {
      const _errs7 = errors;
      if (
        !validate127(data, {
          instancePath,
          parentData,
          parentDataProperty,
          rootData,
          dynamicAnchors,
        })
      ) {
        vErrors =
          vErrors === null
            ? validate127.errors
            : vErrors.concat(validate127.errors);
        errors = vErrors.length;
      }
      var valid0 = _errs7 === errors;
      if (valid0) {
        const _errs8 = errors;
        if (errors === _errs8) {
          if (data && typeof data == "object" && !Array.isArray(data)) {
            let missing0;
            if (
              (data.component === undefined && (missing0 = "component")) ||
              (data.value === undefined && (missing0 = "value"))
            ) {
              validate166.errors = [
                {
                  instancePath,
                  schemaPath: "#/allOf/3/required",
                  keyword: "required",
                  params: { missingProperty: missing0 },
                  message: "must have required property '" + missing0 + "'",
                },
              ];
              return false;
            } else {
              if (data.component !== undefined) {
                const _errs10 = errors;
                if ("DateTimeInput" !== data.component) {
                  validate166.errors = [
                    {
                      instancePath: instancePath + "/component",
                      schemaPath: "#/allOf/3/properties/component/const",
                      keyword: "const",
                      params: { allowedValue: "DateTimeInput" },
                      message: "must be equal to constant",
                    },
                  ];
                  return false;
                }
                var valid3 = _errs10 === errors;
              } else {
                var valid3 = true;
              }
              if (valid3) {
                if (data.value !== undefined) {
                  const _errs11 = errors;
                  if (
                    !validate30(data.value, {
                      instancePath: instancePath + "/value",
                      parentData: data,
                      parentDataProperty: "value",
                      rootData,
                      dynamicAnchors,
                    })
                  ) {
                    vErrors =
                      vErrors === null
                        ? validate30.errors
                        : vErrors.concat(validate30.errors);
                    errors = vErrors.length;
                  }
                  var valid3 = _errs11 === errors;
                } else {
                  var valid3 = true;
                }
                if (valid3) {
                  if (data.enableDate !== undefined) {
                    const _errs12 = errors;
                    if (typeof data.enableDate !== "boolean") {
                      validate166.errors = [
                        {
                          instancePath: instancePath + "/enableDate",
                          schemaPath: "#/allOf/3/properties/enableDate/type",
                          keyword: "type",
                          params: { type: "boolean" },
                          message: "must be boolean",
                        },
                      ];
                      return false;
                    }
                    var valid3 = _errs12 === errors;
                  } else {
                    var valid3 = true;
                  }
                  if (valid3) {
                    if (data.enableTime !== undefined) {
                      const _errs14 = errors;
                      if (typeof data.enableTime !== "boolean") {
                        validate166.errors = [
                          {
                            instancePath: instancePath + "/enableTime",
                            schemaPath: "#/allOf/3/properties/enableTime/type",
                            keyword: "type",
                            params: { type: "boolean" },
                            message: "must be boolean",
                          },
                        ];
                        return false;
                      }
                      var valid3 = _errs14 === errors;
                    } else {
                      var valid3 = true;
                    }
                    if (valid3) {
                      if (data.min !== undefined) {
                        let data5 = data.min;
                        const _errs16 = errors;
                        const _errs17 = errors;
                        if (
                          !validate30(data5, {
                            instancePath: instancePath + "/min",
                            parentData: data,
                            parentDataProperty: "min",
                            rootData,
                            dynamicAnchors,
                          })
                        ) {
                          vErrors =
                            vErrors === null
                              ? validate30.errors
                              : vErrors.concat(validate30.errors);
                          errors = vErrors.length;
                        }
                        var valid4 = _errs17 === errors;
                        if (valid4) {
                          const _errs18 = errors;
                          const _errs19 = errors;
                          let valid5 = true;
                          const _errs20 = errors;
                          if (typeof data5 !== "string") {
                            const err0 = {};
                            if (vErrors === null) {
                              vErrors = [err0];
                            } else {
                              vErrors.push(err0);
                            }
                            errors++;
                          }
                          var _valid0 = _errs20 === errors;
                          errors = _errs19;
                          if (vErrors !== null) {
                            if (_errs19) {
                              vErrors.length = _errs19;
                            } else {
                              vErrors = null;
                            }
                          }
                          if (_valid0) {
                            const _errs22 = errors;
                            const _errs23 = errors;
                            let valid6 = false;
                            let passing0 = null;
                            const _errs24 = errors;
                            var _valid1 = _errs24 === errors;
                            if (_valid1) {
                              valid6 = true;
                              passing0 = 0;
                            }
                            const _errs25 = errors;
                            var _valid1 = _errs25 === errors;
                            if (_valid1 && valid6) {
                              valid6 = false;
                              passing0 = [passing0, 1];
                            } else {
                              if (_valid1) {
                                valid6 = true;
                                passing0 = 1;
                              }
                              const _errs26 = errors;
                              var _valid1 = _errs26 === errors;
                              if (_valid1 && valid6) {
                                valid6 = false;
                                passing0 = [passing0, 2];
                              } else {
                                if (_valid1) {
                                  valid6 = true;
                                  passing0 = 2;
                                }
                              }
                            }
                            if (!valid6) {
                              const err1 = {
                                instancePath: instancePath + "/min",
                                schemaPath:
                                  "#/allOf/3/properties/min/allOf/1/then/oneOf",
                                keyword: "oneOf",
                                params: { passingSchemas: passing0 },
                                message:
                                  "must match exactly one schema in oneOf",
                              };
                              if (vErrors === null) {
                                vErrors = [err1];
                              } else {
                                vErrors.push(err1);
                              }
                              errors++;
                              validate166.errors = vErrors;
                              return false;
                            } else {
                              errors = _errs23;
                              if (vErrors !== null) {
                                if (_errs23) {
                                  vErrors.length = _errs23;
                                } else {
                                  vErrors = null;
                                }
                              }
                            }
                            var _valid0 = _errs22 === errors;
                            valid5 = _valid0;
                          }
                          if (!valid5) {
                            const err2 = {
                              instancePath: instancePath + "/min",
                              schemaPath: "#/allOf/3/properties/min/allOf/1/if",
                              keyword: "if",
                              params: { failingKeyword: "then" },
                              message: 'must match "then" schema',
                            };
                            if (vErrors === null) {
                              vErrors = [err2];
                            } else {
                              vErrors.push(err2);
                            }
                            errors++;
                            validate166.errors = vErrors;
                            return false;
                          }
                          var valid4 = _errs18 === errors;
                        }
                        var valid3 = _errs16 === errors;
                      } else {
                        var valid3 = true;
                      }
                      if (valid3) {
                        if (data.max !== undefined) {
                          let data6 = data.max;
                          const _errs27 = errors;
                          const _errs28 = errors;
                          if (
                            !validate30(data6, {
                              instancePath: instancePath + "/max",
                              parentData: data,
                              parentDataProperty: "max",
                              rootData,
                              dynamicAnchors,
                            })
                          ) {
                            vErrors =
                              vErrors === null
                                ? validate30.errors
                                : vErrors.concat(validate30.errors);
                            errors = vErrors.length;
                          }
                          var valid7 = _errs28 === errors;
                          if (valid7) {
                            const _errs29 = errors;
                            const _errs30 = errors;
                            let valid8 = true;
                            const _errs31 = errors;
                            if (typeof data6 !== "string") {
                              const err3 = {};
                              if (vErrors === null) {
                                vErrors = [err3];
                              } else {
                                vErrors.push(err3);
                              }
                              errors++;
                            }
                            var _valid2 = _errs31 === errors;
                            errors = _errs30;
                            if (vErrors !== null) {
                              if (_errs30) {
                                vErrors.length = _errs30;
                              } else {
                                vErrors = null;
                              }
                            }
                            if (_valid2) {
                              const _errs33 = errors;
                              const _errs34 = errors;
                              let valid9 = false;
                              let passing1 = null;
                              const _errs35 = errors;
                              var _valid3 = _errs35 === errors;
                              if (_valid3) {
                                valid9 = true;
                                passing1 = 0;
                              }
                              const _errs36 = errors;
                              var _valid3 = _errs36 === errors;
                              if (_valid3 && valid9) {
                                valid9 = false;
                                passing1 = [passing1, 1];
                              } else {
                                if (_valid3) {
                                  valid9 = true;
                                  passing1 = 1;
                                }
                                const _errs37 = errors;
                                var _valid3 = _errs37 === errors;
                                if (_valid3 && valid9) {
                                  valid9 = false;
                                  passing1 = [passing1, 2];
                                } else {
                                  if (_valid3) {
                                    valid9 = true;
                                    passing1 = 2;
                                  }
                                }
                              }
                              if (!valid9) {
                                const err4 = {
                                  instancePath: instancePath + "/max",
                                  schemaPath:
                                    "#/allOf/3/properties/max/allOf/1/then/oneOf",
                                  keyword: "oneOf",
                                  params: { passingSchemas: passing1 },
                                  message:
                                    "must match exactly one schema in oneOf",
                                };
                                if (vErrors === null) {
                                  vErrors = [err4];
                                } else {
                                  vErrors.push(err4);
                                }
                                errors++;
                                validate166.errors = vErrors;
                                return false;
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
                              var _valid2 = _errs33 === errors;
                              valid8 = _valid2;
                            }
                            if (!valid8) {
                              const err5 = {
                                instancePath: instancePath + "/max",
                                schemaPath:
                                  "#/allOf/3/properties/max/allOf/1/if",
                                keyword: "if",
                                params: { failingKeyword: "then" },
                                message: 'must match "then" schema',
                              };
                              if (vErrors === null) {
                                vErrors = [err5];
                              } else {
                                vErrors.push(err5);
                              }
                              errors++;
                              validate166.errors = vErrors;
                              return false;
                            }
                            var valid7 = _errs29 === errors;
                          }
                          var valid3 = _errs27 === errors;
                        } else {
                          var valid3 = true;
                        }
                        if (valid3) {
                          if (data.label !== undefined) {
                            const _errs38 = errors;
                            if (
                              !validate30(data.label, {
                                instancePath: instancePath + "/label",
                                parentData: data,
                                parentDataProperty: "label",
                                rootData,
                                dynamicAnchors,
                              })
                            ) {
                              vErrors =
                                vErrors === null
                                  ? validate30.errors
                                  : vErrors.concat(validate30.errors);
                              errors = vErrors.length;
                            }
                            var valid3 = _errs38 === errors;
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
          } else {
            validate166.errors = [
              {
                instancePath,
                schemaPath: "#/allOf/3/type",
                keyword: "type",
                params: { type: "object" },
                message: "must be object",
              },
            ];
            return false;
          }
        }
        var valid0 = _errs8 === errors;
      }
    }
  }
  if (errors === 0) {
    if (data && typeof data == "object" && !Array.isArray(data)) {
      for (const key0 in data) {
        if (
          key0 !== "component" &&
          key0 !== "value" &&
          key0 !== "enableDate" &&
          key0 !== "enableTime" &&
          key0 !== "min" &&
          key0 !== "max" &&
          key0 !== "label" &&
          key0 !== "checks" &&
          key0 !== "weight" &&
          key0 !== "id" &&
          key0 !== "accessibility"
        ) {
          validate166.errors = [
            {
              instancePath,
              schemaPath: "#/unevaluatedProperties",
              keyword: "unevaluatedProperties",
              params: { unevaluatedProperty: key0 },
              message: "must NOT have unevaluated properties",
            },
          ];
          return false;
          break;
        }
      }
    } else {
      validate166.errors = [
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
  validate166.errors = vErrors;
  return errors === 0;
}
validate166.evaluated = {
  props: true,
  dynamicProps: false,
  dynamicItems: false,
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
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (
    !validate26(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate26.errors : vErrors.concat(validate26.errors);
    errors = vErrors.length;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
    var props0 = true;
  }
  const _errs2 = errors;
  if (
    !validate82(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate82.errors : vErrors.concat(validate82.errors);
    errors = vErrors.length;
  }
  var _valid0 = _errs2 === errors;
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
    const _errs3 = errors;
    if (
      !validate87(data, {
        instancePath,
        parentData,
        parentDataProperty,
        rootData,
        dynamicAnchors,
      })
    ) {
      vErrors =
        vErrors === null
          ? validate87.errors
          : vErrors.concat(validate87.errors);
      errors = vErrors.length;
    }
    var _valid0 = _errs3 === errors;
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
      const _errs4 = errors;
      if (
        !validate90(data, {
          instancePath,
          parentData,
          parentDataProperty,
          rootData,
          dynamicAnchors,
        })
      ) {
        vErrors =
          vErrors === null
            ? validate90.errors
            : vErrors.concat(validate90.errors);
        errors = vErrors.length;
      }
      var _valid0 = _errs4 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
          if (props0 !== true) {
            props0 = true;
          }
        }
        const _errs5 = errors;
        if (
          !validate94(data, {
            instancePath,
            parentData,
            parentDataProperty,
            rootData,
            dynamicAnchors,
          })
        ) {
          vErrors =
            vErrors === null
              ? validate94.errors
              : vErrors.concat(validate94.errors);
          errors = vErrors.length;
        }
        var _valid0 = _errs5 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
            if (props0 !== true) {
              props0 = true;
            }
          }
          const _errs6 = errors;
          if (
            !validate99(data, {
              instancePath,
              parentData,
              parentDataProperty,
              rootData,
              dynamicAnchors,
            })
          ) {
            vErrors =
              vErrors === null
                ? validate99.errors
                : vErrors.concat(validate99.errors);
            errors = vErrors.length;
          }
          var _valid0 = _errs6 === errors;
          if (_valid0 && valid0) {
            valid0 = false;
            passing0 = [passing0, 5];
          } else {
            if (_valid0) {
              valid0 = true;
              passing0 = 5;
              if (props0 !== true) {
                props0 = true;
              }
            }
            const _errs7 = errors;
            if (
              !validate104(data, {
                instancePath,
                parentData,
                parentDataProperty,
                rootData,
                dynamicAnchors,
              })
            ) {
              vErrors =
                vErrors === null
                  ? validate104.errors
                  : vErrors.concat(validate104.errors);
              errors = vErrors.length;
            }
            var _valid0 = _errs7 === errors;
            if (_valid0 && valid0) {
              valid0 = false;
              passing0 = [passing0, 6];
            } else {
              if (_valid0) {
                valid0 = true;
                passing0 = 6;
                if (props0 !== true) {
                  props0 = true;
                }
              }
              const _errs8 = errors;
              if (
                !validate108(data, {
                  instancePath,
                  parentData,
                  parentDataProperty,
                  rootData,
                  dynamicAnchors,
                })
              ) {
                vErrors =
                  vErrors === null
                    ? validate108.errors
                    : vErrors.concat(validate108.errors);
                errors = vErrors.length;
              }
              var _valid0 = _errs8 === errors;
              if (_valid0 && valid0) {
                valid0 = false;
                passing0 = [passing0, 7];
              } else {
                if (_valid0) {
                  valid0 = true;
                  passing0 = 7;
                  if (props0 !== true) {
                    props0 = true;
                  }
                }
                const _errs9 = errors;
                if (
                  !validate112(data, {
                    instancePath,
                    parentData,
                    parentDataProperty,
                    rootData,
                    dynamicAnchors,
                  })
                ) {
                  vErrors =
                    vErrors === null
                      ? validate112.errors
                      : vErrors.concat(validate112.errors);
                  errors = vErrors.length;
                }
                var _valid0 = _errs9 === errors;
                if (_valid0 && valid0) {
                  valid0 = false;
                  passing0 = [passing0, 8];
                } else {
                  if (_valid0) {
                    valid0 = true;
                    passing0 = 8;
                    if (props0 !== true) {
                      props0 = true;
                    }
                  }
                  const _errs10 = errors;
                  if (
                    !validate115(data, {
                      instancePath,
                      parentData,
                      parentDataProperty,
                      rootData,
                      dynamicAnchors,
                    })
                  ) {
                    vErrors =
                      vErrors === null
                        ? validate115.errors
                        : vErrors.concat(validate115.errors);
                    errors = vErrors.length;
                  }
                  var _valid0 = _errs10 === errors;
                  if (_valid0 && valid0) {
                    valid0 = false;
                    passing0 = [passing0, 9];
                  } else {
                    if (_valid0) {
                      valid0 = true;
                      passing0 = 9;
                      if (props0 !== true) {
                        props0 = true;
                      }
                    }
                    const _errs11 = errors;
                    if (
                      !validate119(data, {
                        instancePath,
                        parentData,
                        parentDataProperty,
                        rootData,
                        dynamicAnchors,
                      })
                    ) {
                      vErrors =
                        vErrors === null
                          ? validate119.errors
                          : vErrors.concat(validate119.errors);
                      errors = vErrors.length;
                    }
                    var _valid0 = _errs11 === errors;
                    if (_valid0 && valid0) {
                      valid0 = false;
                      passing0 = [passing0, 10];
                    } else {
                      if (_valid0) {
                        valid0 = true;
                        passing0 = 10;
                        if (props0 !== true) {
                          props0 = true;
                        }
                      }
                      const _errs12 = errors;
                      if (
                        !validate122(data, {
                          instancePath,
                          parentData,
                          parentDataProperty,
                          rootData,
                          dynamicAnchors,
                        })
                      ) {
                        vErrors =
                          vErrors === null
                            ? validate122.errors
                            : vErrors.concat(validate122.errors);
                        errors = vErrors.length;
                      }
                      var _valid0 = _errs12 === errors;
                      if (_valid0 && valid0) {
                        valid0 = false;
                        passing0 = [passing0, 11];
                      } else {
                        if (_valid0) {
                          valid0 = true;
                          passing0 = 11;
                          if (props0 !== true) {
                            props0 = true;
                          }
                        }
                        const _errs13 = errors;
                        if (
                          !validate125(data, {
                            instancePath,
                            parentData,
                            parentDataProperty,
                            rootData,
                            dynamicAnchors,
                          })
                        ) {
                          vErrors =
                            vErrors === null
                              ? validate125.errors
                              : vErrors.concat(validate125.errors);
                          errors = vErrors.length;
                        }
                        var _valid0 = _errs13 === errors;
                        if (_valid0 && valid0) {
                          valid0 = false;
                          passing0 = [passing0, 12];
                        } else {
                          if (_valid0) {
                            valid0 = true;
                            passing0 = 12;
                            if (props0 !== true) {
                              props0 = true;
                            }
                          }
                          const _errs14 = errors;
                          if (
                            !validate139(data, {
                              instancePath,
                              parentData,
                              parentDataProperty,
                              rootData,
                              dynamicAnchors,
                            })
                          ) {
                            vErrors =
                              vErrors === null
                                ? validate139.errors
                                : vErrors.concat(validate139.errors);
                            errors = vErrors.length;
                          }
                          var _valid0 = _errs14 === errors;
                          if (_valid0 && valid0) {
                            valid0 = false;
                            passing0 = [passing0, 13];
                          } else {
                            if (_valid0) {
                              valid0 = true;
                              passing0 = 13;
                              if (props0 !== true) {
                                props0 = true;
                              }
                            }
                            const _errs15 = errors;
                            if (
                              !validate145(data, {
                                instancePath,
                                parentData,
                                parentDataProperty,
                                rootData,
                                dynamicAnchors,
                              })
                            ) {
                              vErrors =
                                vErrors === null
                                  ? validate145.errors
                                  : vErrors.concat(validate145.errors);
                              errors = vErrors.length;
                            }
                            var _valid0 = _errs15 === errors;
                            if (_valid0 && valid0) {
                              valid0 = false;
                              passing0 = [passing0, 14];
                            } else {
                              if (_valid0) {
                                valid0 = true;
                                passing0 = 14;
                                if (props0 !== true) {
                                  props0 = true;
                                }
                              }
                              const _errs16 = errors;
                              if (
                                !validate151(data, {
                                  instancePath,
                                  parentData,
                                  parentDataProperty,
                                  rootData,
                                  dynamicAnchors,
                                })
                              ) {
                                vErrors =
                                  vErrors === null
                                    ? validate151.errors
                                    : vErrors.concat(validate151.errors);
                                errors = vErrors.length;
                              }
                              var _valid0 = _errs16 === errors;
                              if (_valid0 && valid0) {
                                valid0 = false;
                                passing0 = [passing0, 15];
                              } else {
                                if (_valid0) {
                                  valid0 = true;
                                  passing0 = 15;
                                  if (props0 !== true) {
                                    props0 = true;
                                  }
                                }
                                const _errs17 = errors;
                                if (
                                  !validate160(data, {
                                    instancePath,
                                    parentData,
                                    parentDataProperty,
                                    rootData,
                                    dynamicAnchors,
                                  })
                                ) {
                                  vErrors =
                                    vErrors === null
                                      ? validate160.errors
                                      : vErrors.concat(validate160.errors);
                                  errors = vErrors.length;
                                }
                                var _valid0 = _errs17 === errors;
                                if (_valid0 && valid0) {
                                  valid0 = false;
                                  passing0 = [passing0, 16];
                                } else {
                                  if (_valid0) {
                                    valid0 = true;
                                    passing0 = 16;
                                    if (props0 !== true) {
                                      props0 = true;
                                    }
                                  }
                                  const _errs18 = errors;
                                  if (
                                    !validate166(data, {
                                      instancePath,
                                      parentData,
                                      parentDataProperty,
                                      rootData,
                                      dynamicAnchors,
                                    })
                                  ) {
                                    vErrors =
                                      vErrors === null
                                        ? validate166.errors
                                        : vErrors.concat(validate166.errors);
                                    errors = vErrors.length;
                                  }
                                  var _valid0 = _errs18 === errors;
                                  if (_valid0 && valid0) {
                                    valid0 = false;
                                    passing0 = [passing0, 17];
                                  } else {
                                    if (_valid0) {
                                      valid0 = true;
                                      passing0 = 17;
                                      if (props0 !== true) {
                                        props0 = true;
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
  if (!valid0) {
    const err0 = {
      instancePath,
      schemaPath: "#/oneOf",
      keyword: "oneOf",
      params: { passingSchemas: passing0 },
      message: "must match exactly one schema in oneOf",
    };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
    validate25.errors = vErrors;
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
  validate25.errors = vErrors;
  evaluated0.props = props0;
  return errors === 0;
}
validate25.evaluated = { dynamicProps: true, dynamicItems: false };
function validate24(
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
  const evaluated0 = validate24.evaluated;
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
        (data.updateComponents === undefined &&
          (missing0 = "updateComponents")) ||
        (data.version === undefined && (missing0 = "version"))
      ) {
        validate24.errors = [
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
          if (!(key0 === "version" || key0 === "updateComponents")) {
            validate24.errors = [
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
          if (data.version !== undefined) {
            let data0 = data.version;
            const _errs2 = errors;
            if (!(data0 === "v0.9" || data0 === "v0.9.1")) {
              validate24.errors = [
                {
                  instancePath: instancePath + "/version",
                  schemaPath: "#/properties/version/enum",
                  keyword: "enum",
                  params: { allowedValues: schema35.properties.version.enum },
                  message: "must be equal to one of the allowed values",
                },
              ];
              return false;
            }
            var valid0 = _errs2 === errors;
          } else {
            var valid0 = true;
          }
          if (valid0) {
            if (data.updateComponents !== undefined) {
              let data1 = data.updateComponents;
              const _errs3 = errors;
              if (errors === _errs3) {
                if (
                  data1 &&
                  typeof data1 == "object" &&
                  !Array.isArray(data1)
                ) {
                  let missing1;
                  if (
                    (data1.surfaceId === undefined &&
                      (missing1 = "surfaceId")) ||
                    (data1.components === undefined &&
                      (missing1 = "components"))
                  ) {
                    validate24.errors = [
                      {
                        instancePath: instancePath + "/updateComponents",
                        schemaPath: "#/properties/updateComponents/required",
                        keyword: "required",
                        params: { missingProperty: missing1 },
                        message:
                          "must have required property '" + missing1 + "'",
                      },
                    ];
                    return false;
                  } else {
                    const _errs5 = errors;
                    for (const key1 in data1) {
                      if (!(key1 === "surfaceId" || key1 === "components")) {
                        validate24.errors = [
                          {
                            instancePath: instancePath + "/updateComponents",
                            schemaPath:
                              "#/properties/updateComponents/additionalProperties",
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
                      if (data1.surfaceId !== undefined) {
                        const _errs6 = errors;
                        if (typeof data1.surfaceId !== "string") {
                          validate24.errors = [
                            {
                              instancePath:
                                instancePath + "/updateComponents/surfaceId",
                              schemaPath:
                                "#/properties/updateComponents/properties/surfaceId/type",
                              keyword: "type",
                              params: { type: "string" },
                              message: "must be string",
                            },
                          ];
                          return false;
                        }
                        var valid1 = _errs6 === errors;
                      } else {
                        var valid1 = true;
                      }
                      if (valid1) {
                        if (data1.components !== undefined) {
                          let data3 = data1.components;
                          const _errs8 = errors;
                          if (errors === _errs8) {
                            if (Array.isArray(data3)) {
                              if (data3.length < 1) {
                                validate24.errors = [
                                  {
                                    instancePath:
                                      instancePath +
                                      "/updateComponents/components",
                                    schemaPath:
                                      "#/properties/updateComponents/properties/components/minItems",
                                    keyword: "minItems",
                                    params: { limit: 1 },
                                    message: "must NOT have fewer than 1 items",
                                  },
                                ];
                                return false;
                              } else {
                                var valid2 = true;
                                const len0 = data3.length;
                                for (let i0 = 0; i0 < len0; i0++) {
                                  const _errs10 = errors;
                                  if (
                                    !validate25(data3[i0], {
                                      instancePath:
                                        instancePath +
                                        "/updateComponents/components/" +
                                        i0,
                                      parentData: data3,
                                      parentDataProperty: i0,
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
                                  var valid2 = _errs10 === errors;
                                  if (!valid2) {
                                    break;
                                  }
                                }
                              }
                            } else {
                              validate24.errors = [
                                {
                                  instancePath:
                                    instancePath +
                                    "/updateComponents/components",
                                  schemaPath:
                                    "#/properties/updateComponents/properties/components/type",
                                  keyword: "type",
                                  params: { type: "array" },
                                  message: "must be array",
                                },
                              ];
                              return false;
                            }
                          }
                          var valid1 = _errs8 === errors;
                        } else {
                          var valid1 = true;
                        }
                      }
                    }
                  }
                } else {
                  validate24.errors = [
                    {
                      instancePath: instancePath + "/updateComponents",
                      schemaPath: "#/properties/updateComponents/type",
                      keyword: "type",
                      params: { type: "object" },
                      message: "must be object",
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
      validate24.errors = [
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
  validate24.errors = vErrors;
  return errors === 0;
}
validate24.evaluated = {
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
  /*# sourceURL="https://a2ui.org/specification/v0_9/server_to_client.json" */ let vErrors =
    null;
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
  var _valid0 = _errs2 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
    var props0 = true;
  }
  const _errs3 = errors;
  if (
    !validate24(data, {
      instancePath,
      parentData,
      parentDataProperty,
      rootData,
      dynamicAnchors,
    })
  ) {
    vErrors =
      vErrors === null ? validate24.errors : vErrors.concat(validate24.errors);
    errors = vErrors.length;
  }
  var _valid0 = _errs3 === errors;
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
    const _errs4 = errors;
    const _errs5 = errors;
    if (errors === _errs5) {
      if (data && typeof data == "object" && !Array.isArray(data)) {
        let missing0;
        if (
          (data.updateDataModel === undefined &&
            (missing0 = "updateDataModel")) ||
          (data.version === undefined && (missing0 = "version"))
        ) {
          const err0 = {
            instancePath,
            schemaPath: "#/$defs/UpdateDataModelMessage/required",
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
          const _errs7 = errors;
          for (const key0 in data) {
            if (!(key0 === "version" || key0 === "updateDataModel")) {
              const err1 = {
                instancePath,
                schemaPath:
                  "#/$defs/UpdateDataModelMessage/additionalProperties",
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
          if (_errs7 === errors) {
            if (data.version !== undefined) {
              let data0 = data.version;
              const _errs8 = errors;
              if (!(data0 === "v0.9" || data0 === "v0.9.1")) {
                const err2 = {
                  instancePath: instancePath + "/version",
                  schemaPath:
                    "#/$defs/UpdateDataModelMessage/properties/version/enum",
                  keyword: "enum",
                  params: { allowedValues: schema119.properties.version.enum },
                  message: "must be equal to one of the allowed values",
                };
                if (vErrors === null) {
                  vErrors = [err2];
                } else {
                  vErrors.push(err2);
                }
                errors++;
              }
              var valid2 = _errs8 === errors;
            } else {
              var valid2 = true;
            }
            if (valid2) {
              if (data.updateDataModel !== undefined) {
                let data1 = data.updateDataModel;
                const _errs9 = errors;
                if (errors === _errs9) {
                  if (
                    data1 &&
                    typeof data1 == "object" &&
                    !Array.isArray(data1)
                  ) {
                    let missing1;
                    if (
                      data1.surfaceId === undefined &&
                      (missing1 = "surfaceId")
                    ) {
                      const err3 = {
                        instancePath: instancePath + "/updateDataModel",
                        schemaPath:
                          "#/$defs/UpdateDataModelMessage/properties/updateDataModel/required",
                        keyword: "required",
                        params: { missingProperty: missing1 },
                        message:
                          "must have required property '" + missing1 + "'",
                      };
                      if (vErrors === null) {
                        vErrors = [err3];
                      } else {
                        vErrors.push(err3);
                      }
                      errors++;
                    } else {
                      const _errs11 = errors;
                      for (const key1 in data1) {
                        if (
                          !(
                            key1 === "surfaceId" ||
                            key1 === "path" ||
                            key1 === "value"
                          )
                        ) {
                          const err4 = {
                            instancePath: instancePath + "/updateDataModel",
                            schemaPath:
                              "#/$defs/UpdateDataModelMessage/properties/updateDataModel/additionalProperties",
                            keyword: "additionalProperties",
                            params: { additionalProperty: key1 },
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
                      if (_errs11 === errors) {
                        if (data1.surfaceId !== undefined) {
                          const _errs12 = errors;
                          if (typeof data1.surfaceId !== "string") {
                            const err5 = {
                              instancePath:
                                instancePath + "/updateDataModel/surfaceId",
                              schemaPath:
                                "#/$defs/UpdateDataModelMessage/properties/updateDataModel/properties/surfaceId/type",
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
                          var valid3 = _errs12 === errors;
                        } else {
                          var valid3 = true;
                        }
                        if (valid3) {
                          if (data1.path !== undefined) {
                            const _errs14 = errors;
                            if (typeof data1.path !== "string") {
                              const err6 = {
                                instancePath:
                                  instancePath + "/updateDataModel/path",
                                schemaPath:
                                  "#/$defs/UpdateDataModelMessage/properties/updateDataModel/properties/path/type",
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
                            var valid3 = _errs14 === errors;
                          } else {
                            var valid3 = true;
                          }
                          if (valid3) {
                            if (data1.value !== undefined) {
                              const _errs16 = errors;
                              var valid3 = _errs16 === errors;
                            } else {
                              var valid3 = true;
                            }
                          }
                        }
                      }
                    }
                  } else {
                    const err7 = {
                      instancePath: instancePath + "/updateDataModel",
                      schemaPath:
                        "#/$defs/UpdateDataModelMessage/properties/updateDataModel/type",
                      keyword: "type",
                      params: { type: "object" },
                      message: "must be object",
                    };
                    if (vErrors === null) {
                      vErrors = [err7];
                    } else {
                      vErrors.push(err7);
                    }
                    errors++;
                  }
                }
                var valid2 = _errs9 === errors;
              } else {
                var valid2 = true;
              }
            }
          }
        }
      } else {
        const err8 = {
          instancePath,
          schemaPath: "#/$defs/UpdateDataModelMessage/type",
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
    var _valid0 = _errs4 === errors;
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
      const _errs18 = errors;
      const _errs19 = errors;
      if (errors === _errs19) {
        if (data && typeof data == "object" && !Array.isArray(data)) {
          let missing2;
          if (
            (data.deleteSurface === undefined &&
              (missing2 = "deleteSurface")) ||
            (data.version === undefined && (missing2 = "version"))
          ) {
            const err9 = {
              instancePath,
              schemaPath: "#/$defs/DeleteSurfaceMessage/required",
              keyword: "required",
              params: { missingProperty: missing2 },
              message: "must have required property '" + missing2 + "'",
            };
            if (vErrors === null) {
              vErrors = [err9];
            } else {
              vErrors.push(err9);
            }
            errors++;
          } else {
            const _errs21 = errors;
            for (const key2 in data) {
              if (!(key2 === "version" || key2 === "deleteSurface")) {
                const err10 = {
                  instancePath,
                  schemaPath:
                    "#/$defs/DeleteSurfaceMessage/additionalProperties",
                  keyword: "additionalProperties",
                  params: { additionalProperty: key2 },
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
            if (_errs21 === errors) {
              if (data.version !== undefined) {
                let data5 = data.version;
                const _errs22 = errors;
                if (!(data5 === "v0.9" || data5 === "v0.9.1")) {
                  const err11 = {
                    instancePath: instancePath + "/version",
                    schemaPath:
                      "#/$defs/DeleteSurfaceMessage/properties/version/enum",
                    keyword: "enum",
                    params: {
                      allowedValues: schema120.properties.version.enum,
                    },
                    message: "must be equal to one of the allowed values",
                  };
                  if (vErrors === null) {
                    vErrors = [err11];
                  } else {
                    vErrors.push(err11);
                  }
                  errors++;
                }
                var valid5 = _errs22 === errors;
              } else {
                var valid5 = true;
              }
              if (valid5) {
                if (data.deleteSurface !== undefined) {
                  let data6 = data.deleteSurface;
                  const _errs23 = errors;
                  if (errors === _errs23) {
                    if (
                      data6 &&
                      typeof data6 == "object" &&
                      !Array.isArray(data6)
                    ) {
                      let missing3;
                      if (
                        data6.surfaceId === undefined &&
                        (missing3 = "surfaceId")
                      ) {
                        const err12 = {
                          instancePath: instancePath + "/deleteSurface",
                          schemaPath:
                            "#/$defs/DeleteSurfaceMessage/properties/deleteSurface/required",
                          keyword: "required",
                          params: { missingProperty: missing3 },
                          message:
                            "must have required property '" + missing3 + "'",
                        };
                        if (vErrors === null) {
                          vErrors = [err12];
                        } else {
                          vErrors.push(err12);
                        }
                        errors++;
                      } else {
                        const _errs25 = errors;
                        for (const key3 in data6) {
                          if (!(key3 === "surfaceId")) {
                            const err13 = {
                              instancePath: instancePath + "/deleteSurface",
                              schemaPath:
                                "#/$defs/DeleteSurfaceMessage/properties/deleteSurface/additionalProperties",
                              keyword: "additionalProperties",
                              params: { additionalProperty: key3 },
                              message: "must NOT have additional properties",
                            };
                            if (vErrors === null) {
                              vErrors = [err13];
                            } else {
                              vErrors.push(err13);
                            }
                            errors++;
                            break;
                          }
                        }
                        if (_errs25 === errors) {
                          if (data6.surfaceId !== undefined) {
                            if (typeof data6.surfaceId !== "string") {
                              const err14 = {
                                instancePath:
                                  instancePath + "/deleteSurface/surfaceId",
                                schemaPath:
                                  "#/$defs/DeleteSurfaceMessage/properties/deleteSurface/properties/surfaceId/type",
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
                        }
                      }
                    } else {
                      const err15 = {
                        instancePath: instancePath + "/deleteSurface",
                        schemaPath:
                          "#/$defs/DeleteSurfaceMessage/properties/deleteSurface/type",
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
                  var valid5 = _errs23 === errors;
                } else {
                  var valid5 = true;
                }
              }
            }
          }
        } else {
          const err16 = {
            instancePath,
            schemaPath: "#/$defs/DeleteSurfaceMessage/type",
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
      var _valid0 = _errs18 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
          if (props0 !== true) {
            props0 = true;
          }
        }
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
