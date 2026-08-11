#include <tree_sitter/parser.h>

#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wmissing-field-initializers"
#endif

#define LANGUAGE_VERSION 14
#define STATE_COUNT 86
#define LARGE_STATE_COUNT 6
#define SYMBOL_COUNT 64
#define ALIAS_COUNT 0
#define TOKEN_COUNT 40
#define EXTERNAL_TOKEN_COUNT 0
#define FIELD_COUNT 1
#define MAX_ALIAS_SEQUENCE_LENGTH 6
#define PRODUCTION_ID_COUNT 2

enum {
  anon_sym_EQ = 1,
  anon_sym_LBRACE = 2,
  anon_sym_RBRACE = 3,
  sym_identifier = 4,
  anon_sym__ = 5,
  anon_sym_AT = 6,
  anon_sym_DOLLAR = 7,
  anon_sym_PIPE = 8,
  anon_sym_LPAREN = 9,
  anon_sym_RPAREN = 10,
  anon_sym_DOT_DOT = 11,
  sym_character = 12,
  anon_sym_DQUOTE = 13,
  sym__unescaped_double_string_fragment = 14,
  sym__escape_sequence = 15,
  anon_sym_CARET = 16,
  anon_sym_PUSH = 17,
  anon_sym_PEEK = 18,
  anon_sym_LBRACK = 19,
  aux_sym__peek_slice_token1 = 20,
  anon_sym_RBRACK = 21,
  anon_sym_POP = 22,
  anon_sym_ANY = 23,
  anon_sym_SOI = 24,
  anon_sym_EOI = 25,
  anon_sym_NEWLINE = 26,
  sym_const = 27,
  anon_sym_POUND = 28,
  anon_sym_TILDE = 29,
  anon_sym_AMP = 30,
  anon_sym_BANG = 31,
  anon_sym_QMARK = 32,
  anon_sym_STAR = 33,
  anon_sym_PLUS = 34,
  anon_sym_COMMA = 35,
  sym_line_comment = 36,
  anon_sym_SLASH_STAR = 37,
  aux_sym_block_comment_token1 = 38,
  anon_sym_SLASH = 39,
  sym_pest = 40,
  sym_grammar_rule = 41,
  sym_modifier = 42,
  sym_expression = 43,
  sym_term = 44,
  sym__node = 45,
  sym__terminal = 46,
  sym_range = 47,
  sym_string = 48,
  sym__insensitive_string = 49,
  sym__push = 50,
  sym__peek_slice = 51,
  sym__pop = 52,
  sym_builtin = 53,
  sym_node_tag = 54,
  sym_infix_operator = 55,
  sym_prefix_operator = 56,
  sym_postfix_operator = 57,
  sym_block_comment = 58,
  aux_sym_pest_repeat1 = 59,
  aux_sym_expression_repeat1 = 60,
  aux_sym_term_repeat1 = 61,
  aux_sym_term_repeat2 = 62,
  aux_sym_string_repeat1 = 63,
};

static const char * const ts_symbol_names[] = {
  [ts_builtin_sym_end] = "end",
  [anon_sym_EQ] = "=",
  [anon_sym_LBRACE] = "{",
  [anon_sym_RBRACE] = "}",
  [sym_identifier] = "identifier",
  [anon_sym__] = "_",
  [anon_sym_AT] = "@",
  [anon_sym_DOLLAR] = "$",
  [anon_sym_PIPE] = "|",
  [anon_sym_LPAREN] = "(",
  [anon_sym_RPAREN] = ")",
  [anon_sym_DOT_DOT] = "..",
  [sym_character] = "character",
  [anon_sym_DQUOTE] = "\"",
  [sym__unescaped_double_string_fragment] = "_unescaped_double_string_fragment",
  [sym__escape_sequence] = "_escape_sequence",
  [anon_sym_CARET] = "^",
  [anon_sym_PUSH] = "PUSH",
  [anon_sym_PEEK] = "PEEK",
  [anon_sym_LBRACK] = "[",
  [aux_sym__peek_slice_token1] = "_peek_slice_token1",
  [anon_sym_RBRACK] = "]",
  [anon_sym_POP] = "POP",
  [anon_sym_ANY] = "ANY",
  [anon_sym_SOI] = "SOI",
  [anon_sym_EOI] = "EOI",
  [anon_sym_NEWLINE] = "NEWLINE",
  [sym_const] = "const",
  [anon_sym_POUND] = "#",
  [anon_sym_TILDE] = "~",
  [anon_sym_AMP] = "&",
  [anon_sym_BANG] = "!",
  [anon_sym_QMARK] = "\?",
  [anon_sym_STAR] = "*",
  [anon_sym_PLUS] = "+",
  [anon_sym_COMMA] = ",",
  [sym_line_comment] = "line_comment",
  [anon_sym_SLASH_STAR] = "/*",
  [aux_sym_block_comment_token1] = "block_comment_token1",
  [anon_sym_SLASH] = "/",
  [sym_pest] = "pest",
  [sym_grammar_rule] = "grammar_rule",
  [sym_modifier] = "modifier",
  [sym_expression] = "expression",
  [sym_term] = "term",
  [sym__node] = "_node",
  [sym__terminal] = "_terminal",
  [sym_range] = "range",
  [sym_string] = "string",
  [sym__insensitive_string] = "_insensitive_string",
  [sym__push] = "_push",
  [sym__peek_slice] = "_peek_slice",
  [sym__pop] = "_pop",
  [sym_builtin] = "builtin",
  [sym_node_tag] = "node_tag",
  [sym_infix_operator] = "infix_operator",
  [sym_prefix_operator] = "prefix_operator",
  [sym_postfix_operator] = "postfix_operator",
  [sym_block_comment] = "block_comment",
  [aux_sym_pest_repeat1] = "pest_repeat1",
  [aux_sym_expression_repeat1] = "expression_repeat1",
  [aux_sym_term_repeat1] = "term_repeat1",
  [aux_sym_term_repeat2] = "term_repeat2",
  [aux_sym_string_repeat1] = "string_repeat1",
};

static const TSSymbol ts_symbol_map[] = {
  [ts_builtin_sym_end] = ts_builtin_sym_end,
  [anon_sym_EQ] = anon_sym_EQ,
  [anon_sym_LBRACE] = anon_sym_LBRACE,
  [anon_sym_RBRACE] = anon_sym_RBRACE,
  [sym_identifier] = sym_identifier,
  [anon_sym__] = anon_sym__,
  [anon_sym_AT] = anon_sym_AT,
  [anon_sym_DOLLAR] = anon_sym_DOLLAR,
  [anon_sym_PIPE] = anon_sym_PIPE,
  [anon_sym_LPAREN] = anon_sym_LPAREN,
  [anon_sym_RPAREN] = anon_sym_RPAREN,
  [anon_sym_DOT_DOT] = anon_sym_DOT_DOT,
  [sym_character] = sym_character,
  [anon_sym_DQUOTE] = anon_sym_DQUOTE,
  [sym__unescaped_double_string_fragment] = sym__unescaped_double_string_fragment,
  [sym__escape_sequence] = sym__escape_sequence,
  [anon_sym_CARET] = anon_sym_CARET,
  [anon_sym_PUSH] = anon_sym_PUSH,
  [anon_sym_PEEK] = anon_sym_PEEK,
  [anon_sym_LBRACK] = anon_sym_LBRACK,
  [aux_sym__peek_slice_token1] = aux_sym__peek_slice_token1,
  [anon_sym_RBRACK] = anon_sym_RBRACK,
  [anon_sym_POP] = anon_sym_POP,
  [anon_sym_ANY] = anon_sym_ANY,
  [anon_sym_SOI] = anon_sym_SOI,
  [anon_sym_EOI] = anon_sym_EOI,
  [anon_sym_NEWLINE] = anon_sym_NEWLINE,
  [sym_const] = sym_const,
  [anon_sym_POUND] = anon_sym_POUND,
  [anon_sym_TILDE] = anon_sym_TILDE,
  [anon_sym_AMP] = anon_sym_AMP,
  [anon_sym_BANG] = anon_sym_BANG,
  [anon_sym_QMARK] = anon_sym_QMARK,
  [anon_sym_STAR] = anon_sym_STAR,
  [anon_sym_PLUS] = anon_sym_PLUS,
  [anon_sym_COMMA] = anon_sym_COMMA,
  [sym_line_comment] = sym_line_comment,
  [anon_sym_SLASH_STAR] = anon_sym_SLASH_STAR,
  [aux_sym_block_comment_token1] = aux_sym_block_comment_token1,
  [anon_sym_SLASH] = anon_sym_SLASH,
  [sym_pest] = sym_pest,
  [sym_grammar_rule] = sym_grammar_rule,
  [sym_modifier] = sym_modifier,
  [sym_expression] = sym_expression,
  [sym_term] = sym_term,
  [sym__node] = sym__node,
  [sym__terminal] = sym__terminal,
  [sym_range] = sym_range,
  [sym_string] = sym_string,
  [sym__insensitive_string] = sym__insensitive_string,
  [sym__push] = sym__push,
  [sym__peek_slice] = sym__peek_slice,
  [sym__pop] = sym__pop,
  [sym_builtin] = sym_builtin,
  [sym_node_tag] = sym_node_tag,
  [sym_infix_operator] = sym_infix_operator,
  [sym_prefix_operator] = sym_prefix_operator,
  [sym_postfix_operator] = sym_postfix_operator,
  [sym_block_comment] = sym_block_comment,
  [aux_sym_pest_repeat1] = aux_sym_pest_repeat1,
  [aux_sym_expression_repeat1] = aux_sym_expression_repeat1,
  [aux_sym_term_repeat1] = aux_sym_term_repeat1,
  [aux_sym_term_repeat2] = aux_sym_term_repeat2,
  [aux_sym_string_repeat1] = aux_sym_string_repeat1,
};

