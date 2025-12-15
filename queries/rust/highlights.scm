; Rust syntax highlighting queries
; Based on nvim-treesitter queries for Rust

; Keywords
[
  "as"
  "async"
  "await"
  "break"
  "const"
  "continue"
  "crate"
  "dyn"
  "else"
  "enum"
  "extern"
  "for"
  "if"
  "impl"
  "in"
  "let"
  "loop"
  "match"
  "mod"
  "move"
  "mut"
  "pub"
  "ref"
  "static"
  "struct"
  "trait"
  "type"
  "union"
  "unsafe"
  "use"
  "where"
  "while"
] @keyword

; Function keyword
"fn" @keyword.function

; Return keyword
"return" @keyword.return

; Self
(self) @variable.builtin

; Super and crate
[
  "super"
  "crate"
] @variable.builtin

; Boolean
(boolean_literal) @boolean

; Strings
(string_literal) @string
(raw_string_literal) @string
(char_literal) @string

; Numbers
(integer_literal) @number
(float_literal) @number

; Comments
(line_comment) @comment
(block_comment) @comment

; Doc comments (special)
(line_comment
  (doc_comment)) @comment

; Attributes
(attribute_item) @attribute
(inner_attribute_item) @attribute

; Types
(type_identifier) @type
(primitive_type) @type.builtin

; Generic type parameters
(type_parameters
  (type_identifier) @type)
(type_arguments
  (type_identifier) @type)

; Lifetime
(lifetime) @label

; Functions
(function_item
  name: (identifier) @function)
(function_signature_item
  name: (identifier) @function)

; Methods
(function_item
  name: (identifier) @function.method
  parameters: (parameters
    (self_parameter)))

; Function calls
(call_expression
  function: (identifier) @function)
(call_expression
  function: (field_expression
    field: (field_identifier) @function.method))
(call_expression
  function: (scoped_identifier
    name: (identifier) @function))

; Macros
(macro_invocation
  macro: (identifier) @function.builtin)
(macro_definition
  name: (identifier) @function.builtin)

; Struct/enum fields
(field_identifier) @property
(shorthand_field_identifier) @property

; Variables and parameters
(identifier) @variable
(parameter
  pattern: (identifier) @variable.parameter)

; Constants
(const_item
  name: (identifier) @constant)
(static_item
  name: (identifier) @constant)

; Enum variants (as constants)
(enum_variant
  name: (identifier) @constant)

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "%"
  "="
  "=="
  "!="
  "<"
  ">"
  "<="
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
  ".."
  "..="
  "=>"
  "->"
  "?"
] @operator

; Punctuation
[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

[
  ","
  "."
  ":"
  "::"
  ";"
] @punctuation.delimiter

; Special punctuation
[
  "#"
  "@"
] @punctuation.special
