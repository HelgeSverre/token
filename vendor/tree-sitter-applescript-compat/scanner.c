// External scanner for AppleScript.
//
// Tree-sitter's regex-based lexer cannot disambiguate two real-world cases
// that show up in everyday AppleScript:
//
//   1. Block comments must skip over `*)` that appears inside a quoted string,
//      since a string can contain any characters. The token() regex used by
//      a pure-grammar comment rule sees `*)` literally and closes early.
//
//   2. The `alias` keyword is a prefix when followed by a value (`alias "X"`,
//      `alias (path to home folder)`), but is a plain identifier when used as
//      a property name (`alias of theItem`). Tree-sitter's GLR can't decide
//      this from the grammar alone because both interpretations consume the
//      same input until the next token.
//
// This scanner emits two external tokens that the grammar wires up:
//
//   - block_comment   — `(* ... *)`, quote-aware, optionally nested.
//   - alias_prefix    — the bare word `alias` only when the next non-space
//                       input is NOT `of`.
//
// A third token (`compound_word`, an identifier that is NOT a reserved
// keyword) was prototyped to stop multi-word compound_names from crossing
// newlines, but it broke too many legitimate parses where keyword-like
// words (e.g. `down`, `option`, `up`) are valid property names in app
// dictionaries. See git history if that approach gets revisited.

#include "tree_sitter/parser.h"
#include <wctype.h>

enum TokenType {
    BLOCK_COMMENT,
    ALIAS_PREFIX,
    PIPED_IDENTIFIER,
    KEYWORD_HANDLER_TO,
    INLINE_MARKER,
};

void *tree_sitter_applescript_external_scanner_create(void) { return NULL; }
void tree_sitter_applescript_external_scanner_destroy(void *payload) { (void)payload; }
unsigned tree_sitter_applescript_external_scanner_serialize(void *payload, char *buffer) {
    (void)payload; (void)buffer;
    return 0;
}
void tree_sitter_applescript_external_scanner_deserialize(void *payload, const char *buffer, unsigned length) {
    (void)payload; (void)buffer; (void)length;
}

static inline void advance(TSLexer *lexer) { lexer->advance(lexer, false); }
static inline void skip(TSLexer *lexer) { lexer->advance(lexer, true); }

// Consume a quoted string body: "..." with `\` escapes. The caller already
// advanced past the opening `"`.
static void consume_string(TSLexer *lexer) {
    while (!lexer->eof(lexer)) {
        int32_t c = lexer->lookahead;
        if (c == '\\') {
            advance(lexer);
            if (!lexer->eof(lexer)) advance(lexer);
            continue;
        }
        if (c == '"') {
            advance(lexer);
            return;
        }
        advance(lexer);
    }
}

// Scan a `(* ... *)` block comment. Supports:
//   - Strings inside the comment (`"*)"` does NOT close it).
//   - Nested block comments — AppleScript Script Editor accepts these.
static bool scan_block_comment(TSLexer *lexer) {
    if (lexer->lookahead != '(') return false;
    advance(lexer);
    if (lexer->lookahead != '*') return false;
    advance(lexer);

    int depth = 1;
    while (depth > 0 && !lexer->eof(lexer)) {
        int32_t c = lexer->lookahead;
        if (c == '"') {
            advance(lexer);
            consume_string(lexer);
            continue;
        }
        if (c == '(') {
            advance(lexer);
            if (lexer->lookahead == '*') {
                advance(lexer);
                depth++;
                continue;
            }
            continue;
        }
        if (c == '*') {
            advance(lexer);
            if (lexer->lookahead == ')') {
                advance(lexer);
                depth--;
                continue;
            }
            continue;
        }
        advance(lexer);
    }
    if (depth != 0) return false;
    lexer->result_symbol = BLOCK_COMMENT;
    return true;
}