static const TSSymbolMetadata ts_symbol_metadata[] = {
  [ts_builtin_sym_end] = {
    .visible = false,
    .named = true,
  },
  [anon_sym_EQ] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_LBRACE] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RBRACE] = {
    .visible = true,
    .named = false,
  },
  [sym_identifier] = {
    .visible = true,
    .named = true,
  },
  [anon_sym__] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_AT] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_DOLLAR] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_PIPE] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_LPAREN] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RPAREN] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_DOT_DOT] = {
    .visible = true,
    .named = false,
  },
  [sym_character] = {
    .visible = true,
    .named = true,
  },
  [anon_sym_DQUOTE] = {
    .visible = true,
    .named = false,
  },
  [sym__unescaped_double_string_fragment] = {
    .visible = false,
    .named = true,
  },
  [sym__escape_sequence] = {
    .visible = false,
    .named = true,
  },
  [anon_sym_CARET] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_PUSH] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_PEEK] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_LBRACK] = {
    .visible = true,
    .named = false,
  },
  [aux_sym__peek_slice_token1] = {
    .visible = false,
    .named = false,
  },
  [anon_sym_RBRACK] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_POP] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_ANY] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_SOI] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_EOI] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_NEWLINE] = {
    .visible = true,
    .named = false,
  },
  [sym_const] = {
    .visible = true,
    .named = true,
  },
  [anon_sym_POUND] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_TILDE] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_AMP] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_BANG] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_QMARK] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_STAR] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_PLUS] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_COMMA] = {
    .visible = true,
    .named = false,
  },
  [sym_line_comment] = {
    .visible = true,
    .named = true,
  },
  [anon_sym_SLASH_STAR] = {
    .visible = true,
    .named = false,
  },
  [aux_sym_block_comment_token1] = {
    .visible = false,
    .named = false,
  },
  [anon_sym_SLASH] = {
    .visible = true,
    .named = false,
  },
  [sym_pest] = {
    .visible = true,
    .named = true,
  },
  [sym_grammar_rule] = {
    .visible = true,
    .named = true,
  },
  [sym_modifier] = {
    .visible = true,
    .named = true,
  },
  [sym_expression] = {
    .visible = true,
    .named = true,
  },
  [sym_term] = {
    .visible = true,
    .named = true,
  },
  [sym__node] = {
    .visible = false,
    .named = true,
  },
  [sym__terminal] = {
    .visible = false,
    .named = true,
  },
  [sym_range] = {
    .visible = true,
    .named = true,
  },
  [sym_string] = {
    .visible = true,
    .named = true,
  },
  [sym__insensitive_string] = {
    .visible = false,
    .named = true,
  },
  [sym__push] = {
    .visible = false,
    .named = true,
  },
  [sym__peek_slice] = {
    .visible = false,
    .named = true,
  },
  [sym__pop] = {
    .visible = false,
    .named = true,
  },
  [sym_builtin] = {
    .visible = true,
    .named = true,
  },
  [sym_node_tag] = {
    .visible = true,
    .named = true,
  },
  [sym_infix_operator] = {
    .visible = true,
    .named = true,
  },
  [sym_prefix_operator] = {
    .visible = true,
    .named = true,
  },
  [sym_postfix_operator] = {
    .visible = true,
    .named = true,
  },
  [sym_block_comment] = {
    .visible = true,
    .named = true,
  },
  [aux_sym_pest_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_expression_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_term_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_term_repeat2] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_string_repeat1] = {
    .visible = false,
    .named = false,
  },
};

enum {
  field_name = 1,
};

static const char * const ts_field_names[] = {
  [0] = NULL,
  [field_name] = "name",
};

static const TSFieldMapSlice ts_field_map_slices[PRODUCTION_ID_COUNT] = {
  [1] = {.index = 0, .length = 1},
};

static const TSFieldMapEntry ts_field_map_entries[] = {
  [0] =
    {field_name, 0},
};

static const TSSymbol ts_alias_sequences[PRODUCTION_ID_COUNT][MAX_ALIAS_SEQUENCE_LENGTH] = {
  [0] = {0},
};

static const uint16_t ts_non_terminal_alias_map[] = {
  0,
};

static const TSStateId ts_primary_state_ids[STATE_COUNT] = {
  [0] = 0,
  [1] = 1,
  [2] = 2,
  [3] = 3,
  [4] = 4,
  [5] = 5,
  [6] = 6,
  [7] = 7,
  [8] = 8,
  [9] = 9,
  [10] = 10,
  [11] = 11,
  [12] = 12,
  [13] = 13,
  [14] = 14,
  [15] = 15,
  [16] = 16,
  [17] = 17,
  [18] = 18,
  [19] = 19,
  [20] = 20,
  [21] = 21,
  [22] = 22,
  [23] = 23,
  [24] = 24,
  [25] = 25,
  [26] = 26,
  [27] = 27,
  [28] = 28,
  [29] = 29,
  [30] = 30,
  [31] = 31,
  [32] = 32,
  [33] = 33,
  [34] = 34,
  [35] = 35,
  [36] = 36,
  [37] = 37,
  [38] = 38,
  [39] = 39,
  [40] = 40,
  [41] = 41,
  [42] = 42,
  [43] = 43,
  [44] = 44,
  [45] = 45,
  [46] = 46,
  [47] = 47,
  [48] = 48,
  [49] = 49,
  [50] = 50,
  [51] = 51,
  [52] = 52,
  [53] = 53,
  [54] = 54,
  [55] = 55,
  [56] = 56,
  [57] = 57,
  [58] = 58,
  [59] = 59,
  [60] = 60,
  [61] = 61,
  [62] = 62,
  [63] = 63,
  [64] = 64,
  [65] = 65,
  [66] = 66,
  [67] = 67,
  [68] = 68,
  [69] = 69,
  [70] = 70,
  [71] = 71,
  [72] = 72,
  [73] = 73,
  [74] = 74,
  [75] = 75,
  [76] = 76,
  [77] = 77,
  [78] = 78,
  [79] = 79,
  [80] = 80,
  [81] = 81,
  [82] = 82,
  [83] = 83,
  [84] = 84,
  [85] = 85,
};

