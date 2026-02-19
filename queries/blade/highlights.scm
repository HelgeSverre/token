; Blade template syntax highlighting

; ── HTML (inherited structure) ──────────────────────────────────

; Tags
(tag_name) @tag
(erroneous_end_tag_name) @tag

; Attributes
(attribute_name) @attribute
(attribute_value) @string
(quoted_attribute_value) @string

; Doctype
(doctype) @keyword

; HTML comments (inherited)
; Note: Blade comments override via (comment) below

; Special elements
(script_element
  (start_tag (tag_name) @tag)
  (end_tag (tag_name) @tag))

(style_element
  (start_tag (tag_name) @tag)
  (end_tag (tag_name) @tag))

; HTML punctuation
"<" @punctuation.bracket
">" @punctuation.bracket
"</" @punctuation.bracket
"/>" @punctuation.special
"=" @operator

; ── Blade comments ──────────────────────────────────────────────

(comment) @comment

; ── Blade directives ────────────────────────────────────────────

; Block directive start/end (@if, @foreach, @section, @endif, etc.)
(directive_start) @keyword
(directive_end) @keyword

; Inline and standalone directives (@include, @extends, @yield, etc.)
(directive) @keyword

; Conditional midpoints (@else, @elseif, etc.)
(conditional_keyword) @keyword

; Standalone keyword directives (@csrf, @parent, etc.)
(keyword) @keyword

; ── Blade echo / PHP delimiters ─────────────────────────────────

; Escaped echo {{ }}
"{{" @punctuation.special
"}}" @punctuation.special

; Unescaped echo {!! !!}
"{!!" @punctuation.special
"!!}" @punctuation.special

; Directive parameter parentheses
"(" @punctuation.bracket
")" @punctuation.bracket

; Native PHP tags
(php_tag) @punctuation.special
(php_end_tag) @punctuation.special

; ── PHP content (opaque, no injection) ──────────────────────────

(php_only) @variable

; Directive parameters (PHP expressions inside parens)
(parameter) @variable
