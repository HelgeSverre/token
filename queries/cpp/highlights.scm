; queries/cpp/highlights.scm
; C++ syntax highlighting queries

; Keywords
[
  "alignas"
  "alignof"
  "break"
  "case"
  "catch"
  "class"
  "const"
  "const_cast"
  "constexpr"
  "continue"
  "decltype"
  "default"
  "delete"
  "do"
  "dynamic_cast"
  "else"
  "enum"
  "explicit"
  "export"
  "extern"
  "final"
  "for"
  "friend"
  "goto"
  "if"
  "inline"
  "mutable"
  "namespace"
  "new"
  "noexcept"
  "operator"
  "override"
  "private"
  "protected"
  "public"
  "register"
  "reinterpret_cast"
  "return"
  "sizeof"
  "static"
  "static_assert"
  "static_cast"
  "struct"
  "switch"
  "template"
  "this"
  "throw"
  "try"
  "typedef"
  "typeid"
  "typename"
  "union"
  "using"
  "virtual"
  "volatile"
  "while"
] @keyword

; Function definitions
(function_definition
  declarator: (function_declarator
    declarator: (identifier) @function))

(function_definition
  declarator: (function_declarator
    declarator: (field_identifier) @function.method))

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

(call_expression
  function: (qualified_identifier
    name: (identifier) @function))

; Class definitions
(class_specifier
  name: (type_identifier) @type)

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

; Namespace
(namespace_identifier) @type

; Field names
(field_identifier) @property

; Parameters
(parameter_declaration
  declarator: (identifier) @variable.parameter)

(parameter_declaration
  declarator: (pointer_declarator
    declarator: (identifier) @variable.parameter))

(parameter_declaration
  declarator: (reference_declarator
    (identifier) @variable.parameter))

; This
(this) @variable.builtin

; Strings
(string_literal) @string
(raw_string_literal) @string
(char_literal) @string

; Comments
(comment) @comment

; Numbers
(number_literal) @number

; Booleans
(true) @boolean
(false) @boolean
(nullptr) @constant.builtin

; Identifier
(identifier) @variable

; Preprocessor
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
  "::"
] @operator

; Punctuation
["(" ")" "[" "]" "{" "}" "<" ">"] @punctuation.bracket
["," ";"] @punctuation.delimiter
