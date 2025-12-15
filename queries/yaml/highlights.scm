; YAML syntax highlighting queries
; Based on nvim-treesitter queries

; Comments
(comment) @comment

; Keys in mappings
(block_mapping_pair
  key: (flow_node) @property)
(block_mapping_pair
  key: (flow_node
    (plain_scalar
      (string_scalar) @property)))

; String values
(double_quote_scalar) @string
(single_quote_scalar) @string
(block_scalar) @string

; Plain scalars (unquoted strings) as values
(flow_node
  (plain_scalar
    (string_scalar) @string))

; Boolean values
(boolean_scalar) @boolean

; Null values
(null_scalar) @constant.builtin

; Numbers
(integer_scalar) @number
(float_scalar) @number

; Anchors and aliases
(anchor) @label
(alias) @label

; Tags (like !!str, !!int)
(tag) @tag

; Punctuation
[
  ":"
  "-"
  ">"
  "|"
  "["
  "]"
  "{"
  "}"
  ","
] @punctuation.delimiter

; Document markers
[
  "---"
  "..."
] @punctuation.special
