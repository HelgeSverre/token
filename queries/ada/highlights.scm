(comment) @comment
(string_literal) @string
(character_literal) @string
(numeric_literal) @number
(identifier) @variable
(function_specification (identifier) @function)
[
  "package" "body" "procedure" "function" "is" "begin" "end"
  "type" "subtype" "record" "private" "with" "use" "return"
  "if" "then" "elsif" "else" "case" "when" "loop" "while" "for"
] @keyword
