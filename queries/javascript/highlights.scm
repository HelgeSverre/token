; JavaScript syntax highlighting queries - Enhanced
; Based on official tree-sitter-javascript queries with additional captures

; Properties (must come early to be overridden by more specific patterns)
(property_identifier) @property

; Function parameters - MUST come before generic identifier capture
(formal_parameters
  (identifier) @variable.parameter)

(arrow_function
  parameter: (identifier) @variable.parameter)

(formal_parameters
  (assignment_pattern
    left: (identifier) @variable.parameter))

(formal_parameters
  (rest_pattern
    (identifier) @variable.parameter))

; Destructuring in parameters - object
(formal_parameters
  (object_pattern
    (shorthand_property_identifier_pattern) @variable.parameter))

(formal_parameters
  (object_pattern
    (pair_pattern
      value: (identifier) @variable.parameter)))

; Destructuring in parameters - array
(formal_parameters
  (array_pattern
    (identifier) @variable.parameter))

; Variables (fallback - comes after parameters)
(identifier) @variable

; Function and method definitions
(function_expression
  name: (identifier) @function)
(function_declaration
  name: (identifier) @function)
(method_definition
  name: (property_identifier) @function.method)

(pair
  key: (property_identifier) @function.method
  value: [(function_expression) (arrow_function)])

(variable_declarator
  name: (identifier) @function
  value: [(function_expression) (arrow_function)])

; Function and method calls
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

; Literals
(this) @variable.builtin
(super) @variable.builtin

[
  (true)
  (false)
  (null)
  (undefined)
] @constant.builtin

(comment) @comment

[
  (string)
  (template_string)
] @string

(regex) @string.special

(number) @number

; Escape sequences in strings
(escape_sequence) @escape

; Template string interpolation
(template_substitution
  "${" @punctuation.special
  "}" @punctuation.special)

; Optional chaining
(optional_chain) @punctuation.delimiter

; Punctuation
[
  ";"
  "."
  ","
] @punctuation.delimiter

[
  "("
  ")"
  "["
  "]"
  "{"
  "}"
] @punctuation.bracket

; Operators
[
  "-"
  "--"
  "-="
  "+"
  "++"
  "+="
  "*"
  "*="
  "**"
  "**="
  "/"
  "/="
  "%"
  "%="
  "<"
  "<="
  "<<"
  "<<="
  "="
  "=="
  "==="
  "!"
  "!="
  "!=="
  ">"
  ">="
  ">>"
  ">>="
  ">>>"
  ">>>="
  "~"
  "^"
  "&"
  "|"
  "^="
  "&="
  "|="
  "&&"
  "||"
  "??"
  "&&="
  "||="
  "??="
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
] @keyword.operator

; General keywords (remaining)
[
  "as"
  "await"
  "break"
  "case"
  "catch"
  "class"
  "const"
  "continue"
  "debugger"
  "default"
  "do"
  "else"
  "export"
  "extends"
  "finally"
  "for"
  "from"
  "get"
  "if"
  "import"
  "let"
  "of"
  "set"
  "static"
  "switch"
  "target"
  "throw"
  "try"
  "var"
  "while"
  "with"
] @keyword
