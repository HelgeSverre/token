[(line_comment) (block_comment)] @comment
[(float_literal) (int_literal)] @number
[(bool_literal) (const_literal)] @constant.builtin
(identifier) @variable
(function_declaration name: (identifier) @function)
(struct_declaration name: (identifier) @type)
(type_alias_declaration (identifier) @type)
(struct_member (variable_identifier_declaration name: (identifier) @property))
[
  "fn" "struct" "var" "let" "override"
  "if" "else" "switch" "case" "default" "loop" "for" "while"
  "break" "continue" "return" "discard" "enable"
] @keyword
