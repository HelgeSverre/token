; Synchronized from HelgeSverre/tree-sitter-applescript at
; 1676f5fe99eee6b6532ab6d13559323c48d26190 (MIT).

(comment) @comment
(block_comment) @comment

(string) @string
(raw_data) @string.special
(number) @number
(boolean) @boolean

; Token resolves identical capture spans last-match-wins, so generic captures
; precede the more specific function/property/parameter overrides below.
[
  (identifier)
  (piped_identifier)
] @variable

[
  (missing_value)
  (null_value)
  (applescript_constant)
  (current_date)
] @constant.builtin

[
  (it_reference)
  (its_reference)
  (me_reference)
  (current_application)
  (result_reference)
] @variable.builtin

(handler_definition
  name: [(identifier) (command_name) (folder_action_event)] @function)

(handler_call
  . (identifier) @function)

(command_name) @function.builtin
(folder_action_event) @function.builtin

(parameter_list
  [(identifier) (piped_identifier)] @variable.parameter)
(labeled_parameter
  label: (identifier) @variable.parameter)
(labeled_parameter
  name: [(identifier) (piped_identifier)] @variable.parameter)
(command_parameter
  name: (parameter_name) @variable.parameter)
(command_flag
  name: (command_flag_name) @variable.parameter)

(property_declaration
  name: [(identifier) (piped_identifier)] @property)
(record_entry
  (compound_name) @property)

(element_type) @type.builtin
(type_specifier) @type

[
  (comparison_operator)
  (logical_operator)
] @keyword.operator

[
  (additive_operator)
  (multiplicative_operator)
  (unary_operator)
  (range_operator)
  (possessive)
] @operator

"&" @operator

[
  (specifier_prefix)
  (relative_position)
  (the_keyword)
] @keyword

[
  (keyword_if)
  (keyword_else)
  (keyword_else_if)
  (keyword_then)
  (keyword_repeat)
  (keyword_return)
  (keyword_try)
  (keyword_error)
  (keyword_on_error)
  (keyword_use)
  (keyword_exit)
  (keyword_continue)
  (keyword_tell)
  (keyword_end)
  (keyword_on)
  (keyword_function)
  (keyword_script)
  (keyword_to)
  (keyword_handler_to)
  (keyword_set)
  (keyword_copy)
  (keyword_global)
  (keyword_local)
  (keyword_property)
  (keyword_log)
  (keyword_my)
  (keyword_application)
  (keyword_considering)
  (keyword_ignoring)
  (keyword_using_terms_from)
  (keyword_with_timeout)
  (keyword_with_transaction)
] @keyword

["(" ")" "{" "}"] @punctuation.bracket
["," ":"] @punctuation.delimiter
