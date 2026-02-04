; JavaScript/TypeScript Symbol Extraction Queries for Coderev

; ============ CALLABLE (Functions) ============

; Function declarations
(function_declaration
  name: (identifier) @callable.name
  parameters: (formal_parameters) @callable.params
  body: (statement_block) @callable.body) @callable.def

; Arrow functions assigned to variables
(lexical_declaration
  (variable_declarator
    name: (identifier) @callable.name
    value: (arrow_function
      parameters: (formal_parameters) @callable.params
      body: (_) @callable.body))) @callable.def

; Function expressions assigned to variables
(lexical_declaration
  (variable_declarator
    name: (identifier) @callable.name
    value: (function_expression
      parameters: (formal_parameters) @callable.params
      body: (statement_block) @callable.body))) @callable.def

; Method definitions
(method_definition
  name: (property_identifier) @method.name
  parameters: (formal_parameters) @method.params
  body: (statement_block) @method.body) @method.def

; ============ CONTAINER (Classes) ============

; Class declarations
(class_declaration
  name: (identifier) @container.name
  body: (class_body) @container.body) @container.def

; Class with extends
; (class_declaration
;   name: (identifier) @container.name
;   (class_heritage
;     (extends_clause
;       (identifier) @inherits.base))
;   body: (class_body) @container.body) @container.def

; ============ IMPORTS ============

; import x from 'y'
(import_statement
  (import_clause
    (identifier) @import.name)
  source: (string) @import.module) @import.def

; import { x, y } from 'z'
(import_statement
  (import_clause
    (named_imports
      (import_specifier
        name: (identifier) @import.name)))
  source: (string) @import.module) @import.def

; import * as x from 'y'
(import_statement
  (import_clause
    (namespace_import
      (identifier) @import.alias))
  source: (string) @import.module) @import.def

; require()
(call_expression
  function: (identifier) @_require
  arguments: (arguments
    (string) @import.module)
  (#eq? @_require "require")) @import.def

; ============ CALLS ============

; Function calls
(call_expression
  function: (identifier) @call.name
  arguments: (arguments) @call.args) @call.expr

; Method calls
(call_expression
  function: (member_expression
    object: (_) @call.receiver
    property: (property_identifier) @call.name)
  arguments: (arguments) @call.args) @call.expr

; ============ EXPORTS ============

; export function
(export_statement
  declaration: (function_declaration
    name: (identifier) @callable.name
    parameters: (formal_parameters) @callable.params
    body: (statement_block) @callable.body)) @callable.def

; export class
(export_statement
  declaration: (class_declaration
    name: (identifier) @container.name
    body: (class_body) @container.body)) @container.def
