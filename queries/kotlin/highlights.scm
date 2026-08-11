; Token-maintained query for tree-sitter-kotlin-ng 1.1.x (MIT).
(identifier) @variable
(function_declaration (identifier) @function)
(class_declaration (identifier) @type)
(parameter (identifier) @variable.parameter)
[(line_comment) (block_comment)] @comment
[(string_literal) (multiline_string_literal)] @string
(character_literal) @string.special
[(number_literal) (float_literal)] @number
[
 "class" "interface" "object" "fun" "val" "var" "typealias"
 "if" "else" "when" "for" "while" "do" "try" "catch" "finally"
 "return" "throw" "package" "import"
] @keyword
["(" ")" "[" "]" "{" "}"] @punctuation.bracket
["," "." ":" ";"] @punctuation.delimiter
