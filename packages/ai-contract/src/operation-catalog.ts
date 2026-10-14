// @generated from operation.schema.json and fixed upstream catalog.
export const operationCatalog = {
  catalogId: "urn:rss-mdm-agent:a2ui:operation:2",
  components: {
    Text: {
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
    },
    Column: {
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
    },
    Button: {
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
    },
    TextField: {
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
    },
    OperationSurface: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "OperationSurface" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    Section: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "Section" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    FieldGroup: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "FieldGroup" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    Tabs: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "Tabs" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    Disclosure: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "Disclosure" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    NumberField: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "NumberField" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    SelectField: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "SelectField" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    Switch: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "Switch" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    Checkbox: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "Checkbox" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    RadioGroup: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "RadioGroup" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    PathPicker: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "PathPicker" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    DateTimeField: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "DateTimeField" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    SecretField: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "SecretField" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    ReadonlyRow: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "ReadonlyRow" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    ObjectList: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "ObjectList" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    SelectableTable: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "SelectableTable" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    ChangePreview: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "ChangePreview" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    Notice: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "Notice" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    Progress: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "Progress" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    ResultSummary: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "ResultSummary" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    LogView: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "LogView" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    ActionBar: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "ActionBar" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    ConfirmationPanel: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "ConfirmationPanel" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    ApprovalStatus: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "ApprovalStatus" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    StepFlow: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "StepFlow" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
    PendingInteraction: {
      type: "object",
      properties: {
        id: { type: "string" },
        component: { const: "PendingInteraction" },
        label: { type: "string" },
        fieldId: { type: "string" },
        children: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/ChildList",
        },
        value: {
          $ref: "https://a2ui.org/specification/v0_9/common_types.json#/$defs/DynamicValue",
        },
      },
      required: ["id", "component"],
      additionalProperties: false,
    },
  },
  functions: {},
  $defs: {
    CatalogComponentCommon: {
      type: "object",
      properties: {
        weight: {
          type: "number",
          description:
            "The relative weight of this component within a Row or Column. This is similar to the CSS 'flex-grow' property. Note: this may ONLY be set when the component is a direct descendant of a Row or Column.",
        },
      },
    },
  },
};