static bool ts_lex(TSLexer *lexer, TSStateId state) {
  START_LEXER();
  eof = lexer->eof(lexer);
  switch (state) {
    case 0:
      if (eof) ADVANCE(34);
      if (lookahead == '!') ADVANCE(83);
      if (lookahead == '"') ADVANCE(49);
      if (lookahead == '#') ADVANCE(80);
      if (lookahead == '$') ADVANCE(43);
      if (lookahead == '&') ADVANCE(82);
      if (lookahead == '\'') ADVANCE(6);
      if (lookahead == '(') ADVANCE(45);
      if (lookahead == ')') ADVANCE(46);
      if (lookahead == '*') ADVANCE(85);
      if (lookahead == '+') ADVANCE(86);
      if (lookahead == ',') ADVANCE(87);
      if (lookahead == '.') ADVANCE(11);
      if (lookahead == '/') ADVANCE(96);
      if (lookahead == '=') ADVANCE(35);
      if (lookahead == '?') ADVANCE(84);
      if (lookahead == '@') ADVANCE(42);
      if (lookahead == 'A') ADVANCE(14);
      if (lookahead == 'E') ADVANCE(15);
      if (lookahead == 'N') ADVANCE(12);
      if (lookahead == 'P') ADVANCE(13);
      if (lookahead == 'S') ADVANCE(16);
      if (lookahead == '[') ADVANCE(58);
      if (lookahead == '\\') ADVANCE(17);
      if (lookahead == ']') ADVANCE(60);
      if (lookahead == '^') ADVANCE(55);
      if (lookahead == '_') ADVANCE(41);
      if (lookahead == '{') ADVANCE(36);
      if (lookahead == '|') ADVANCE(44);
      if (lookahead == '}') ADVANCE(37);
      if (lookahead == '~') ADVANCE(81);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') SKIP(32)
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(59);
      if (('B' <= lookahead && lookahead <= 'Z')) ADVANCE(31);
      if (('a' <= lookahead && lookahead <= 'z')) ADVANCE(39);
      END_STATE();
    case 1:
      if (lookahead == '!') ADVANCE(83);
      if (lookahead == '"') ADVANCE(49);
      if (lookahead == '#') ADVANCE(80);
      if (lookahead == '&') ADVANCE(82);
      if (lookahead == '\'') ADVANCE(6);
      if (lookahead == '(') ADVANCE(45);
      if (lookahead == '/') ADVANCE(7);
      if (lookahead == 'A') ADVANCE(14);
      if (lookahead == 'E') ADVANCE(15);
      if (lookahead == 'N') ADVANCE(12);
      if (lookahead == 'P') ADVANCE(13);
      if (lookahead == 'S') ADVANCE(16);
      if (lookahead == '^') ADVANCE(55);
      if (lookahead == '_') ADVANCE(38);
      if (lookahead == '|') ADVANCE(44);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') SKIP(1)
      if (('B' <= lookahead && lookahead <= 'Z')) ADVANCE(31);
      if (('a' <= lookahead && lookahead <= 'z')) ADVANCE(39);
      END_STATE();
    case 2:
      if (lookahead == '"') ADVANCE(49);
      if (lookahead == '/') ADVANCE(7);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') SKIP(2)
      END_STATE();
    case 3:
      if (lookahead == '"') ADVANCE(49);
      if (lookahead == '/') ADVANCE(50);
      if (lookahead == '\\') ADVANCE(17);
      if (lookahead == '\t' ||
          lookahead == ' ') ADVANCE(51);
      if (lookahead == '\n' ||
          lookahead == '\r') SKIP(2)
      if (lookahead != 0) ADVANCE(53);
      END_STATE();
    case 4:
      if (lookahead == '$') ADVANCE(43);
      if (lookahead == '/') ADVANCE(7);
      if (lookahead == '@') ADVANCE(42);
      if (lookahead == '_') ADVANCE(40);
      if (lookahead == '{') ADVANCE(36);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') SKIP(4)
      END_STATE();
    case 5:
      if (lookahead == '\'') ADVANCE(48);
      END_STATE();
    case 6:
      if (lookahead == '\'') ADVANCE(48);
      if (lookahead == '\\') ADVANCE(18);
      if (lookahead != 0) ADVANCE(5);
      END_STATE();
    case 7:
      if (lookahead == '*') ADVANCE(93);
      if (lookahead == '/') ADVANCE(91);
      END_STATE();
    case 8:
      if (lookahead == '*') ADVANCE(95);
      if (lookahead == '/') ADVANCE(10);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') ADVANCE(8);
      if (lookahead != 0) ADVANCE(9);
      END_STATE();
    case 9:
      if (lookahead == '*') ADVANCE(95);
      if (lookahead != 0) ADVANCE(9);
      END_STATE();
    case 10:
      if (lookahead == '*') ADVANCE(94);
      if (lookahead == '/') ADVANCE(90);
      if (lookahead != 0) ADVANCE(9);
      END_STATE();
    case 11:
      if (lookahead == '.') ADVANCE(47);
      END_STATE();
    case 12:
      if (lookahead == 'E') ADVANCE(77);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 13:
      if (lookahead == 'E') ADVANCE(66);
      if (lookahead == 'O') ADVANCE(75);
      if (lookahead == 'U') ADVANCE(76);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 14:
      if (lookahead == 'N') ADVANCE(78);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 15:
      if (lookahead == 'O') ADVANCE(69);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 16:
      if (lookahead == 'O') ADVANCE(70);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 17:
      if (lookahead == 'u') ADVANCE(19);
      if (lookahead == 'x') ADVANCE(28);
      if (lookahead != 0) ADVANCE(54);
      END_STATE();
    case 18:
      if (lookahead == 'u') ADVANCE(20);
      if (lookahead == 'x') ADVANCE(29);
      if (lookahead != 0) ADVANCE(5);
      END_STATE();
    case 19:
      if (lookahead == '{') ADVANCE(26);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(24);
      END_STATE();
    case 20:
      if (lookahead == '{') ADVANCE(27);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(30);
      END_STATE();
    case 21:
      if (lookahead == '}') ADVANCE(5);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(21);
      END_STATE();
    case 22:
      if (lookahead == '}') ADVANCE(54);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(22);
      END_STATE();
    case 23:
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(5);
      END_STATE();
    case 24:
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(28);
      END_STATE();
    case 25:
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(54);
      END_STATE();
    case 26:
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(22);
      END_STATE();
    case 27:
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(21);
      END_STATE();
    case 28:
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(25);
      END_STATE();
    case 29:
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(23);
      END_STATE();
    case 30:
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(29);
      END_STATE();
    case 31:
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 32:
      if (eof) ADVANCE(34);
      if (lookahead == '!') ADVANCE(83);
      if (lookahead == '"') ADVANCE(49);
      if (lookahead == '#') ADVANCE(80);
      if (lookahead == '$') ADVANCE(43);
      if (lookahead == '&') ADVANCE(82);
      if (lookahead == '\'') ADVANCE(6);
      if (lookahead == '(') ADVANCE(45);
      if (lookahead == ')') ADVANCE(46);
      if (lookahead == '*') ADVANCE(85);
      if (lookahead == '+') ADVANCE(86);
      if (lookahead == ',') ADVANCE(87);
      if (lookahead == '.') ADVANCE(11);
      if (lookahead == '/') ADVANCE(96);
      if (lookahead == '=') ADVANCE(35);
      if (lookahead == '?') ADVANCE(84);
      if (lookahead == '@') ADVANCE(42);
      if (lookahead == 'A') ADVANCE(14);
      if (lookahead == 'E') ADVANCE(15);
      if (lookahead == 'N') ADVANCE(12);
      if (lookahead == 'P') ADVANCE(13);
      if (lookahead == 'S') ADVANCE(16);
      if (lookahead == '[') ADVANCE(58);
      if (lookahead == ']') ADVANCE(60);
      if (lookahead == '^') ADVANCE(55);
      if (lookahead == '_') ADVANCE(41);
      if (lookahead == '{') ADVANCE(36);
      if (lookahead == '|') ADVANCE(44);
      if (lookahead == '}') ADVANCE(37);
      if (lookahead == '~') ADVANCE(81);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') SKIP(32)
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(59);
      if (('B' <= lookahead && lookahead <= 'Z')) ADVANCE(31);
      if (('a' <= lookahead && lookahead <= 'z')) ADVANCE(39);
      END_STATE();
    case 33:
      if (eof) ADVANCE(34);
      if (lookahead == '/') ADVANCE(7);
      if (lookahead == '\t' ||
          lookahead == '\n' ||
          lookahead == '\r' ||
          lookahead == ' ') SKIP(33)
      if (lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(39);
      END_STATE();
    case 34:
      ACCEPT_TOKEN(ts_builtin_sym_end);
      END_STATE();
    case 35:
      ACCEPT_TOKEN(anon_sym_EQ);
      END_STATE();
    case 36:
      ACCEPT_TOKEN(anon_sym_LBRACE);
      END_STATE();
    case 37:
      ACCEPT_TOKEN(anon_sym_RBRACE);
      END_STATE();
    case 38:
      ACCEPT_TOKEN(sym_identifier);
      if (('0' <= lookahead && lookahead <= '9') ||
          lookahead == '_') ADVANCE(38);
      if (('A' <= lookahead && lookahead <= 'Z')) ADVANCE(79);
      if (('a' <= lookahead && lookahead <= 'z')) ADVANCE(39);
      END_STATE();
    case 39:
      ACCEPT_TOKEN(sym_identifier);
      if (('0' <= lookahead && lookahead <= '9') ||
          lookahead == '_' ||
          ('a' <= lookahead && lookahead <= 'z')) ADVANCE(39);
      END_STATE();
    case 40:
      ACCEPT_TOKEN(anon_sym__);
      END_STATE();
    case 41:
      ACCEPT_TOKEN(anon_sym__);
      if (('0' <= lookahead && lookahead <= '9') ||
          lookahead == '_') ADVANCE(38);
      if (('A' <= lookahead && lookahead <= 'Z')) ADVANCE(79);
      if (('a' <= lookahead && lookahead <= 'z')) ADVANCE(39);
      END_STATE();
    case 42:
      ACCEPT_TOKEN(anon_sym_AT);
      END_STATE();
    case 43:
      ACCEPT_TOKEN(anon_sym_DOLLAR);
      END_STATE();
    case 44:
      ACCEPT_TOKEN(anon_sym_PIPE);
      END_STATE();
    case 45:
      ACCEPT_TOKEN(anon_sym_LPAREN);
      END_STATE();
    case 46:
      ACCEPT_TOKEN(anon_sym_RPAREN);
      END_STATE();
    case 47:
      ACCEPT_TOKEN(anon_sym_DOT_DOT);
      END_STATE();
    case 48:
      ACCEPT_TOKEN(sym_character);
      END_STATE();
    case 49:
      ACCEPT_TOKEN(anon_sym_DQUOTE);
      END_STATE();
    case 50:
      ACCEPT_TOKEN(sym__unescaped_double_string_fragment);
      if (lookahead == '*') ADVANCE(53);
      if (lookahead == '/') ADVANCE(52);
      if (lookahead != 0 &&
          lookahead != '\n' &&
          lookahead != '\r' &&
          lookahead != '"' &&
          lookahead != '\\') ADVANCE(53);
      END_STATE();
    case 51:
      ACCEPT_TOKEN(sym__unescaped_double_string_fragment);
      if (lookahead == '/') ADVANCE(50);
      if (lookahead == '\t' ||
          lookahead == ' ') ADVANCE(51);
      if (lookahead != 0 &&
          lookahead != '\n' &&
          lookahead != '\r' &&
          lookahead != '"' &&
          lookahead != '\\') ADVANCE(53);
      END_STATE();
    case 52:
      ACCEPT_TOKEN(sym__unescaped_double_string_fragment);
      if (lookahead == '!' ||
          lookahead == '/') ADVANCE(53);
      if (lookahead != 0 &&
          lookahead != '\n' &&
          lookahead != '\r' &&
          lookahead != '"' &&
          lookahead != '\\') ADVANCE(53);
      END_STATE();
    case 53:
      ACCEPT_TOKEN(sym__unescaped_double_string_fragment);
      if (lookahead != 0 &&
          lookahead != '\n' &&
          lookahead != '\r' &&
          lookahead != '"' &&
          lookahead != '\\') ADVANCE(53);
      END_STATE();
    case 54:
      ACCEPT_TOKEN(sym__escape_sequence);
      END_STATE();
    case 55:
      ACCEPT_TOKEN(anon_sym_CARET);
      END_STATE();
    case 56:
      ACCEPT_TOKEN(anon_sym_PUSH);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 57:
      ACCEPT_TOKEN(anon_sym_PEEK);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 58:
      ACCEPT_TOKEN(anon_sym_LBRACK);
      END_STATE();
    case 59:
      ACCEPT_TOKEN(aux_sym__peek_slice_token1);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(59);
      END_STATE();
    case 60:
      ACCEPT_TOKEN(anon_sym_RBRACK);
      END_STATE();
    case 61:
      ACCEPT_TOKEN(anon_sym_POP);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 62:
      ACCEPT_TOKEN(anon_sym_ANY);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 63:
      ACCEPT_TOKEN(anon_sym_SOI);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 64:
      ACCEPT_TOKEN(anon_sym_EOI);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 65:
      ACCEPT_TOKEN(anon_sym_NEWLINE);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 66:
      ACCEPT_TOKEN(sym_const);
      if (lookahead == 'E') ADVANCE(72);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 67:
      ACCEPT_TOKEN(sym_const);
      if (lookahead == 'E') ADVANCE(65);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 68:
      ACCEPT_TOKEN(sym_const);
      if (lookahead == 'H') ADVANCE(56);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 69:
      ACCEPT_TOKEN(sym_const);
      if (lookahead == 'I') ADVANCE(64);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 70:
      ACCEPT_TOKEN(sym_const);
      if (lookahead == 'I') ADVANCE(63);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 71:
      ACCEPT_TOKEN(sym_const);
      if (lookahead == 'I') ADVANCE(74);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 72:
      ACCEPT_TOKEN(sym_const);
      if (lookahead == 'K') ADVANCE(57);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 73:
      ACCEPT_TOKEN(sym_const);
      if (lookahead == 'L') ADVANCE(71);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 74:
      ACCEPT_TOKEN(sym_const);
      if (lookahead == 'N') ADVANCE(67);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 75:
      ACCEPT_TOKEN(sym_const);
      if (lookahead == 'P') ADVANCE(61);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 76:
      ACCEPT_TOKEN(sym_const);
      if (lookahead == 'S') ADVANCE(68);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 77:
      ACCEPT_TOKEN(sym_const);
      if (lookahead == 'W') ADVANCE(73);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 78:
      ACCEPT_TOKEN(sym_const);
      if (lookahead == 'Y') ADVANCE(62);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 79:
      ACCEPT_TOKEN(sym_const);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'Z') ||
          lookahead == '_') ADVANCE(79);
      END_STATE();
    case 80:
      ACCEPT_TOKEN(anon_sym_POUND);
      END_STATE();
    case 81:
      ACCEPT_TOKEN(anon_sym_TILDE);
      END_STATE();
    case 82:
      ACCEPT_TOKEN(anon_sym_AMP);
      END_STATE();
    case 83:
      ACCEPT_TOKEN(anon_sym_BANG);
      END_STATE();
    case 84:
      ACCEPT_TOKEN(anon_sym_QMARK);
      END_STATE();
    case 85:
      ACCEPT_TOKEN(anon_sym_STAR);
      END_STATE();
    case 86:
      ACCEPT_TOKEN(anon_sym_PLUS);
      END_STATE();
    case 87:
      ACCEPT_TOKEN(anon_sym_COMMA);
      END_STATE();
    case 88:
      ACCEPT_TOKEN(sym_line_comment);
      if (lookahead == '\n') ADVANCE(9);
      if (lookahead == '*') ADVANCE(88);
      if (lookahead == '/') ADVANCE(92);
      if (lookahead != 0) ADVANCE(89);
      END_STATE();
    case 89:
      ACCEPT_TOKEN(sym_line_comment);
      if (lookahead == '\n') ADVANCE(9);
      if (lookahead == '*') ADVANCE(88);
      if (lookahead != 0) ADVANCE(89);
      END_STATE();
    case 90:
      ACCEPT_TOKEN(sym_line_comment);
      if (lookahead == '\n') ADVANCE(9);
      if (lookahead == '!' ||
          lookahead == '/') ADVANCE(89);
      if (lookahead == '*') ADVANCE(88);
      if (lookahead != 0) ADVANCE(89);
      END_STATE();
    case 91:
      ACCEPT_TOKEN(sym_line_comment);
      if (lookahead == '!' ||
          lookahead == '/') ADVANCE(92);
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(92);
      END_STATE();
    case 92:
      ACCEPT_TOKEN(sym_line_comment);
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(92);
      END_STATE();
    case 93:
      ACCEPT_TOKEN(anon_sym_SLASH_STAR);
      END_STATE();
    case 94:
      ACCEPT_TOKEN(anon_sym_SLASH_STAR);
      if (lookahead == '*') ADVANCE(95);
      if (lookahead != 0 &&
          lookahead != '/') ADVANCE(9);
      END_STATE();
    case 95:
      ACCEPT_TOKEN(aux_sym_block_comment_token1);
      if (lookahead == '*') ADVANCE(95);
      if (lookahead != 0 &&
          lookahead != '/') ADVANCE(9);
      END_STATE();
    case 96:
      ACCEPT_TOKEN(anon_sym_SLASH);
      if (lookahead == '*') ADVANCE(93);
      if (lookahead == '/') ADVANCE(91);
      END_STATE();
    default:
      return false;
  }
}

