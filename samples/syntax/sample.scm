; Tree-sitter highlights query for sample language
; This is a Scheme file used for syntax highlighting definitions

; Keywords
[
  "if"
  "else"
  "while"
  "for"
  "return"
  "function"
  "let"
  "const"
  "var"
] @keyword

; Function definitions
(function_definition
  name: (identifier) @function)

; Function calls
(call_expression
  function: (identifier) @function.call)

; Types
(type_identifier) @type
(primitive_type) @type.builtin

; Strings
(string_literal) @string
(template_string) @string

; Numbers
(number) @number
(float) @number.float

; Comments
(comment) @comment
(block_comment) @comment

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "="
  "=="
  "!="
  "<"
  ">"
  "<="
  ">="
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," ";" ":" "."] @punctuation.delimiter

; Variables
(identifier) @variable
(property_identifier) @property

; Parameters
(formal_parameters
  (identifier) @variable.parameter)

; Constants and booleans
(true) @boolean
(false) @boolean
(null) @constant.builtin

; Predicates (tree-sitter specific)
((identifier) @keyword
  (#match? @keyword "^#"))
