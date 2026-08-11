; Token-maintained query for tree-sitter-ocaml's interface grammar (MIT).
[(comment) (line_number_directive) (directive)] @comment
[(string) (character)] @string
(number) @number
(boolean) @boolean
(value_name) @variable
(value_specification (value_name) @function)
[(class_name) (class_type_name) (type_constructor)] @type
[(module_name) (module_type_name)] @type
[(constructor_name) (tag)] @constructor
[
 "and" "class" "exception" "external" "include" "module" "open"
 "private" "sig" "type" "val" "virtual" "with"
] @keyword
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
