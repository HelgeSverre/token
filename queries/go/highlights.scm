; queries/go/highlights.scm
; Go syntax highlighting queries

; Keywords
[
  "break"
  "case"
  "chan"
  "const"
  "continue"
  "default"
  "defer"
  "else"
  "fallthrough"
  "for"
  "func"
  "go"
  "goto"
  "if"
  "import"
  "interface"
  "map"
  "package"
  "range"
  "return"
  "select"
  "struct"
  "switch"
  "type"
  "var"
] @keyword

; Function definitions
(function_declaration
  name: (identifier) @function)

(method_declaration
  name: (field_identifier) @function.method)

; Function calls
(call_expression
  function: (identifier) @function)

(call_expression
  function: (selector_expression
    field: (field_identifier) @function.method))

; Type definitions
(type_spec
  name: (type_identifier) @type)

; Type references
(type_identifier) @type

; Package names
(package_clause
  (package_identifier) @type)

; Field names
(field_identifier) @property

; Parameters
(parameter_declaration
  name: (identifier) @variable.parameter)

(variadic_parameter_declaration
  name: (identifier) @variable.parameter)

; Variables
(short_var_declaration
  left: (expression_list
    (identifier) @variable))

(var_spec
  name: (identifier) @variable)

; Const
(const_spec
  name: (identifier) @constant)

; Strings
(interpreted_string_literal) @string
(raw_string_literal) @string
(rune_literal) @string

; Comments
(comment) @comment

; Numbers
(int_literal) @number
(float_literal) @number
(imaginary_literal) @number

; Booleans
(true) @boolean
(false) @boolean
(nil) @constant.builtin
(iota) @constant.builtin

; Identifier
(identifier) @variable

; Field declaration
(field_declaration
  name: (field_identifier) @property)

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "%"
  "&"
  "|"
  "^"
  "<<"
  ">>"
  "&^"
  "+="
  "-="
  "*="
  "/="
  "%="
  "&="
  "|="
  "^="
  "<<="
  ">>="
  "&^="
  "&&"
  "||"
  "<-"
  "++"
  "--"
  "=="
  "!="
  "<"
  "<="
  ">"
  ">="
  "="
  ":="
  "!"
  "..."
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," "." ";" ":"] @punctuation.delimiter
