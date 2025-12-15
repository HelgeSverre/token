; queries/typescript/highlights.scm
; TypeScript syntax highlighting queries
; Note: TypeScript extends JavaScript, so many patterns are similar

; Keywords
[
  "abstract"
  "as"
  "async"
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
  "delete"
  "do"
  "else"
  "enum"
  "export"
  "extends"
  "finally"
  "for"
  "from"
  "function"
  "get"
  "if"
  "implements"
  "import"
  "in"
  "instanceof"
  "interface"
  "keyof"
  "let"
  "module"
  "namespace"
  "new"
  "of"
  "override"
  "private"
  "protected"
  "public"
  "readonly"
  "return"
  "satisfies"
  "set"
  "static"
  "switch"
  "throw"
  "try"
  "type"
  "typeof"
  "var"
  "void"
  "while"
  "with"
  "yield"
] @keyword

; Function definitions
(function_declaration
  name: (identifier) @function)

(method_definition
  name: (property_identifier) @function.method)

(arrow_function) @function

; Function calls
(call_expression
  function: (identifier) @function)

(call_expression
  function: (member_expression
    property: (property_identifier) @function.method))

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

; Parameters
(required_parameter
  pattern: (identifier) @variable.parameter)

(optional_parameter
  pattern: (identifier) @variable.parameter)

; Properties
(property_identifier) @property

(shorthand_property_identifier) @property

; Variables
(variable_declarator
  name: (identifier) @variable)

; Strings
(string) @string
(template_string) @string
(template_substitution
  "${" @punctuation.special
  "}" @punctuation.special)

; Comments
(comment) @comment

; Numbers
(number) @number

; Booleans
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
  "=>"
  "++"
  "--"
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," "." ";" ":"] @punctuation.delimiter
["<" ">"] @punctuation.bracket
