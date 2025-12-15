; CSS syntax highlighting queries

; Selectors
(tag_name) @tag
(class_name) @type
(id_name) @constant
(universal_selector) @operator
(pseudo_class_selector (class_name) @function)
(pseudo_element_selector (tag_name) @function)
(attribute_selector (attribute_name) @attribute)

; Properties and values
(property_name) @property
(plain_value) @string
(color_value) @constant
(integer_value) @number
(float_value) @number

; Units
(unit) @type

; Strings
(string_value) @string

; Functions
(function_name) @function

; Keywords
(important) @keyword

; At-rules
(at_keyword) @keyword
(keyword_query) @keyword
(feature_name) @property

; Comments
(comment) @comment

; Punctuation
"{" @punctuation.bracket
"}" @punctuation.bracket
"(" @punctuation.bracket
")" @punctuation.bracket
"[" @punctuation.bracket
"]" @punctuation.bracket
":" @punctuation.delimiter
";" @punctuation.delimiter
"," @punctuation.delimiter

; Operators
">" @operator
"+" @operator
"~" @operator
"*" @operator
