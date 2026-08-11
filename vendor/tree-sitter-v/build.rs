fn main() {
    let src_dir = std::path::Path::new("src");
    let lib_name = "tree-sitter-v";
    let parser_path = src_dir.join("parser.c");

    println!("cargo:rerun-if-changed={}", parser_path.to_str().unwrap());

    cc::Build::new()
        .include(&src_dir)
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-trigraphs")
        .file(&parser_path)
        .compile(lib_name);
}
