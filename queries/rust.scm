; Rust Symbol Extraction Queries for Coderev

; ============ CALLABLE (Functions) ============

; Function definitions
(function_item
  name: (identifier) @callable.name
  parameters: (parameters) @callable.params
  return_type: (type_identifier)? @callable.return_type
  body: (block) @callable.body) @callable.def

; Method definitions in impl blocks
(impl_item
  body: (declaration_list
    (function_item
      name: (identifier) @method.name
      parameters: (parameters) @method.params
      return_type: (_)? @method.return_type
      body: (block) @method.body) @method.def))

; Trait method definitions
(trait_item
  body: (declaration_list
    (function_signature_item
      name: (identifier) @method.name
      parameters: (parameters) @method.params
      return_type: (_)? @method.return_type) @method.def))

; ============ CONTAINER (Structs, Enums, Traits) ============

; Struct definitions
(struct_item
  name: (type_identifier) @container.name
  body: (field_declaration_list)? @container.body) @container.def

; Enum definitions
(enum_item
  name: (type_identifier) @container.name
  body: (enum_variant_list) @container.body) @container.def

; Trait definitions
(trait_item
  name: (type_identifier) @container.name
  body: (declaration_list) @container.body) @container.def

; Impl blocks (for the type being implemented)
(impl_item
  type: (type_identifier) @container.name
  body: (declaration_list) @container.body) @container.def

; ============ IMPORTS ============

; use statements
(use_declaration
  argument: (scoped_identifier
    path: (identifier) @import.module
    name: (identifier) @import.name)) @import.def

; use mod::*
(use_declaration
  argument: (use_wildcard
    (scoped_identifier) @import.module)) @import.def

; use mod::{a, b}
(use_declaration
  argument: (scoped_use_list
    path: (identifier) @import.module
    list: (use_list) @import.symbols)) @import.def

; mod declarations
(mod_item
  name: (identifier) @import.module) @import.def

; ============ CALLS ============

; Function calls
(call_expression
  function: (identifier) @call.name
  arguments: (arguments) @call.args) @call.expr

; Method calls
(call_expression
  function: (field_expression
    value: (_) @call.receiver
    field: (field_identifier) @call.name)
  arguments: (arguments) @call.args) @call.expr

; Scoped function calls (e.g., Module::function())
(call_expression
  function: (scoped_identifier
    path: (_) @call.receiver
    name: (identifier) @call.name)
  arguments: (arguments) @call.args) @call.expr

; Macro invocations
(macro_invocation
  macro: (identifier) @call.name) @call.expr

; ============ INHERITANCE (Trait Bounds) ============

; Trait implementations
(impl_item
  trait: (type_identifier) @inherits.base
  type: (type_identifier)) @inherits.class

; Generic trait bounds
; (type_bound_list
;   (type_identifier) @inherits.base)

; ============ CONSTANTS ============

; const definitions
(const_item
  name: (identifier) @value.name
  type: (_) @value.type
  value: (_) @value.init) @value.def

; static definitions
(static_item
  name: (identifier) @value.name
  type: (_) @value.type
  value: (_) @value.init) @value.def
