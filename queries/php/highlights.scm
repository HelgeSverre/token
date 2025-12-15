; queries/php/highlights.scm
; PHP syntax highlighting queries

; Keywords
[
  "abstract"
  "and"
  "as"
  "break"
  "case"
  "catch"
  "class"
  "clone"
  "const"
  "continue"
  "declare"
  "default"
  "do"
  "echo"
  "else"
  "elseif"
  "empty"
  "enddeclare"
  "endfor"
  "endforeach"
  "endif"
  "endswitch"
  "endwhile"
  "enum"
  "extends"
  "final"
  "finally"
  "fn"
  "for"
  "foreach"
  "function"
  "global"
  "goto"
  "if"
  "implements"
  "include"
  "include_once"
  "instanceof"
  "interface"
  "list"
  "match"
  "namespace"
  "new"
  "or"
  "print"
  "private"
  "protected"
  "public"
  "readonly"
  "require"
  "require_once"
  "return"
  "static"
  "switch"
  "throw"
  "trait"
  "try"
  "use"
  "while"
  "xor"
  "yield"
] @keyword

; Function definitions
(function_definition
  name: (name) @function)

(method_declaration
  name: (name) @function.method)

; Class definitions
(class_declaration
  name: (name) @type)

(interface_declaration
  name: (name) @type)

(trait_declaration
  name: (name) @type)

(enum_declaration
  name: (name) @type)

; Function calls
(function_call_expression
  function: (name) @function)

(member_call_expression
  name: (name) @function.method)

(scoped_call_expression
  name: (name) @function.method)

; Namespace
(namespace_definition
  name: (namespace_name) @type)

; Variables
(variable_name) @variable

; Named types
(named_type) @type
(primitive_type) @type.builtin

; Parameters
(simple_parameter
  name: (variable_name) @variable.parameter)

; Strings
(string) @string
(encapsed_string) @string
(heredoc) @string
(nowdoc) @string

; Comments
(comment) @comment

; Numbers
(integer) @number
(float) @number

; Booleans
(boolean) @boolean
(null) @constant.builtin

; Attributes
(attribute) @attribute

; PHP tags
["<?php" "<?=" "?>"] @keyword

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
  ".="
  "&="
  "|="
  "^="
  "<<="
  ">>="
  "??="
  "=="
  "==="
  "!="
  "!=="
  "<"
  "<="
  ">"
  ">="
  "<=>"
  "&&"
  "||"
  "!"
  "&"
  "|"
  "^"
  "~"
  "<<"
  ">>"
  "."
  "->"
  "=>"
  "??"
  "++"
  "--"
  "@"
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," ";" ":"] @punctuation.delimiter
