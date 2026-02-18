; Sema highlight queries for Token editor
; Uses tree-sitter-racket (Scheme) grammar
;
; Token editor uses FIRST-MATCH-WINS semantics:
;   - Multiple captures for the same node are sorted by (start_col, end_col)
;   - highlight_at() returns the first matching token
;   - Earlier patterns in this file win over later ones for the same node
;
; Therefore: MOST SPECIFIC patterns go FIRST, generic fallbacks go LAST.

; =====================================================================
; 1. COMMENTS & LITERALS (leaf nodes, rarely conflict)
; =====================================================================

(comment) @comment
(block_comment) @comment

(string) @string
(escape_sequence) @escape
(number) @number
(boolean) @boolean
(character) @constant

; =====================================================================
; 2. PUNCTUATION & QUOTE OPERATORS
; =====================================================================

["(" ")" "[" "]" "{" "}"] @punctuation.bracket

(quote "'") @operator
(unquote_splicing ",@") @operator
(unquote ",") @operator
(quasiquote "`") @operator

; =====================================================================
; 3. SYMBOL SPECIALIZATIONS (must come before generic symbol fallback)
; =====================================================================

; Boolean symbols (Scheme grammar parses true/false as symbols)
((symbol) @boolean
  (#any-of? @boolean "true" "false"))

; nil
((symbol) @constant.builtin
  (#eq? @constant.builtin "nil"))

; Dot as punctuation delimiter (variadic args, dotted pairs)
((symbol) @punctuation.delimiter
  (#eq? @punctuation.delimiter "."))

; Ellipsis
((symbol) @variable.builtin
  (#eq? @variable.builtin "..."))

; Arithmetic and comparison operators
((symbol) @operator
  (#any-of? @operator
    "+" "-" "*" "/" "%" "=" ">" "<" ">=" "<="
    "eq?" "equal?" "eqv?"))

; Keyword accessor in call position: (:name person)
(list
  .
  ((symbol) @property
    (#match? @property "^:")))

; Keyword literals / map keys: :foo (non-call position)
((symbol) @property
  (#match? @property "^:"))

; =====================================================================
; 4. KEYWORDS & SPECIAL FORMS (head position in lists)
; =====================================================================

; Core special forms
(list
  .
  (symbol) @keyword
  (#any-of? @keyword
    "define" "defun" "lambda" "fn" "set!"
    "let" "let*" "letrec" "begin" "do"
    "and" "or"
    "quote" "quasiquote" "unquote" "unquote-splicing"
    "define-record-type" "defmacro"
    "delay" "force" "eval" "macroexpand"
    "with-budget"
    "prompt" "message"))

; LLM-specific special forms
(list
  .
  (symbol) @keyword.function
  (#any-of? @keyword.function
    "defagent" "deftool"))

; Conditionals
(list
  .
  (symbol) @keyword
  (#any-of? @keyword
    "if" "cond" "case" "when" "unless" "else"))

; Exception handling
(list
  .
  (symbol) @keyword
  (#any-of? @keyword
    "try" "catch" "throw"))

; Module / import
(list
  .
  (symbol) @keyword
  (#any-of? @keyword
    "import" "module" "export" "load"))

; Threading macros
(list
  .
  (symbol) @keyword.operator
  (#any-of? @keyword.operator
    "->" "->>" "as->"))

; =====================================================================
; 5. BINDING & DEFINITION FORMS (highlight the bound names)
; =====================================================================

; (define (fname args...) body...) — function definition with params
(list
  .
  (symbol) @_kw
  .
  (list
    .
    (symbol) @function
    (symbol) @variable.parameter)
  (#eq? @_kw "define"))

; (define name value) — simple variable binding
(list
  .
  (symbol) @_kw
  .
  (symbol) @variable
  (#eq? @_kw "define"))

; (set! name value)
(list
  .
  (symbol) @_kw
  .
  (symbol) @variable
  (#eq? @_kw "set!"))

; (defun name ...)
(list
  .
  (symbol) @_kw
  .
  (symbol) @function
  (#eq? @_kw "defun"))

; (defmacro name ...)
(list
  .
  (symbol) @_kw
  .
  (symbol) @function
  (#eq? @_kw "defmacro"))

; (defagent name ...)
(list
  .
  (symbol) @_kw
  .
  (symbol) @function
  (#eq? @_kw "defagent"))

; (deftool name ...)
(list
  .
  (symbol) @_kw
  .
  (symbol) @function
  (#eq? @_kw "deftool"))

; (lambda (a b) ...) / (fn (a b) ...) — parameter lists
(list
  .
  (symbol) @_kw
  .
  (list
    (symbol) @variable.parameter)
  (#any-of? @_kw "lambda" "fn"))

; let/letrec/do bindings: (let ((x 1) (y 2)) ...)
(list
  .
  (symbol) @_kw
  .
  (list
    (list
      (symbol) @variable.parameter))
  (#any-of? @_kw "let" "let*" "letrec" "do"))

; =====================================================================
; 6. BUILTIN FUNCTIONS (call head position)
; =====================================================================

(list
  .
  (symbol) @function.builtin
  (#any-of? @function.builtin
    ; Higher-order / functional
    "map" "filter" "foldl" "foldr" "reduce" "for-each" "apply"
    ; LLM primitives
    "conversation/new" "conversation/say"
    "conversation/messages" "conversation/last-reply" "conversation/fork"
    "conversation/add-message" "conversation/model"
    "llm/complete" "llm/chat" "llm/stream" "llm/send"
    "llm/extract" "llm/classify" "llm/batch" "llm/pmap"
    "llm/embed" "llm/auto-configure" "llm/configure"
    "llm/set-budget" "llm/budget-remaining"
    "llm/define-provider" "llm/last-usage" "llm/session-usage"
    "llm/similarity" "llm/clear-budget"
    "llm/configure-embeddings" "llm/current-provider" "llm/list-providers"
    "llm/pricing-status" "llm/reset-usage" "llm/set-default" "llm/set-pricing"
    "prompt/append" "prompt/messages" "prompt/set-system"
    "message/role" "message/content"
    "agent/run" "agent/max-turns" "agent/model"
    "agent/name" "agent/system" "agent/tools"
    ; Embedding functions
    "embedding/->list" "embedding/length"
    "embedding/list->embedding" "embedding/ref"
    ; Tool query functions
    "tool/name" "tool/description" "tool/parameters"
    ; I/O
    "display" "print" "println" "newline" "format"
    "read" "read-line" "read-many"
    "print-error" "println-error" "read-stdin"
    ; Lists
    "list" "cons" "car" "cdr" "first" "rest" "nth"
    "append" "reverse" "length" "null?" "list?" "member"
    "vector" "sort" "sort-by" "take" "drop" "zip" "flatten"
    "range" "make-list" "flat-map" "take-while" "drop-while"
    "every" "any" "partition" "last" "iota"
    ; ca*r/cd*r variants
    "caar" "cadr" "cdar" "cddr"
    "caaar" "caadr" "cadar" "caddr" "cdaar" "cdadr" "cddar" "cdddr"
    ; list/* namespaced functions
    "list/chunk" "list/dedupe" "list/drop-while" "list/group-by"
    "list/index-of" "list/interleave" "list/max" "list/min"
    "list/pick" "list/repeat" "list/shuffle" "list/split-at"
    "list/sum" "list/take-while" "list/unique"
    "list->bytevector" "list->string" "list->vector"
    ; Additional list functions
    "assq" "assv" "flatten-deep" "frequencies" "interpose" "vector->list"
    ; Strings
    "string-append" "string/join" "string/split"
    "string/trim" "string/upper" "string/lower" "string/replace"
    "string/contains?" "string/starts-with?" "string/ends-with?"
    "string/capitalize" "string/empty?" "string/index-of"
    "string/reverse" "string/repeat"
    "string/pad-left" "string/pad-right"
    "str" "substring" "string-length" "string-ref"
    "string->keyword" "keyword->string"
    "string->char" "string->float" "string->list" "string->utf8"
    "string-ci=?"
    "string/byte-length" "string/chars" "string/codepoints"
    "string/foldcase" "string/from-codepoints" "string/last-index-of"
    "string/map" "string/normalize" "string/number?"
    "string/title-case" "string/trim-left" "string/trim-right"
    ; Char functions
    "char->integer" "char->string" "integer->char"
    "char-alphabetic?" "char-ci<?" "char-ci<=?" "char-ci=?"
    "char-ci>?" "char-ci>=?" "char-downcase" "char-lower-case?"
    "char-numeric?" "char-upcase" "char-upper-case?"
    "char-whitespace?" "char<?" "char<=?" "char=?" "char>?" "char>=?"
    ; Math
    "abs" "min" "max" "round" "floor" "ceiling" "sqrt" "expt"
    "pow" "log" "sin" "cos" "ceil" "int" "float"
    "truncate" "mod" "modulo"
    "math/remainder" "math/gcd" "math/lcm" "math/pow"
    "math/tan" "math/random" "math/random-int" "math/clamp"
    "math/sign" "math/exp" "math/log10" "math/log2"
    "math/acos" "math/asin" "math/atan" "math/atan2"
    "math/cosh" "math/degrees->radians" "math/infinite?" "math/lerp"
    "math/map-range" "math/nan?" "math/quotient"
    "math/radians->degrees" "math/sinh" "math/tanh"
    ; Hash maps
    "hash-map" "get" "assoc" "keys" "vals"
    "dissoc" "merge" "contains?" "count" "empty?"
    ; map/* functions
    "map/entries" "map/filter" "map/from-entries"
    "map/map-keys" "map/map-vals" "map/select-keys" "map/update"
    ; hashmap/* functions
    "hashmap/new" "hashmap/get" "hashmap/assoc"
    "hashmap/keys" "hashmap/contains?" "hashmap/to-map"
    ; Type predicates
    "number?" "string?" "symbol?" "pair?" "boolean?" "procedure?"
    "integer?" "float?" "keyword?" "nil?" "fn?" "map?" "record?" "type"
    "equal?" "eq?" "zero?" "positive?" "negative?"
    "even?" "odd?" "bool?" "bytevector?" "char?" "vector?" "promise?"
    "agent?" "conversation?" "message?" "prompt?" "tool?" "promise-forced?"
    ; Conversions
    "string->number" "number->string" "string->symbol" "symbol->string"
    ; File I/O
    "file/read" "file/write" "file/exists?"
    "file/append" "file/delete" "file/list" "file/rename"
    "file/mkdir" "file/info" "file/read-lines" "file/write-lines"
    "file/copy" "file/is-directory?" "file/is-file?"
    "file/fold-lines" "file/for-each-line" "file/is-symlink?"
    ; Path functions
    "path/absolute" "path/basename" "path/dirname"
    "path/extension" "path/join"
    ; JSON / HTTP
    "json/decode" "json/encode" "json/encode-pretty"
    "http/get" "http/post" "http/put" "http/delete" "http/request"
    ; Regex
    "regex/match?" "regex/match" "regex/find-all"
    "regex/replace" "regex/replace-all" "regex/split"
    ; Crypto
    "uuid/v4" "base64/encode" "base64/decode" "hash/md5" "hash/sha256"
    "hash/hmac-sha256"
    ; DateTime
    "time/now" "time/format" "time/parse" "time/date-parts"
    "time/add" "time/diff"
    ; CSV
    "csv/parse" "csv/encode" "csv/parse-maps"
    ; Bitwise
    "bit/and" "bit/or" "bit/xor" "bit/not"
    "bit/shift-left" "bit/shift-right"
    ; Terminal functions
    "term/style" "term/strip" "term/rgb"
    "term/spinner-start" "term/spinner-stop" "term/spinner-update"
    ; Bytevector functions
    "bytevector" "make-bytevector" "bytevector-length"
    "bytevector-u8-ref" "bytevector-u8-set!" "bytevector-copy"
    "bytevector-append" "bytevector->list" "utf8->string"
    ; System
    "env" "shell" "exit" "time-ms" "sleep"
    "sys/args" "sys/cwd" "sys/platform" "sys/set-env" "sys/env-all"
    "sys/arch" "sys/elapsed" "sys/home-dir" "sys/hostname"
    "sys/interactive?" "sys/os" "sys/pid" "sys/temp-dir"
    "sys/tty" "sys/user" "sys/which"
    ; Misc
    "not" "error" "gensym"))

; =====================================================================
; 7. GENERIC FALLBACKS (MUST BE LAST — catch-all for unmatched nodes)
; =====================================================================

; Generic function call: first symbol in a list
(list
  .
  (symbol) @function)

; Any remaining symbol is a variable
(symbol) @variable
