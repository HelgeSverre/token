; Token-maintained query for tree-sitter-commonlisp 0.4.x (MIT).
(comment) @comment
(block_comment) @comment
(str_lit) @string
(char_lit) @string.special
[(num_lit) (complex_num_lit)] @number
[(nil_lit) (kwd_lit)] @constant.builtin
(kwd_symbol) @property
(package_lit) @type
(defun_keyword) @keyword.function
(defun) @function
["defmacro" "defmethod" "defgeneric"] @keyword.function
[(quoting_lit) (syn_quoting_lit) (unquoting_lit) (unquote_splicing_lit)] @punctuation.special
["(" ")" "{" "}"] @punctuation.bracket
(sym_lit) @variable
