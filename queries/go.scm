; Go Symbol Extraction Queries for Coderev

; ============ CALLABLE (Functions) ============

; Function declarations
(function_declaration
  name: (identifier) @callable.name
  parameters: (parameter_list) @callable.params
  result: (_)? @callable.return_type
  body: (block) @callable.body) @callable.def

; Method declarations (receiver functions)
(method_declaration
  receiver: (parameter_list) @method.receiver
  name: (field_identifier) @method.name
  parameters: (parameter_list) @method.params
  result: (_)? @method.return_type
  body: (block) @method.body) @method.def

; ============ CONTAINER (Structs, Interfaces) ============

; Struct type definitions
(type_declaration
  (type_spec
    name: (type_identifier) @container.name
    type: (struct_type
      (field_declaration_list) @container.body))) @container.def

; Interface type definitions
(type_declaration
  (type_spec
    name: (type_identifier) @container.name
    type: (interface_type
      (method_spec_list)? @container.body))) @container.def

; Type aliases
(type_declaration
  (type_spec
    name: (type_identifier) @container.name
    type: (_) @container.alias)) @container.def

; ============ IMPORTS ============

; Single import
(import_declaration
  (import_spec
    path: (interpreted_string_literal) @import.module)) @import.def

; Named import
(import_declaration
  (import_spec
    name: (package_identifier) @import.alias
    path: (interpreted_string_literal) @import.module)) @import.def

; Import block
(import_declaration
  (import_spec_list
    (import_spec
      path: (interpreted_string_literal) @import.module))) @import.def

; ============ CALLS ============

; Function calls
(call_expression
  function: (identifier) @call.name
  arguments: (argument_list) @call.args) @call.expr

; Method calls
(call_expression
  function: (selector_expression
    operand: (_) @call.receiver
    field: (field_identifier) @call.name)
  arguments: (argument_list) @call.args) @call.expr

; Package function calls
(call_expression
  function: (selector_expression
    operand: (identifier) @call.receiver
    field: (field_identifier) @call.name)
  arguments: (argument_list) @call.args) @call.expr

; ============ INTERFACE EMBEDDING ============

; Embedded interface
(interface_type
  (method_spec_list
    (type_identifier) @inherits.base))

; Struct embedding
(struct_type
  (field_declaration_list
    (field_declaration
      type: (type_identifier) @inherits.base
      !name)))

; ============ CONSTANTS/VARIABLES ============

; const declarations
(const_declaration
  (const_spec
    name: (identifier) @value.name
    type: (_)? @value.type
    value: (expression_list) @value.init)) @value.def

; var declarations
(var_declaration
  (var_spec
    name: (identifier) @value.name
    type: (_)? @value.type
    value: (expression_list)? @value.init)) @value.def

; Package declaration
(package_clause
  (package_identifier) @namespace.name) @namespace.def
