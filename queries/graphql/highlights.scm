(comment) @comment
(string_value) @string
[(int_value) (float_value)] @number
[(boolean_value) (null_value)] @constant.builtin
(variable (name) @variable)
(fragment_name) @type
(named_type (name) @type)
(field (name) @property)
(directive (name) @attribute)
[
  "query"
  "mutation"
  "subscription"
  "fragment"
  "on"
  "schema"
  "scalar"
  "type"
  "interface"
  "union"
  "enum"
  "input"
  "extend"
  "directive"
  "implements"
] @keyword
