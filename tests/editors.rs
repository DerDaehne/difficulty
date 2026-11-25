//! The editor files must mention every keyword and predefined name (no drift).
use dif::lexer::KEYWORDS;

const TYPES: &[&str] = &[
    "int", "i8", "i16", "i32", "u8", "u16", "u32", "u64", "f32", "f64", "bool", "str", "rune",
];
const BUILTINS: &[&str] = &["print", "range", "repeat", "expect", "assert"];

#[test]
fn editor_files_know_every_word() {
    for file in ["editors/vim/syntax/dif.vim", "editors/emacs/dif-mode.el"] {
        let text = std::fs::read_to_string(file).unwrap();
        let words: Vec<&str> = text
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .collect();
        for w in KEYWORDS.iter().chain(TYPES).chain(BUILTINS) {
            assert!(words.contains(w), "{file} is missing `{w}`");
        }
    }
}
