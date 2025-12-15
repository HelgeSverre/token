; queries/toml/highlights.scm
; TOML syntax highlighting queries

; Comments
(comment) @comment

; Strings
(string) @string

; Numbers
(integer) @number
(float) @number

; Booleans
(boolean) @boolean

; Date/time
(local_date) @string
(local_time) @string
(local_date_time) @string
(offset_date_time) @string

; Keys
(bare_key) @property
(dotted_key) @property
(quoted_key) @property

; Table headers
(table
  (bare_key) @type)
(table
  (dotted_key) @type)
(table
  (quoted_key) @type)

(table_array_element
  (bare_key) @type)
(table_array_element
  (dotted_key) @type)
(table_array_element
  (quoted_key) @type)

; Punctuation
["[" "]" "[[" "]]"] @punctuation.bracket
["{" "}"] @punctuation.bracket
["." "," "="] @punctuation.delimiter