static const TSLexMode ts_lex_modes[STATE_COUNT] = {
  [0] = {.lex_state = 0},
  [1] = {.lex_state = 33},
  [2] = {.lex_state = 1},
  [3] = {.lex_state = 1},
  [4] = {.lex_state = 1},
  [5] = {.lex_state = 1},
  [6] = {.lex_state = 1},
  [7] = {.lex_state = 1},
  [8] = {.lex_state = 1},
  [9] = {.lex_state = 1},
  [10] = {.lex_state = 1},
  [11] = {.lex_state = 1},
  [12] = {.lex_state = 1},
  [13] = {.lex_state = 1},
  [14] = {.lex_state = 1},
  [15] = {.lex_state = 1},
  [16] = {.lex_state = 0},
  [17] = {.lex_state = 0},
  [18] = {.lex_state = 0},
  [19] = {.lex_state = 0},
  [20] = {.lex_state = 0},
  [21] = {.lex_state = 0},
  [22] = {.lex_state = 0},
  [23] = {.lex_state = 0},
  [24] = {.lex_state = 0},
  [25] = {.lex_state = 0},
  [26] = {.lex_state = 0},
  [27] = {.lex_state = 0},
  [28] = {.lex_state = 0},
  [29] = {.lex_state = 0},
  [30] = {.lex_state = 0},
  [31] = {.lex_state = 0},
  [32] = {.lex_state = 0},
  [33] = {.lex_state = 0},
  [34] = {.lex_state = 0},
  [35] = {.lex_state = 0},
  [36] = {.lex_state = 0},
  [37] = {.lex_state = 0},
  [38] = {.lex_state = 0},
  [39] = {.lex_state = 0},
  [40] = {.lex_state = 0},
  [41] = {.lex_state = 0},
  [42] = {.lex_state = 0},
  [43] = {.lex_state = 0},
  [44] = {.lex_state = 0},
  [45] = {.lex_state = 0},
  [46] = {.lex_state = 4},
  [47] = {.lex_state = 3},
  [48] = {.lex_state = 33},
  [49] = {.lex_state = 33},
  [50] = {.lex_state = 0},
  [51] = {.lex_state = 3},
  [52] = {.lex_state = 3},
  [53] = {.lex_state = 3},
  [54] = {.lex_state = 0},
  [55] = {.lex_state = 0},
  [56] = {.lex_state = 33},
  [57] = {.lex_state = 0},
  [58] = {.lex_state = 33},
  [59] = {.lex_state = 0},
  [60] = {.lex_state = 0},
  [61] = {.lex_state = 0},
  [62] = {.lex_state = 33},
  [63] = {.lex_state = 0},
  [64] = {.lex_state = 0},
  [65] = {.lex_state = 0},
  [66] = {.lex_state = 33},
  [67] = {.lex_state = 0},
  [68] = {.lex_state = 0},
  [69] = {.lex_state = 0},
  [70] = {.lex_state = 8},
  [71] = {.lex_state = 0},
  [72] = {.lex_state = 0},
  [73] = {.lex_state = 0},
  [74] = {.lex_state = 0},
  [75] = {.lex_state = 0},
  [76] = {.lex_state = 0},
  [77] = {.lex_state = 0},
  [78] = {.lex_state = 0},
  [79] = {.lex_state = 0},
  [80] = {.lex_state = 0},
  [81] = {.lex_state = 0},
  [82] = {.lex_state = 0},
  [83] = {.lex_state = 0},
  [84] = {.lex_state = 0},
  [85] = {(TSStateId)(-1)},
};

