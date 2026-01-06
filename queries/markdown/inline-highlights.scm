;; Inline markdown elements (for tree-sitter-md INLINE_LANGUAGE)
;; These patterns highlight inline formatting within paragraphs, headings, etc.

;; Emphasis (*italic* or _italic_)
(emphasis) @text.emphasis

;; Strong emphasis (**bold** or __bold__)
(strong_emphasis) @text.strong

;; Inline code (`code`)
(code_span) @string

;; Inline links [text](url)
(inline_link
  (link_text) @text
  (link_destination) @text.uri)

;; Full reference links [text][ref]
(full_reference_link
  (link_text) @text
  (link_label) @label)

;; Collapsed reference links [ref][]
(collapsed_reference_link
  (link_text) @text.uri)

;; Shortcut links [ref]
(shortcut_link
  (link_text) @text.uri)

;; Images ![alt](url)
(image
  (image_description) @text
  (link_destination) @text.uri)

;; Autolinks <https://...>
(uri_autolink) @text.uri

;; Email autolinks <user@example.com>
(email_autolink) @text.uri

;; Escaped characters
(backslash_escape) @escape

;; HTML entities &amp; etc.
(entity_reference) @escape
(numeric_character_reference) @escape