// Recognize the literal word `alias` followed by NOT `of`. Used to express
// `alias <expr>` (a value-creating prefix) without collision with the
// `alias of theItem` property reference.
//
// We accept `alias` (case-insensitive) only if the next non-whitespace token
// isn't the keyword `of`. If `of` follows, we return false so the grammar
// falls back to matching `alias` as a plain identifier.
static bool scan_alias_prefix(TSLexer *lexer) {
    const char target[] = {'a', 'l', 'i', 'a', 's'};
    for (int i = 0; i < 5; i++) {
        int32_t lo = lexer->lookahead;
        if (lo == 0) return false;
        // Case-insensitive ASCII match.
        if (lo >= 'A' && lo <= 'Z') lo += 32;
        if (lo != target[i]) return false;
        advance(lexer);
    }

    // Next character must be a word boundary — otherwise we're inside a
    // longer identifier like `aliasing` or `aliased`.
    int32_t next = lexer->lookahead;
    if (next == '_' || iswalnum(next)) return false;

    // Mark end after consuming `alias`. Now look ahead to see what follows,
    // skipping whitespace and line continuations.
    lexer->mark_end(lexer);

    while (!lexer->eof(lexer)) {
        int32_t c = lexer->lookahead;
        if (c == ' ' || c == '\t' || c == '\n' || c == '\r') { advance(lexer); continue; }
        // U+00AC ¬ — the line-continuation character in AppleScript.
        if (c == 0x00AC) { advance(lexer); continue; }
        break;
    }

    // If the next two characters spell "of" followed by a word boundary,
    // this `alias` is the property-reference variant, not the prefix.
    if (lexer->lookahead == 'o' || lexer->lookahead == 'O') {
        advance(lexer);
        if (lexer->lookahead == 'f' || lexer->lookahead == 'F') {
            advance(lexer);
            int32_t after = lexer->lookahead;
            if (after == 0 || after == ' ' || after == '\t' || after == '\n' ||
                after == '\r' || (after != '_' && !iswalnum(after))) {
                return false;
            }
        }
    }

    lexer->result_symbol = ALIAS_PREFIX;
    return true;
}

// Scan |name with any chars except |, newline, or EOF|. The caller must have
// confirmed the lookahead is '|'. Returns true (and sets PIPED_IDENTIFIER)
// only on a successfully closed `|...|`. A newline or EOF inside the bars
// causes rejection so we don't silently swallow the rest of the file.
// Empty `||` is rejected — AppleScript doesn't allow zero-length names.
static bool scan_piped_identifier(TSLexer *lexer) {
    if (lexer->lookahead != '|') return false;
    advance(lexer);  // consume opening |

    bool saw_any = false;
    while (!lexer->eof(lexer)) {
        int32_t c = lexer->lookahead;
        if (c == '\n' || c == '\r') return false;  // unterminated
        if (c == '|') {
            if (!saw_any) return false;  // empty `||` is not an identifier
            advance(lexer);  // consume closing |
            lexer->result_symbol = PIPED_IDENTIFIER;
            return true;
        }
        saw_any = true;
        advance(lexer);
    }
    return false;  // EOF before closing |
}

// `to` is overloaded in AppleScript. Recognise it as a HANDLER opener
// only when it is at the start of a logical line (column 0) AND followed
// by an identifier (or whitespace). `move x to trash` and `from N to M`
// have `to` mid-line, so they fall through to the regular keyword.
//
// We deliberately don't peek ahead at the next word — the column check
// alone disambiguates cleanly in practice. A user who writes a handler
// header indented (rare but legal) will fall back to the regular `to`,
// and the rest of the grammar still parses the header correctly because
// `to` is allowed in non-handler positions too.
static bool scan_keyword_handler_to(TSLexer *lexer) {
    // `get_column` returns the current byte offset from the most recent
    // newline, including any whitespace the dispatcher's `skip()` calls
    // already advanced past. A top-level `to` has no leading whitespace,
    // so column == 0 and we accept it. An indented `to` (column > 0,
    // whether the indent is spaces, tabs, or a `¬` continuation) is
    // rejected here and falls through to the regular `keyword_to` from
    // the in-grammar lexer.
    if (lexer->get_column(lexer) != 0) return false;

    // Match the word `to` case-insensitively.
    const char target[] = {'t', 'o'};
    for (int i = 0; i < 2; i++) {
        int32_t c = lexer->lookahead;
        if (c >= 'A' && c <= 'Z') c += 32;
        if (c != target[i]) return false;
        advance(lexer);
    }

    // Word boundary check — refuse to consume a partial token like `toString`.
    int32_t after = lexer->lookahead;
    if (after == '_' || iswalnum(after)) return false;

    lexer->result_symbol = KEYWORD_HANDLER_TO;
    return true;
}