static const uint16_t ts_parse_table[LARGE_STATE_COUNT][SYMBOL_COUNT] = {
  [0] = {
    [sym_block_comment] = STATE(0),
    [ts_builtin_sym_end] = ACTIONS(1),
    [anon_sym_EQ] = ACTIONS(1),
    [anon_sym_LBRACE] = ACTIONS(1),
    [anon_sym_RBRACE] = ACTIONS(1),
    [sym_identifier] = ACTIONS(1),
    [anon_sym__] = ACTIONS(1),
    [anon_sym_AT] = ACTIONS(1),
    [anon_sym_DOLLAR] = ACTIONS(1),
    [anon_sym_PIPE] = ACTIONS(1),
    [anon_sym_LPAREN] = ACTIONS(1),
    [anon_sym_RPAREN] = ACTIONS(1),
    [anon_sym_DOT_DOT] = ACTIONS(1),
    [sym_character] = ACTIONS(1),
    [anon_sym_DQUOTE] = ACTIONS(1),
    [sym__escape_sequence] = ACTIONS(1),
    [anon_sym_CARET] = ACTIONS(1),
    [anon_sym_PUSH] = ACTIONS(1),
    [anon_sym_PEEK] = ACTIONS(1),
    [anon_sym_LBRACK] = ACTIONS(1),
    [aux_sym__peek_slice_token1] = ACTIONS(1),
    [anon_sym_RBRACK] = ACTIONS(1),
    [anon_sym_POP] = ACTIONS(1),
    [anon_sym_ANY] = ACTIONS(1),
    [anon_sym_SOI] = ACTIONS(1),
    [anon_sym_EOI] = ACTIONS(1),
    [anon_sym_NEWLINE] = ACTIONS(1),
    [sym_const] = ACTIONS(1),
    [anon_sym_POUND] = ACTIONS(1),
    [anon_sym_TILDE] = ACTIONS(1),
    [anon_sym_AMP] = ACTIONS(1),
    [anon_sym_BANG] = ACTIONS(1),
    [anon_sym_QMARK] = ACTIONS(1),
    [anon_sym_STAR] = ACTIONS(1),
    [anon_sym_PLUS] = ACTIONS(1),
    [anon_sym_COMMA] = ACTIONS(1),
    [sym_line_comment] = ACTIONS(3),
    [anon_sym_SLASH_STAR] = ACTIONS(5),
    [anon_sym_SLASH] = ACTIONS(1),
  },
  [1] = {
    [sym_pest] = STATE(84),
    [sym_grammar_rule] = STATE(58),
    [sym_block_comment] = STATE(1),
    [aux_sym_pest_repeat1] = STATE(49),
    [ts_builtin_sym_end] = ACTIONS(7),
    [sym_identifier] = ACTIONS(9),
    [sym_line_comment] = ACTIONS(3),
    [anon_sym_SLASH_STAR] = ACTIONS(5),
  },
  [2] = {
    [sym_expression] = STATE(78),
    [sym_term] = STATE(43),
    [sym__node] = STATE(22),
    [sym__terminal] = STATE(29),
    [sym_range] = STATE(32),
    [sym_string] = STATE(32),
    [sym__insensitive_string] = STATE(32),
    [sym__push] = STATE(32),
    [sym__peek_slice] = STATE(32),
    [sym__pop] = STATE(32),
    [sym_builtin] = STATE(32),
    [sym_node_tag] = STATE(9),
    [sym_prefix_operator] = STATE(14),
    [sym_block_comment] = STATE(2),
    [aux_sym_term_repeat1] = STATE(8),
    [sym_identifier] = ACTIONS(11),
    [anon_sym_PIPE] = ACTIONS(13),
    [anon_sym_LPAREN] = ACTIONS(15),
    [sym_character] = ACTIONS(17),
    [anon_sym_DQUOTE] = ACTIONS(19),
    [anon_sym_CARET] = ACTIONS(21),
    [anon_sym_PUSH] = ACTIONS(23),
    [anon_sym_PEEK] = ACTIONS(25),
    [anon_sym_POP] = ACTIONS(27),
    [anon_sym_ANY] = ACTIONS(29),
    [anon_sym_SOI] = ACTIONS(29),
    [anon_sym_EOI] = ACTIONS(29),
    [anon_sym_NEWLINE] = ACTIONS(29),
    [sym_const] = ACTIONS(11),
    [anon_sym_POUND] = ACTIONS(31),
    [anon_sym_AMP] = ACTIONS(33),
    [anon_sym_BANG] = ACTIONS(33),
    [sym_line_comment] = ACTIONS(3),
    [anon_sym_SLASH_STAR] = ACTIONS(5),
  },
  [3] = {
    [sym_expression] = STATE(65),
    [sym_term] = STATE(43),
    [sym__node] = STATE(22),
    [sym__terminal] = STATE(29),
    [sym_range] = STATE(32),
    [sym_string] = STATE(32),
    [sym__insensitive_string] = STATE(32),
    [sym__push] = STATE(32),
    [sym__peek_slice] = STATE(32),
    [sym__pop] = STATE(32),
    [sym_builtin] = STATE(32),
    [sym_node_tag] = STATE(9),
    [sym_prefix_operator] = STATE(14),
    [sym_block_comment] = STATE(3),
    [aux_sym_term_repeat1] = STATE(8),
    [sym_identifier] = ACTIONS(11),
    [anon_sym_PIPE] = ACTIONS(13),
    [anon_sym_LPAREN] = ACTIONS(15),
    [sym_character] = ACTIONS(17),
    [anon_sym_DQUOTE] = ACTIONS(19),
    [anon_sym_CARET] = ACTIONS(21),
    [anon_sym_PUSH] = ACTIONS(23),
    [anon_sym_PEEK] = ACTIONS(25),
    [anon_sym_POP] = ACTIONS(27),
    [anon_sym_ANY] = ACTIONS(29),
    [anon_sym_SOI] = ACTIONS(29),
    [anon_sym_EOI] = ACTIONS(29),
    [anon_sym_NEWLINE] = ACTIONS(29),
    [sym_const] = ACTIONS(11),
    [anon_sym_POUND] = ACTIONS(31),
    [anon_sym_AMP] = ACTIONS(33),
    [anon_sym_BANG] = ACTIONS(33),
    [sym_line_comment] = ACTIONS(3),
    [anon_sym_SLASH_STAR] = ACTIONS(5),
  },
  [4] = {
    [sym_expression] = STATE(69),
    [sym_term] = STATE(43),
    [sym__node] = STATE(22),
    [sym__terminal] = STATE(29),
    [sym_range] = STATE(32),
    [sym_string] = STATE(32),
    [sym__insensitive_string] = STATE(32),
    [sym__push] = STATE(32),
    [sym__peek_slice] = STATE(32),
    [sym__pop] = STATE(32),
    [sym_builtin] = STATE(32),
    [sym_node_tag] = STATE(9),
    [sym_prefix_operator] = STATE(14),
    [sym_block_comment] = STATE(4),
    [aux_sym_term_repeat1] = STATE(8),
    [sym_identifier] = ACTIONS(11),
    [anon_sym_PIPE] = ACTIONS(13),
    [anon_sym_LPAREN] = ACTIONS(15),
    [sym_character] = ACTIONS(17),
    [anon_sym_DQUOTE] = ACTIONS(19),
    [anon_sym_CARET] = ACTIONS(21),
    [anon_sym_PUSH] = ACTIONS(23),
    [anon_sym_PEEK] = ACTIONS(25),
    [anon_sym_POP] = ACTIONS(27),
    [anon_sym_ANY] = ACTIONS(29),
    [anon_sym_SOI] = ACTIONS(29),
    [anon_sym_EOI] = ACTIONS(29),
    [anon_sym_NEWLINE] = ACTIONS(29),
    [sym_const] = ACTIONS(11),
    [anon_sym_POUND] = ACTIONS(31),
    [anon_sym_AMP] = ACTIONS(33),
    [anon_sym_BANG] = ACTIONS(33),
    [sym_line_comment] = ACTIONS(3),
    [anon_sym_SLASH_STAR] = ACTIONS(5),
  },
  [5] = {
    [sym_expression] = STATE(75),
    [sym_term] = STATE(43),
    [sym__node] = STATE(22),
    [sym__terminal] = STATE(29),
    [sym_range] = STATE(32),
    [sym_string] = STATE(32),
    [sym__insensitive_string] = STATE(32),
    [sym__push] = STATE(32),
    [sym__peek_slice] = STATE(32),
    [sym__pop] = STATE(32),
    [sym_builtin] = STATE(32),
    [sym_node_tag] = STATE(9),
    [sym_prefix_operator] = STATE(14),
    [sym_block_comment] = STATE(5),
    [aux_sym_term_repeat1] = STATE(8),
    [sym_identifier] = ACTIONS(11),
    [anon_sym_PIPE] = ACTIONS(13),
    [anon_sym_LPAREN] = ACTIONS(15),
    [sym_character] = ACTIONS(17),
    [anon_sym_DQUOTE] = ACTIONS(19),
    [anon_sym_CARET] = ACTIONS(21),
    [anon_sym_PUSH] = ACTIONS(23),
    [anon_sym_PEEK] = ACTIONS(25),
    [anon_sym_POP] = ACTIONS(27),
    [anon_sym_ANY] = ACTIONS(29),
    [anon_sym_SOI] = ACTIONS(29),
    [anon_sym_EOI] = ACTIONS(29),
    [anon_sym_NEWLINE] = ACTIONS(29),
    [sym_const] = ACTIONS(11),
    [anon_sym_POUND] = ACTIONS(31),
    [anon_sym_AMP] = ACTIONS(33),
    [anon_sym_BANG] = ACTIONS(33),
    [sym_line_comment] = ACTIONS(3),
    [anon_sym_SLASH_STAR] = ACTIONS(5),
  },
};

