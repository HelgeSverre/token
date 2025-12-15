; queries/json/highlights.scm
; JSON syntax highlighting queries

; Strings
(string) @string

; Property names (keys)
(pair
  key: (string) @property)

; Numbers
(number) @number

; Booleans
(true) @boolean
(false) @boolean

; Null
(null) @constant.builtin

; Punctuation
["{" "}"] @punctuation.bracket
["[" "]"] @punctuation.bracket
["," ":"] @punctuation.delimiter
