; Python Symbol Extraction Queries for Coderev
; These queries extract symbols for the Universal Code Intelligence Substrate

; ============ CALLABLE (Functions and Methods) ============

; Top-level function definitions
(function_definition
  name: (identifier) @callable.name
  parameters: (parameters) @callable.params
  return_type: (type)? @callable.return_type
  body: (block) @callable.body) @callable.def

; Method definitions inside classes
(class_definition
  body: (block
    (function_definition
      name: (identifier) @method.name
      parameters: (parameters) @method.params
      return_type: (type)? @method.return_type
      body: (block) @method.body) @method.def))

; Async function definitions
(function_definition
  "async" @callable.async
  name: (identifier) @callable.name
  parameters: (parameters) @callable.params
  return_type: (type)? @callable.return_type
  body: (block) @callable.body) @callable.def

; ============ CONTAINER (Classes) ============

; Class definitions
(class_definition
  name: (identifier) @container.name
  superclasses: (argument_list)? @container.bases
  body: (block) @container.body) @container.def

; ============ VALUE (Variables and Constants) ============

; Module-level assignments (potential constants)
(module
  (expression_statement
    (assignment
      left: (identifier) @value.name
      right: (_) @value.init))) @value.def

; Class attributes
(class_definition
  body: (block
    (expression_statement
      (assignment
        left: (identifier) @class_attr.name
        right: (_) @class_attr.init)))) @class_attr.def

; ============ IMPORTS ============

; import x
(import_statement
  name: (dotted_name) @import.module) @import.def

; from x import y
(import_from_statement
  module_name: (dotted_name)? @import.from_module
  name: (dotted_name) @import.name) @import.def

; from x import y as z
(import_from_statement
  name: (aliased_import
    name: (dotted_name) @import.name
    alias: (identifier) @import.alias)) @import.def

; ============ CALLS ============

; Function/method calls
(call
  function: (identifier) @call.name
  arguments: (argument_list) @call.args) @call.expr

; Attribute calls (obj.method())
(call
  function: (attribute
    object: (_) @call.receiver
    attribute: (identifier) @call.name)
  arguments: (argument_list) @call.args) @call.expr

; ============ INHERITANCE ============

; Class base/parent classes
(class_definition
  superclasses: (argument_list
    (identifier) @inherits.base)) @inherits.class

; ============ DOCSTRINGS ============

; Function docstrings
(function_definition
  body: (block
    . (expression_statement
        (string) @docstring.function)))

; Class docstrings
(class_definition
  body: (block
    . (expression_statement
        (string) @docstring.class)))

; Module docstrings
(module
  . (expression_statement
      (string) @docstring.module))