static const uint16_t ts_small_parse_table[] = {
  [0] = 21,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(15), 1,
      anon_sym_LPAREN,
    ACTIONS(17), 1,
      sym_character,
    ACTIONS(19), 1,
      anon_sym_DQUOTE,
    ACTIONS(21), 1,
      anon_sym_CARET,
    ACTIONS(23), 1,
      anon_sym_PUSH,
    ACTIONS(25), 1,
      anon_sym_PEEK,
    ACTIONS(27), 1,
      anon_sym_POP,
    ACTIONS(31), 1,
      anon_sym_POUND,
    STATE(6), 1,
      sym_block_comment,
    STATE(8), 1,
      aux_sym_term_repeat1,
    STATE(9), 1,
      sym_node_tag,
    STATE(14), 1,
      sym_prefix_operator,
    STATE(22), 1,
      sym__node,
    STATE(29), 1,
      sym__terminal,
    STATE(41), 1,
      sym_term,
    ACTIONS(11), 2,
      sym_identifier,
      sym_const,
    ACTIONS(33), 2,
      anon_sym_AMP,
      anon_sym_BANG,
    ACTIONS(29), 4,
      anon_sym_ANY,
      anon_sym_SOI,
      anon_sym_EOI,
      anon_sym_NEWLINE,
    STATE(32), 7,
      sym_range,
      sym_string,
      sym__insensitive_string,
      sym__push,
      sym__peek_slice,
      sym__pop,
      sym_builtin,
  [75] = 21,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(15), 1,
      anon_sym_LPAREN,
    ACTIONS(17), 1,
      sym_character,
    ACTIONS(19), 1,
      anon_sym_DQUOTE,
    ACTIONS(21), 1,
      anon_sym_CARET,
    ACTIONS(23), 1,
      anon_sym_PUSH,
    ACTIONS(25), 1,
      anon_sym_PEEK,
    ACTIONS(27), 1,
      anon_sym_POP,
    ACTIONS(31), 1,
      anon_sym_POUND,
    STATE(7), 1,
      sym_block_comment,
    STATE(8), 1,
      aux_sym_term_repeat1,
    STATE(9), 1,
      sym_node_tag,
    STATE(14), 1,
      sym_prefix_operator,
    STATE(22), 1,
      sym__node,
    STATE(29), 1,
      sym__terminal,
    STATE(50), 1,
      sym_term,
    ACTIONS(11), 2,
      sym_identifier,
      sym_const,
    ACTIONS(33), 2,
      anon_sym_AMP,
      anon_sym_BANG,
    ACTIONS(29), 4,
      anon_sym_ANY,
      anon_sym_SOI,
      anon_sym_EOI,
      anon_sym_NEWLINE,
    STATE(32), 7,
      sym_range,
      sym_string,
      sym__insensitive_string,
      sym__push,
      sym__peek_slice,
      sym__pop,
      sym_builtin,
  [150] = 18,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(15), 1,
      anon_sym_LPAREN,
    ACTIONS(17), 1,
      sym_character,
    ACTIONS(19), 1,
      anon_sym_DQUOTE,
    ACTIONS(21), 1,
      anon_sym_CARET,
    ACTIONS(23), 1,
      anon_sym_PUSH,
    ACTIONS(25), 1,
      anon_sym_PEEK,
    ACTIONS(27), 1,
      anon_sym_POP,
    STATE(8), 1,
      sym_block_comment,
    STATE(11), 1,
      aux_sym_term_repeat1,
    STATE(14), 1,
      sym_prefix_operator,
    STATE(18), 1,
      sym__node,
    STATE(29), 1,
      sym__terminal,
    ACTIONS(11), 2,
      sym_identifier,
      sym_const,
    ACTIONS(33), 2,
      anon_sym_AMP,
      anon_sym_BANG,
    ACTIONS(29), 4,
      anon_sym_ANY,
      anon_sym_SOI,
      anon_sym_EOI,
      anon_sym_NEWLINE,
    STATE(32), 7,
      sym_range,
      sym_string,
      sym__insensitive_string,
      sym__push,
      sym__peek_slice,
      sym__pop,
      sym_builtin,
  [216] = 18,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(15), 1,
      anon_sym_LPAREN,
    ACTIONS(17), 1,
      sym_character,
    ACTIONS(19), 1,
      anon_sym_DQUOTE,
    ACTIONS(21), 1,
      anon_sym_CARET,
    ACTIONS(23), 1,
      anon_sym_PUSH,
    ACTIONS(25), 1,
      anon_sym_PEEK,
    ACTIONS(27), 1,
      anon_sym_POP,
    STATE(9), 1,
      sym_block_comment,
    STATE(10), 1,
      aux_sym_term_repeat1,
    STATE(14), 1,
      sym_prefix_operator,
    STATE(18), 1,
      sym__node,
    STATE(29), 1,
      sym__terminal,
    ACTIONS(11), 2,
      sym_identifier,
      sym_const,
    ACTIONS(33), 2,
      anon_sym_AMP,
      anon_sym_BANG,
    ACTIONS(29), 4,
      anon_sym_ANY,
      anon_sym_SOI,
      anon_sym_EOI,
      anon_sym_NEWLINE,
    STATE(32), 7,
      sym_range,
      sym_string,
      sym__insensitive_string,
      sym__push,
      sym__peek_slice,
      sym__pop,
      sym_builtin,
  [282] = 18,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(15), 1,
      anon_sym_LPAREN,
    ACTIONS(17), 1,
      sym_character,
    ACTIONS(19), 1,
      anon_sym_DQUOTE,
    ACTIONS(21), 1,
      anon_sym_CARET,
    ACTIONS(23), 1,
      anon_sym_PUSH,
    ACTIONS(25), 1,
      anon_sym_PEEK,
    ACTIONS(27), 1,
      anon_sym_POP,
    STATE(10), 1,
      sym_block_comment,
    STATE(11), 1,
      aux_sym_term_repeat1,
    STATE(14), 1,
      sym_prefix_operator,
    STATE(19), 1,
      sym__node,
    STATE(29), 1,
      sym__terminal,
    ACTIONS(11), 2,
      sym_identifier,
      sym_const,
    ACTIONS(33), 2,
      anon_sym_AMP,
      anon_sym_BANG,
    ACTIONS(29), 4,
      anon_sym_ANY,
      anon_sym_SOI,
      anon_sym_EOI,
      anon_sym_NEWLINE,
    STATE(32), 7,
      sym_range,
      sym_string,
      sym__insensitive_string,
      sym__push,
      sym__peek_slice,
      sym__pop,
      sym_builtin,
  [348] = 7,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(14), 1,
      sym_prefix_operator,
    ACTIONS(39), 2,
      anon_sym_AMP,
      anon_sym_BANG,
    STATE(11), 2,
      sym_block_comment,
      aux_sym_term_repeat1,
    ACTIONS(37), 4,
      anon_sym_LPAREN,
      sym_character,
      anon_sym_DQUOTE,
      anon_sym_CARET,
    ACTIONS(35), 9,
      sym_identifier,
      anon_sym_PUSH,
      anon_sym_PEEK,
      anon_sym_POP,
      anon_sym_ANY,
      anon_sym_SOI,
      anon_sym_EOI,
      anon_sym_NEWLINE,
      sym_const,
  [383] = 5,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(12), 1,
      sym_block_comment,
    ACTIONS(44), 7,
      anon_sym_LPAREN,
      sym_character,
      anon_sym_DQUOTE,
      anon_sym_CARET,
      anon_sym_POUND,
      anon_sym_AMP,
      anon_sym_BANG,
    ACTIONS(42), 9,
      sym_identifier,
      anon_sym_PUSH,
      anon_sym_PEEK,
      anon_sym_POP,
      anon_sym_ANY,
      anon_sym_SOI,
      anon_sym_EOI,
      anon_sym_NEWLINE,
      sym_const,
  [413] = 5,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(13), 1,
      sym_block_comment,
    ACTIONS(48), 6,
      anon_sym_LPAREN,
      sym_character,
      anon_sym_DQUOTE,
      anon_sym_CARET,
      anon_sym_AMP,
      anon_sym_BANG,
    ACTIONS(46), 9,
      sym_identifier,
      anon_sym_PUSH,
      anon_sym_PEEK,
      anon_sym_POP,
      anon_sym_ANY,
      anon_sym_SOI,
      anon_sym_EOI,
      anon_sym_NEWLINE,
      sym_const,
  [442] = 5,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(14), 1,
      sym_block_comment,
    ACTIONS(52), 6,
      anon_sym_LPAREN,
      sym_character,
      anon_sym_DQUOTE,
      anon_sym_CARET,
      anon_sym_AMP,
      anon_sym_BANG,
    ACTIONS(50), 9,
      sym_identifier,
      anon_sym_PUSH,
      anon_sym_PEEK,
      anon_sym_POP,
      anon_sym_ANY,
      anon_sym_SOI,
      anon_sym_EOI,
      anon_sym_NEWLINE,
      sym_const,
  [471] = 5,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(15), 1,
      sym_block_comment,
    ACTIONS(56), 6,
      anon_sym_LPAREN,
      sym_character,
      anon_sym_DQUOTE,
      anon_sym_CARET,
      anon_sym_AMP,
      anon_sym_BANG,
    ACTIONS(54), 9,
      sym_identifier,
      anon_sym_PUSH,
      anon_sym_PEEK,
      anon_sym_POP,
      anon_sym_ANY,
      anon_sym_SOI,
      anon_sym_EOI,
      anon_sym_NEWLINE,
      sym_const,
  [500] = 8,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(60), 1,
      anon_sym_LBRACK,
    STATE(16), 1,
      sym_block_comment,
    STATE(21), 1,
      aux_sym_term_repeat2,
    STATE(39), 1,
      sym_postfix_operator,
    ACTIONS(62), 3,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
    ACTIONS(58), 4,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_TILDE,
  [530] = 8,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(60), 1,
      anon_sym_LBRACK,
    STATE(17), 1,
      sym_block_comment,
    STATE(21), 1,
      aux_sym_term_repeat2,
    STATE(39), 1,
      sym_postfix_operator,
    ACTIONS(62), 3,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
    ACTIONS(64), 4,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_TILDE,
  [560] = 8,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(60), 1,
      anon_sym_LBRACK,
    STATE(18), 1,
      sym_block_comment,
    STATE(20), 1,
      aux_sym_term_repeat2,
    STATE(39), 1,
      sym_postfix_operator,
    ACTIONS(62), 3,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
    ACTIONS(58), 4,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_TILDE,
  [590] = 8,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(60), 1,
      anon_sym_LBRACK,
    STATE(17), 1,
      aux_sym_term_repeat2,
    STATE(19), 1,
      sym_block_comment,
    STATE(39), 1,
      sym_postfix_operator,
    ACTIONS(62), 3,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
    ACTIONS(66), 4,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_TILDE,
  [620] = 8,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(60), 1,
      anon_sym_LBRACK,
    STATE(20), 1,
      sym_block_comment,
    STATE(21), 1,
      aux_sym_term_repeat2,
    STATE(39), 1,
      sym_postfix_operator,
    ACTIONS(62), 3,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
    ACTIONS(66), 4,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_TILDE,
  [650] = 7,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(70), 1,
      anon_sym_LBRACK,
    STATE(39), 1,
      sym_postfix_operator,
    STATE(21), 2,
      sym_block_comment,
      aux_sym_term_repeat2,
    ACTIONS(73), 3,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
    ACTIONS(68), 4,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_TILDE,
  [678] = 8,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(60), 1,
      anon_sym_LBRACK,
    STATE(16), 1,
      aux_sym_term_repeat2,
    STATE(22), 1,
      sym_block_comment,
    STATE(39), 1,
      sym_postfix_operator,
    ACTIONS(62), 3,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
    ACTIONS(76), 4,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_TILDE,
  [708] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(23), 1,
      sym_block_comment,
    ACTIONS(78), 8,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_LBRACK,
      anon_sym_TILDE,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
  [728] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(24), 1,
      sym_block_comment,
    ACTIONS(80), 8,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_LBRACK,
      anon_sym_TILDE,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
  [748] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(25), 1,
      sym_block_comment,
    ACTIONS(82), 8,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_LBRACK,
      anon_sym_TILDE,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
  [768] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(26), 1,
      sym_block_comment,
    ACTIONS(84), 8,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_LBRACK,
      anon_sym_TILDE,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
  [788] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(27), 1,
      sym_block_comment,
    ACTIONS(86), 8,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_LBRACK,
      anon_sym_TILDE,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
  [808] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(28), 1,
      sym_block_comment,
    ACTIONS(88), 8,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_LBRACK,
      anon_sym_TILDE,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
  [828] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(29), 1,
      sym_block_comment,
    ACTIONS(90), 8,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_LBRACK,
      anon_sym_TILDE,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
  [848] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(30), 1,
      sym_block_comment,
    ACTIONS(92), 8,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_LBRACK,
      anon_sym_TILDE,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
  [868] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(31), 1,
      sym_block_comment,
    ACTIONS(94), 8,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_LBRACK,
      anon_sym_TILDE,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
  [888] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(32), 1,
      sym_block_comment,
    ACTIONS(96), 8,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_LBRACK,
      anon_sym_TILDE,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
  [908] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(33), 1,
      sym_block_comment,
    ACTIONS(98), 8,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_LBRACK,
      anon_sym_TILDE,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
  [928] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(34), 1,
      sym_block_comment,
    ACTIONS(100), 8,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_LBRACK,
      anon_sym_TILDE,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
  [948] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(35), 1,
      sym_block_comment,
    ACTIONS(102), 8,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_LBRACK,
      anon_sym_TILDE,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
  [968] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(36), 1,
      sym_block_comment,
    ACTIONS(104), 8,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_LBRACK,
      anon_sym_TILDE,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
  [988] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(37), 1,
      sym_block_comment,
    ACTIONS(106), 8,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_LBRACK,
      anon_sym_TILDE,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
  [1008] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(38), 1,
      sym_block_comment,
    ACTIONS(108), 8,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_LBRACK,
      anon_sym_TILDE,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
  [1028] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(39), 1,
      sym_block_comment,
    ACTIONS(110), 8,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_LBRACK,
      anon_sym_TILDE,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
  [1048] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(40), 1,
      sym_block_comment,
    ACTIONS(112), 8,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_LBRACK,
      anon_sym_TILDE,
      anon_sym_QMARK,
      anon_sym_STAR,
      anon_sym_PLUS,
  [1068] = 7,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(7), 1,
      sym_infix_operator,
    STATE(41), 1,
      sym_block_comment,
    STATE(45), 1,
      aux_sym_expression_repeat1,
    ACTIONS(114), 2,
      anon_sym_RBRACE,
      anon_sym_RPAREN,
    ACTIONS(116), 2,
      anon_sym_PIPE,
      anon_sym_TILDE,
  [1092] = 6,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(7), 1,
      sym_infix_operator,
    ACTIONS(118), 2,
      anon_sym_RBRACE,
      anon_sym_RPAREN,
    ACTIONS(120), 2,
      anon_sym_PIPE,
      anon_sym_TILDE,
    STATE(42), 2,
      sym_block_comment,
      aux_sym_expression_repeat1,
  [1114] = 7,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(7), 1,
      sym_infix_operator,
    STATE(43), 1,
      sym_block_comment,
    STATE(44), 1,
      aux_sym_expression_repeat1,
    ACTIONS(116), 2,
      anon_sym_PIPE,
      anon_sym_TILDE,
    ACTIONS(123), 2,
      anon_sym_RBRACE,
      anon_sym_RPAREN,
  [1138] = 7,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(7), 1,
      sym_infix_operator,
    STATE(42), 1,
      aux_sym_expression_repeat1,
    STATE(44), 1,
      sym_block_comment,
    ACTIONS(114), 2,
      anon_sym_RBRACE,
      anon_sym_RPAREN,
    ACTIONS(116), 2,
      anon_sym_PIPE,
      anon_sym_TILDE,
  [1162] = 7,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(7), 1,
      sym_infix_operator,
    STATE(42), 1,
      aux_sym_expression_repeat1,
    STATE(45), 1,
      sym_block_comment,
    ACTIONS(116), 2,
      anon_sym_PIPE,
      anon_sym_TILDE,
    ACTIONS(125), 2,
      anon_sym_RBRACE,
      anon_sym_RPAREN,
  [1186] = 6,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(127), 1,
      anon_sym_LBRACE,
    STATE(46), 1,
      sym_block_comment,
    STATE(74), 1,
      sym_modifier,
    ACTIONS(129), 3,
      anon_sym__,
      anon_sym_AT,
      anon_sym_DOLLAR,
  [1207] = 6,
    ACTIONS(131), 1,
      anon_sym_DQUOTE,
    ACTIONS(135), 1,
      sym_line_comment,
    ACTIONS(137), 1,
      anon_sym_SLASH_STAR,
    STATE(47), 1,
      sym_block_comment,
    STATE(51), 1,
      aux_sym_string_repeat1,
    ACTIONS(133), 2,
      sym__unescaped_double_string_fragment,
      sym__escape_sequence,
  [1227] = 6,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(139), 1,
      ts_builtin_sym_end,
    ACTIONS(141), 1,
      sym_identifier,
    STATE(58), 1,
      sym_grammar_rule,
    STATE(48), 2,
      sym_block_comment,
      aux_sym_pest_repeat1,
  [1247] = 7,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(9), 1,
      sym_identifier,
    ACTIONS(144), 1,
      ts_builtin_sym_end,
    STATE(48), 1,
      aux_sym_pest_repeat1,
    STATE(49), 1,
      sym_block_comment,
    STATE(58), 1,
      sym_grammar_rule,
  [1269] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(50), 1,
      sym_block_comment,
    ACTIONS(118), 4,
      anon_sym_RBRACE,
      anon_sym_PIPE,
      anon_sym_RPAREN,
      anon_sym_TILDE,
  [1285] = 6,
    ACTIONS(135), 1,
      sym_line_comment,
    ACTIONS(137), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(146), 1,
      anon_sym_DQUOTE,
    STATE(51), 1,
      sym_block_comment,
    STATE(52), 1,
      aux_sym_string_repeat1,
    ACTIONS(133), 2,
      sym__unescaped_double_string_fragment,
      sym__escape_sequence,
  [1305] = 5,
    ACTIONS(135), 1,
      sym_line_comment,
    ACTIONS(137), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(148), 1,
      anon_sym_DQUOTE,
    ACTIONS(150), 2,
      sym__unescaped_double_string_fragment,
      sym__escape_sequence,
    STATE(52), 2,
      sym_block_comment,
      aux_sym_string_repeat1,
  [1323] = 5,
    ACTIONS(135), 1,
      sym_line_comment,
    ACTIONS(137), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(153), 1,
      anon_sym_DQUOTE,
    STATE(53), 1,
      sym_block_comment,
    ACTIONS(155), 2,
      sym__unescaped_double_string_fragment,
      sym__escape_sequence,
  [1340] = 5,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(157), 1,
      aux_sym__peek_slice_token1,
    ACTIONS(159), 1,
      anon_sym_RBRACK,
    STATE(54), 1,
      sym_block_comment,
  [1356] = 5,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(161), 1,
      anon_sym_RBRACK,
    ACTIONS(163), 1,
      anon_sym_COMMA,
    STATE(55), 1,
      sym_block_comment,
  [1372] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(56), 1,
      sym_block_comment,
    ACTIONS(165), 2,
      ts_builtin_sym_end,
      sym_identifier,
  [1386] = 5,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(167), 1,
      aux_sym__peek_slice_token1,
    ACTIONS(169), 1,
      anon_sym_COMMA,
    STATE(57), 1,
      sym_block_comment,
  [1402] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(58), 1,
      sym_block_comment,
    ACTIONS(171), 2,
      ts_builtin_sym_end,
      sym_identifier,
  [1416] = 5,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(173), 1,
      anon_sym_DOT_DOT,
    ACTIONS(175), 1,
      aux_sym__peek_slice_token1,
    STATE(59), 1,
      sym_block_comment,
  [1432] = 5,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(177), 1,
      anon_sym_RBRACK,
    ACTIONS(179), 1,
      anon_sym_COMMA,
    STATE(60), 1,
      sym_block_comment,
  [1448] = 5,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(181), 1,
      aux_sym__peek_slice_token1,
    ACTIONS(183), 1,
      anon_sym_RBRACK,
    STATE(61), 1,
      sym_block_comment,
  [1464] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    STATE(62), 1,
      sym_block_comment,
    ACTIONS(185), 2,
      ts_builtin_sym_end,
      sym_identifier,
  [1478] = 5,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(19), 1,
      anon_sym_DQUOTE,
    STATE(40), 1,
      sym_string,
    STATE(63), 1,
      sym_block_comment,
  [1494] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(187), 1,
      anon_sym_LPAREN,
    STATE(64), 1,
      sym_block_comment,
  [1507] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(189), 1,
      anon_sym_RPAREN,
    STATE(65), 1,
      sym_block_comment,
  [1520] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(191), 1,
      sym_identifier,
    STATE(66), 1,
      sym_block_comment,
  [1533] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(193), 1,
      aux_sym__peek_slice_token1,
    STATE(67), 1,
      sym_block_comment,
  [1546] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(195), 1,
      anon_sym_LBRACK,
    STATE(68), 1,
      sym_block_comment,
  [1559] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(197), 1,
      anon_sym_RBRACE,
    STATE(69), 1,
      sym_block_comment,
  [1572] = 4,
    ACTIONS(135), 1,
      sym_line_comment,
    ACTIONS(137), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(199), 1,
      aux_sym_block_comment_token1,
    STATE(70), 1,
      sym_block_comment,
  [1585] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(201), 1,
      anon_sym_DOT_DOT,
    STATE(71), 1,
      sym_block_comment,
  [1598] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(203), 1,
      anon_sym_DOT_DOT,
    STATE(72), 1,
      sym_block_comment,
  [1611] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(183), 1,
      anon_sym_RBRACK,
    STATE(73), 1,
      sym_block_comment,
  [1624] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(205), 1,
      anon_sym_LBRACE,
    STATE(74), 1,
      sym_block_comment,
  [1637] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(207), 1,
      anon_sym_RBRACE,
    STATE(75), 1,
      sym_block_comment,
  [1650] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(209), 1,
      anon_sym_LBRACE,
    STATE(76), 1,
      sym_block_comment,
  [1663] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(177), 1,
      anon_sym_RBRACK,
    STATE(77), 1,
      sym_block_comment,
  [1676] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(211), 1,
      anon_sym_RPAREN,
    STATE(78), 1,
      sym_block_comment,
  [1689] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(213), 1,
      anon_sym_EQ,
    STATE(79), 1,
      sym_block_comment,
  [1702] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(215), 1,
      anon_sym_SLASH,
    STATE(80), 1,
      sym_block_comment,
  [1715] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(217), 1,
      anon_sym_RBRACK,
    STATE(81), 1,
      sym_block_comment,
  [1728] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(219), 1,
      sym_character,
    STATE(82), 1,
      sym_block_comment,
  [1741] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(221), 1,
      anon_sym_RBRACK,
    STATE(83), 1,
      sym_block_comment,
  [1754] = 4,
    ACTIONS(3), 1,
      sym_line_comment,
    ACTIONS(5), 1,
      anon_sym_SLASH_STAR,
    ACTIONS(223), 1,
      ts_builtin_sym_end,
    STATE(84), 1,
      sym_block_comment,
  [1767] = 1,
    ACTIONS(225), 1,
      ts_builtin_sym_end,
};

