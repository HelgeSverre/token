; HTML syntax highlighting queries

; Tags
(tag_name) @tag
(erroneous_end_tag_name) @tag

; Attributes
(attribute_name) @attribute
(attribute_value) @string
(quoted_attribute_value) @string

; Doctype
(doctype) @keyword

; Comments
(comment) @comment

; Text content
(text) @text

; Special elements
(script_element
  (start_tag (tag_name) @tag)
  (end_tag (tag_name) @tag))

(style_element
  (start_tag (tag_name) @tag)
  (end_tag (tag_name) @tag))

; Punctuation
"<" @punctuation.bracket
">" @punctuation.bracket
"</" @punctuation.bracket
"/>" @punctuation.bracket
"=" @operator
