#include "tree_sitter/parser.h"

#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic ignored "-Wmissing-field-initializers"
#endif

#define LANGUAGE_VERSION 14
#define STATE_COUNT 75
#define LARGE_STATE_COUNT 67
#define SYMBOL_COUNT 40
#define ALIAS_COUNT 0
#define TOKEN_COUNT 25
#define EXTERNAL_TOKEN_COUNT 1
#define FIELD_COUNT 0
#define MAX_ALIAS_SEQUENCE_LENGTH 5
#define PRODUCTION_ID_COUNT 1

enum ts_symbol_identifiers {
  sym_symbol = 1,
  anon_sym_LPAREN = 2,
  anon_sym_RPAREN = 3,
  anon_sym_DOT = 4,
  anon_sym_POUND_LPAREN = 5,
  anon_sym_LBRACK = 6,
  anon_sym_RBRACK = 7,
  anon_sym_LBRACE = 8,
  anon_sym_RBRACE = 9,
  anon_sym_SQUOTE = 10,
  anon_sym_BQUOTE = 11,
  anon_sym_COMMA = 12,
  anon_sym_COMMA_AT = 13,
  anon_sym_AT = 14,
  anon_sym_POUNDu8_LPAREN = 15,
  sym_number = 16,
  sym_string = 17,
  sym_regex = 18,
  sym_shebang = 19,
  sym_keyword = 20,
  sym_boolean = 21,
  sym_character = 22,
  sym_comment = 23,
  sym_block_comment = 24,
  sym_source_file = 25,
  sym__form = 26,
  sym_list = 27,
  sym_short_lambda = 28,
  sym_vector = 29,
  sym_hash_map = 30,
  sym_quote = 31,
  sym_quasiquote = 32,
  sym_unquote = 33,
  sym_unquote_splicing = 34,
  sym_deref = 35,
  sym_byte_vector = 36,
  sym__atom = 37,
  aux_sym_source_file_repeat1 = 38,
  aux_sym_byte_vector_repeat1 = 39,
};

static const char * const ts_symbol_names[] = {
  [ts_builtin_sym_end] = "end",
  [sym_symbol] = "symbol",
  [anon_sym_LPAREN] = "(",
  [anon_sym_RPAREN] = ")",
  [anon_sym_DOT] = ".",
  [anon_sym_POUND_LPAREN] = "#(",
  [anon_sym_LBRACK] = "[",
  [anon_sym_RBRACK] = "]",
  [anon_sym_LBRACE] = "{",
  [anon_sym_RBRACE] = "}",
  [anon_sym_SQUOTE] = "'",
  [anon_sym_BQUOTE] = "`",
  [anon_sym_COMMA] = ",",
  [anon_sym_COMMA_AT] = ",@",
  [anon_sym_AT] = "@",
  [anon_sym_POUNDu8_LPAREN] = "#u8(",
  [sym_number] = "number",
  [sym_string] = "string",
  [sym_regex] = "regex",
  [sym_shebang] = "shebang",
  [sym_keyword] = "keyword",
  [sym_boolean] = "boolean",
  [sym_character] = "character",
  [sym_comment] = "comment",
  [sym_block_comment] = "block_comment",
  [sym_source_file] = "source_file",
  [sym__form] = "_form",
  [sym_list] = "list",
  [sym_short_lambda] = "short_lambda",
  [sym_vector] = "vector",
  [sym_hash_map] = "hash_map",
  [sym_quote] = "quote",
  [sym_quasiquote] = "quasiquote",
  [sym_unquote] = "unquote",
  [sym_unquote_splicing] = "unquote_splicing",
  [sym_deref] = "deref",
  [sym_byte_vector] = "byte_vector",
  [sym__atom] = "_atom",
  [aux_sym_source_file_repeat1] = "source_file_repeat1",
  [aux_sym_byte_vector_repeat1] = "byte_vector_repeat1",
};

static const TSSymbol ts_symbol_map[] = {
  [ts_builtin_sym_end] = ts_builtin_sym_end,
  [sym_symbol] = sym_symbol,
  [anon_sym_LPAREN] = anon_sym_LPAREN,
  [anon_sym_RPAREN] = anon_sym_RPAREN,
  [anon_sym_DOT] = anon_sym_DOT,
  [anon_sym_POUND_LPAREN] = anon_sym_POUND_LPAREN,
  [anon_sym_LBRACK] = anon_sym_LBRACK,
  [anon_sym_RBRACK] = anon_sym_RBRACK,
  [anon_sym_LBRACE] = anon_sym_LBRACE,
  [anon_sym_RBRACE] = anon_sym_RBRACE,
  [anon_sym_SQUOTE] = anon_sym_SQUOTE,
  [anon_sym_BQUOTE] = anon_sym_BQUOTE,
  [anon_sym_COMMA] = anon_sym_COMMA,
  [anon_sym_COMMA_AT] = anon_sym_COMMA_AT,
  [anon_sym_AT] = anon_sym_AT,
  [anon_sym_POUNDu8_LPAREN] = anon_sym_POUNDu8_LPAREN,
  [sym_number] = sym_number,
  [sym_string] = sym_string,
  [sym_regex] = sym_regex,
  [sym_shebang] = sym_shebang,
  [sym_keyword] = sym_keyword,
  [sym_boolean] = sym_boolean,
  [sym_character] = sym_character,
  [sym_comment] = sym_comment,
  [sym_block_comment] = sym_block_comment,
  [sym_source_file] = sym_source_file,
  [sym__form] = sym__form,
  [sym_list] = sym_list,
  [sym_short_lambda] = sym_short_lambda,
  [sym_vector] = sym_vector,
  [sym_hash_map] = sym_hash_map,
  [sym_quote] = sym_quote,
  [sym_quasiquote] = sym_quasiquote,
  [sym_unquote] = sym_unquote,
  [sym_unquote_splicing] = sym_unquote_splicing,
  [sym_deref] = sym_deref,
  [sym_byte_vector] = sym_byte_vector,
  [sym__atom] = sym__atom,
  [aux_sym_source_file_repeat1] = aux_sym_source_file_repeat1,
  [aux_sym_byte_vector_repeat1] = aux_sym_byte_vector_repeat1,
};

static const TSSymbolMetadata ts_symbol_metadata[] = {
  [ts_builtin_sym_end] = {
    .visible = false,
    .named = true,
  },
  [sym_symbol] = {
    .visible = true,
    .named = true,
  },
  [anon_sym_LPAREN] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RPAREN] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_DOT] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_POUND_LPAREN] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_LBRACK] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_RBRACK] = {
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
  [anon_sym_SQUOTE] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_BQUOTE] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_COMMA] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_COMMA_AT] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_AT] = {
    .visible = true,
    .named = false,
  },
  [anon_sym_POUNDu8_LPAREN] = {
    .visible = true,
    .named = false,
  },
  [sym_number] = {
    .visible = true,
    .named = true,
  },
  [sym_string] = {
    .visible = true,
    .named = true,
  },
  [sym_regex] = {
    .visible = true,
    .named = true,
  },
  [sym_shebang] = {
    .visible = true,
    .named = true,
  },
  [sym_keyword] = {
    .visible = true,
    .named = true,
  },
  [sym_boolean] = {
    .visible = true,
    .named = true,
  },
  [sym_character] = {
    .visible = true,
    .named = true,
  },
  [sym_comment] = {
    .visible = true,
    .named = true,
  },
  [sym_block_comment] = {
    .visible = true,
    .named = true,
  },
  [sym_source_file] = {
    .visible = true,
    .named = true,
  },
  [sym__form] = {
    .visible = false,
    .named = true,
  },
  [sym_list] = {
    .visible = true,
    .named = true,
  },
  [sym_short_lambda] = {
    .visible = true,
    .named = true,
  },
  [sym_vector] = {
    .visible = true,
    .named = true,
  },
  [sym_hash_map] = {
    .visible = true,
    .named = true,
  },
  [sym_quote] = {
    .visible = true,
    .named = true,
  },
  [sym_quasiquote] = {
    .visible = true,
    .named = true,
  },
  [sym_unquote] = {
    .visible = true,
    .named = true,
  },
  [sym_unquote_splicing] = {
    .visible = true,
    .named = true,
  },
  [sym_deref] = {
    .visible = true,
    .named = true,
  },
  [sym_byte_vector] = {
    .visible = true,
    .named = true,
  },
  [sym__atom] = {
    .visible = false,
    .named = true,
  },
  [aux_sym_source_file_repeat1] = {
    .visible = false,
    .named = false,
  },
  [aux_sym_byte_vector_repeat1] = {
    .visible = false,
    .named = false,
  },
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
  [4] = 2,
  [5] = 3,
  [6] = 6,
  [7] = 7,
  [8] = 8,
  [9] = 9,
  [10] = 10,
  [11] = 11,
  [12] = 12,
  [13] = 13,
  [14] = 14,
  [15] = 11,
  [16] = 7,
  [17] = 8,
  [18] = 18,
  [19] = 6,
  [20] = 12,
  [21] = 13,
  [22] = 14,
  [23] = 23,
  [24] = 24,
  [25] = 25,
  [26] = 24,
  [27] = 23,
  [28] = 28,
  [29] = 29,
  [30] = 25,
  [31] = 28,
  [32] = 29,
  [33] = 33,
  [34] = 33,
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
  [51] = 46,
  [52] = 43,
  [53] = 45,
  [54] = 37,
  [55] = 49,
  [56] = 36,
  [57] = 39,
  [58] = 40,
  [59] = 42,
  [60] = 47,
  [61] = 48,
  [62] = 35,
  [63] = 44,
  [64] = 50,
  [65] = 41,
  [66] = 38,
  [67] = 67,
  [68] = 68,
  [69] = 69,
  [70] = 69,
  [71] = 67,
  [72] = 72,
  [73] = 73,
  [74] = 72,
};

