//! Adapters for grammars whose published Rust bindings use an older
//! `tree-sitter::Language` wrapper while their generated parser ABI remains
//! compatible with the application's Tree-sitter runtime.

use tree_sitter::Language;
use tree_sitter_language::LanguageFn;

unsafe extern "C" {
    fn tree_sitter_cue() -> *const ();
    fn tree_sitter_fennel() -> *const ();
    fn tree_sitter_pest() -> *const ();
    fn tree_sitter_pony() -> *const ();
}

const CUE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_cue) };
const FENNEL: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_fennel) };
const PEST: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_pest) };
const PONY: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_pony) };

pub fn cue_language() -> Language {
    CUE.into()
}

pub fn fennel_language() -> Language {
    FENNEL.into()
}

pub fn pest_language() -> Language {
    PEST.into()
}

pub fn pony_language() -> Language {
    PONY.into()
}
