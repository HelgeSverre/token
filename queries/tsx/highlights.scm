; TSX/JSX syntax highlighting queries
; Inherits all TypeScript patterns plus JSX-specific patterns

; Properties (must come early to be overridden by more specific patterns)
(property_identifier) @property
(shorthand_property_identifier) @property

; Parameters - TypeScript specific
(required_parameter
  pattern: (identifier) @variable.parameter)

(optional_parameter
  pattern: (identifier) @variable.parameter)

; Rest parameters
(rest_pattern
  (identifier) @variable.parameter)

; Destructuring in parameters
(object_pattern
  (shorthand_property_identifier_pattern) @variable.parameter)

; Variables
(variable_declarator
  name: (identifier) @variable)

; Function definitions
(function_declaration
  name: (identifier) @function)

(method_definition
  name: (property_identifier) @function.method)

(variable_declarator
  name: (identifier) @function
  value: [(function_expression) (arrow_function)])

; Function calls
(call_expression
  function: (identifier) @function)

(call_expression
  function: (member_expression
    property: (property_identifier) @function.method))

; Constructor calls
(new_expression
  constructor: (identifier) @constructor)

(new_expression
  constructor: (member_expression
    property: (property_identifier) @constructor))

; Class definitions
(class_declaration
  name: (type_identifier) @type)

(interface_declaration
  name: (type_identifier) @type)

(type_alias_declaration
  name: (type_identifier) @type)

(enum_declaration
  name: (identifier) @type)

; Type annotations
(type_identifier) @type
(predefined_type) @type.builtin

; Strings
(string) @string
(template_string) @string
(template_substitution
  "${" @punctuation.special
  "}" @punctuation.special)

; Escape sequences
(escape_sequence) @escape

; Comments
(comment) @comment

; Numbers
(number) @number

; Booleans and constants
(true) @boolean
(false) @boolean
(null) @constant.builtin
(undefined) @constant.builtin

; This/super
(this) @variable.builtin
(super) @variable.builtin

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "%"
  "**"
  "="
  "+="
  "-="
  "*="
  "/="
  "%="
  "**="
  "=="
  "==="
  "!="
  "!=="
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
  "??"
  "?."
  "?:"
  "++"
  "--"
] @operator

; Function-defining keywords
[
  "function"
  "async"
] @keyword.function

"=>" @keyword.function

; Return keywords
[
  "return"
  "yield"
] @keyword.return

; Keyword operators
[
  "typeof"
  "instanceof"
  "in"
  "delete"
  "void"
  "new"
  "keyof"
] @keyword.operator

; General keywords (TypeScript-specific and remaining)
[
  "abstract"
  "as"
  "await"
  "break"
  "case"
  "catch"
  "class"
  "const"
  "continue"
  "debugger"
  "declare"
  "default"
  "do"
  "else"
  "enum"
  "export"
  "extends"
  "finally"
  "for"
  "from"
  "get"
  "if"
  "implements"
  "import"
  "interface"
  "let"
  "module"
  "namespace"
  "of"
  "override"
  "private"
  "protected"
  "public"
  "readonly"
  "satisfies"
  "set"
  "static"
  "switch"
  "throw"
  "try"
  "type"
  "var"
  "while"
  "with"
] @keyword

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," "." ";" ":"] @punctuation.delimiter

; JSX support
(jsx_opening_element
  name: (identifier) @tag)
(jsx_closing_element
  name: (identifier) @tag)
(jsx_self_closing_element
  name: (identifier) @tag)

; JSX component names (member expressions like Foo.Bar)
(jsx_opening_element
  name: (member_expression) @tag)
(jsx_closing_element
  name: (member_expression) @tag)
(jsx_self_closing_element
  name: (member_expression) @tag)

; JSX attributes
(jsx_attribute
  (property_identifier) @property)

; JSX string attribute values
(jsx_attribute
  (string) @string)

; JSX expression containers
(jsx_expression
  "{" @punctuation.special
  "}" @punctuation.special)