static TSCharacterRange sym_symbol_character_set_2[] = {
  {'!', '!'}, {'#', '#'}, {'%', '&'}, {'*', '+'}, {'-', '9'}, {'<', '?'}, {'A', 'Z'}, {'^', '_'},
  {'a', 'z'}, {'~', '~'}, {0xaa, 0xaa}, {0xb5, 0xb5}, {0xba, 0xba}, {0xc0, 0xd6}, {0xd8, 0xf6}, {0xf8, 0x2c1},
  {0x2c6, 0x2d1}, {0x2e0, 0x2e4}, {0x2ec, 0x2ec}, {0x2ee, 0x2ee}, {0x345, 0x345}, {0x370, 0x374}, {0x376, 0x377}, {0x37a, 0x37d},
  {0x37f, 0x37f}, {0x386, 0x386}, {0x388, 0x38a}, {0x38c, 0x38c}, {0x38e, 0x3a1}, {0x3a3, 0x3f5}, {0x3f7, 0x481}, {0x48a, 0x52f},
  {0x531, 0x556}, {0x559, 0x559}, {0x560, 0x588}, {0x5b0, 0x5bd}, {0x5bf, 0x5bf}, {0x5c1, 0x5c2}, {0x5c4, 0x5c5}, {0x5c7, 0x5c7},
  {0x5d0, 0x5ea}, {0x5ef, 0x5f2}, {0x610, 0x61a}, {0x620, 0x657}, {0x659, 0x65f}, {0x66e, 0x6d3}, {0x6d5, 0x6dc}, {0x6e1, 0x6e8},
  {0x6ed, 0x6ef}, {0x6fa, 0x6fc}, {0x6ff, 0x6ff}, {0x710, 0x73f}, {0x74d, 0x7b1}, {0x7ca, 0x7ea}, {0x7f4, 0x7f5}, {0x7fa, 0x7fa},
  {0x800, 0x817}, {0x81a, 0x82c}, {0x840, 0x858}, {0x860, 0x86a}, {0x870, 0x887}, {0x889, 0x88e}, {0x8a0, 0x8c9}, {0x8d4, 0x8df},
  {0x8e3, 0x8e9}, {0x8f0, 0x93b}, {0x93d, 0x94c}, {0x94e, 0x950}, {0x955, 0x963}, {0x971, 0x983}, {0x985, 0x98c}, {0x98f, 0x990},
  {0x993, 0x9a8}, {0x9aa, 0x9b0}, {0x9b2, 0x9b2}, {0x9b6, 0x9b9}, {0x9bd, 0x9c4}, {0x9c7, 0x9c8}, {0x9cb, 0x9cc}, {0x9ce, 0x9ce},
  {0x9d7, 0x9d7}, {0x9dc, 0x9dd}, {0x9df, 0x9e3}, {0x9f0, 0x9f1}, {0x9fc, 0x9fc}, {0xa01, 0xa03}, {0xa05, 0xa0a}, {0xa0f, 0xa10},
  {0xa13, 0xa28}, {0xa2a, 0xa30}, {0xa32, 0xa33}, {0xa35, 0xa36}, {0xa38, 0xa39}, {0xa3e, 0xa42}, {0xa47, 0xa48}, {0xa4b, 0xa4c},
  {0xa51, 0xa51}, {0xa59, 0xa5c}, {0xa5e, 0xa5e}, {0xa70, 0xa75}, {0xa81, 0xa83}, {0xa85, 0xa8d}, {0xa8f, 0xa91}, {0xa93, 0xaa8},
  {0xaaa, 0xab0}, {0xab2, 0xab3}, {0xab5, 0xab9}, {0xabd, 0xac5}, {0xac7, 0xac9}, {0xacb, 0xacc}, {0xad0, 0xad0}, {0xae0, 0xae3},
  {0xaf9, 0xafc}, {0xb01, 0xb03}, {0xb05, 0xb0c}, {0xb0f, 0xb10}, {0xb13, 0xb28}, {0xb2a, 0xb30}, {0xb32, 0xb33}, {0xb35, 0xb39},
  {0xb3d, 0xb44}, {0xb47, 0xb48}, {0xb4b, 0xb4c}, {0xb56, 0xb57}, {0xb5c, 0xb5d}, {0xb5f, 0xb63}, {0xb71, 0xb71}, {0xb82, 0xb83},
  {0xb85, 0xb8a}, {0xb8e, 0xb90}, {0xb92, 0xb95}, {0xb99, 0xb9a}, {0xb9c, 0xb9c}, {0xb9e, 0xb9f}, {0xba3, 0xba4}, {0xba8, 0xbaa},
  {0xbae, 0xbb9}, {0xbbe, 0xbc2}, {0xbc6, 0xbc8}, {0xbca, 0xbcc}, {0xbd0, 0xbd0}, {0xbd7, 0xbd7}, {0xc00, 0xc0c}, {0xc0e, 0xc10},
  {0xc12, 0xc28}, {0xc2a, 0xc39}, {0xc3d, 0xc44}, {0xc46, 0xc48}, {0xc4a, 0xc4c}, {0xc55, 0xc56}, {0xc58, 0xc5a}, {0xc5d, 0xc5d},
  {0xc60, 0xc63}, {0xc80, 0xc83}, {0xc85, 0xc8c}, {0xc8e, 0xc90}, {0xc92, 0xca8}, {0xcaa, 0xcb3}, {0xcb5, 0xcb9}, {0xcbd, 0xcc4},
  {0xcc6, 0xcc8}, {0xcca, 0xccc}, {0xcd5, 0xcd6}, {0xcdd, 0xcde}, {0xce0, 0xce3}, {0xcf1, 0xcf3}, {0xd00, 0xd0c}, {0xd0e, 0xd10},
  {0xd12, 0xd3a}, {0xd3d, 0xd44}, {0xd46, 0xd48}, {0xd4a, 0xd4c}, {0xd4e, 0xd4e}, {0xd54, 0xd57}, {0xd5f, 0xd63}, {0xd7a, 0xd7f},
  {0xd81, 0xd83}, {0xd85, 0xd96}, {0xd9a, 0xdb1}, {0xdb3, 0xdbb}, {0xdbd, 0xdbd}, {0xdc0, 0xdc6}, {0xdcf, 0xdd4}, {0xdd6, 0xdd6},
  {0xdd8, 0xddf}, {0xdf2, 0xdf3}, {0xe01, 0xe3a}, {0xe40, 0xe46}, {0xe4d, 0xe4d}, {0xe81, 0xe82}, {0xe84, 0xe84}, {0xe86, 0xe8a},
  {0xe8c, 0xea3}, {0xea5, 0xea5}, {0xea7, 0xeb9}, {0xebb, 0xebd}, {0xec0, 0xec4}, {0xec6, 0xec6}, {0xecd, 0xecd}, {0xedc, 0xedf},
  {0xf00, 0xf00}, {0xf40, 0xf47}, {0xf49, 0xf6c}, {0xf71, 0xf83}, {0xf88, 0xf97}, {0xf99, 0xfbc}, {0x1000, 0x1036}, {0x1038, 0x1038},
  {0x103b, 0x103f}, {0x1050, 0x108f}, {0x109a, 0x109d}, {0x10a0, 0x10c5}, {0x10c7, 0x10c7}, {0x10cd, 0x10cd}, {0x10d0, 0x10fa}, {0x10fc, 0x1248},
  {0x124a, 0x124d}, {0x1250, 0x1256}, {0x1258, 0x1258}, {0x125a, 0x125d}, {0x1260, 0x1288}, {0x128a, 0x128d}, {0x1290, 0x12b0}, {0x12b2, 0x12b5},
  {0x12b8, 0x12be}, {0x12c0, 0x12c0}, {0x12c2, 0x12c5}, {0x12c8, 0x12d6}, {0x12d8, 0x1310}, {0x1312, 0x1315}, {0x1318, 0x135a}, {0x1380, 0x138f},
  {0x13a0, 0x13f5}, {0x13f8, 0x13fd}, {0x1401, 0x166c}, {0x166f, 0x167f}, {0x1681, 0x169a}, {0x16a0, 0x16ea}, {0x16ee, 0x16f8}, {0x1700, 0x1713},
  {0x171f, 0x1733}, {0x1740, 0x1753}, {0x1760, 0x176c}, {0x176e, 0x1770}, {0x1772, 0x1773}, {0x1780, 0x17b3}, {0x17b6, 0x17c8}, {0x17d7, 0x17d7},
  {0x17dc, 0x17dc}, {0x1820, 0x1878}, {0x1880, 0x18aa}, {0x18b0, 0x18f5}, {0x1900, 0x191e}, {0x1920, 0x192b}, {0x1930, 0x1938}, {0x1950, 0x196d},
  {0x1970, 0x1974}, {0x1980, 0x19ab}, {0x19b0, 0x19c9}, {0x1a00, 0x1a1b}, {0x1a20, 0x1a5e}, {0x1a61, 0x1a74}, {0x1aa7, 0x1aa7}, {0x1abf, 0x1ac0},
  {0x1acc, 0x1ace}, {0x1b00, 0x1b33}, {0x1b35, 0x1b43}, {0x1b45, 0x1b4c}, {0x1b80, 0x1ba9}, {0x1bac, 0x1baf}, {0x1bba, 0x1be5}, {0x1be7, 0x1bf1},
  {0x1c00, 0x1c36}, {0x1c4d, 0x1c4f}, {0x1c5a, 0x1c7d}, {0x1c80, 0x1c88}, {0x1c90, 0x1cba}, {0x1cbd, 0x1cbf}, {0x1ce9, 0x1cec}, {0x1cee, 0x1cf3},
  {0x1cf5, 0x1cf6}, {0x1cfa, 0x1cfa}, {0x1d00, 0x1dbf}, {0x1de7, 0x1df4}, {0x1e00, 0x1f15}, {0x1f18, 0x1f1d}, {0x1f20, 0x1f45}, {0x1f48, 0x1f4d},
  {0x1f50, 0x1f57}, {0x1f59, 0x1f59}, {0x1f5b, 0x1f5b}, {0x1f5d, 0x1f5d}, {0x1f5f, 0x1f7d}, {0x1f80, 0x1fb4}, {0x1fb6, 0x1fbc}, {0x1fbe, 0x1fbe},
  {0x1fc2, 0x1fc4}, {0x1fc6, 0x1fcc}, {0x1fd0, 0x1fd3}, {0x1fd6, 0x1fdb}, {0x1fe0, 0x1fec}, {0x1ff2, 0x1ff4}, {0x1ff6, 0x1ffc}, {0x2071, 0x2071},
  {0x207f, 0x207f}, {0x2090, 0x209c}, {0x2102, 0x2102}, {0x2107, 0x2107}, {0x210a, 0x2113}, {0x2115, 0x2115}, {0x2119, 0x211d}, {0x2124, 0x2124},
  {0x2126, 0x2126}, {0x2128, 0x2128}, {0x212a, 0x212d}, {0x212f, 0x2139}, {0x213c, 0x213f}, {0x2145, 0x2149}, {0x214e, 0x214e}, {0x2160, 0x2188},
  {0x24b6, 0x24e9}, {0x2c00, 0x2ce4}, {0x2ceb, 0x2cee}, {0x2cf2, 0x2cf3}, {0x2d00, 0x2d25}, {0x2d27, 0x2d27}, {0x2d2d, 0x2d2d}, {0x2d30, 0x2d67},
  {0x2d6f, 0x2d6f}, {0x2d80, 0x2d96}, {0x2da0, 0x2da6}, {0x2da8, 0x2dae}, {0x2db0, 0x2db6}, {0x2db8, 0x2dbe}, {0x2dc0, 0x2dc6}, {0x2dc8, 0x2dce},
  {0x2dd0, 0x2dd6}, {0x2dd8, 0x2dde}, {0x2de0, 0x2dff}, {0x2e2f, 0x2e2f}, {0x3005, 0x3007}, {0x3021, 0x3029}, {0x3031, 0x3035}, {0x3038, 0x303c},
  {0x3041, 0x3096}, {0x309d, 0x309f}, {0x30a1, 0x30fa}, {0x30fc, 0x30ff}, {0x3105, 0x312f}, {0x3131, 0x318e}, {0x31a0, 0x31bf}, {0x31f0, 0x31ff},
  {0x3400, 0x4dbf}, {0x4e00, 0xa48c}, {0xa4d0, 0xa4fd}, {0xa500, 0xa60c}, {0xa610, 0xa61f}, {0xa62a, 0xa62b}, {0xa640, 0xa66e}, {0xa674, 0xa67b},
  {0xa67f, 0xa6ef}, {0xa717, 0xa71f}, {0xa722, 0xa788}, {0xa78b, 0xa7ca}, {0xa7d0, 0xa7d1}, {0xa7d3, 0xa7d3}, {0xa7d5, 0xa7d9}, {0xa7f2, 0xa805},
  {0xa807, 0xa827}, {0xa840, 0xa873}, {0xa880, 0xa8c3}, {0xa8c5, 0xa8c5}, {0xa8f2, 0xa8f7}, {0xa8fb, 0xa8fb}, {0xa8fd, 0xa8ff}, {0xa90a, 0xa92a},
  {0xa930, 0xa952}, {0xa960, 0xa97c}, {0xa980, 0xa9b2}, {0xa9b4, 0xa9bf}, {0xa9cf, 0xa9cf}, {0xa9e0, 0xa9ef}, {0xa9fa, 0xa9fe}, {0xaa00, 0xaa36},
  {0xaa40, 0xaa4d}, {0xaa60, 0xaa76}, {0xaa7a, 0xaabe}, {0xaac0, 0xaac0}, {0xaac2, 0xaac2}, {0xaadb, 0xaadd}, {0xaae0, 0xaaef}, {0xaaf2, 0xaaf5},
  {0xab01, 0xab06}, {0xab09, 0xab0e}, {0xab11, 0xab16}, {0xab20, 0xab26}, {0xab28, 0xab2e}, {0xab30, 0xab5a}, {0xab5c, 0xab69}, {0xab70, 0xabea},
  {0xac00, 0xd7a3}, {0xd7b0, 0xd7c6}, {0xd7cb, 0xd7fb}, {0xf900, 0xfa6d}, {0xfa70, 0xfad9}, {0xfb00, 0xfb06}, {0xfb13, 0xfb17}, {0xfb1d, 0xfb28},
  {0xfb2a, 0xfb36}, {0xfb38, 0xfb3c}, {0xfb3e, 0xfb3e}, {0xfb40, 0xfb41}, {0xfb43, 0xfb44}, {0xfb46, 0xfbb1}, {0xfbd3, 0xfd3d}, {0xfd50, 0xfd8f},
  {0xfd92, 0xfdc7}, {0xfdf0, 0xfdfb}, {0xfe70, 0xfe74}, {0xfe76, 0xfefc}, {0xff21, 0xff3a}, {0xff41, 0xff5a}, {0xff66, 0xffbe}, {0xffc2, 0xffc7},
  {0xffca, 0xffcf}, {0xffd2, 0xffd7}, {0xffda, 0xffdc}, {0x10000, 0x1000b}, {0x1000d, 0x10026}, {0x10028, 0x1003a}, {0x1003c, 0x1003d}, {0x1003f, 0x1004d},
  {0x10050, 0x1005d}, {0x10080, 0x100fa}, {0x10140, 0x10174}, {0x10280, 0x1029c}, {0x102a0, 0x102d0}, {0x10300, 0x1031f}, {0x1032d, 0x1034a}, {0x10350, 0x1037a},
  {0x10380, 0x1039d}, {0x103a0, 0x103c3}, {0x103c8, 0x103cf}, {0x103d1, 0x103d5}, {0x10400, 0x1049d}, {0x104b0, 0x104d3}, {0x104d8, 0x104fb}, {0x10500, 0x10527},
  {0x10530, 0x10563}, {0x10570, 0x1057a}, {0x1057c, 0x1058a}, {0x1058c, 0x10592}, {0x10594, 0x10595}, {0x10597, 0x105a1}, {0x105a3, 0x105b1}, {0x105b3, 0x105b9},
  {0x105bb, 0x105bc}, {0x10600, 0x10736}, {0x10740, 0x10755}, {0x10760, 0x10767}, {0x10780, 0x10785}, {0x10787, 0x107b0}, {0x107b2, 0x107ba}, {0x10800, 0x10805},
  {0x10808, 0x10808}, {0x1080a, 0x10835}, {0x10837, 0x10838}, {0x1083c, 0x1083c}, {0x1083f, 0x10855}, {0x10860, 0x10876}, {0x10880, 0x1089e}, {0x108e0, 0x108f2},
  {0x108f4, 0x108f5}, {0x10900, 0x10915}, {0x10920, 0x10939}, {0x10980, 0x109b7}, {0x109be, 0x109bf}, {0x10a00, 0x10a03}, {0x10a05, 0x10a06}, {0x10a0c, 0x10a13},
  {0x10a15, 0x10a17}, {0x10a19, 0x10a35}, {0x10a60, 0x10a7c}, {0x10a80, 0x10a9c}, {0x10ac0, 0x10ac7}, {0x10ac9, 0x10ae4}, {0x10b00, 0x10b35}, {0x10b40, 0x10b55},
  {0x10b60, 0x10b72}, {0x10b80, 0x10b91}, {0x10c00, 0x10c48}, {0x10c80, 0x10cb2}, {0x10cc0, 0x10cf2}, {0x10d00, 0x10d27}, {0x10e80, 0x10ea9}, {0x10eab, 0x10eac},
  {0x10eb0, 0x10eb1}, {0x10f00, 0x10f1c}, {0x10f27, 0x10f27}, {0x10f30, 0x10f45}, {0x10f70, 0x10f81}, {0x10fb0, 0x10fc4}, {0x10fe0, 0x10ff6}, {0x11000, 0x11045},
  {0x11071, 0x11075}, {0x11080, 0x110b8}, {0x110c2, 0x110c2}, {0x110d0, 0x110e8}, {0x11100, 0x11132}, {0x11144, 0x11147}, {0x11150, 0x11172}, {0x11176, 0x11176},
  {0x11180, 0x111bf}, {0x111c1, 0x111c4}, {0x111ce, 0x111cf}, {0x111da, 0x111da}, {0x111dc, 0x111dc}, {0x11200, 0x11211}, {0x11213, 0x11234}, {0x11237, 0x11237},
  {0x1123e, 0x11241}, {0x11280, 0x11286}, {0x11288, 0x11288}, {0x1128a, 0x1128d}, {0x1128f, 0x1129d}, {0x1129f, 0x112a8}, {0x112b0, 0x112e8}, {0x11300, 0x11303},
  {0x11305, 0x1130c}, {0x1130f, 0x11310}, {0x11313, 0x11328}, {0x1132a, 0x11330}, {0x11332, 0x11333}, {0x11335, 0x11339}, {0x1133d, 0x11344}, {0x11347, 0x11348},
  {0x1134b, 0x1134c}, {0x11350, 0x11350}, {0x11357, 0x11357}, {0x1135d, 0x11363}, {0x11400, 0x11441}, {0x11443, 0x11445}, {0x11447, 0x1144a}, {0x1145f, 0x11461},
  {0x11480, 0x114c1}, {0x114c4, 0x114c5}, {0x114c7, 0x114c7}, {0x11580, 0x115b5}, {0x115b8, 0x115be}, {0x115d8, 0x115dd}, {0x11600, 0x1163e}, {0x11640, 0x11640},
  {0x11644, 0x11644}, {0x11680, 0x116b5}, {0x116b8, 0x116b8}, {0x11700, 0x1171a}, {0x1171d, 0x1172a}, {0x11740, 0x11746}, {0x11800, 0x11838}, {0x118a0, 0x118df},
  {0x118ff, 0x11906}, {0x11909, 0x11909}, {0x1190c, 0x11913}, {0x11915, 0x11916}, {0x11918, 0x11935}, {0x11937, 0x11938}, {0x1193b, 0x1193c}, {0x1193f, 0x11942},
  {0x119a0, 0x119a7}, {0x119aa, 0x119d7}, {0x119da, 0x119df}, {0x119e1, 0x119e1}, {0x119e3, 0x119e4}, {0x11a00, 0x11a32}, {0x11a35, 0x11a3e}, {0x11a50, 0x11a97},
  {0x11a9d, 0x11a9d}, {0x11ab0, 0x11af8}, {0x11c00, 0x11c08}, {0x11c0a, 0x11c36}, {0x11c38, 0x11c3e}, {0x11c40, 0x11c40}, {0x11c72, 0x11c8f}, {0x11c92, 0x11ca7},
  {0x11ca9, 0x11cb6}, {0x11d00, 0x11d06}, {0x11d08, 0x11d09}, {0x11d0b, 0x11d36}, {0x11d3a, 0x11d3a}, {0x11d3c, 0x11d3d}, {0x11d3f, 0x11d41}, {0x11d43, 0x11d43},
  {0x11d46, 0x11d47}, {0x11d60, 0x11d65}, {0x11d67, 0x11d68}, {0x11d6a, 0x11d8e}, {0x11d90, 0x11d91}, {0x11d93, 0x11d96}, {0x11d98, 0x11d98}, {0x11ee0, 0x11ef6},
  {0x11f00, 0x11f10}, {0x11f12, 0x11f3a}, {0x11f3e, 0x11f40}, {0x11fb0, 0x11fb0}, {0x12000, 0x12399}, {0x12400, 0x1246e}, {0x12480, 0x12543}, {0x12f90, 0x12ff0},
  {0x13000, 0x1342f}, {0x13441, 0x13446}, {0x14400, 0x14646}, {0x16800, 0x16a38}, {0x16a40, 0x16a5e}, {0x16a70, 0x16abe}, {0x16ad0, 0x16aed}, {0x16b00, 0x16b2f},
  {0x16b40, 0x16b43}, {0x16b63, 0x16b77}, {0x16b7d, 0x16b8f}, {0x16e40, 0x16e7f}, {0x16f00, 0x16f4a}, {0x16f4f, 0x16f87}, {0x16f8f, 0x16f9f}, {0x16fe0, 0x16fe1},
  {0x16fe3, 0x16fe3}, {0x16ff0, 0x16ff1}, {0x17000, 0x187f7}, {0x18800, 0x18cd5}, {0x18d00, 0x18d08}, {0x1aff0, 0x1aff3}, {0x1aff5, 0x1affb}, {0x1affd, 0x1affe},
  {0x1b000, 0x1b122}, {0x1b132, 0x1b132}, {0x1b150, 0x1b152}, {0x1b155, 0x1b155}, {0x1b164, 0x1b167}, {0x1b170, 0x1b2fb}, {0x1bc00, 0x1bc6a}, {0x1bc70, 0x1bc7c},
  {0x1bc80, 0x1bc88}, {0x1bc90, 0x1bc99}, {0x1bc9e, 0x1bc9e}, {0x1d400, 0x1d454}, {0x1d456, 0x1d49c}, {0x1d49e, 0x1d49f}, {0x1d4a2, 0x1d4a2}, {0x1d4a5, 0x1d4a6},
  {0x1d4a9, 0x1d4ac}, {0x1d4ae, 0x1d4b9}, {0x1d4bb, 0x1d4bb}, {0x1d4bd, 0x1d4c3}, {0x1d4c5, 0x1d505}, {0x1d507, 0x1d50a}, {0x1d50d, 0x1d514}, {0x1d516, 0x1d51c},
  {0x1d51e, 0x1d539}, {0x1d53b, 0x1d53e}, {0x1d540, 0x1d544}, {0x1d546, 0x1d546}, {0x1d54a, 0x1d550}, {0x1d552, 0x1d6a5}, {0x1d6a8, 0x1d6c0}, {0x1d6c2, 0x1d6da},
  {0x1d6dc, 0x1d6fa}, {0x1d6fc, 0x1d714}, {0x1d716, 0x1d734}, {0x1d736, 0x1d74e}, {0x1d750, 0x1d76e}, {0x1d770, 0x1d788}, {0x1d78a, 0x1d7a8}, {0x1d7aa, 0x1d7c2},
  {0x1d7c4, 0x1d7cb}, {0x1df00, 0x1df1e}, {0x1df25, 0x1df2a}, {0x1e000, 0x1e006}, {0x1e008, 0x1e018}, {0x1e01b, 0x1e021}, {0x1e023, 0x1e024}, {0x1e026, 0x1e02a},
  {0x1e030, 0x1e06d}, {0x1e08f, 0x1e08f}, {0x1e100, 0x1e12c}, {0x1e137, 0x1e13d}, {0x1e14e, 0x1e14e}, {0x1e290, 0x1e2ad}, {0x1e2c0, 0x1e2eb}, {0x1e4d0, 0x1e4eb},
  {0x1e7e0, 0x1e7e6}, {0x1e7e8, 0x1e7eb}, {0x1e7ed, 0x1e7ee}, {0x1e7f0, 0x1e7fe}, {0x1e800, 0x1e8c4}, {0x1e900, 0x1e943}, {0x1e947, 0x1e947}, {0x1e94b, 0x1e94b},
  {0x1ee00, 0x1ee03}, {0x1ee05, 0x1ee1f}, {0x1ee21, 0x1ee22}, {0x1ee24, 0x1ee24}, {0x1ee27, 0x1ee27}, {0x1ee29, 0x1ee32}, {0x1ee34, 0x1ee37}, {0x1ee39, 0x1ee39},
  {0x1ee3b, 0x1ee3b}, {0x1ee42, 0x1ee42}, {0x1ee47, 0x1ee47}, {0x1ee49, 0x1ee49}, {0x1ee4b, 0x1ee4b}, {0x1ee4d, 0x1ee4f}, {0x1ee51, 0x1ee52}, {0x1ee54, 0x1ee54},
  {0x1ee57, 0x1ee57}, {0x1ee59, 0x1ee59}, {0x1ee5b, 0x1ee5b}, {0x1ee5d, 0x1ee5d}, {0x1ee5f, 0x1ee5f}, {0x1ee61, 0x1ee62}, {0x1ee64, 0x1ee64}, {0x1ee67, 0x1ee6a},
  {0x1ee6c, 0x1ee72}, {0x1ee74, 0x1ee77}, {0x1ee79, 0x1ee7c}, {0x1ee7e, 0x1ee7e}, {0x1ee80, 0x1ee89}, {0x1ee8b, 0x1ee9b}, {0x1eea1, 0x1eea3}, {0x1eea5, 0x1eea9},
  {0x1eeab, 0x1eebb}, {0x1f130, 0x1f149}, {0x1f150, 0x1f169}, {0x1f170, 0x1f189}, {0x20000, 0x2a6df}, {0x2a700, 0x2b739}, {0x2b740, 0x2b81d}, {0x2b820, 0x2cea1},
  {0x2ceb0, 0x2ebe0}, {0x2ebf0, 0x2ee5d}, {0x2f800, 0x2fa1d}, {0x30000, 0x3134a}, {0x31350, 0x323af},
};