static const uint32_t ts_small_parse_table_map[] = {
  [SMALL_STATE(6)] = 0,
  [SMALL_STATE(7)] = 75,
  [SMALL_STATE(8)] = 150,
  [SMALL_STATE(9)] = 216,
  [SMALL_STATE(10)] = 282,
  [SMALL_STATE(11)] = 348,
  [SMALL_STATE(12)] = 383,
  [SMALL_STATE(13)] = 413,
  [SMALL_STATE(14)] = 442,
  [SMALL_STATE(15)] = 471,
  [SMALL_STATE(16)] = 500,
  [SMALL_STATE(17)] = 530,
  [SMALL_STATE(18)] = 560,
  [SMALL_STATE(19)] = 590,
  [SMALL_STATE(20)] = 620,
  [SMALL_STATE(21)] = 650,
  [SMALL_STATE(22)] = 678,
  [SMALL_STATE(23)] = 708,
  [SMALL_STATE(24)] = 728,
  [SMALL_STATE(25)] = 748,
  [SMALL_STATE(26)] = 768,
  [SMALL_STATE(27)] = 788,
  [SMALL_STATE(28)] = 808,
  [SMALL_STATE(29)] = 828,
  [SMALL_STATE(30)] = 848,
  [SMALL_STATE(31)] = 868,
  [SMALL_STATE(32)] = 888,
  [SMALL_STATE(33)] = 908,
  [SMALL_STATE(34)] = 928,
  [SMALL_STATE(35)] = 948,
  [SMALL_STATE(36)] = 968,
  [SMALL_STATE(37)] = 988,
  [SMALL_STATE(38)] = 1008,
  [SMALL_STATE(39)] = 1028,
  [SMALL_STATE(40)] = 1048,
  [SMALL_STATE(41)] = 1068,
  [SMALL_STATE(42)] = 1092,
  [SMALL_STATE(43)] = 1114,
  [SMALL_STATE(44)] = 1138,
  [SMALL_STATE(45)] = 1162,
  [SMALL_STATE(46)] = 1186,
  [SMALL_STATE(47)] = 1207,
  [SMALL_STATE(48)] = 1227,
  [SMALL_STATE(49)] = 1247,
  [SMALL_STATE(50)] = 1269,
  [SMALL_STATE(51)] = 1285,
  [SMALL_STATE(52)] = 1305,
  [SMALL_STATE(53)] = 1323,
  [SMALL_STATE(54)] = 1340,
  [SMALL_STATE(55)] = 1356,
  [SMALL_STATE(56)] = 1372,
  [SMALL_STATE(57)] = 1386,
  [SMALL_STATE(58)] = 1402,
  [SMALL_STATE(59)] = 1416,
  [SMALL_STATE(60)] = 1432,
  [SMALL_STATE(61)] = 1448,
  [SMALL_STATE(62)] = 1464,
  [SMALL_STATE(63)] = 1478,
  [SMALL_STATE(64)] = 1494,
  [SMALL_STATE(65)] = 1507,
  [SMALL_STATE(66)] = 1520,
  [SMALL_STATE(67)] = 1533,
  [SMALL_STATE(68)] = 1546,
  [SMALL_STATE(69)] = 1559,
  [SMALL_STATE(70)] = 1572,
  [SMALL_STATE(71)] = 1585,
  [SMALL_STATE(72)] = 1598,
  [SMALL_STATE(73)] = 1611,
  [SMALL_STATE(74)] = 1624,
  [SMALL_STATE(75)] = 1637,
  [SMALL_STATE(76)] = 1650,
  [SMALL_STATE(77)] = 1663,
  [SMALL_STATE(78)] = 1676,
  [SMALL_STATE(79)] = 1689,
  [SMALL_STATE(80)] = 1702,
  [SMALL_STATE(81)] = 1715,
  [SMALL_STATE(82)] = 1728,
  [SMALL_STATE(83)] = 1741,
  [SMALL_STATE(84)] = 1754,
  [SMALL_STATE(85)] = 1767,
};

