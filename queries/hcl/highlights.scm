(comment) @comment
[(quoted_template) (heredoc_template)] @string
(numeric_lit) @number
[(bool_lit) (null_lit)] @constant.builtin
(variable_expr (identifier) @variable)
(get_attr (identifier) @property)
(function_call (identifier) @function)
(block (identifier) @type)
(attribute (identifier) @property)
[
  "for"
  "in"
  "if"
] @keyword