static bool ts_lex(TSLexer *lexer, TSStateId state) {
  START_LEXER();
  eof = lexer->eof(lexer);
  switch (state) {
    case 0:
      if (eof) ADVANCE(54);
      ADVANCE_MAP(
        '"', 2,
        '#', 1,
        '\'', 63,
        '(', 55,
        ')', 56,
        ',', 65,
        '.', 57,
        ':', 50,
        ';', 108,
        '@', 67,
        '[', 59,
        ']', 60,
        '`', 64,
        'f', 84,
        't', 91,
        '{', 61,
        '}', 62,
        '+', 87,
        '-', 87,
      );
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ') SKIP(0);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(70);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 1:
      ADVANCE_MAP(
        '!', 82,
        '"', 3,
        '(', 58,
        '\\', 24,
        'u', 12,
        'B', 4,
        'b', 4,
        'D', 7,
        'd', 7,
        'O', 8,
        'o', 8,
        'X', 9,
        'x', 9,
        'f', 102,
        't', 102,
        'E', 5,
        'I', 5,
        'e', 5,
        'i', 5,
      );
      END_STATE();
    case 2:
      if (lookahead == '"') ADVANCE(80);
      if (lookahead == '\\') ADVANCE(51);
      if (lookahead != 0) ADVANCE(2);
      END_STATE();
    case 3:
      if (lookahead == '"') ADVANCE(81);
      if (lookahead == '\\') ADVANCE(52);
      if (lookahead != 0) ADVANCE(3);
      END_STATE();
    case 4:
      if (lookahead == '#') ADVANCE(40);
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(39);
      if (lookahead == '0' ||
          lookahead == '1') ADVANCE(76);
      END_STATE();
    case 5:
      if (lookahead == '#') ADVANCE(38);
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(18);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(70);
      END_STATE();
    case 6:
      if (lookahead == '#') ADVANCE(37);
      if (lookahead == ')') ADVANCE(56);
      if (lookahead == ';') ADVANCE(108);
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(18);
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ') SKIP(6);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(70);
      END_STATE();
    case 7:
      if (lookahead == '#') ADVANCE(43);
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(18);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(70);
      END_STATE();
    case 8:
      if (lookahead == '#') ADVANCE(41);
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(44);
      if (('0' <= lookahead && lookahead <= '7')) ADVANCE(77);
      END_STATE();
    case 9:
      if (lookahead == '#') ADVANCE(42);
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(49);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(78);
      END_STATE();
    case 10:
      if (lookahead == '(') ADVANCE(68);
      END_STATE();
    case 11:
      if (lookahead == '.') ADVANCE(47);
      if (lookahead == '/') ADVANCE(48);
      if (lookahead == 'i') ADVANCE(69);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(36);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(11);
      END_STATE();
    case 12:
      if (lookahead == '8') ADVANCE(10);
      END_STATE();
    case 13:
      if (lookahead == 'a') ADVANCE(15);
      END_STATE();
    case 14:
      if (lookahead == 'b') ADVANCE(103);
      END_STATE();
    case 15:
      if (lookahead == 'c') ADVANCE(16);
      END_STATE();
    case 16:
      if (lookahead == 'e') ADVANCE(103);
      END_STATE();
    case 17:
      if (lookahead == 'i') ADVANCE(69);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(36);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(17);
      END_STATE();
    case 18:
      if (lookahead == 'i') ADVANCE(69);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(70);
      END_STATE();
    case 19:
      if (lookahead == 'i') ADVANCE(69);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(11);
      END_STATE();
    case 20:
      if (lookahead == 'i') ADVANCE(69);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(20);
      END_STATE();
    case 21:
      if (lookahead == 'i') ADVANCE(26);
      END_STATE();
    case 22:
      if (lookahead == 'l') ADVANCE(103);
      END_STATE();
    case 23:
      if (lookahead == 'l') ADVANCE(21);
      END_STATE();
    case 24:
      if (lookahead == 'n') ADVANCE(105);
      if (lookahead == 'r') ADVANCE(106);
      if (lookahead == 's') ADVANCE(107);
      if (lookahead == 't') ADVANCE(104);
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(103);
      END_STATE();
    case 25:
      if (lookahead == 'n') ADVANCE(103);
      END_STATE();
    case 26:
      if (lookahead == 'n') ADVANCE(16);
      END_STATE();
    case 27:
      if (lookahead == 'r') ADVANCE(25);
      END_STATE();
    case 28:
      if (lookahead == 't') ADVANCE(29);
      END_STATE();
    case 29:
      if (lookahead == 'u') ADVANCE(27);
      END_STATE();
    case 30:
      if (lookahead == 'w') ADVANCE(23);
      END_STATE();
    case 31:
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(39);
      if (lookahead == '0' ||
          lookahead == '1') ADVANCE(76);
      END_STATE();
    case 32:
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(44);
      if (('0' <= lookahead && lookahead <= '7')) ADVANCE(77);
      END_STATE();
    case 33:
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(49);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(78);
      END_STATE();
    case 34:
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(46);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(75);
      END_STATE();
    case 35:
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(18);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(70);
      END_STATE();
    case 36:
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(48);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(20);
      END_STATE();
    case 37:
      ADVANCE_MAP(
        'B', 4,
        'b', 4,
        'D', 7,
        'd', 7,
        'O', 8,
        'o', 8,
        'X', 9,
        'x', 9,
        'E', 5,
        'I', 5,
        'e', 5,
        'i', 5,
      );
      END_STATE();
    case 38:
      ADVANCE_MAP(
        'B', 31,
        'b', 31,
        'D', 35,
        'd', 35,
        'O', 32,
        'o', 32,
        'X', 33,
        'x', 33,
      );
      END_STATE();
    case 39:
      if (lookahead == '0' ||
          lookahead == '1') ADVANCE(76);
      END_STATE();
    case 40:
      if (lookahead == 'E' ||
          lookahead == 'I' ||
          lookahead == 'e' ||
          lookahead == 'i') ADVANCE(31);
      END_STATE();
    case 41:
      if (lookahead == 'E' ||
          lookahead == 'I' ||
          lookahead == 'e' ||
          lookahead == 'i') ADVANCE(32);
      END_STATE();
    case 42:
      if (lookahead == 'E' ||
          lookahead == 'I' ||
          lookahead == 'e' ||
          lookahead == 'i') ADVANCE(33);
      END_STATE();
    case 43:
      if (lookahead == 'E' ||
          lookahead == 'I' ||
          lookahead == 'e' ||
          lookahead == 'i') ADVANCE(35);
      END_STATE();
    case 44:
      if (('0' <= lookahead && lookahead <= '7')) ADVANCE(77);
      END_STATE();
    case 45:
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(74);
      END_STATE();
    case 46:
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(75);
      END_STATE();
    case 47:
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(17);
      END_STATE();
    case 48:
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(20);
      END_STATE();
    case 49:
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(78);
      END_STATE();
    case 50:
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(101);
      END_STATE();
    case 51:
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(2);
      END_STATE();
    case 52:
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(3);
      END_STATE();
    case 53:
      if (eof) ADVANCE(54);
      ADVANCE_MAP(
        '"', 2,
        '#', 1,
        '\'', 63,
        '(', 55,
        ')', 56,
        ',', 65,
        ':', 50,
        ';', 108,
        '@', 67,
        '[', 59,
        ']', 60,
        '`', 64,
        'f', 84,
        't', 91,
        '{', 61,
        '}', 62,
        '+', 87,
        '-', 87,
      );
      if (('\t' <= lookahead && lookahead <= '\r') ||
          lookahead == ' ') SKIP(53);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(70);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 54:
      ACCEPT_TOKEN(ts_builtin_sym_end);
      END_STATE();
    case 55:
      ACCEPT_TOKEN(anon_sym_LPAREN);
      END_STATE();
    case 56:
      ACCEPT_TOKEN(anon_sym_RPAREN);
      END_STATE();
    case 57:
      ACCEPT_TOKEN(anon_sym_DOT);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 58:
      ACCEPT_TOKEN(anon_sym_POUND_LPAREN);
      END_STATE();
    case 59:
      ACCEPT_TOKEN(anon_sym_LBRACK);
      END_STATE();
    case 60:
      ACCEPT_TOKEN(anon_sym_RBRACK);
      END_STATE();
    case 61:
      ACCEPT_TOKEN(anon_sym_LBRACE);
      END_STATE();
    case 62:
      ACCEPT_TOKEN(anon_sym_RBRACE);
      END_STATE();
    case 63:
      ACCEPT_TOKEN(anon_sym_SQUOTE);
      END_STATE();
    case 64:
      ACCEPT_TOKEN(anon_sym_BQUOTE);
      END_STATE();
    case 65:
      ACCEPT_TOKEN(anon_sym_COMMA);
      if (lookahead == '@') ADVANCE(66);
      END_STATE();
    case 66:
      ACCEPT_TOKEN(anon_sym_COMMA_AT);
      END_STATE();
    case 67:
      ACCEPT_TOKEN(anon_sym_AT);
      END_STATE();
    case 68:
      ACCEPT_TOKEN(anon_sym_POUNDu8_LPAREN);
      END_STATE();
    case 69:
      ACCEPT_TOKEN(sym_number);
      END_STATE();
    case 70:
      ACCEPT_TOKEN(sym_number);
      if (lookahead == '.') ADVANCE(45);
      if (lookahead == '/') ADVANCE(46);
      if (lookahead == 'i') ADVANCE(69);
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(19);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(34);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(70);
      END_STATE();
    case 71:
      ACCEPT_TOKEN(sym_number);
      if (lookahead == '.') ADVANCE(96);
      if (lookahead == '/') ADVANCE(97);
      if (lookahead == 'i') ADVANCE(79);
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(88);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(94);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(71);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 72:
      ACCEPT_TOKEN(sym_number);
      if (lookahead == 'i') ADVANCE(79);
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(88);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(94);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(72);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 73:
      ACCEPT_TOKEN(sym_number);
      if (lookahead == 'i') ADVANCE(79);
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(88);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(73);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 74:
      ACCEPT_TOKEN(sym_number);
      if (lookahead == 'i') ADVANCE(69);
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(19);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(34);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(74);
      END_STATE();
    case 75:
      ACCEPT_TOKEN(sym_number);
      if (lookahead == 'i') ADVANCE(69);
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(19);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(75);
      END_STATE();
    case 76:
      ACCEPT_TOKEN(sym_number);
      if (lookahead == '0' ||
          lookahead == '1') ADVANCE(76);
      END_STATE();
    case 77:
      ACCEPT_TOKEN(sym_number);
      if (('0' <= lookahead && lookahead <= '7')) ADVANCE(77);
      END_STATE();
    case 78:
      ACCEPT_TOKEN(sym_number);
      if (('0' <= lookahead && lookahead <= '9') ||
          ('A' <= lookahead && lookahead <= 'F') ||
          ('a' <= lookahead && lookahead <= 'f')) ADVANCE(78);
      END_STATE();
    case 79:
      ACCEPT_TOKEN(sym_number);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 80:
      ACCEPT_TOKEN(sym_string);
      END_STATE();
    case 81:
      ACCEPT_TOKEN(sym_regex);
      END_STATE();
    case 82:
      ACCEPT_TOKEN(sym_shebang);
      if (lookahead != 0 &&
          lookahead != '\n' &&
          lookahead != '\r') ADVANCE(82);
      END_STATE();
    case 83:
      ACCEPT_TOKEN(sym_symbol);
      if (lookahead == '.') ADVANCE(98);
      if (lookahead == '/') ADVANCE(99);
      if (lookahead == 'i') ADVANCE(79);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(95);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(83);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 84:
      ACCEPT_TOKEN(sym_symbol);
      if (lookahead == 'a') ADVANCE(90);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 85:
      ACCEPT_TOKEN(sym_symbol);
      if (lookahead == 'e') ADVANCE(102);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 86:
      ACCEPT_TOKEN(sym_symbol);
      if (lookahead == 'i') ADVANCE(79);
      if (lookahead == 'E' ||
          lookahead == 'e') ADVANCE(95);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(86);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 87:
      ACCEPT_TOKEN(sym_symbol);
      if (lookahead == 'i') ADVANCE(79);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(71);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 88:
      ACCEPT_TOKEN(sym_symbol);
      if (lookahead == 'i') ADVANCE(79);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(83);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 89:
      ACCEPT_TOKEN(sym_symbol);
      if (lookahead == 'i') ADVANCE(79);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(89);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 90:
      ACCEPT_TOKEN(sym_symbol);
      if (lookahead == 'l') ADVANCE(92);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 91:
      ACCEPT_TOKEN(sym_symbol);
      if (lookahead == 'r') ADVANCE(93);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 92:
      ACCEPT_TOKEN(sym_symbol);
      if (lookahead == 's') ADVANCE(85);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 93:
      ACCEPT_TOKEN(sym_symbol);
      if (lookahead == 'u') ADVANCE(85);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 94:
      ACCEPT_TOKEN(sym_symbol);
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(97);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(73);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 95:
      ACCEPT_TOKEN(sym_symbol);
      if (lookahead == '+' ||
          lookahead == '-') ADVANCE(99);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(89);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 96:
      ACCEPT_TOKEN(sym_symbol);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(72);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 97:
      ACCEPT_TOKEN(sym_symbol);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(73);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 98:
      ACCEPT_TOKEN(sym_symbol);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(86);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 99:
      ACCEPT_TOKEN(sym_symbol);
      if (('0' <= lookahead && lookahead <= '9')) ADVANCE(89);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 100:
      ACCEPT_TOKEN(sym_symbol);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(100);
      END_STATE();
    case 101:
      ACCEPT_TOKEN(sym_keyword);
      if (set_contains(sym_symbol_character_set_2, 741, lookahead)) ADVANCE(101);
      END_STATE();
    case 102:
      ACCEPT_TOKEN(sym_boolean);
      END_STATE();
    case 103:
      ACCEPT_TOKEN(sym_character);
      END_STATE();
    case 104:
      ACCEPT_TOKEN(sym_character);
      if (lookahead == 'a') ADVANCE(14);
      END_STATE();
    case 105:
      ACCEPT_TOKEN(sym_character);
      if (lookahead == 'e') ADVANCE(30);
      if (lookahead == 'u') ADVANCE(22);
      END_STATE();
    case 106:
      ACCEPT_TOKEN(sym_character);
      if (lookahead == 'e') ADVANCE(28);
      END_STATE();
    case 107:
      ACCEPT_TOKEN(sym_character);
      if (lookahead == 'p') ADVANCE(13);
      END_STATE();
    case 108:
      ACCEPT_TOKEN(sym_comment);
      if (lookahead != 0 &&
          lookahead != '\n') ADVANCE(108);
      END_STATE();
    default:
      return false;
  }
}

static bool ts_lex_keywords(TSLexer *lexer, TSStateId state) {
  START_LEXER();
  eof = lexer->eof(lexer);
  switch (state) {
    case 0:
      ACCEPT_TOKEN(ts_builtin_sym_end);
      END_STATE();
    default:
      return false;
  }
}

static const TSLexMode ts_lex_modes[STATE_COUNT] = {
  [0] = {.lex_state = 0, .external_lex_state = 1},
  [1] = {.lex_state = 53, .external_lex_state = 1},
  [2] = {.lex_state = 53, .external_lex_state = 1},
  [3] = {.lex_state = 0, .external_lex_state = 1},
  [4] = {.lex_state = 0, .external_lex_state = 1},
  [5] = {.lex_state = 0, .external_lex_state = 1},
  [6] = {.lex_state = 53, .external_lex_state = 1},
  [7] = {.lex_state = 53, .external_lex_state = 1},
  [8] = {.lex_state = 53, .external_lex_state = 1},
  [9] = {.lex_state = 53, .external_lex_state = 1},
  [10] = {.lex_state = 53, .external_lex_state = 1},
  [11] = {.lex_state = 53, .external_lex_state = 1},
  [12] = {.lex_state = 53, .external_lex_state = 1},
  [13] = {.lex_state = 53, .external_lex_state = 1},
  [14] = {.lex_state = 53, .external_lex_state = 1},
  [15] = {.lex_state = 53, .external_lex_state = 1},
  [16] = {.lex_state = 53, .external_lex_state = 1},
  [17] = {.lex_state = 53, .external_lex_state = 1},
  [18] = {.lex_state = 53, .external_lex_state = 1},
  [19] = {.lex_state = 53, .external_lex_state = 1},
  [20] = {.lex_state = 53, .external_lex_state = 1},
  [21] = {.lex_state = 53, .external_lex_state = 1},
  [22] = {.lex_state = 53, .external_lex_state = 1},
  [23] = {.lex_state = 53, .external_lex_state = 1},
  [24] = {.lex_state = 53, .external_lex_state = 1},
  [25] = {.lex_state = 53, .external_lex_state = 1},
  [26] = {.lex_state = 53, .external_lex_state = 1},
  [27] = {.lex_state = 53, .external_lex_state = 1},
  [28] = {.lex_state = 53, .external_lex_state = 1},
  [29] = {.lex_state = 53, .external_lex_state = 1},
  [30] = {.lex_state = 53, .external_lex_state = 1},
  [31] = {.lex_state = 53, .external_lex_state = 1},
  [32] = {.lex_state = 53, .external_lex_state = 1},
  [33] = {.lex_state = 53, .external_lex_state = 1},
  [34] = {.lex_state = 53, .external_lex_state = 1},
  [35] = {.lex_state = 53, .external_lex_state = 1},
  [36] = {.lex_state = 53, .external_lex_state = 1},
  [37] = {.lex_state = 53, .external_lex_state = 1},
  [38] = {.lex_state = 53, .external_lex_state = 1},
  [39] = {.lex_state = 53, .external_lex_state = 1},
  [40] = {.lex_state = 53, .external_lex_state = 1},
  [41] = {.lex_state = 53, .external_lex_state = 1},
  [42] = {.lex_state = 53, .external_lex_state = 1},
  [43] = {.lex_state = 53, .external_lex_state = 1},
  [44] = {.lex_state = 53, .external_lex_state = 1},
  [45] = {.lex_state = 53, .external_lex_state = 1},
  [46] = {.lex_state = 53, .external_lex_state = 1},
  [47] = {.lex_state = 53, .external_lex_state = 1},
  [48] = {.lex_state = 53, .external_lex_state = 1},
  [49] = {.lex_state = 53, .external_lex_state = 1},
  [50] = {.lex_state = 53, .external_lex_state = 1},
  [51] = {.lex_state = 0, .external_lex_state = 1},
  [52] = {.lex_state = 0, .external_lex_state = 1},
  [53] = {.lex_state = 0, .external_lex_state = 1},
  [54] = {.lex_state = 0, .external_lex_state = 1},
  [55] = {.lex_state = 0, .external_lex_state = 1},
  [56] = {.lex_state = 0, .external_lex_state = 1},
  [57] = {.lex_state = 0, .external_lex_state = 1},
  [58] = {.lex_state = 0, .external_lex_state = 1},
  [59] = {.lex_state = 0, .external_lex_state = 1},
  [60] = {.lex_state = 0, .external_lex_state = 1},
  [61] = {.lex_state = 0, .external_lex_state = 1},
  [62] = {.lex_state = 0, .external_lex_state = 1},
  [63] = {.lex_state = 0, .external_lex_state = 1},
  [64] = {.lex_state = 0, .external_lex_state = 1},
  [65] = {.lex_state = 0, .external_lex_state = 1},
  [66] = {.lex_state = 0, .external_lex_state = 1},
  [67] = {.lex_state = 6, .external_lex_state = 1},
  [68] = {.lex_state = 6, .external_lex_state = 1},
  [69] = {.lex_state = 6, .external_lex_state = 1},
  [70] = {.lex_state = 6, .external_lex_state = 1},
  [71] = {.lex_state = 6, .external_lex_state = 1},
  [72] = {.lex_state = 0, .external_lex_state = 1},
  [73] = {.lex_state = 0, .external_lex_state = 1},
  [74] = {.lex_state = 0, .external_lex_state = 1},
};

