; queries/bash/highlights.scm
; Bash/Shell syntax highlighting queries

; Keywords
[
  "case"
  "do"
  "done"
  "elif"
  "else"
  "esac"
  "export"
  "fi"
  "for"
  "function"
  "if"
  "in"
  "local"
  "select"
  "then"
  "unset"
  "until"
  "while"
] @keyword

; Function definitions
(function_definition
  name: (word) @function)

; Function calls / commands
(command_name) @function

; Variables
(variable_name) @variable

(special_variable_name) @variable.builtin

(simple_expansion
  (variable_name) @variable)

; Strings
(string) @string
(raw_string) @string
(heredoc_body) @string
(heredoc_start) @string

; Comments
(comment) @comment

; Numbers
(number) @number

; Operators
[
  "="
  "=="
  "!="
  "<"
  ">"
  "&&"
  "||"
  "!"
  "&"
  "|"
] @operator

; Redirections
[
  ">"
  ">>"
  "<"
  "<<"
  "<<<"
] @operator

; File descriptors
(file_descriptor) @number

; Command substitution
(command_substitution
  "$(" @punctuation.special
  ")" @punctuation.special)

; Variable expansion
(simple_expansion
  "$" @punctuation.special)

(expansion
  "${" @punctuation.special
  "}" @punctuation.special)

; Punctuation
["(" ")" "{" "}" "[" "]" "[[" "]]"] @punctuation.bracket
[";" ";;" ";&" ";;&"] @punctuation.delimiter
