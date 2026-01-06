; CSS syntax highlighting queries - Enhanced

; Comments
(comment) @comment

; Selectors
(tag_name) @tag
(nesting_selector) @tag
(universal_selector) @operator
(class_name) @type
(id_name) @constant
(namespace_name) @property

; Pseudo-elements (::before, ::after) - use @attribute
(pseudo_element_selector (tag_name) @attribute)

; Pseudo-classes (:hover, :focus) - use @function
(pseudo_class_selector (class_name) @function)

; Attribute selectors
(attribute_selector (attribute_name) @attribute)
(attribute_selector (plain_value) @string)

; Combinators - use @keyword.operator
">" @keyword.operator
"+" @keyword.operator
"~" @keyword.operator

; Properties and Values
(property_name) @property
(plain_value) @string
(color_value) @constant
(integer_value) @number
(float_value) @number
(unit) @type

; Strings
(string_value) @string

; Functions
(function_name) @function

; At-rules (at_keyword captures @media, @import, @keyframes, etc.)
(at_keyword) @keyword

; Media query keywords
(keyword_query) @keyword
(feature_name) @property

; Keyframe stops
(to) @keyword
(from) @keyword
(important) @keyword

; Logical operators in media queries
"and" @keyword.operator
"or" @keyword.operator
"not" @keyword.operator
"only" @keyword.operator

; Comparison operators (attribute selectors)
"=" @operator
"^=" @operator
"|=" @operator
"~=" @operator
"$=" @operator
"*=" @operator

; Arithmetic operators (calc, etc.)
"-" @operator
"*" @operator
"/" @operator

; Punctuation
["#" "," "." ":" "::" ";"] @punctuation.delimiter
["{" "}" "(" ")" "[" "]"] @punctuation.bracket
