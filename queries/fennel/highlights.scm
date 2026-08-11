[(comment) (shebang)] @comment
[(string) (docstring)] @string
(escape_sequence) @string.escape
(number) @number
[(boolean) (nil)] @constant.builtin
(symbol) @variable
[(fn_form) (lambda_form) (macro_form)] @keyword.function
[
  (if_form) (let_form) (local_form) (global_form) (var_form)
  (set_form) (for_form) (each_form) (match_form) (case_form)
  (quote_form) (quasi_quote_reader_macro) (unquote_form)
] @keyword