// `inline_marker` is a zero-width token emitted between `then` and the
// one-liner tail of an `if_simple_statement`. It emits when the next
// real character is on the same LOGICAL line — same physical row, or
// reached via one-or-more `¬` (U+00AC) line-continuation glyphs each
// followed by a newline and any indent. A bare newline (no `¬`) means
// the tail is on a separate logical line and we refuse — forcing the
// parser to use the `if_block` form instead.
//
// Zero-width — `mark_end` is called immediately so the resulting token
// occupies no input. The lexer does advance past `¬`+newline+indent
// sequences while checking (so the next lex sees the right position),
// but that whitespace is normally consumed as `extras` anyway.
static bool scan_inline_marker(TSLexer *lexer) {
    lexer->mark_end(lexer);

    for (;;) {
        int32_t c = lexer->lookahead;
        // EOF after `then` — no tail.
        if (c == 0 && lexer->eof(lexer)) return false;
        // `¬` line-continuation: skip it, the following newline, and
        // any leading whitespace on the next line, then re-check.
        if (c == 0x00AC) {
            advance(lexer);
            while (lexer->lookahead == ' ' || lexer->lookahead == '\t') {
                advance(lexer);
            }
            if (lexer->lookahead == '\n' || lexer->lookahead == '\r') {
                advance(lexer);
                if (lexer->lookahead == '\n') advance(lexer);
                while (lexer->lookahead == ' ' || lexer->lookahead == '\t') {
                    advance(lexer);
                }
            }
            // `mark_end` stays at the original start — token remains
            // zero-width — but the lexer position is now past the
            // continuation. Continue the loop to check what comes next.
            continue;
        }
        // Bare newline (no preceding ¬): tail is on a separate logical
        // line; reject and let if_block take over.
        if (c == '\n' || c == '\r') return false;
        // Comment starters strongly suggest no inline tail.
        if (c == '#') return false;
        if (c == '-') {
            // We can't distinguish `--` (comment) from `-N` (negative
            // number) without consuming `-`, which would advance the
            // lexer past it. Be conservative — reject. `if x then -5`
            // is exotic.
            return false;
        }
        if (c == '(') return false;

        // Real character — accept.
        lexer->result_symbol = INLINE_MARKER;
        return true;
    }
}

bool tree_sitter_applescript_external_scanner_scan(void *payload, TSLexer *lexer, const bool *valid_symbols) {
    (void)payload;

    // INLINE_MARKER is checked FIRST, before any newline-skipping, because
    // its whole purpose is to detect a newline between `then` and the tail.
    // We only skip spaces/tabs (not newlines) before delegating so that the
    // marker sees the same lookahead any in-grammar token would see.
    if (valid_symbols[INLINE_MARKER]) {
        while (lexer->lookahead == ' ' || lexer->lookahead == '\t') {
            skip(lexer);
        }
        return scan_inline_marker(lexer);
    }

    // For block_comment we MUST skip newlines — otherwise the internal lexer
    // races us to the first significant character and consumes `(` as a
    // literal `(` token before we ever see it. For alias_prefix and
    // compound_word we deliberately do NOT skip newlines, so a multi-word
    // compound_name can't reach across a newline into the next statement.
    if (valid_symbols[BLOCK_COMMENT]) {
        while (lexer->lookahead == ' ' || lexer->lookahead == '\t' ||
               lexer->lookahead == '\n' || lexer->lookahead == '\r' ||
               lexer->lookahead == 0x00AC) {
            skip(lexer);
        }
        if (lexer->lookahead == '(') {
            return scan_block_comment(lexer);
        }
    } else {
        // Skip spaces and tabs only — newlines stop a compound_name.
        while (lexer->lookahead == ' ' || lexer->lookahead == '\t') {
            skip(lexer);
        }
    }

    if (valid_symbols[KEYWORD_HANDLER_TO] &&
        (lexer->lookahead == 't' || lexer->lookahead == 'T')) {
        if (scan_keyword_handler_to(lexer)) return true;
        // Not at column 0 — fall through; the regular keyword_to from the
        // grammar lexer handles non-handler `to`.
    }

    if (valid_symbols[ALIAS_PREFIX] &&
        (lexer->lookahead == 'a' || lexer->lookahead == 'A')) {
        return scan_alias_prefix(lexer);
    }

    if (valid_symbols[PIPED_IDENTIFIER] && lexer->lookahead == '|') {
        return scan_piped_identifier(lexer);
    }

    return false;
}
