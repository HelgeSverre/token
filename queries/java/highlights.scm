; queries/java/highlights.scm
; Java syntax highlighting queries

; Keywords
[
  "abstract"
  "assert"
  "break"
  "case"
  "catch"
  "class"
  "continue"
  "default"
  "do"
  "else"
  "enum"
  "extends"
  "final"
  "finally"
  "for"
  "if"
  "implements"
  "import"
  "instanceof"
  "interface"
  "native"
  "new"
  "package"
  "private"
  "protected"
  "public"
  "return"
  "static"
  "switch"
  "synchronized"
  "throw"
  "throws"
  "transient"
  "try"
  "volatile"
  "while"
] @keyword

; Function definitions
(method_declaration
  name: (identifier) @function.method)

(constructor_declaration
  name: (identifier) @constructor)

; Function calls
(method_invocation
  name: (identifier) @function.method)

(object_creation_expression
  type: (type_identifier) @constructor)

; Class definitions
(class_declaration
  name: (identifier) @type)

(interface_declaration
  name: (identifier) @type)

(enum_declaration
  name: (identifier) @type)

; Type references
(type_identifier) @type

; Primitive types
(integral_type) @type.builtin
(floating_point_type) @type.builtin
(boolean_type) @type.builtin
(void_type) @type.builtin

; Field names
(field_access
  field: (identifier) @property)

(field_declaration
  declarator: (variable_declarator
    name: (identifier) @property))

; Parameters
(formal_parameter
  name: (identifier) @variable.parameter)

; Variables
(local_variable_declaration
  declarator: (variable_declarator
    name: (identifier) @variable))

; This/super
(this) @variable.builtin
(super) @variable.builtin

; Annotations
(annotation
  name: (identifier) @attribute)

(marker_annotation
  name: (identifier) @attribute)

; Strings
(string_literal) @string
(character_literal) @string

; Comments
(line_comment) @comment
(block_comment) @comment

; Numbers
(decimal_integer_literal) @number
(hex_integer_literal) @number
(octal_integer_literal) @number
(binary_integer_literal) @number
(decimal_floating_point_literal) @number
(hex_floating_point_literal) @number

; Booleans
(true) @boolean
(false) @boolean
(null_literal) @constant.builtin

; Identifier
(identifier) @variable

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "%"
  "="
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
  ">>>="
  "=="
  "!="
  "<"
  "<="
  ">"
  ">="
  "&&"
  "||"
  "!"
  "&"
  "|"
  "^"
  "~"
  "<<"
  ">>"
  ">>>"
  "++"
  "--"
  "->"
  "::"
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}" "<" ">"] @punctuation.bracket
["," "." ";" "@"] @punctuation.delimiter