static const TSParseActionEntry ts_parse_actions[] = {
  [0] = {.entry = {.count = 0, .reusable = false}},
  [1] = {.entry = {.count = 1, .reusable = false}}, RECOVER(),
  [3] = {.entry = {.count = 1, .reusable = true}}, SHIFT_EXTRA(),
  [5] = {.entry = {.count = 1, .reusable = true}}, SHIFT(70),
  [7] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_pest, 0),
  [9] = {.entry = {.count = 1, .reusable = true}}, SHIFT(79),
  [11] = {.entry = {.count = 1, .reusable = false}}, SHIFT(32),
  [13] = {.entry = {.count = 1, .reusable = true}}, SHIFT(6),
  [15] = {.entry = {.count = 1, .reusable = true}}, SHIFT(3),
  [17] = {.entry = {.count = 1, .reusable = true}}, SHIFT(72),
  [19] = {.entry = {.count = 1, .reusable = true}}, SHIFT(47),
  [21] = {.entry = {.count = 1, .reusable = true}}, SHIFT(63),
  [23] = {.entry = {.count = 1, .reusable = false}}, SHIFT(64),
  [25] = {.entry = {.count = 1, .reusable = false}}, SHIFT(68),
  [27] = {.entry = {.count = 1, .reusable = false}}, SHIFT(31),
  [29] = {.entry = {.count = 1, .reusable = false}}, SHIFT(25),
  [31] = {.entry = {.count = 1, .reusable = true}}, SHIFT(66),
  [33] = {.entry = {.count = 1, .reusable = true}}, SHIFT(15),
  [35] = {.entry = {.count = 1, .reusable = false}}, REDUCE(aux_sym_term_repeat1, 2),
  [37] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_term_repeat1, 2),
  [39] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_term_repeat1, 2), SHIFT_REPEAT(15),
  [42] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_infix_operator, 1),
  [44] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_infix_operator, 1),
  [46] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_node_tag, 2),
  [48] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_node_tag, 2),
  [50] = {.entry = {.count = 1, .reusable = false}}, REDUCE(aux_sym_term_repeat1, 1),
  [52] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_term_repeat1, 1),
  [54] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_prefix_operator, 1),
  [56] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_prefix_operator, 1),
  [58] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_term, 2),
  [60] = {.entry = {.count = 1, .reusable = true}}, SHIFT(57),
  [62] = {.entry = {.count = 1, .reusable = true}}, SHIFT(26),
  [64] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_term, 4),
  [66] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_term, 3),
  [68] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_term_repeat2, 2),
  [70] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_term_repeat2, 2), SHIFT_REPEAT(57),
  [73] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_term_repeat2, 2), SHIFT_REPEAT(26),
  [76] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_term, 1),
  [78] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym__push, 4),
  [80] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym__peek_slice, 5),
  [82] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_builtin, 1),
  [84] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_postfix_operator, 1),
  [86] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_postfix_operator, 3),
  [88] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_postfix_operator, 5),
  [90] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym__node, 1),
  [92] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_range, 3),
  [94] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym__pop, 1),
  [96] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym__terminal, 1),
  [98] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym__node, 3),
  [100] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym__peek_slice, 4),
  [102] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym__peek_slice, 6),
  [104] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_string, 3),
  [106] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_string, 2),
  [108] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_postfix_operator, 4),
  [110] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_term_repeat2, 1),
  [112] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym__insensitive_string, 2),
  [114] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_expression, 2),
  [116] = {.entry = {.count = 1, .reusable = true}}, SHIFT(12),
  [118] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_expression_repeat1, 2),
  [120] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_expression_repeat1, 2), SHIFT_REPEAT(12),
  [123] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_expression, 1),
  [125] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_expression, 3),
  [127] = {.entry = {.count = 1, .reusable = true}}, SHIFT(5),
  [129] = {.entry = {.count = 1, .reusable = true}}, SHIFT(76),
  [131] = {.entry = {.count = 1, .reusable = false}}, SHIFT(37),
  [133] = {.entry = {.count = 1, .reusable = true}}, SHIFT(53),
  [135] = {.entry = {.count = 1, .reusable = false}}, SHIFT_EXTRA(),
  [137] = {.entry = {.count = 1, .reusable = false}}, SHIFT(70),
  [139] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_pest_repeat1, 2),
  [141] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_pest_repeat1, 2), SHIFT_REPEAT(79),
  [144] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_pest, 1),
  [146] = {.entry = {.count = 1, .reusable = false}}, SHIFT(36),
  [148] = {.entry = {.count = 1, .reusable = false}}, REDUCE(aux_sym_string_repeat1, 2),
  [150] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_string_repeat1, 2), SHIFT_REPEAT(53),
  [153] = {.entry = {.count = 1, .reusable = false}}, REDUCE(aux_sym_string_repeat1, 1),
  [155] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_string_repeat1, 1),
  [157] = {.entry = {.count = 1, .reusable = true}}, SHIFT(73),
  [159] = {.entry = {.count = 1, .reusable = true}}, SHIFT(34),
  [161] = {.entry = {.count = 1, .reusable = true}}, SHIFT(27),
  [163] = {.entry = {.count = 1, .reusable = true}}, SHIFT(77),
  [165] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_grammar_rule, 5, .production_id = 1),
  [167] = {.entry = {.count = 1, .reusable = true}}, SHIFT(55),
  [169] = {.entry = {.count = 1, .reusable = true}}, SHIFT(67),
  [171] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_pest_repeat1, 1),
  [173] = {.entry = {.count = 1, .reusable = true}}, SHIFT(54),
  [175] = {.entry = {.count = 1, .reusable = true}}, SHIFT(71),
  [177] = {.entry = {.count = 1, .reusable = true}}, SHIFT(38),
  [179] = {.entry = {.count = 1, .reusable = true}}, SHIFT(83),
  [181] = {.entry = {.count = 1, .reusable = true}}, SHIFT(81),
  [183] = {.entry = {.count = 1, .reusable = true}}, SHIFT(24),
  [185] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_grammar_rule, 6, .production_id = 1),
  [187] = {.entry = {.count = 1, .reusable = true}}, SHIFT(2),
  [189] = {.entry = {.count = 1, .reusable = true}}, SHIFT(33),
  [191] = {.entry = {.count = 1, .reusable = true}}, SHIFT(13),
  [193] = {.entry = {.count = 1, .reusable = true}}, SHIFT(60),
  [195] = {.entry = {.count = 1, .reusable = true}}, SHIFT(59),
  [197] = {.entry = {.count = 1, .reusable = true}}, SHIFT(62),
  [199] = {.entry = {.count = 1, .reusable = false}}, SHIFT(80),
  [201] = {.entry = {.count = 1, .reusable = true}}, SHIFT(61),
  [203] = {.entry = {.count = 1, .reusable = true}}, SHIFT(82),
  [205] = {.entry = {.count = 1, .reusable = true}}, SHIFT(4),
  [207] = {.entry = {.count = 1, .reusable = true}}, SHIFT(56),
  [209] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_modifier, 1),
  [211] = {.entry = {.count = 1, .reusable = true}}, SHIFT(23),
  [213] = {.entry = {.count = 1, .reusable = true}}, SHIFT(46),
  [215] = {.entry = {.count = 1, .reusable = false}}, SHIFT(85),
  [217] = {.entry = {.count = 1, .reusable = true}}, SHIFT(35),
  [219] = {.entry = {.count = 1, .reusable = true}}, SHIFT(30),
  [221] = {.entry = {.count = 1, .reusable = true}}, SHIFT(28),
  [223] = {.entry = {.count = 1, .reusable = true}},  ACCEPT_INPUT(),
  [225] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_block_comment, 3),
};

#ifdef __cplusplus
extern "C" {
#endif
#ifdef _WIN32
#define extern __declspec(dllexport)
#endif

extern const TSLanguage *tree_sitter_pest(void) {
  static const TSLanguage language = {
    .version = LANGUAGE_VERSION,
    .symbol_count = SYMBOL_COUNT,
    .alias_count = ALIAS_COUNT,
    .token_count = TOKEN_COUNT,
    .external_token_count = EXTERNAL_TOKEN_COUNT,
    .state_count = STATE_COUNT,
    .large_state_count = LARGE_STATE_COUNT,
    .production_id_count = PRODUCTION_ID_COUNT,
    .field_count = FIELD_COUNT,
    .max_alias_sequence_length = MAX_ALIAS_SEQUENCE_LENGTH,
    .parse_table = &ts_parse_table[0][0],
    .small_parse_table = ts_small_parse_table,
    .small_parse_table_map = ts_small_parse_table_map,
    .parse_actions = ts_parse_actions,
    .symbol_names = ts_symbol_names,
    .field_names = ts_field_names,
    .field_map_slices = ts_field_map_slices,
    .field_map_entries = ts_field_map_entries,
    .symbol_metadata = ts_symbol_metadata,
    .public_symbol_map = ts_symbol_map,
    .alias_map = ts_non_terminal_alias_map,
    .alias_sequences = &ts_alias_sequences[0][0],
    .lex_modes = ts_lex_modes,
    .lex_fn = ts_lex,
    .primary_state_ids = ts_primary_state_ids,
  };
  return &language;
}
#ifdef __cplusplus
}
#endif
