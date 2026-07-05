/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

// Tree-sitter grammar for Sema — a Lisp dialect.
// Token rules derived from crates/sema-reader/src/lexer.rs.

// Symbol character classes matching is_symbol_start / is_symbol_char in lexer.rs:
//   start: alphabetic | + - * / ! ? < > = _ & % ^ ~ .
//   rest:  start | ascii digit | - / .
// Note: `-` and `.` appear in both sets; digits only in rest.
const SYMBOL_START = /[a-zA-Z\u00C0-\u024F+\-*\/!?<>=_&%^~.]/;
const SYMBOL_CHAR = /[a-zA-Z\u00C0-\u024F+\-*\/!?<>=_&%^~.#0-9]/;

module.exports = grammar({
  name: 'sema',

  externals: $ => [
    $.block_comment,
  ],

  extras: $ => [
    /\s/,
    $.comment,
    $.block_comment,
  ],

  word: $ => $.symbol,

  rules: {
    // A shebang (`#!/usr/bin/env sema`) is valid only as the first line of a
    // file, so it is an optional leading child rather than a general form.
    source_file: $ => seq(
      optional($.shebang),
      repeat($._form),
    ),

    _form: $ => choice(
      $.list,
      $.short_lambda,
      $.vector,
      $.hash_map,
      $.quote,
      $.quasiquote,
      $.unquote,
      $.unquote_splicing,
      $.deref,
      $.byte_vector,
      $._atom,
    ),

    // ── Compound forms ──────────────────────────────────────────────

    list: $ => choice(
      seq('(', repeat($._form), ')'),
      // Dotted pair: (a b . c)
      seq('(', repeat1($._form), '.', $._form, ')'),
    ),

    // Short lambda: #(...) — a `#(`-opened form whose body may reference the
    // implicit placeholder args %, %1..%9, %& (all lexed as ordinary symbols).
    short_lambda: $ => seq('#(', repeat($._form), ')'),

    vector: $ => seq('[', repeat($._form), ']'),

    hash_map: $ => seq('{', repeat($._form), '}'),

    // ── Quote / unquote ─────────────────────────────────────────────

    quote: $ => seq("'", $._form),

    quasiquote: $ => seq('`', $._form),

    unquote: $ => seq(',', $._form),

    unquote_splicing: $ => seq(',@', $._form),

    // Deref reader macro: @expr → (deref expr) ───────────────────────

    deref: $ => seq('@', $._form),

    // ── Byte vector: #u8( num* ) ────────────────────────────────────

    byte_vector: $ => seq('#u8(', repeat($.number), ')'),

    // ── Atoms ───────────────────────────────────────────────────────

    _atom: $ => choice(
      $.number,
      $.string,
      $.regex,
      $.symbol,
      $.keyword,
      $.boolean,
      $.character,
    ),

    // Numbers: integer or float, optional leading minus.
    // Negative numbers only when `-` is immediately followed by a digit
    // (otherwise `-` is a symbol).
    // Float requires digit(s) on both sides of the dot: 3.14, -0.5
    number: _$ => token(
      seq(
        optional('-'),
        /[0-9]+/,
        optional(seq('.', /[0-9]+/)),
      ),
    ),

    // Strings with escape sequences ──────────────────────────────────

    // A string is a single token, mirroring the reader (one Token::String).
    // Making it one token is also what keeps a `;` in the body (e.g. "a;b")
    // from being lexed as a line comment that swallows the closing quote —
    // `extras` can never be injected inside a single token. The only escape
    // that matters for delimiting is `\"`; every other `\x` is content (the
    // reader preserves unknown escapes literally).
    string: _$ => token(/"([^"\\]|\\.)*"/),

    // Regex literal: #"pattern" ──────────────────────────────────────
    // A raw string where the only recognized escape is \" (which does NOT
    // terminate the literal); every other backslash is literal (e.g. \d).
    regex: _$ => token(/#"([^"\\]|\\.)*"/),

    // Shebang: #!... — first line of a file only (enforced via source_file)
    shebang: _$ => token(/#![^\r\n]*/),

    // Symbols ────────────────────────────────────────────────────────
    // Must not match "true", "false", or a bare "." (those are
    // handled by `boolean` and the dot in `list`).

    symbol: _$ => token(
      seq(SYMBOL_START, repeat(SYMBOL_CHAR)),
    ),

    // Keywords: colon followed by symbol chars ───────────────────────

    keyword: _$ => token(
      seq(':', SYMBOL_CHAR, repeat(SYMBOL_CHAR)),
    ),

    // Booleans ───────────────────────────────────────────────────────

    boolean: _$ => token(prec(1, choice('#t', '#f', 'true', 'false'))),

    // Character literals ─────────────────────────────────────────────
    // #\a  #\space  #\newline  #\tab  #\return  #\nul
    // Single non-alpha char: #\(  #\)  etc.

    character: _$ => token(
      seq(
        '#\\',
        choice(
          'space',
          'newline',
          'tab',
          'return',
          'nul',
          /./,   // any single character
        ),
      ),
    ),

    // Line comment ───────────────────────────────────────────────────

    comment: _$ => token(seq(';', /[^\n]*/)),
  },
});