static const uint16_t ts_parse_table[LARGE_STATE_COUNT][SYMBOL_COUNT] = {
  [0] = {
    [ts_builtin_sym_end] = ACTIONS(1),
    [sym_symbol] = ACTIONS(1),
    [anon_sym_LPAREN] = ACTIONS(1),
    [anon_sym_RPAREN] = ACTIONS(1),
    [anon_sym_DOT] = ACTIONS(1),
    [anon_sym_POUND_LPAREN] = ACTIONS(1),
    [anon_sym_LBRACK] = ACTIONS(1),
    [anon_sym_RBRACK] = ACTIONS(1),
    [anon_sym_LBRACE] = ACTIONS(1),
    [anon_sym_RBRACE] = ACTIONS(1),
    [anon_sym_SQUOTE] = ACTIONS(1),
    [anon_sym_BQUOTE] = ACTIONS(1),
    [anon_sym_COMMA] = ACTIONS(1),
    [anon_sym_COMMA_AT] = ACTIONS(1),
    [anon_sym_AT] = ACTIONS(1),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(1),
    [sym_number] = ACTIONS(1),
    [sym_string] = ACTIONS(1),
    [sym_regex] = ACTIONS(1),
    [sym_shebang] = ACTIONS(1),
    [sym_keyword] = ACTIONS(1),
    [sym_boolean] = ACTIONS(1),
    [sym_character] = ACTIONS(1),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [1] = {
    [sym_source_file] = STATE(73),
    [sym__form] = STATE(10),
    [sym_list] = STATE(10),
    [sym_short_lambda] = STATE(10),
    [sym_vector] = STATE(10),
    [sym_hash_map] = STATE(10),
    [sym_quote] = STATE(10),
    [sym_quasiquote] = STATE(10),
    [sym_unquote] = STATE(10),
    [sym_unquote_splicing] = STATE(10),
    [sym_deref] = STATE(10),
    [sym_byte_vector] = STATE(10),
    [sym__atom] = STATE(10),
    [aux_sym_source_file_repeat1] = STATE(10),
    [ts_builtin_sym_end] = ACTIONS(5),
    [sym_symbol] = ACTIONS(7),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(7),
    [sym_string] = ACTIONS(29),
    [sym_regex] = ACTIONS(29),
    [sym_shebang] = ACTIONS(31),
    [sym_keyword] = ACTIONS(29),
    [sym_boolean] = ACTIONS(29),
    [sym_character] = ACTIONS(29),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [2] = {
    [sym__form] = STATE(2),
    [sym_list] = STATE(2),
    [sym_short_lambda] = STATE(2),
    [sym_vector] = STATE(2),
    [sym_hash_map] = STATE(2),
    [sym_quote] = STATE(2),
    [sym_quasiquote] = STATE(2),
    [sym_unquote] = STATE(2),
    [sym_unquote_splicing] = STATE(2),
    [sym_deref] = STATE(2),
    [sym_byte_vector] = STATE(2),
    [sym__atom] = STATE(2),
    [aux_sym_source_file_repeat1] = STATE(2),
    [ts_builtin_sym_end] = ACTIONS(33),
    [sym_symbol] = ACTIONS(35),
    [anon_sym_LPAREN] = ACTIONS(38),
    [anon_sym_RPAREN] = ACTIONS(33),
    [anon_sym_POUND_LPAREN] = ACTIONS(41),
    [anon_sym_LBRACK] = ACTIONS(44),
    [anon_sym_RBRACK] = ACTIONS(33),
    [anon_sym_LBRACE] = ACTIONS(47),
    [anon_sym_RBRACE] = ACTIONS(33),
    [anon_sym_SQUOTE] = ACTIONS(50),
    [anon_sym_BQUOTE] = ACTIONS(53),
    [anon_sym_COMMA] = ACTIONS(56),
    [anon_sym_COMMA_AT] = ACTIONS(59),
    [anon_sym_AT] = ACTIONS(62),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(65),
    [sym_number] = ACTIONS(35),
    [sym_string] = ACTIONS(68),
    [sym_regex] = ACTIONS(68),
    [sym_keyword] = ACTIONS(68),
    [sym_boolean] = ACTIONS(68),
    [sym_character] = ACTIONS(68),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [3] = {
    [sym__form] = STATE(4),
    [sym_list] = STATE(4),
    [sym_short_lambda] = STATE(4),
    [sym_vector] = STATE(4),
    [sym_hash_map] = STATE(4),
    [sym_quote] = STATE(4),
    [sym_quasiquote] = STATE(4),
    [sym_unquote] = STATE(4),
    [sym_unquote_splicing] = STATE(4),
    [sym_deref] = STATE(4),
    [sym_byte_vector] = STATE(4),
    [sym__atom] = STATE(4),
    [aux_sym_source_file_repeat1] = STATE(4),
    [sym_symbol] = ACTIONS(71),
    [anon_sym_LPAREN] = ACTIONS(73),
    [anon_sym_RPAREN] = ACTIONS(75),
    [anon_sym_DOT] = ACTIONS(77),
    [anon_sym_POUND_LPAREN] = ACTIONS(79),
    [anon_sym_LBRACK] = ACTIONS(81),
    [anon_sym_LBRACE] = ACTIONS(83),
    [anon_sym_SQUOTE] = ACTIONS(85),
    [anon_sym_BQUOTE] = ACTIONS(87),
    [anon_sym_COMMA] = ACTIONS(89),
    [anon_sym_COMMA_AT] = ACTIONS(91),
    [anon_sym_AT] = ACTIONS(93),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(95),
    [sym_number] = ACTIONS(71),
    [sym_string] = ACTIONS(97),
    [sym_regex] = ACTIONS(97),
    [sym_keyword] = ACTIONS(97),
    [sym_boolean] = ACTIONS(97),
    [sym_character] = ACTIONS(97),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [4] = {
    [sym__form] = STATE(4),
    [sym_list] = STATE(4),
    [sym_short_lambda] = STATE(4),
    [sym_vector] = STATE(4),
    [sym_hash_map] = STATE(4),
    [sym_quote] = STATE(4),
    [sym_quasiquote] = STATE(4),
    [sym_unquote] = STATE(4),
    [sym_unquote_splicing] = STATE(4),
    [sym_deref] = STATE(4),
    [sym_byte_vector] = STATE(4),
    [sym__atom] = STATE(4),
    [aux_sym_source_file_repeat1] = STATE(4),
    [sym_symbol] = ACTIONS(99),
    [anon_sym_LPAREN] = ACTIONS(102),
    [anon_sym_RPAREN] = ACTIONS(33),
    [anon_sym_DOT] = ACTIONS(105),
    [anon_sym_POUND_LPAREN] = ACTIONS(107),
    [anon_sym_LBRACK] = ACTIONS(110),
    [anon_sym_LBRACE] = ACTIONS(113),
    [anon_sym_SQUOTE] = ACTIONS(116),
    [anon_sym_BQUOTE] = ACTIONS(119),
    [anon_sym_COMMA] = ACTIONS(122),
    [anon_sym_COMMA_AT] = ACTIONS(125),
    [anon_sym_AT] = ACTIONS(128),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(131),
    [sym_number] = ACTIONS(99),
    [sym_string] = ACTIONS(134),
    [sym_regex] = ACTIONS(134),
    [sym_keyword] = ACTIONS(134),
    [sym_boolean] = ACTIONS(134),
    [sym_character] = ACTIONS(134),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [5] = {
    [sym__form] = STATE(4),
    [sym_list] = STATE(4),
    [sym_short_lambda] = STATE(4),
    [sym_vector] = STATE(4),
    [sym_hash_map] = STATE(4),
    [sym_quote] = STATE(4),
    [sym_quasiquote] = STATE(4),
    [sym_unquote] = STATE(4),
    [sym_unquote_splicing] = STATE(4),
    [sym_deref] = STATE(4),
    [sym_byte_vector] = STATE(4),
    [sym__atom] = STATE(4),
    [aux_sym_source_file_repeat1] = STATE(4),
    [sym_symbol] = ACTIONS(71),
    [anon_sym_LPAREN] = ACTIONS(73),
    [anon_sym_RPAREN] = ACTIONS(137),
    [anon_sym_DOT] = ACTIONS(139),
    [anon_sym_POUND_LPAREN] = ACTIONS(79),
    [anon_sym_LBRACK] = ACTIONS(81),
    [anon_sym_LBRACE] = ACTIONS(83),
    [anon_sym_SQUOTE] = ACTIONS(85),
    [anon_sym_BQUOTE] = ACTIONS(87),
    [anon_sym_COMMA] = ACTIONS(89),
    [anon_sym_COMMA_AT] = ACTIONS(91),
    [anon_sym_AT] = ACTIONS(93),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(95),
    [sym_number] = ACTIONS(71),
    [sym_string] = ACTIONS(97),
    [sym_regex] = ACTIONS(97),
    [sym_keyword] = ACTIONS(97),
    [sym_boolean] = ACTIONS(97),
    [sym_character] = ACTIONS(97),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [6] = {
    [sym__form] = STATE(12),
    [sym_list] = STATE(12),
    [sym_short_lambda] = STATE(12),
    [sym_vector] = STATE(12),
    [sym_hash_map] = STATE(12),
    [sym_quote] = STATE(12),
    [sym_quasiquote] = STATE(12),
    [sym_unquote] = STATE(12),
    [sym_unquote_splicing] = STATE(12),
    [sym_deref] = STATE(12),
    [sym_byte_vector] = STATE(12),
    [sym__atom] = STATE(12),
    [aux_sym_source_file_repeat1] = STATE(12),
    [sym_symbol] = ACTIONS(141),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_RPAREN] = ACTIONS(143),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(141),
    [sym_string] = ACTIONS(145),
    [sym_regex] = ACTIONS(145),
    [sym_keyword] = ACTIONS(145),
    [sym_boolean] = ACTIONS(145),
    [sym_character] = ACTIONS(145),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [7] = {
    [sym__form] = STATE(13),
    [sym_list] = STATE(13),
    [sym_short_lambda] = STATE(13),
    [sym_vector] = STATE(13),
    [sym_hash_map] = STATE(13),
    [sym_quote] = STATE(13),
    [sym_quasiquote] = STATE(13),
    [sym_unquote] = STATE(13),
    [sym_unquote_splicing] = STATE(13),
    [sym_deref] = STATE(13),
    [sym_byte_vector] = STATE(13),
    [sym__atom] = STATE(13),
    [aux_sym_source_file_repeat1] = STATE(13),
    [sym_symbol] = ACTIONS(147),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_RBRACK] = ACTIONS(149),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(147),
    [sym_string] = ACTIONS(151),
    [sym_regex] = ACTIONS(151),
    [sym_keyword] = ACTIONS(151),
    [sym_boolean] = ACTIONS(151),
    [sym_character] = ACTIONS(151),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [8] = {
    [sym__form] = STATE(14),
    [sym_list] = STATE(14),
    [sym_short_lambda] = STATE(14),
    [sym_vector] = STATE(14),
    [sym_hash_map] = STATE(14),
    [sym_quote] = STATE(14),
    [sym_quasiquote] = STATE(14),
    [sym_unquote] = STATE(14),
    [sym_unquote_splicing] = STATE(14),
    [sym_deref] = STATE(14),
    [sym_byte_vector] = STATE(14),
    [sym__atom] = STATE(14),
    [aux_sym_source_file_repeat1] = STATE(14),
    [sym_symbol] = ACTIONS(153),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_RBRACE] = ACTIONS(155),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(153),
    [sym_string] = ACTIONS(157),
    [sym_regex] = ACTIONS(157),
    [sym_keyword] = ACTIONS(157),
    [sym_boolean] = ACTIONS(157),
    [sym_character] = ACTIONS(157),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [9] = {
    [sym__form] = STATE(18),
    [sym_list] = STATE(18),
    [sym_short_lambda] = STATE(18),
    [sym_vector] = STATE(18),
    [sym_hash_map] = STATE(18),
    [sym_quote] = STATE(18),
    [sym_quasiquote] = STATE(18),
    [sym_unquote] = STATE(18),
    [sym_unquote_splicing] = STATE(18),
    [sym_deref] = STATE(18),
    [sym_byte_vector] = STATE(18),
    [sym__atom] = STATE(18),
    [aux_sym_source_file_repeat1] = STATE(18),
    [ts_builtin_sym_end] = ACTIONS(159),
    [sym_symbol] = ACTIONS(161),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(161),
    [sym_string] = ACTIONS(163),
    [sym_regex] = ACTIONS(163),
    [sym_keyword] = ACTIONS(163),
    [sym_boolean] = ACTIONS(163),
    [sym_character] = ACTIONS(163),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [10] = {
    [sym__form] = STATE(2),
    [sym_list] = STATE(2),
    [sym_short_lambda] = STATE(2),
    [sym_vector] = STATE(2),
    [sym_hash_map] = STATE(2),
    [sym_quote] = STATE(2),
    [sym_quasiquote] = STATE(2),
    [sym_unquote] = STATE(2),
    [sym_unquote_splicing] = STATE(2),
    [sym_deref] = STATE(2),
    [sym_byte_vector] = STATE(2),
    [sym__atom] = STATE(2),
    [aux_sym_source_file_repeat1] = STATE(2),
    [ts_builtin_sym_end] = ACTIONS(159),
    [sym_symbol] = ACTIONS(165),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(165),
    [sym_string] = ACTIONS(167),
    [sym_regex] = ACTIONS(167),
    [sym_keyword] = ACTIONS(167),
    [sym_boolean] = ACTIONS(167),
    [sym_character] = ACTIONS(167),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [11] = {
    [sym__form] = STATE(3),
    [sym_list] = STATE(3),
    [sym_short_lambda] = STATE(3),
    [sym_vector] = STATE(3),
    [sym_hash_map] = STATE(3),
    [sym_quote] = STATE(3),
    [sym_quasiquote] = STATE(3),
    [sym_unquote] = STATE(3),
    [sym_unquote_splicing] = STATE(3),
    [sym_deref] = STATE(3),
    [sym_byte_vector] = STATE(3),
    [sym__atom] = STATE(3),
    [aux_sym_source_file_repeat1] = STATE(3),
    [sym_symbol] = ACTIONS(169),
    [anon_sym_LPAREN] = ACTIONS(73),
    [anon_sym_RPAREN] = ACTIONS(171),
    [anon_sym_POUND_LPAREN] = ACTIONS(79),
    [anon_sym_LBRACK] = ACTIONS(81),
    [anon_sym_LBRACE] = ACTIONS(83),
    [anon_sym_SQUOTE] = ACTIONS(85),
    [anon_sym_BQUOTE] = ACTIONS(87),
    [anon_sym_COMMA] = ACTIONS(89),
    [anon_sym_COMMA_AT] = ACTIONS(91),
    [anon_sym_AT] = ACTIONS(93),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(95),
    [sym_number] = ACTIONS(169),
    [sym_string] = ACTIONS(173),
    [sym_regex] = ACTIONS(173),
    [sym_keyword] = ACTIONS(173),
    [sym_boolean] = ACTIONS(173),
    [sym_character] = ACTIONS(173),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [12] = {
    [sym__form] = STATE(2),
    [sym_list] = STATE(2),
    [sym_short_lambda] = STATE(2),
    [sym_vector] = STATE(2),
    [sym_hash_map] = STATE(2),
    [sym_quote] = STATE(2),
    [sym_quasiquote] = STATE(2),
    [sym_unquote] = STATE(2),
    [sym_unquote_splicing] = STATE(2),
    [sym_deref] = STATE(2),
    [sym_byte_vector] = STATE(2),
    [sym__atom] = STATE(2),
    [aux_sym_source_file_repeat1] = STATE(2),
    [sym_symbol] = ACTIONS(165),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_RPAREN] = ACTIONS(175),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(165),
    [sym_string] = ACTIONS(167),
    [sym_regex] = ACTIONS(167),
    [sym_keyword] = ACTIONS(167),
    [sym_boolean] = ACTIONS(167),
    [sym_character] = ACTIONS(167),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [13] = {
    [sym__form] = STATE(2),
    [sym_list] = STATE(2),
    [sym_short_lambda] = STATE(2),
    [sym_vector] = STATE(2),
    [sym_hash_map] = STATE(2),
    [sym_quote] = STATE(2),
    [sym_quasiquote] = STATE(2),
    [sym_unquote] = STATE(2),
    [sym_unquote_splicing] = STATE(2),
    [sym_deref] = STATE(2),
    [sym_byte_vector] = STATE(2),
    [sym__atom] = STATE(2),
    [aux_sym_source_file_repeat1] = STATE(2),
    [sym_symbol] = ACTIONS(165),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_RBRACK] = ACTIONS(177),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(165),
    [sym_string] = ACTIONS(167),
    [sym_regex] = ACTIONS(167),
    [sym_keyword] = ACTIONS(167),
    [sym_boolean] = ACTIONS(167),
    [sym_character] = ACTIONS(167),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [14] = {
    [sym__form] = STATE(2),
    [sym_list] = STATE(2),
    [sym_short_lambda] = STATE(2),
    [sym_vector] = STATE(2),
    [sym_hash_map] = STATE(2),
    [sym_quote] = STATE(2),
    [sym_quasiquote] = STATE(2),
    [sym_unquote] = STATE(2),
    [sym_unquote_splicing] = STATE(2),
    [sym_deref] = STATE(2),
    [sym_byte_vector] = STATE(2),
    [sym__atom] = STATE(2),
    [aux_sym_source_file_repeat1] = STATE(2),
    [sym_symbol] = ACTIONS(165),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_RBRACE] = ACTIONS(179),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(165),
    [sym_string] = ACTIONS(167),
    [sym_regex] = ACTIONS(167),
    [sym_keyword] = ACTIONS(167),
    [sym_boolean] = ACTIONS(167),
    [sym_character] = ACTIONS(167),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [15] = {
    [sym__form] = STATE(5),
    [sym_list] = STATE(5),
    [sym_short_lambda] = STATE(5),
    [sym_vector] = STATE(5),
    [sym_hash_map] = STATE(5),
    [sym_quote] = STATE(5),
    [sym_quasiquote] = STATE(5),
    [sym_unquote] = STATE(5),
    [sym_unquote_splicing] = STATE(5),
    [sym_deref] = STATE(5),
    [sym_byte_vector] = STATE(5),
    [sym__atom] = STATE(5),
    [aux_sym_source_file_repeat1] = STATE(5),
    [sym_symbol] = ACTIONS(181),
    [anon_sym_LPAREN] = ACTIONS(73),
    [anon_sym_RPAREN] = ACTIONS(183),
    [anon_sym_POUND_LPAREN] = ACTIONS(79),
    [anon_sym_LBRACK] = ACTIONS(81),
    [anon_sym_LBRACE] = ACTIONS(83),
    [anon_sym_SQUOTE] = ACTIONS(85),
    [anon_sym_BQUOTE] = ACTIONS(87),
    [anon_sym_COMMA] = ACTIONS(89),
    [anon_sym_COMMA_AT] = ACTIONS(91),
    [anon_sym_AT] = ACTIONS(93),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(95),
    [sym_number] = ACTIONS(181),
    [sym_string] = ACTIONS(185),
    [sym_regex] = ACTIONS(185),
    [sym_keyword] = ACTIONS(185),
    [sym_boolean] = ACTIONS(185),
    [sym_character] = ACTIONS(185),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [16] = {
    [sym__form] = STATE(21),
    [sym_list] = STATE(21),
    [sym_short_lambda] = STATE(21),
    [sym_vector] = STATE(21),
    [sym_hash_map] = STATE(21),
    [sym_quote] = STATE(21),
    [sym_quasiquote] = STATE(21),
    [sym_unquote] = STATE(21),
    [sym_unquote_splicing] = STATE(21),
    [sym_deref] = STATE(21),
    [sym_byte_vector] = STATE(21),
    [sym__atom] = STATE(21),
    [aux_sym_source_file_repeat1] = STATE(21),
    [sym_symbol] = ACTIONS(187),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_RBRACK] = ACTIONS(189),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(187),
    [sym_string] = ACTIONS(191),
    [sym_regex] = ACTIONS(191),
    [sym_keyword] = ACTIONS(191),
    [sym_boolean] = ACTIONS(191),
    [sym_character] = ACTIONS(191),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [17] = {
    [sym__form] = STATE(22),
    [sym_list] = STATE(22),
    [sym_short_lambda] = STATE(22),
    [sym_vector] = STATE(22),
    [sym_hash_map] = STATE(22),
    [sym_quote] = STATE(22),
    [sym_quasiquote] = STATE(22),
    [sym_unquote] = STATE(22),
    [sym_unquote_splicing] = STATE(22),
    [sym_deref] = STATE(22),
    [sym_byte_vector] = STATE(22),
    [sym__atom] = STATE(22),
    [aux_sym_source_file_repeat1] = STATE(22),
    [sym_symbol] = ACTIONS(193),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_RBRACE] = ACTIONS(195),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(193),
    [sym_string] = ACTIONS(197),
    [sym_regex] = ACTIONS(197),
    [sym_keyword] = ACTIONS(197),
    [sym_boolean] = ACTIONS(197),
    [sym_character] = ACTIONS(197),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [18] = {
    [sym__form] = STATE(2),
    [sym_list] = STATE(2),
    [sym_short_lambda] = STATE(2),
    [sym_vector] = STATE(2),
    [sym_hash_map] = STATE(2),
    [sym_quote] = STATE(2),
    [sym_quasiquote] = STATE(2),
    [sym_unquote] = STATE(2),
    [sym_unquote_splicing] = STATE(2),
    [sym_deref] = STATE(2),
    [sym_byte_vector] = STATE(2),
    [sym__atom] = STATE(2),
    [aux_sym_source_file_repeat1] = STATE(2),
    [ts_builtin_sym_end] = ACTIONS(199),
    [sym_symbol] = ACTIONS(165),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(165),
    [sym_string] = ACTIONS(167),
    [sym_regex] = ACTIONS(167),
    [sym_keyword] = ACTIONS(167),
    [sym_boolean] = ACTIONS(167),
    [sym_character] = ACTIONS(167),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [19] = {
    [sym__form] = STATE(20),
    [sym_list] = STATE(20),
    [sym_short_lambda] = STATE(20),
    [sym_vector] = STATE(20),
    [sym_hash_map] = STATE(20),
    [sym_quote] = STATE(20),
    [sym_quasiquote] = STATE(20),
    [sym_unquote] = STATE(20),
    [sym_unquote_splicing] = STATE(20),
    [sym_deref] = STATE(20),
    [sym_byte_vector] = STATE(20),
    [sym__atom] = STATE(20),
    [aux_sym_source_file_repeat1] = STATE(20),
    [sym_symbol] = ACTIONS(201),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_RPAREN] = ACTIONS(203),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(201),
    [sym_string] = ACTIONS(205),
    [sym_regex] = ACTIONS(205),
    [sym_keyword] = ACTIONS(205),
    [sym_boolean] = ACTIONS(205),
    [sym_character] = ACTIONS(205),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [20] = {
    [sym__form] = STATE(2),
    [sym_list] = STATE(2),
    [sym_short_lambda] = STATE(2),
    [sym_vector] = STATE(2),
    [sym_hash_map] = STATE(2),
    [sym_quote] = STATE(2),
    [sym_quasiquote] = STATE(2),
    [sym_unquote] = STATE(2),
    [sym_unquote_splicing] = STATE(2),
    [sym_deref] = STATE(2),
    [sym_byte_vector] = STATE(2),
    [sym__atom] = STATE(2),
    [aux_sym_source_file_repeat1] = STATE(2),
    [sym_symbol] = ACTIONS(165),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_RPAREN] = ACTIONS(207),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(165),
    [sym_string] = ACTIONS(167),
    [sym_regex] = ACTIONS(167),
    [sym_keyword] = ACTIONS(167),
    [sym_boolean] = ACTIONS(167),
    [sym_character] = ACTIONS(167),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [21] = {
    [sym__form] = STATE(2),
    [sym_list] = STATE(2),
    [sym_short_lambda] = STATE(2),
    [sym_vector] = STATE(2),
    [sym_hash_map] = STATE(2),
    [sym_quote] = STATE(2),
    [sym_quasiquote] = STATE(2),
    [sym_unquote] = STATE(2),
    [sym_unquote_splicing] = STATE(2),
    [sym_deref] = STATE(2),
    [sym_byte_vector] = STATE(2),
    [sym__atom] = STATE(2),
    [aux_sym_source_file_repeat1] = STATE(2),
    [sym_symbol] = ACTIONS(165),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_RBRACK] = ACTIONS(209),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(165),
    [sym_string] = ACTIONS(167),
    [sym_regex] = ACTIONS(167),
    [sym_keyword] = ACTIONS(167),
    [sym_boolean] = ACTIONS(167),
    [sym_character] = ACTIONS(167),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [22] = {
    [sym__form] = STATE(2),
    [sym_list] = STATE(2),
    [sym_short_lambda] = STATE(2),
    [sym_vector] = STATE(2),
    [sym_hash_map] = STATE(2),
    [sym_quote] = STATE(2),
    [sym_quasiquote] = STATE(2),
    [sym_unquote] = STATE(2),
    [sym_unquote_splicing] = STATE(2),
    [sym_deref] = STATE(2),
    [sym_byte_vector] = STATE(2),
    [sym__atom] = STATE(2),
    [aux_sym_source_file_repeat1] = STATE(2),
    [sym_symbol] = ACTIONS(165),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_RBRACE] = ACTIONS(211),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(165),
    [sym_string] = ACTIONS(167),
    [sym_regex] = ACTIONS(167),
    [sym_keyword] = ACTIONS(167),
    [sym_boolean] = ACTIONS(167),
    [sym_character] = ACTIONS(167),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [23] = {
    [sym__form] = STATE(62),
    [sym_list] = STATE(62),
    [sym_short_lambda] = STATE(62),
    [sym_vector] = STATE(62),
    [sym_hash_map] = STATE(62),
    [sym_quote] = STATE(62),
    [sym_quasiquote] = STATE(62),
    [sym_unquote] = STATE(62),
    [sym_unquote_splicing] = STATE(62),
    [sym_deref] = STATE(62),
    [sym_byte_vector] = STATE(62),
    [sym__atom] = STATE(62),
    [sym_symbol] = ACTIONS(213),
    [anon_sym_LPAREN] = ACTIONS(73),
    [anon_sym_POUND_LPAREN] = ACTIONS(79),
    [anon_sym_LBRACK] = ACTIONS(81),
    [anon_sym_LBRACE] = ACTIONS(83),
    [anon_sym_SQUOTE] = ACTIONS(85),
    [anon_sym_BQUOTE] = ACTIONS(87),
    [anon_sym_COMMA] = ACTIONS(89),
    [anon_sym_COMMA_AT] = ACTIONS(91),
    [anon_sym_AT] = ACTIONS(93),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(95),
    [sym_number] = ACTIONS(213),
    [sym_string] = ACTIONS(215),
    [sym_regex] = ACTIONS(215),
    [sym_keyword] = ACTIONS(215),
    [sym_boolean] = ACTIONS(215),
    [sym_character] = ACTIONS(215),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [24] = {
    [sym__form] = STATE(74),
    [sym_list] = STATE(74),
    [sym_short_lambda] = STATE(74),
    [sym_vector] = STATE(74),
    [sym_hash_map] = STATE(74),
    [sym_quote] = STATE(74),
    [sym_quasiquote] = STATE(74),
    [sym_unquote] = STATE(74),
    [sym_unquote_splicing] = STATE(74),
    [sym_deref] = STATE(74),
    [sym_byte_vector] = STATE(74),
    [sym__atom] = STATE(74),
    [sym_symbol] = ACTIONS(217),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(217),
    [sym_string] = ACTIONS(219),
    [sym_regex] = ACTIONS(219),
    [sym_keyword] = ACTIONS(219),
    [sym_boolean] = ACTIONS(219),
    [sym_character] = ACTIONS(219),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [25] = {
    [sym__form] = STATE(48),
    [sym_list] = STATE(48),
    [sym_short_lambda] = STATE(48),
    [sym_vector] = STATE(48),
    [sym_hash_map] = STATE(48),
    [sym_quote] = STATE(48),
    [sym_quasiquote] = STATE(48),
    [sym_unquote] = STATE(48),
    [sym_unquote_splicing] = STATE(48),
    [sym_deref] = STATE(48),
    [sym_byte_vector] = STATE(48),
    [sym__atom] = STATE(48),
    [sym_symbol] = ACTIONS(221),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(221),
    [sym_string] = ACTIONS(223),
    [sym_regex] = ACTIONS(223),
    [sym_keyword] = ACTIONS(223),
    [sym_boolean] = ACTIONS(223),
    [sym_character] = ACTIONS(223),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [26] = {
    [sym__form] = STATE(72),
    [sym_list] = STATE(72),
    [sym_short_lambda] = STATE(72),
    [sym_vector] = STATE(72),
    [sym_hash_map] = STATE(72),
    [sym_quote] = STATE(72),
    [sym_quasiquote] = STATE(72),
    [sym_unquote] = STATE(72),
    [sym_unquote_splicing] = STATE(72),
    [sym_deref] = STATE(72),
    [sym_byte_vector] = STATE(72),
    [sym__atom] = STATE(72),
    [sym_symbol] = ACTIONS(225),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(225),
    [sym_string] = ACTIONS(227),
    [sym_regex] = ACTIONS(227),
    [sym_keyword] = ACTIONS(227),
    [sym_boolean] = ACTIONS(227),
    [sym_character] = ACTIONS(227),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [27] = {
    [sym__form] = STATE(35),
    [sym_list] = STATE(35),
    [sym_short_lambda] = STATE(35),
    [sym_vector] = STATE(35),
    [sym_hash_map] = STATE(35),
    [sym_quote] = STATE(35),
    [sym_quasiquote] = STATE(35),
    [sym_unquote] = STATE(35),
    [sym_unquote_splicing] = STATE(35),
    [sym_deref] = STATE(35),
    [sym_byte_vector] = STATE(35),
    [sym__atom] = STATE(35),
    [sym_symbol] = ACTIONS(229),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(229),
    [sym_string] = ACTIONS(231),
    [sym_regex] = ACTIONS(231),
    [sym_keyword] = ACTIONS(231),
    [sym_boolean] = ACTIONS(231),
    [sym_character] = ACTIONS(231),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [28] = {
    [sym__form] = STATE(50),
    [sym_list] = STATE(50),
    [sym_short_lambda] = STATE(50),
    [sym_vector] = STATE(50),
    [sym_hash_map] = STATE(50),
    [sym_quote] = STATE(50),
    [sym_quasiquote] = STATE(50),
    [sym_unquote] = STATE(50),
    [sym_unquote_splicing] = STATE(50),
    [sym_deref] = STATE(50),
    [sym_byte_vector] = STATE(50),
    [sym__atom] = STATE(50),
    [sym_symbol] = ACTIONS(233),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(233),
    [sym_string] = ACTIONS(235),
    [sym_regex] = ACTIONS(235),
    [sym_keyword] = ACTIONS(235),
    [sym_boolean] = ACTIONS(235),
    [sym_character] = ACTIONS(235),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [29] = {
    [sym__form] = STATE(41),
    [sym_list] = STATE(41),
    [sym_short_lambda] = STATE(41),
    [sym_vector] = STATE(41),
    [sym_hash_map] = STATE(41),
    [sym_quote] = STATE(41),
    [sym_quasiquote] = STATE(41),
    [sym_unquote] = STATE(41),
    [sym_unquote_splicing] = STATE(41),
    [sym_deref] = STATE(41),
    [sym_byte_vector] = STATE(41),
    [sym__atom] = STATE(41),
    [sym_symbol] = ACTIONS(237),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(237),
    [sym_string] = ACTIONS(239),
    [sym_regex] = ACTIONS(239),
    [sym_keyword] = ACTIONS(239),
    [sym_boolean] = ACTIONS(239),
    [sym_character] = ACTIONS(239),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [30] = {
    [sym__form] = STATE(61),
    [sym_list] = STATE(61),
    [sym_short_lambda] = STATE(61),
    [sym_vector] = STATE(61),
    [sym_hash_map] = STATE(61),
    [sym_quote] = STATE(61),
    [sym_quasiquote] = STATE(61),
    [sym_unquote] = STATE(61),
    [sym_unquote_splicing] = STATE(61),
    [sym_deref] = STATE(61),
    [sym_byte_vector] = STATE(61),
    [sym__atom] = STATE(61),
    [sym_symbol] = ACTIONS(241),
    [anon_sym_LPAREN] = ACTIONS(73),
    [anon_sym_POUND_LPAREN] = ACTIONS(79),
    [anon_sym_LBRACK] = ACTIONS(81),
    [anon_sym_LBRACE] = ACTIONS(83),
    [anon_sym_SQUOTE] = ACTIONS(85),
    [anon_sym_BQUOTE] = ACTIONS(87),
    [anon_sym_COMMA] = ACTIONS(89),
    [anon_sym_COMMA_AT] = ACTIONS(91),
    [anon_sym_AT] = ACTIONS(93),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(95),
    [sym_number] = ACTIONS(241),
    [sym_string] = ACTIONS(243),
    [sym_regex] = ACTIONS(243),
    [sym_keyword] = ACTIONS(243),
    [sym_boolean] = ACTIONS(243),
    [sym_character] = ACTIONS(243),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [31] = {
    [sym__form] = STATE(64),
    [sym_list] = STATE(64),
    [sym_short_lambda] = STATE(64),
    [sym_vector] = STATE(64),
    [sym_hash_map] = STATE(64),
    [sym_quote] = STATE(64),
    [sym_quasiquote] = STATE(64),
    [sym_unquote] = STATE(64),
    [sym_unquote_splicing] = STATE(64),
    [sym_deref] = STATE(64),
    [sym_byte_vector] = STATE(64),
    [sym__atom] = STATE(64),
    [sym_symbol] = ACTIONS(245),
    [anon_sym_LPAREN] = ACTIONS(73),
    [anon_sym_POUND_LPAREN] = ACTIONS(79),
    [anon_sym_LBRACK] = ACTIONS(81),
    [anon_sym_LBRACE] = ACTIONS(83),
    [anon_sym_SQUOTE] = ACTIONS(85),
    [anon_sym_BQUOTE] = ACTIONS(87),
    [anon_sym_COMMA] = ACTIONS(89),
    [anon_sym_COMMA_AT] = ACTIONS(91),
    [anon_sym_AT] = ACTIONS(93),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(95),
    [sym_number] = ACTIONS(245),
    [sym_string] = ACTIONS(247),
    [sym_regex] = ACTIONS(247),
    [sym_keyword] = ACTIONS(247),
    [sym_boolean] = ACTIONS(247),
    [sym_character] = ACTIONS(247),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [32] = {
    [sym__form] = STATE(65),
    [sym_list] = STATE(65),
    [sym_short_lambda] = STATE(65),
    [sym_vector] = STATE(65),
    [sym_hash_map] = STATE(65),
    [sym_quote] = STATE(65),
    [sym_quasiquote] = STATE(65),
    [sym_unquote] = STATE(65),
    [sym_unquote_splicing] = STATE(65),
    [sym_deref] = STATE(65),
    [sym_byte_vector] = STATE(65),
    [sym__atom] = STATE(65),
    [sym_symbol] = ACTIONS(249),
    [anon_sym_LPAREN] = ACTIONS(73),
    [anon_sym_POUND_LPAREN] = ACTIONS(79),
    [anon_sym_LBRACK] = ACTIONS(81),
    [anon_sym_LBRACE] = ACTIONS(83),
    [anon_sym_SQUOTE] = ACTIONS(85),
    [anon_sym_BQUOTE] = ACTIONS(87),
    [anon_sym_COMMA] = ACTIONS(89),
    [anon_sym_COMMA_AT] = ACTIONS(91),
    [anon_sym_AT] = ACTIONS(93),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(95),
    [sym_number] = ACTIONS(249),
    [sym_string] = ACTIONS(251),
    [sym_regex] = ACTIONS(251),
    [sym_keyword] = ACTIONS(251),
    [sym_boolean] = ACTIONS(251),
    [sym_character] = ACTIONS(251),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [33] = {
    [sym__form] = STATE(63),
    [sym_list] = STATE(63),
    [sym_short_lambda] = STATE(63),
    [sym_vector] = STATE(63),
    [sym_hash_map] = STATE(63),
    [sym_quote] = STATE(63),
    [sym_quasiquote] = STATE(63),
    [sym_unquote] = STATE(63),
    [sym_unquote_splicing] = STATE(63),
    [sym_deref] = STATE(63),
    [sym_byte_vector] = STATE(63),
    [sym__atom] = STATE(63),
    [sym_symbol] = ACTIONS(253),
    [anon_sym_LPAREN] = ACTIONS(73),
    [anon_sym_POUND_LPAREN] = ACTIONS(79),
    [anon_sym_LBRACK] = ACTIONS(81),
    [anon_sym_LBRACE] = ACTIONS(83),
    [anon_sym_SQUOTE] = ACTIONS(85),
    [anon_sym_BQUOTE] = ACTIONS(87),
    [anon_sym_COMMA] = ACTIONS(89),
    [anon_sym_COMMA_AT] = ACTIONS(91),
    [anon_sym_AT] = ACTIONS(93),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(95),
    [sym_number] = ACTIONS(253),
    [sym_string] = ACTIONS(255),
    [sym_regex] = ACTIONS(255),
    [sym_keyword] = ACTIONS(255),
    [sym_boolean] = ACTIONS(255),
    [sym_character] = ACTIONS(255),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [34] = {
    [sym__form] = STATE(44),
    [sym_list] = STATE(44),
    [sym_short_lambda] = STATE(44),
    [sym_vector] = STATE(44),
    [sym_hash_map] = STATE(44),
    [sym_quote] = STATE(44),
    [sym_quasiquote] = STATE(44),
    [sym_unquote] = STATE(44),
    [sym_unquote_splicing] = STATE(44),
    [sym_deref] = STATE(44),
    [sym_byte_vector] = STATE(44),
    [sym__atom] = STATE(44),
    [sym_symbol] = ACTIONS(257),
    [anon_sym_LPAREN] = ACTIONS(9),
    [anon_sym_POUND_LPAREN] = ACTIONS(11),
    [anon_sym_LBRACK] = ACTIONS(13),
    [anon_sym_LBRACE] = ACTIONS(15),
    [anon_sym_SQUOTE] = ACTIONS(17),
    [anon_sym_BQUOTE] = ACTIONS(19),
    [anon_sym_COMMA] = ACTIONS(21),
    [anon_sym_COMMA_AT] = ACTIONS(23),
    [anon_sym_AT] = ACTIONS(25),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(27),
    [sym_number] = ACTIONS(257),
    [sym_string] = ACTIONS(259),
    [sym_regex] = ACTIONS(259),
    [sym_keyword] = ACTIONS(259),
    [sym_boolean] = ACTIONS(259),
    [sym_character] = ACTIONS(259),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [35] = {
    [ts_builtin_sym_end] = ACTIONS(261),
    [sym_symbol] = ACTIONS(263),
    [anon_sym_LPAREN] = ACTIONS(261),
    [anon_sym_RPAREN] = ACTIONS(261),
    [anon_sym_POUND_LPAREN] = ACTIONS(261),
    [anon_sym_LBRACK] = ACTIONS(261),
    [anon_sym_RBRACK] = ACTIONS(261),
    [anon_sym_LBRACE] = ACTIONS(261),
    [anon_sym_RBRACE] = ACTIONS(261),
    [anon_sym_SQUOTE] = ACTIONS(261),
    [anon_sym_BQUOTE] = ACTIONS(261),
    [anon_sym_COMMA] = ACTIONS(263),
    [anon_sym_COMMA_AT] = ACTIONS(261),
    [anon_sym_AT] = ACTIONS(261),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(261),
    [sym_number] = ACTIONS(263),
    [sym_string] = ACTIONS(261),
    [sym_regex] = ACTIONS(261),
    [sym_keyword] = ACTIONS(261),
    [sym_boolean] = ACTIONS(261),
    [sym_character] = ACTIONS(261),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [36] = {
    [ts_builtin_sym_end] = ACTIONS(265),
    [sym_symbol] = ACTIONS(267),
    [anon_sym_LPAREN] = ACTIONS(265),
    [anon_sym_RPAREN] = ACTIONS(265),
    [anon_sym_POUND_LPAREN] = ACTIONS(265),
    [anon_sym_LBRACK] = ACTIONS(265),
    [anon_sym_RBRACK] = ACTIONS(265),
    [anon_sym_LBRACE] = ACTIONS(265),
    [anon_sym_RBRACE] = ACTIONS(265),
    [anon_sym_SQUOTE] = ACTIONS(265),
    [anon_sym_BQUOTE] = ACTIONS(265),
    [anon_sym_COMMA] = ACTIONS(267),
    [anon_sym_COMMA_AT] = ACTIONS(265),
    [anon_sym_AT] = ACTIONS(265),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(265),
    [sym_number] = ACTIONS(267),
    [sym_string] = ACTIONS(265),
    [sym_regex] = ACTIONS(265),
    [sym_keyword] = ACTIONS(265),
    [sym_boolean] = ACTIONS(265),
    [sym_character] = ACTIONS(265),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [37] = {
    [ts_builtin_sym_end] = ACTIONS(269),
    [sym_symbol] = ACTIONS(271),
    [anon_sym_LPAREN] = ACTIONS(269),
    [anon_sym_RPAREN] = ACTIONS(269),
    [anon_sym_POUND_LPAREN] = ACTIONS(269),
    [anon_sym_LBRACK] = ACTIONS(269),
    [anon_sym_RBRACK] = ACTIONS(269),
    [anon_sym_LBRACE] = ACTIONS(269),
    [anon_sym_RBRACE] = ACTIONS(269),
    [anon_sym_SQUOTE] = ACTIONS(269),
    [anon_sym_BQUOTE] = ACTIONS(269),
    [anon_sym_COMMA] = ACTIONS(271),
    [anon_sym_COMMA_AT] = ACTIONS(269),
    [anon_sym_AT] = ACTIONS(269),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(269),
    [sym_number] = ACTIONS(271),
    [sym_string] = ACTIONS(269),
    [sym_regex] = ACTIONS(269),
    [sym_keyword] = ACTIONS(269),
    [sym_boolean] = ACTIONS(269),
    [sym_character] = ACTIONS(269),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [38] = {
    [ts_builtin_sym_end] = ACTIONS(273),
    [sym_symbol] = ACTIONS(275),
    [anon_sym_LPAREN] = ACTIONS(273),
    [anon_sym_RPAREN] = ACTIONS(273),
    [anon_sym_POUND_LPAREN] = ACTIONS(273),
    [anon_sym_LBRACK] = ACTIONS(273),
    [anon_sym_RBRACK] = ACTIONS(273),
    [anon_sym_LBRACE] = ACTIONS(273),
    [anon_sym_RBRACE] = ACTIONS(273),
    [anon_sym_SQUOTE] = ACTIONS(273),
    [anon_sym_BQUOTE] = ACTIONS(273),
    [anon_sym_COMMA] = ACTIONS(275),
    [anon_sym_COMMA_AT] = ACTIONS(273),
    [anon_sym_AT] = ACTIONS(273),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(273),
    [sym_number] = ACTIONS(275),
    [sym_string] = ACTIONS(273),
    [sym_regex] = ACTIONS(273),
    [sym_keyword] = ACTIONS(273),
    [sym_boolean] = ACTIONS(273),
    [sym_character] = ACTIONS(273),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [39] = {
    [ts_builtin_sym_end] = ACTIONS(277),
    [sym_symbol] = ACTIONS(279),
    [anon_sym_LPAREN] = ACTIONS(277),
    [anon_sym_RPAREN] = ACTIONS(277),
    [anon_sym_POUND_LPAREN] = ACTIONS(277),
    [anon_sym_LBRACK] = ACTIONS(277),
    [anon_sym_RBRACK] = ACTIONS(277),
    [anon_sym_LBRACE] = ACTIONS(277),
    [anon_sym_RBRACE] = ACTIONS(277),
    [anon_sym_SQUOTE] = ACTIONS(277),
    [anon_sym_BQUOTE] = ACTIONS(277),
    [anon_sym_COMMA] = ACTIONS(279),
    [anon_sym_COMMA_AT] = ACTIONS(277),
    [anon_sym_AT] = ACTIONS(277),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(277),
    [sym_number] = ACTIONS(279),
    [sym_string] = ACTIONS(277),
    [sym_regex] = ACTIONS(277),
    [sym_keyword] = ACTIONS(277),
    [sym_boolean] = ACTIONS(277),
    [sym_character] = ACTIONS(277),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [40] = {
    [ts_builtin_sym_end] = ACTIONS(281),
    [sym_symbol] = ACTIONS(283),
    [anon_sym_LPAREN] = ACTIONS(281),
    [anon_sym_RPAREN] = ACTIONS(281),
    [anon_sym_POUND_LPAREN] = ACTIONS(281),
    [anon_sym_LBRACK] = ACTIONS(281),
    [anon_sym_RBRACK] = ACTIONS(281),
    [anon_sym_LBRACE] = ACTIONS(281),
    [anon_sym_RBRACE] = ACTIONS(281),
    [anon_sym_SQUOTE] = ACTIONS(281),
    [anon_sym_BQUOTE] = ACTIONS(281),
    [anon_sym_COMMA] = ACTIONS(283),
    [anon_sym_COMMA_AT] = ACTIONS(281),
    [anon_sym_AT] = ACTIONS(281),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(281),
    [sym_number] = ACTIONS(283),
    [sym_string] = ACTIONS(281),
    [sym_regex] = ACTIONS(281),
    [sym_keyword] = ACTIONS(281),
    [sym_boolean] = ACTIONS(281),
    [sym_character] = ACTIONS(281),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [41] = {
    [ts_builtin_sym_end] = ACTIONS(285),
    [sym_symbol] = ACTIONS(287),
    [anon_sym_LPAREN] = ACTIONS(285),
    [anon_sym_RPAREN] = ACTIONS(285),
    [anon_sym_POUND_LPAREN] = ACTIONS(285),
    [anon_sym_LBRACK] = ACTIONS(285),
    [anon_sym_RBRACK] = ACTIONS(285),
    [anon_sym_LBRACE] = ACTIONS(285),
    [anon_sym_RBRACE] = ACTIONS(285),
    [anon_sym_SQUOTE] = ACTIONS(285),
    [anon_sym_BQUOTE] = ACTIONS(285),
    [anon_sym_COMMA] = ACTIONS(287),
    [anon_sym_COMMA_AT] = ACTIONS(285),
    [anon_sym_AT] = ACTIONS(285),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(285),
    [sym_number] = ACTIONS(287),
    [sym_string] = ACTIONS(285),
    [sym_regex] = ACTIONS(285),
    [sym_keyword] = ACTIONS(285),
    [sym_boolean] = ACTIONS(285),
    [sym_character] = ACTIONS(285),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [42] = {
    [ts_builtin_sym_end] = ACTIONS(289),
    [sym_symbol] = ACTIONS(291),
    [anon_sym_LPAREN] = ACTIONS(289),
    [anon_sym_RPAREN] = ACTIONS(289),
    [anon_sym_POUND_LPAREN] = ACTIONS(289),
    [anon_sym_LBRACK] = ACTIONS(289),
    [anon_sym_RBRACK] = ACTIONS(289),
    [anon_sym_LBRACE] = ACTIONS(289),
    [anon_sym_RBRACE] = ACTIONS(289),
    [anon_sym_SQUOTE] = ACTIONS(289),
    [anon_sym_BQUOTE] = ACTIONS(289),
    [anon_sym_COMMA] = ACTIONS(291),
    [anon_sym_COMMA_AT] = ACTIONS(289),
    [anon_sym_AT] = ACTIONS(289),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(289),
    [sym_number] = ACTIONS(291),
    [sym_string] = ACTIONS(289),
    [sym_regex] = ACTIONS(289),
    [sym_keyword] = ACTIONS(289),
    [sym_boolean] = ACTIONS(289),
    [sym_character] = ACTIONS(289),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [43] = {
    [ts_builtin_sym_end] = ACTIONS(293),
    [sym_symbol] = ACTIONS(295),
    [anon_sym_LPAREN] = ACTIONS(293),
    [anon_sym_RPAREN] = ACTIONS(293),
    [anon_sym_POUND_LPAREN] = ACTIONS(293),
    [anon_sym_LBRACK] = ACTIONS(293),
    [anon_sym_RBRACK] = ACTIONS(293),
    [anon_sym_LBRACE] = ACTIONS(293),
    [anon_sym_RBRACE] = ACTIONS(293),
    [anon_sym_SQUOTE] = ACTIONS(293),
    [anon_sym_BQUOTE] = ACTIONS(293),
    [anon_sym_COMMA] = ACTIONS(295),
    [anon_sym_COMMA_AT] = ACTIONS(293),
    [anon_sym_AT] = ACTIONS(293),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(293),
    [sym_number] = ACTIONS(295),
    [sym_string] = ACTIONS(293),
    [sym_regex] = ACTIONS(293),
    [sym_keyword] = ACTIONS(293),
    [sym_boolean] = ACTIONS(293),
    [sym_character] = ACTIONS(293),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [44] = {
    [ts_builtin_sym_end] = ACTIONS(297),
    [sym_symbol] = ACTIONS(299),
    [anon_sym_LPAREN] = ACTIONS(297),
    [anon_sym_RPAREN] = ACTIONS(297),
    [anon_sym_POUND_LPAREN] = ACTIONS(297),
    [anon_sym_LBRACK] = ACTIONS(297),
    [anon_sym_RBRACK] = ACTIONS(297),
    [anon_sym_LBRACE] = ACTIONS(297),
    [anon_sym_RBRACE] = ACTIONS(297),
    [anon_sym_SQUOTE] = ACTIONS(297),
    [anon_sym_BQUOTE] = ACTIONS(297),
    [anon_sym_COMMA] = ACTIONS(299),
    [anon_sym_COMMA_AT] = ACTIONS(297),
    [anon_sym_AT] = ACTIONS(297),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(297),
    [sym_number] = ACTIONS(299),
    [sym_string] = ACTIONS(297),
    [sym_regex] = ACTIONS(297),
    [sym_keyword] = ACTIONS(297),
    [sym_boolean] = ACTIONS(297),
    [sym_character] = ACTIONS(297),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [45] = {
    [ts_builtin_sym_end] = ACTIONS(301),
    [sym_symbol] = ACTIONS(303),
    [anon_sym_LPAREN] = ACTIONS(301),
    [anon_sym_RPAREN] = ACTIONS(301),
    [anon_sym_POUND_LPAREN] = ACTIONS(301),
    [anon_sym_LBRACK] = ACTIONS(301),
    [anon_sym_RBRACK] = ACTIONS(301),
    [anon_sym_LBRACE] = ACTIONS(301),
    [anon_sym_RBRACE] = ACTIONS(301),
    [anon_sym_SQUOTE] = ACTIONS(301),
    [anon_sym_BQUOTE] = ACTIONS(301),
    [anon_sym_COMMA] = ACTIONS(303),
    [anon_sym_COMMA_AT] = ACTIONS(301),
    [anon_sym_AT] = ACTIONS(301),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(301),
    [sym_number] = ACTIONS(303),
    [sym_string] = ACTIONS(301),
    [sym_regex] = ACTIONS(301),
    [sym_keyword] = ACTIONS(301),
    [sym_boolean] = ACTIONS(301),
    [sym_character] = ACTIONS(301),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [46] = {
    [ts_builtin_sym_end] = ACTIONS(305),
    [sym_symbol] = ACTIONS(307),
    [anon_sym_LPAREN] = ACTIONS(305),
    [anon_sym_RPAREN] = ACTIONS(305),
    [anon_sym_POUND_LPAREN] = ACTIONS(305),
    [anon_sym_LBRACK] = ACTIONS(305),
    [anon_sym_RBRACK] = ACTIONS(305),
    [anon_sym_LBRACE] = ACTIONS(305),
    [anon_sym_RBRACE] = ACTIONS(305),
    [anon_sym_SQUOTE] = ACTIONS(305),
    [anon_sym_BQUOTE] = ACTIONS(305),
    [anon_sym_COMMA] = ACTIONS(307),
    [anon_sym_COMMA_AT] = ACTIONS(305),
    [anon_sym_AT] = ACTIONS(305),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(305),
    [sym_number] = ACTIONS(307),
    [sym_string] = ACTIONS(305),
    [sym_regex] = ACTIONS(305),
    [sym_keyword] = ACTIONS(305),
    [sym_boolean] = ACTIONS(305),
    [sym_character] = ACTIONS(305),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [47] = {
    [ts_builtin_sym_end] = ACTIONS(309),
    [sym_symbol] = ACTIONS(311),
    [anon_sym_LPAREN] = ACTIONS(309),
    [anon_sym_RPAREN] = ACTIONS(309),
    [anon_sym_POUND_LPAREN] = ACTIONS(309),
    [anon_sym_LBRACK] = ACTIONS(309),
    [anon_sym_RBRACK] = ACTIONS(309),
    [anon_sym_LBRACE] = ACTIONS(309),
    [anon_sym_RBRACE] = ACTIONS(309),
    [anon_sym_SQUOTE] = ACTIONS(309),
    [anon_sym_BQUOTE] = ACTIONS(309),
    [anon_sym_COMMA] = ACTIONS(311),
    [anon_sym_COMMA_AT] = ACTIONS(309),
    [anon_sym_AT] = ACTIONS(309),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(309),
    [sym_number] = ACTIONS(311),
    [sym_string] = ACTIONS(309),
    [sym_regex] = ACTIONS(309),
    [sym_keyword] = ACTIONS(309),
    [sym_boolean] = ACTIONS(309),
    [sym_character] = ACTIONS(309),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [48] = {
    [ts_builtin_sym_end] = ACTIONS(313),
    [sym_symbol] = ACTIONS(315),
    [anon_sym_LPAREN] = ACTIONS(313),
    [anon_sym_RPAREN] = ACTIONS(313),
    [anon_sym_POUND_LPAREN] = ACTIONS(313),
    [anon_sym_LBRACK] = ACTIONS(313),
    [anon_sym_RBRACK] = ACTIONS(313),
    [anon_sym_LBRACE] = ACTIONS(313),
    [anon_sym_RBRACE] = ACTIONS(313),
    [anon_sym_SQUOTE] = ACTIONS(313),
    [anon_sym_BQUOTE] = ACTIONS(313),
    [anon_sym_COMMA] = ACTIONS(315),
    [anon_sym_COMMA_AT] = ACTIONS(313),
    [anon_sym_AT] = ACTIONS(313),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(313),
    [sym_number] = ACTIONS(315),
    [sym_string] = ACTIONS(313),
    [sym_regex] = ACTIONS(313),
    [sym_keyword] = ACTIONS(313),
    [sym_boolean] = ACTIONS(313),
    [sym_character] = ACTIONS(313),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [49] = {
    [ts_builtin_sym_end] = ACTIONS(317),
    [sym_symbol] = ACTIONS(319),
    [anon_sym_LPAREN] = ACTIONS(317),
    [anon_sym_RPAREN] = ACTIONS(317),
    [anon_sym_POUND_LPAREN] = ACTIONS(317),
    [anon_sym_LBRACK] = ACTIONS(317),
    [anon_sym_RBRACK] = ACTIONS(317),
    [anon_sym_LBRACE] = ACTIONS(317),
    [anon_sym_RBRACE] = ACTIONS(317),
    [anon_sym_SQUOTE] = ACTIONS(317),
    [anon_sym_BQUOTE] = ACTIONS(317),
    [anon_sym_COMMA] = ACTIONS(319),
    [anon_sym_COMMA_AT] = ACTIONS(317),
    [anon_sym_AT] = ACTIONS(317),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(317),
    [sym_number] = ACTIONS(319),
    [sym_string] = ACTIONS(317),
    [sym_regex] = ACTIONS(317),
    [sym_keyword] = ACTIONS(317),
    [sym_boolean] = ACTIONS(317),
    [sym_character] = ACTIONS(317),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [50] = {
    [ts_builtin_sym_end] = ACTIONS(321),
    [sym_symbol] = ACTIONS(323),
    [anon_sym_LPAREN] = ACTIONS(321),
    [anon_sym_RPAREN] = ACTIONS(321),
    [anon_sym_POUND_LPAREN] = ACTIONS(321),
    [anon_sym_LBRACK] = ACTIONS(321),
    [anon_sym_RBRACK] = ACTIONS(321),
    [anon_sym_LBRACE] = ACTIONS(321),
    [anon_sym_RBRACE] = ACTIONS(321),
    [anon_sym_SQUOTE] = ACTIONS(321),
    [anon_sym_BQUOTE] = ACTIONS(321),
    [anon_sym_COMMA] = ACTIONS(323),
    [anon_sym_COMMA_AT] = ACTIONS(321),
    [anon_sym_AT] = ACTIONS(321),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(321),
    [sym_number] = ACTIONS(323),
    [sym_string] = ACTIONS(321),
    [sym_regex] = ACTIONS(321),
    [sym_keyword] = ACTIONS(321),
    [sym_boolean] = ACTIONS(321),
    [sym_character] = ACTIONS(321),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [51] = {
    [sym_symbol] = ACTIONS(307),
    [anon_sym_LPAREN] = ACTIONS(305),
    [anon_sym_RPAREN] = ACTIONS(305),
    [anon_sym_DOT] = ACTIONS(307),
    [anon_sym_POUND_LPAREN] = ACTIONS(305),
    [anon_sym_LBRACK] = ACTIONS(305),
    [anon_sym_LBRACE] = ACTIONS(305),
    [anon_sym_SQUOTE] = ACTIONS(305),
    [anon_sym_BQUOTE] = ACTIONS(305),
    [anon_sym_COMMA] = ACTIONS(307),
    [anon_sym_COMMA_AT] = ACTIONS(305),
    [anon_sym_AT] = ACTIONS(305),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(305),
    [sym_number] = ACTIONS(307),
    [sym_string] = ACTIONS(305),
    [sym_regex] = ACTIONS(305),
    [sym_keyword] = ACTIONS(305),
    [sym_boolean] = ACTIONS(305),
    [sym_character] = ACTIONS(305),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [52] = {
    [sym_symbol] = ACTIONS(295),
    [anon_sym_LPAREN] = ACTIONS(293),
    [anon_sym_RPAREN] = ACTIONS(293),
    [anon_sym_DOT] = ACTIONS(295),
    [anon_sym_POUND_LPAREN] = ACTIONS(293),
    [anon_sym_LBRACK] = ACTIONS(293),
    [anon_sym_LBRACE] = ACTIONS(293),
    [anon_sym_SQUOTE] = ACTIONS(293),
    [anon_sym_BQUOTE] = ACTIONS(293),
    [anon_sym_COMMA] = ACTIONS(295),
    [anon_sym_COMMA_AT] = ACTIONS(293),
    [anon_sym_AT] = ACTIONS(293),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(293),
    [sym_number] = ACTIONS(295),
    [sym_string] = ACTIONS(293),
    [sym_regex] = ACTIONS(293),
    [sym_keyword] = ACTIONS(293),
    [sym_boolean] = ACTIONS(293),
    [sym_character] = ACTIONS(293),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [53] = {
    [sym_symbol] = ACTIONS(303),
    [anon_sym_LPAREN] = ACTIONS(301),
    [anon_sym_RPAREN] = ACTIONS(301),
    [anon_sym_DOT] = ACTIONS(303),
    [anon_sym_POUND_LPAREN] = ACTIONS(301),
    [anon_sym_LBRACK] = ACTIONS(301),
    [anon_sym_LBRACE] = ACTIONS(301),
    [anon_sym_SQUOTE] = ACTIONS(301),
    [anon_sym_BQUOTE] = ACTIONS(301),
    [anon_sym_COMMA] = ACTIONS(303),
    [anon_sym_COMMA_AT] = ACTIONS(301),
    [anon_sym_AT] = ACTIONS(301),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(301),
    [sym_number] = ACTIONS(303),
    [sym_string] = ACTIONS(301),
    [sym_regex] = ACTIONS(301),
    [sym_keyword] = ACTIONS(301),
    [sym_boolean] = ACTIONS(301),
    [sym_character] = ACTIONS(301),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [54] = {
    [sym_symbol] = ACTIONS(271),
    [anon_sym_LPAREN] = ACTIONS(269),
    [anon_sym_RPAREN] = ACTIONS(269),
    [anon_sym_DOT] = ACTIONS(271),
    [anon_sym_POUND_LPAREN] = ACTIONS(269),
    [anon_sym_LBRACK] = ACTIONS(269),
    [anon_sym_LBRACE] = ACTIONS(269),
    [anon_sym_SQUOTE] = ACTIONS(269),
    [anon_sym_BQUOTE] = ACTIONS(269),
    [anon_sym_COMMA] = ACTIONS(271),
    [anon_sym_COMMA_AT] = ACTIONS(269),
    [anon_sym_AT] = ACTIONS(269),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(269),
    [sym_number] = ACTIONS(271),
    [sym_string] = ACTIONS(269),
    [sym_regex] = ACTIONS(269),
    [sym_keyword] = ACTIONS(269),
    [sym_boolean] = ACTIONS(269),
    [sym_character] = ACTIONS(269),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [55] = {
    [sym_symbol] = ACTIONS(319),
    [anon_sym_LPAREN] = ACTIONS(317),
    [anon_sym_RPAREN] = ACTIONS(317),
    [anon_sym_DOT] = ACTIONS(319),
    [anon_sym_POUND_LPAREN] = ACTIONS(317),
    [anon_sym_LBRACK] = ACTIONS(317),
    [anon_sym_LBRACE] = ACTIONS(317),
    [anon_sym_SQUOTE] = ACTIONS(317),
    [anon_sym_BQUOTE] = ACTIONS(317),
    [anon_sym_COMMA] = ACTIONS(319),
    [anon_sym_COMMA_AT] = ACTIONS(317),
    [anon_sym_AT] = ACTIONS(317),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(317),
    [sym_number] = ACTIONS(319),
    [sym_string] = ACTIONS(317),
    [sym_regex] = ACTIONS(317),
    [sym_keyword] = ACTIONS(317),
    [sym_boolean] = ACTIONS(317),
    [sym_character] = ACTIONS(317),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [56] = {
    [sym_symbol] = ACTIONS(267),
    [anon_sym_LPAREN] = ACTIONS(265),
    [anon_sym_RPAREN] = ACTIONS(265),
    [anon_sym_DOT] = ACTIONS(267),
    [anon_sym_POUND_LPAREN] = ACTIONS(265),
    [anon_sym_LBRACK] = ACTIONS(265),
    [anon_sym_LBRACE] = ACTIONS(265),
    [anon_sym_SQUOTE] = ACTIONS(265),
    [anon_sym_BQUOTE] = ACTIONS(265),
    [anon_sym_COMMA] = ACTIONS(267),
    [anon_sym_COMMA_AT] = ACTIONS(265),
    [anon_sym_AT] = ACTIONS(265),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(265),
    [sym_number] = ACTIONS(267),
    [sym_string] = ACTIONS(265),
    [sym_regex] = ACTIONS(265),
    [sym_keyword] = ACTIONS(265),
    [sym_boolean] = ACTIONS(265),
    [sym_character] = ACTIONS(265),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [57] = {
    [sym_symbol] = ACTIONS(279),
    [anon_sym_LPAREN] = ACTIONS(277),
    [anon_sym_RPAREN] = ACTIONS(277),
    [anon_sym_DOT] = ACTIONS(279),
    [anon_sym_POUND_LPAREN] = ACTIONS(277),
    [anon_sym_LBRACK] = ACTIONS(277),
    [anon_sym_LBRACE] = ACTIONS(277),
    [anon_sym_SQUOTE] = ACTIONS(277),
    [anon_sym_BQUOTE] = ACTIONS(277),
    [anon_sym_COMMA] = ACTIONS(279),
    [anon_sym_COMMA_AT] = ACTIONS(277),
    [anon_sym_AT] = ACTIONS(277),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(277),
    [sym_number] = ACTIONS(279),
    [sym_string] = ACTIONS(277),
    [sym_regex] = ACTIONS(277),
    [sym_keyword] = ACTIONS(277),
    [sym_boolean] = ACTIONS(277),
    [sym_character] = ACTIONS(277),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [58] = {
    [sym_symbol] = ACTIONS(283),
    [anon_sym_LPAREN] = ACTIONS(281),
    [anon_sym_RPAREN] = ACTIONS(281),
    [anon_sym_DOT] = ACTIONS(283),
    [anon_sym_POUND_LPAREN] = ACTIONS(281),
    [anon_sym_LBRACK] = ACTIONS(281),
    [anon_sym_LBRACE] = ACTIONS(281),
    [anon_sym_SQUOTE] = ACTIONS(281),
    [anon_sym_BQUOTE] = ACTIONS(281),
    [anon_sym_COMMA] = ACTIONS(283),
    [anon_sym_COMMA_AT] = ACTIONS(281),
    [anon_sym_AT] = ACTIONS(281),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(281),
    [sym_number] = ACTIONS(283),
    [sym_string] = ACTIONS(281),
    [sym_regex] = ACTIONS(281),
    [sym_keyword] = ACTIONS(281),
    [sym_boolean] = ACTIONS(281),
    [sym_character] = ACTIONS(281),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [59] = {
    [sym_symbol] = ACTIONS(291),
    [anon_sym_LPAREN] = ACTIONS(289),
    [anon_sym_RPAREN] = ACTIONS(289),
    [anon_sym_DOT] = ACTIONS(291),
    [anon_sym_POUND_LPAREN] = ACTIONS(289),
    [anon_sym_LBRACK] = ACTIONS(289),
    [anon_sym_LBRACE] = ACTIONS(289),
    [anon_sym_SQUOTE] = ACTIONS(289),
    [anon_sym_BQUOTE] = ACTIONS(289),
    [anon_sym_COMMA] = ACTIONS(291),
    [anon_sym_COMMA_AT] = ACTIONS(289),
    [anon_sym_AT] = ACTIONS(289),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(289),
    [sym_number] = ACTIONS(291),
    [sym_string] = ACTIONS(289),
    [sym_regex] = ACTIONS(289),
    [sym_keyword] = ACTIONS(289),
    [sym_boolean] = ACTIONS(289),
    [sym_character] = ACTIONS(289),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [60] = {
    [sym_symbol] = ACTIONS(311),
    [anon_sym_LPAREN] = ACTIONS(309),
    [anon_sym_RPAREN] = ACTIONS(309),
    [anon_sym_DOT] = ACTIONS(311),
    [anon_sym_POUND_LPAREN] = ACTIONS(309),
    [anon_sym_LBRACK] = ACTIONS(309),
    [anon_sym_LBRACE] = ACTIONS(309),
    [anon_sym_SQUOTE] = ACTIONS(309),
    [anon_sym_BQUOTE] = ACTIONS(309),
    [anon_sym_COMMA] = ACTIONS(311),
    [anon_sym_COMMA_AT] = ACTIONS(309),
    [anon_sym_AT] = ACTIONS(309),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(309),
    [sym_number] = ACTIONS(311),
    [sym_string] = ACTIONS(309),
    [sym_regex] = ACTIONS(309),
    [sym_keyword] = ACTIONS(309),
    [sym_boolean] = ACTIONS(309),
    [sym_character] = ACTIONS(309),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [61] = {
    [sym_symbol] = ACTIONS(315),
    [anon_sym_LPAREN] = ACTIONS(313),
    [anon_sym_RPAREN] = ACTIONS(313),
    [anon_sym_DOT] = ACTIONS(315),
    [anon_sym_POUND_LPAREN] = ACTIONS(313),
    [anon_sym_LBRACK] = ACTIONS(313),
    [anon_sym_LBRACE] = ACTIONS(313),
    [anon_sym_SQUOTE] = ACTIONS(313),
    [anon_sym_BQUOTE] = ACTIONS(313),
    [anon_sym_COMMA] = ACTIONS(315),
    [anon_sym_COMMA_AT] = ACTIONS(313),
    [anon_sym_AT] = ACTIONS(313),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(313),
    [sym_number] = ACTIONS(315),
    [sym_string] = ACTIONS(313),
    [sym_regex] = ACTIONS(313),
    [sym_keyword] = ACTIONS(313),
    [sym_boolean] = ACTIONS(313),
    [sym_character] = ACTIONS(313),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [62] = {
    [sym_symbol] = ACTIONS(263),
    [anon_sym_LPAREN] = ACTIONS(261),
    [anon_sym_RPAREN] = ACTIONS(261),
    [anon_sym_DOT] = ACTIONS(263),
    [anon_sym_POUND_LPAREN] = ACTIONS(261),
    [anon_sym_LBRACK] = ACTIONS(261),
    [anon_sym_LBRACE] = ACTIONS(261),
    [anon_sym_SQUOTE] = ACTIONS(261),
    [anon_sym_BQUOTE] = ACTIONS(261),
    [anon_sym_COMMA] = ACTIONS(263),
    [anon_sym_COMMA_AT] = ACTIONS(261),
    [anon_sym_AT] = ACTIONS(261),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(261),
    [sym_number] = ACTIONS(263),
    [sym_string] = ACTIONS(261),
    [sym_regex] = ACTIONS(261),
    [sym_keyword] = ACTIONS(261),
    [sym_boolean] = ACTIONS(261),
    [sym_character] = ACTIONS(261),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [63] = {
    [sym_symbol] = ACTIONS(299),
    [anon_sym_LPAREN] = ACTIONS(297),
    [anon_sym_RPAREN] = ACTIONS(297),
    [anon_sym_DOT] = ACTIONS(299),
    [anon_sym_POUND_LPAREN] = ACTIONS(297),
    [anon_sym_LBRACK] = ACTIONS(297),
    [anon_sym_LBRACE] = ACTIONS(297),
    [anon_sym_SQUOTE] = ACTIONS(297),
    [anon_sym_BQUOTE] = ACTIONS(297),
    [anon_sym_COMMA] = ACTIONS(299),
    [anon_sym_COMMA_AT] = ACTIONS(297),
    [anon_sym_AT] = ACTIONS(297),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(297),
    [sym_number] = ACTIONS(299),
    [sym_string] = ACTIONS(297),
    [sym_regex] = ACTIONS(297),
    [sym_keyword] = ACTIONS(297),
    [sym_boolean] = ACTIONS(297),
    [sym_character] = ACTIONS(297),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [64] = {
    [sym_symbol] = ACTIONS(323),
    [anon_sym_LPAREN] = ACTIONS(321),
    [anon_sym_RPAREN] = ACTIONS(321),
    [anon_sym_DOT] = ACTIONS(323),
    [anon_sym_POUND_LPAREN] = ACTIONS(321),
    [anon_sym_LBRACK] = ACTIONS(321),
    [anon_sym_LBRACE] = ACTIONS(321),
    [anon_sym_SQUOTE] = ACTIONS(321),
    [anon_sym_BQUOTE] = ACTIONS(321),
    [anon_sym_COMMA] = ACTIONS(323),
    [anon_sym_COMMA_AT] = ACTIONS(321),
    [anon_sym_AT] = ACTIONS(321),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(321),
    [sym_number] = ACTIONS(323),
    [sym_string] = ACTIONS(321),
    [sym_regex] = ACTIONS(321),
    [sym_keyword] = ACTIONS(321),
    [sym_boolean] = ACTIONS(321),
    [sym_character] = ACTIONS(321),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [65] = {
    [sym_symbol] = ACTIONS(287),
    [anon_sym_LPAREN] = ACTIONS(285),
    [anon_sym_RPAREN] = ACTIONS(285),
    [anon_sym_DOT] = ACTIONS(287),
    [anon_sym_POUND_LPAREN] = ACTIONS(285),
    [anon_sym_LBRACK] = ACTIONS(285),
    [anon_sym_LBRACE] = ACTIONS(285),
    [anon_sym_SQUOTE] = ACTIONS(285),
    [anon_sym_BQUOTE] = ACTIONS(285),
    [anon_sym_COMMA] = ACTIONS(287),
    [anon_sym_COMMA_AT] = ACTIONS(285),
    [anon_sym_AT] = ACTIONS(285),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(285),
    [sym_number] = ACTIONS(287),
    [sym_string] = ACTIONS(285),
    [sym_regex] = ACTIONS(285),
    [sym_keyword] = ACTIONS(285),
    [sym_boolean] = ACTIONS(285),
    [sym_character] = ACTIONS(285),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
  [66] = {
    [sym_symbol] = ACTIONS(275),
    [anon_sym_LPAREN] = ACTIONS(273),
    [anon_sym_RPAREN] = ACTIONS(273),
    [anon_sym_DOT] = ACTIONS(275),
    [anon_sym_POUND_LPAREN] = ACTIONS(273),
    [anon_sym_LBRACK] = ACTIONS(273),
    [anon_sym_LBRACE] = ACTIONS(273),
    [anon_sym_SQUOTE] = ACTIONS(273),
    [anon_sym_BQUOTE] = ACTIONS(273),
    [anon_sym_COMMA] = ACTIONS(275),
    [anon_sym_COMMA_AT] = ACTIONS(273),
    [anon_sym_AT] = ACTIONS(273),
    [anon_sym_POUNDu8_LPAREN] = ACTIONS(273),
    [sym_number] = ACTIONS(275),
    [sym_string] = ACTIONS(273),
    [sym_regex] = ACTIONS(273),
    [sym_keyword] = ACTIONS(273),
    [sym_boolean] = ACTIONS(273),
    [sym_character] = ACTIONS(273),
    [sym_comment] = ACTIONS(3),
    [sym_block_comment] = ACTIONS(3),
  },
};

static const uint16_t ts_small_parse_table[] = {
  [0] = 4,
    ACTIONS(325), 1,
      anon_sym_RPAREN,
    ACTIONS(327), 1,
      sym_number,
    STATE(68), 1,
      aux_sym_byte_vector_repeat1,
    ACTIONS(3), 2,
      sym_block_comment,
      sym_comment,
  [14] = 4,
    ACTIONS(329), 1,
      anon_sym_RPAREN,
    ACTIONS(331), 1,
      sym_number,
    STATE(68), 1,
      aux_sym_byte_vector_repeat1,
    ACTIONS(3), 2,
      sym_block_comment,
      sym_comment,
  [28] = 4,
    ACTIONS(334), 1,
      anon_sym_RPAREN,
    ACTIONS(336), 1,
      sym_number,
    STATE(67), 1,
      aux_sym_byte_vector_repeat1,
    ACTIONS(3), 2,
      sym_block_comment,
      sym_comment,
  [42] = 4,
    ACTIONS(338), 1,
      anon_sym_RPAREN,
    ACTIONS(340), 1,
      sym_number,
    STATE(71), 1,
      aux_sym_byte_vector_repeat1,
    ACTIONS(3), 2,
      sym_block_comment,
      sym_comment,
  [56] = 4,
    ACTIONS(327), 1,
      sym_number,
    ACTIONS(342), 1,
      anon_sym_RPAREN,
    STATE(68), 1,
      aux_sym_byte_vector_repeat1,
    ACTIONS(3), 2,
      sym_block_comment,
      sym_comment,
  [70] = 2,
    ACTIONS(344), 1,
      anon_sym_RPAREN,
    ACTIONS(3), 2,
      sym_block_comment,
      sym_comment,
  [78] = 2,
    ACTIONS(346), 1,
      ts_builtin_sym_end,
    ACTIONS(3), 2,
      sym_block_comment,
      sym_comment,
  [86] = 2,
    ACTIONS(348), 1,
      anon_sym_RPAREN,
    ACTIONS(3), 2,
      sym_block_comment,
      sym_comment,
};

static const uint32_t ts_small_parse_table_map[] = {
  [SMALL_STATE(67)] = 0,
  [SMALL_STATE(68)] = 14,
  [SMALL_STATE(69)] = 28,
  [SMALL_STATE(70)] = 42,
  [SMALL_STATE(71)] = 56,
  [SMALL_STATE(72)] = 70,
  [SMALL_STATE(73)] = 78,
  [SMALL_STATE(74)] = 86,
};

static const TSParseActionEntry ts_parse_actions[] = {
  [0] = {.entry = {.count = 0, .reusable = false}},
  [1] = {.entry = {.count = 1, .reusable = false}}, RECOVER(),
  [3] = {.entry = {.count = 1, .reusable = true}}, SHIFT_EXTRA(),
  [5] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_source_file, 0, 0, 0),
  [7] = {.entry = {.count = 1, .reusable = false}}, SHIFT(10),
  [9] = {.entry = {.count = 1, .reusable = true}}, SHIFT(11),
  [11] = {.entry = {.count = 1, .reusable = true}}, SHIFT(6),
  [13] = {.entry = {.count = 1, .reusable = true}}, SHIFT(7),
  [15] = {.entry = {.count = 1, .reusable = true}}, SHIFT(8),
  [17] = {.entry = {.count = 1, .reusable = true}}, SHIFT(25),
  [19] = {.entry = {.count = 1, .reusable = true}}, SHIFT(27),
  [21] = {.entry = {.count = 1, .reusable = false}}, SHIFT(28),
  [23] = {.entry = {.count = 1, .reusable = true}}, SHIFT(29),
  [25] = {.entry = {.count = 1, .reusable = true}}, SHIFT(34),
  [27] = {.entry = {.count = 1, .reusable = true}}, SHIFT(69),
  [29] = {.entry = {.count = 1, .reusable = true}}, SHIFT(10),
  [31] = {.entry = {.count = 1, .reusable = true}}, SHIFT(9),
  [33] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0),
  [35] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(2),
  [38] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(11),
  [41] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(6),
  [44] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(7),
  [47] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(8),
  [50] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(25),
  [53] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(27),
  [56] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(28),
  [59] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(29),
  [62] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(34),
  [65] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(69),
  [68] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(2),
  [71] = {.entry = {.count = 1, .reusable = false}}, SHIFT(4),
  [73] = {.entry = {.count = 1, .reusable = true}}, SHIFT(15),
  [75] = {.entry = {.count = 1, .reusable = true}}, SHIFT(43),
  [77] = {.entry = {.count = 1, .reusable = false}}, SHIFT(26),
  [79] = {.entry = {.count = 1, .reusable = true}}, SHIFT(19),
  [81] = {.entry = {.count = 1, .reusable = true}}, SHIFT(16),
  [83] = {.entry = {.count = 1, .reusable = true}}, SHIFT(17),
  [85] = {.entry = {.count = 1, .reusable = true}}, SHIFT(30),
  [87] = {.entry = {.count = 1, .reusable = true}}, SHIFT(23),
  [89] = {.entry = {.count = 1, .reusable = false}}, SHIFT(31),
  [91] = {.entry = {.count = 1, .reusable = true}}, SHIFT(32),
  [93] = {.entry = {.count = 1, .reusable = true}}, SHIFT(33),
  [95] = {.entry = {.count = 1, .reusable = true}}, SHIFT(70),
  [97] = {.entry = {.count = 1, .reusable = true}}, SHIFT(4),
  [99] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(4),
  [102] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(15),
  [105] = {.entry = {.count = 1, .reusable = false}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0),
  [107] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(19),
  [110] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(16),
  [113] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(17),
  [116] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(30),
  [119] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(23),
  [122] = {.entry = {.count = 2, .reusable = false}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(31),
  [125] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(32),
  [128] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(33),
  [131] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(70),
  [134] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_source_file_repeat1, 2, 0, 0), SHIFT_REPEAT(4),
  [137] = {.entry = {.count = 1, .reusable = true}}, SHIFT(52),
  [139] = {.entry = {.count = 1, .reusable = false}}, SHIFT(24),
  [141] = {.entry = {.count = 1, .reusable = false}}, SHIFT(12),
  [143] = {.entry = {.count = 1, .reusable = true}}, SHIFT(40),
  [145] = {.entry = {.count = 1, .reusable = true}}, SHIFT(12),
  [147] = {.entry = {.count = 1, .reusable = false}}, SHIFT(13),
  [149] = {.entry = {.count = 1, .reusable = true}}, SHIFT(42),
  [151] = {.entry = {.count = 1, .reusable = true}}, SHIFT(13),
  [153] = {.entry = {.count = 1, .reusable = false}}, SHIFT(14),
  [155] = {.entry = {.count = 1, .reusable = true}}, SHIFT(47),
  [157] = {.entry = {.count = 1, .reusable = true}}, SHIFT(14),
  [159] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_source_file, 1, 0, 0),
  [161] = {.entry = {.count = 1, .reusable = false}}, SHIFT(18),
  [163] = {.entry = {.count = 1, .reusable = true}}, SHIFT(18),
  [165] = {.entry = {.count = 1, .reusable = false}}, SHIFT(2),
  [167] = {.entry = {.count = 1, .reusable = true}}, SHIFT(2),
  [169] = {.entry = {.count = 1, .reusable = false}}, SHIFT(3),
  [171] = {.entry = {.count = 1, .reusable = true}}, SHIFT(39),
  [173] = {.entry = {.count = 1, .reusable = true}}, SHIFT(3),
  [175] = {.entry = {.count = 1, .reusable = true}}, SHIFT(45),
  [177] = {.entry = {.count = 1, .reusable = true}}, SHIFT(46),
  [179] = {.entry = {.count = 1, .reusable = true}}, SHIFT(49),
  [181] = {.entry = {.count = 1, .reusable = false}}, SHIFT(5),
  [183] = {.entry = {.count = 1, .reusable = true}}, SHIFT(57),
  [185] = {.entry = {.count = 1, .reusable = true}}, SHIFT(5),
  [187] = {.entry = {.count = 1, .reusable = false}}, SHIFT(21),
  [189] = {.entry = {.count = 1, .reusable = true}}, SHIFT(59),
  [191] = {.entry = {.count = 1, .reusable = true}}, SHIFT(21),
  [193] = {.entry = {.count = 1, .reusable = false}}, SHIFT(22),
  [195] = {.entry = {.count = 1, .reusable = true}}, SHIFT(60),
  [197] = {.entry = {.count = 1, .reusable = true}}, SHIFT(22),
  [199] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_source_file, 2, 0, 0),
  [201] = {.entry = {.count = 1, .reusable = false}}, SHIFT(20),
  [203] = {.entry = {.count = 1, .reusable = true}}, SHIFT(58),
  [205] = {.entry = {.count = 1, .reusable = true}}, SHIFT(20),
  [207] = {.entry = {.count = 1, .reusable = true}}, SHIFT(53),
  [209] = {.entry = {.count = 1, .reusable = true}}, SHIFT(51),
  [211] = {.entry = {.count = 1, .reusable = true}}, SHIFT(55),
  [213] = {.entry = {.count = 1, .reusable = false}}, SHIFT(62),
  [215] = {.entry = {.count = 1, .reusable = true}}, SHIFT(62),
  [217] = {.entry = {.count = 1, .reusable = false}}, SHIFT(74),
  [219] = {.entry = {.count = 1, .reusable = true}}, SHIFT(74),
  [221] = {.entry = {.count = 1, .reusable = false}}, SHIFT(48),
  [223] = {.entry = {.count = 1, .reusable = true}}, SHIFT(48),
  [225] = {.entry = {.count = 1, .reusable = false}}, SHIFT(72),
  [227] = {.entry = {.count = 1, .reusable = true}}, SHIFT(72),
  [229] = {.entry = {.count = 1, .reusable = false}}, SHIFT(35),
  [231] = {.entry = {.count = 1, .reusable = true}}, SHIFT(35),
  [233] = {.entry = {.count = 1, .reusable = false}}, SHIFT(50),
  [235] = {.entry = {.count = 1, .reusable = true}}, SHIFT(50),
  [237] = {.entry = {.count = 1, .reusable = false}}, SHIFT(41),
  [239] = {.entry = {.count = 1, .reusable = true}}, SHIFT(41),
  [241] = {.entry = {.count = 1, .reusable = false}}, SHIFT(61),
  [243] = {.entry = {.count = 1, .reusable = true}}, SHIFT(61),
  [245] = {.entry = {.count = 1, .reusable = false}}, SHIFT(64),
  [247] = {.entry = {.count = 1, .reusable = true}}, SHIFT(64),
  [249] = {.entry = {.count = 1, .reusable = false}}, SHIFT(65),
  [251] = {.entry = {.count = 1, .reusable = true}}, SHIFT(65),
  [253] = {.entry = {.count = 1, .reusable = false}}, SHIFT(63),
  [255] = {.entry = {.count = 1, .reusable = true}}, SHIFT(63),
  [257] = {.entry = {.count = 1, .reusable = false}}, SHIFT(44),
  [259] = {.entry = {.count = 1, .reusable = true}}, SHIFT(44),
  [261] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_quasiquote, 2, 0, 0),
  [263] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_quasiquote, 2, 0, 0),
  [265] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_byte_vector, 3, 0, 0),
  [267] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_byte_vector, 3, 0, 0),
  [269] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_list, 5, 0, 0),
  [271] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_list, 5, 0, 0),
  [273] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_byte_vector, 2, 0, 0),
  [275] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_byte_vector, 2, 0, 0),
  [277] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_list, 2, 0, 0),
  [279] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_list, 2, 0, 0),
  [281] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_short_lambda, 2, 0, 0),
  [283] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_short_lambda, 2, 0, 0),
  [285] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_unquote_splicing, 2, 0, 0),
  [287] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_unquote_splicing, 2, 0, 0),
  [289] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_vector, 2, 0, 0),
  [291] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_vector, 2, 0, 0),
  [293] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_list, 3, 0, 0),
  [295] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_list, 3, 0, 0),
  [297] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_deref, 2, 0, 0),
  [299] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_deref, 2, 0, 0),
  [301] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_short_lambda, 3, 0, 0),
  [303] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_short_lambda, 3, 0, 0),
  [305] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_vector, 3, 0, 0),
  [307] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_vector, 3, 0, 0),
  [309] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_hash_map, 2, 0, 0),
  [311] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_hash_map, 2, 0, 0),
  [313] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_quote, 2, 0, 0),
  [315] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_quote, 2, 0, 0),
  [317] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_hash_map, 3, 0, 0),
  [319] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_hash_map, 3, 0, 0),
  [321] = {.entry = {.count = 1, .reusable = true}}, REDUCE(sym_unquote, 2, 0, 0),
  [323] = {.entry = {.count = 1, .reusable = false}}, REDUCE(sym_unquote, 2, 0, 0),
  [325] = {.entry = {.count = 1, .reusable = true}}, SHIFT(36),
  [327] = {.entry = {.count = 1, .reusable = true}}, SHIFT(68),
  [329] = {.entry = {.count = 1, .reusable = true}}, REDUCE(aux_sym_byte_vector_repeat1, 2, 0, 0),
  [331] = {.entry = {.count = 2, .reusable = true}}, REDUCE(aux_sym_byte_vector_repeat1, 2, 0, 0), SHIFT_REPEAT(68),
  [334] = {.entry = {.count = 1, .reusable = true}}, SHIFT(38),
  [336] = {.entry = {.count = 1, .reusable = true}}, SHIFT(67),
  [338] = {.entry = {.count = 1, .reusable = true}}, SHIFT(66),
  [340] = {.entry = {.count = 1, .reusable = true}}, SHIFT(71),
  [342] = {.entry = {.count = 1, .reusable = true}}, SHIFT(56),
  [344] = {.entry = {.count = 1, .reusable = true}}, SHIFT(37),
  [346] = {.entry = {.count = 1, .reusable = true}},  ACCEPT_INPUT(),
  [348] = {.entry = {.count = 1, .reusable = true}}, SHIFT(54),
};

enum ts_external_scanner_symbol_identifiers {
  ts_external_token_block_comment = 0,
};

static const TSSymbol ts_external_scanner_symbol_map[EXTERNAL_TOKEN_COUNT] = {
  [ts_external_token_block_comment] = sym_block_comment,
};

static const bool ts_external_scanner_states[2][EXTERNAL_TOKEN_COUNT] = {
  [1] = {
    [ts_external_token_block_comment] = true,
  },
};

#ifdef __cplusplus
extern "C" {
#endif
void *tree_sitter_sema_external_scanner_create(void);
void tree_sitter_sema_external_scanner_destroy(void *);
bool tree_sitter_sema_external_scanner_scan(void *, TSLexer *, const bool *);
unsigned tree_sitter_sema_external_scanner_serialize(void *, char *);
void tree_sitter_sema_external_scanner_deserialize(void *, const char *, unsigned);

#ifdef TREE_SITTER_HIDE_SYMBOLS
#define TS_PUBLIC
#elif defined(_WIN32)
#define TS_PUBLIC __declspec(dllexport)
#else
#define TS_PUBLIC __attribute__((visibility("default")))
#endif

TS_PUBLIC const TSLanguage *tree_sitter_sema(void) {
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
    .symbol_metadata = ts_symbol_metadata,
    .public_symbol_map = ts_symbol_map,
    .alias_map = ts_non_terminal_alias_map,
    .alias_sequences = &ts_alias_sequences[0][0],
    .lex_modes = ts_lex_modes,
    .lex_fn = ts_lex,
    .keyword_lex_fn = ts_lex_keywords,
    .keyword_capture_token = sym_symbol,
    .external_scanner = {
      &ts_external_scanner_states[0][0],
      ts_external_scanner_symbol_map,
      tree_sitter_sema_external_scanner_create,
      tree_sitter_sema_external_scanner_destroy,
      tree_sitter_sema_external_scanner_scan,
      tree_sitter_sema_external_scanner_serialize,
      tree_sitter_sema_external_scanner_deserialize,
    },
    .primary_state_ids = ts_primary_state_ids,
  };
  return &language;
}
#ifdef __cplusplus
}
#endif
