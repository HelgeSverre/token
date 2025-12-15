; Markdown syntax highlighting queries
; For tree-sitter-md BLOCK grammar only
; (Inline elements like code_span require separate inline grammar)

; Headings
(atx_heading
  (atx_h1_marker) @punctuation.special
  heading_content: (_) @text.title)
(atx_heading
  (atx_h2_marker) @punctuation.special
  heading_content: (_) @text.title)
(atx_heading
  (atx_h3_marker) @punctuation.special
  heading_content: (_) @text.title)
(atx_heading
  (atx_h4_marker) @punctuation.special
  heading_content: (_) @text.title)
(atx_heading
  (atx_h5_marker) @punctuation.special
  heading_content: (_) @text.title)
(atx_heading
  (atx_h6_marker) @punctuation.special
  heading_content: (_) @text.title)

; Setext headings
(setext_heading
  heading_content: (_) @text.title)
(setext_h1_underline) @punctuation.special
(setext_h2_underline) @punctuation.special

; Code blocks (fenced)
(fenced_code_block) @text
(fenced_code_block
  (fenced_code_block_delimiter) @punctuation.delimiter)
(fenced_code_block
  (info_string
    (language) @label))
(code_fence_content) @string

; Indented code blocks
(indented_code_block) @string

; Block quotes
(block_quote) @text
(block_quote_marker) @punctuation.special

; Lists
(list_marker_minus) @punctuation.delimiter
(list_marker_plus) @punctuation.delimiter
(list_marker_star) @punctuation.delimiter
(list_marker_dot) @punctuation.delimiter
(list_marker_parenthesis) @punctuation.delimiter

; Thematic breaks
(thematic_break) @punctuation.special

; HTML in markdown
(html_block) @tag

; Links (block level - reference definitions)
(link_reference_definition
  (link_label) @label
  (link_destination) @text.uri)

; Tables (GFM)
(pipe_table_header
  (pipe_table_cell) @text.strong)
(pipe_table_delimiter_row) @punctuation.delimiter
(pipe_table_row
  (pipe_table_cell) @text)
