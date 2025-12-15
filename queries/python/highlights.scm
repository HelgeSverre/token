; queries/python/highlights.scm
; Python syntax highlighting queries

; Keywords
[
  "and"
  "as"
  "assert"
  "async"
  "await"
  "break"
  "class"
  "continue"
  "def"
  "del"
  "elif"
  "else"
  "except"
  "finally"
  "for"
  "from"
  "global"
  "if"
  "import"
  "in"
  "is"
  "lambda"
  "nonlocal"
  "not"
  "or"
  "pass"
  "raise"
  "return"
  "try"
  "while"
  "with"
  "yield"
  "match"
  "case"
] @keyword

; Function definitions
(function_definition
  name: (identifier) @function)

; Class definitions
(class_definition
  name: (identifier) @type)

; Function calls
(call
  function: (identifier) @function)

(call
  function: (attribute
    attribute: (identifier) @function.method))

; Decorators
(decorator
  (identifier) @attribute)

(decorator
  (attribute
    attribute: (identifier) @attribute))

; Parameters
(parameters
  (identifier) @variable.parameter)

(default_parameter
  name: (identifier) @variable.parameter)

(typed_parameter
  (identifier) @variable.parameter)

(typed_default_parameter
  name: (identifier) @variable.parameter)

; Properties/attributes
(attribute
  attribute: (identifier) @property)

; Strings
(string) @string

; Comments
(comment) @comment

; Numbers
(integer) @number
(float) @number

; Booleans and None
(true) @boolean
(false) @boolean
(none) @constant.builtin

; Variables (general identifier)
(identifier) @variable

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "%"
  "**"
  "//"
  "=="
  "!="
  "<"
  "<="
  ">"
  ">="
  "="
  "+="
  "-="
  "*="
  "/="
  "%="
  "**="
  "//="
  "&"
  "|"
  "^"
  "~"
  "<<"
  ">>"
  "@"
  ":="
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," ":" "." ";"] @punctuation.delimiter
