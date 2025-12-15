; queries/c/highlights.scm
; C syntax highlighting queries

; Keywords
[
  "break"
  "case"
  "const"
  "continue"
  "default"
  "do"
  "else"
  "enum"
  "extern"
  "for"
  "goto"
  "if"
  "inline"
  "register"
  "return"
  "sizeof"
  "static"
  "struct"
  "switch"
  "typedef"
  "union"
  "volatile"
  "while"
] @keyword

; Function definitions
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @function))

(function_definition
  declarator: (pointer_declarator
    declarator: (function_declarator
      declarator: (identifier) @function)))

; Function declarations
(declaration
  declarator: (function_declarator
    declarator: (identifier) @function))

; Function calls
(call_expression
  function: (identifier) @function)

(call_expression
  function: (field_expression
    field: (field_identifier) @function.method))

; Type definitions
(type_definition
  declarator: (type_identifier) @type)

(struct_specifier
  name: (type_identifier) @type)

(union_specifier
  name: (type_identifier) @type)

(enum_specifier
  name: (type_identifier) @type)

; Type references
(type_identifier) @type

; Primitive types
(primitive_type) @type.builtin
(sized_type_specifier) @type.builtin

; Field names
(field_identifier) @property

; Enumerator
(enumerator
  name: (identifier) @constant)

; Parameters
(parameter_declaration
  declarator: (identifier) @variable.parameter)

(parameter_declaration
  declarator: (pointer_declarator
    declarator: (identifier) @variable.parameter))

; Strings
(string_literal) @string
(system_lib_string) @string
(char_literal) @string

; Comments
(comment) @comment

; Numbers
(number_literal) @number

; Booleans
(true) @boolean
(false) @boolean
(null) @constant.builtin

; Variables
(identifier) @variable

; Preprocessor
(preproc_directive) @keyword

"#define" @keyword
"#include" @keyword
"#if" @keyword
"#ifdef" @keyword
"#ifndef" @keyword
"#else" @keyword
"#elif" @keyword
"#endif" @keyword

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
  "++"
  "--"
  "->"
  "."
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," ";"] @punctuation.delimiter
