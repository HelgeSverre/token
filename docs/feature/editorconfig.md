# EditorConfig Integration

Automatic editor settings from `.editorconfig` files

> **Status:** Planned
> **Priority:** P2
> **Effort:** M
> **Created:** 2025-12-20
> **Milestone:** 3 - Quality of Life

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Data Structures](#data-structures)
4. [Implementation Plan](#implementation-plan)
5. [Testing Strategy](#testing-strategy)
6. [References](#references)

---

## Overview

### Current State

The editor currently:
- Uses global/default settings for indentation
- Has no per-file or per-project configuration
- Requires manual setting changes per file type

### Goals

1. **Parse `.editorconfig` files**: Hierarchical config up to root
2. **Apply settings per file**: Match file patterns to rules
3. **Core properties**: indent_style, indent_size, tab_width, end_of_line, charset, trim_trailing_whitespace, insert_final_newline
4. **Live reload**: Detect `.editorconfig` changes
5. **Status bar indicator**: Show active config source
6. **Override UI**: Manual override for current file

### Non-Goals

- Custom editor-specific properties (first iteration)
- Remote `.editorconfig` fetching
- EditorConfig file editing UI

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        EditorConfig Resolution Flow                          │
│                                                                              │
│  File: /home/user/project/src/utils/helper.ts                               │
│                                                                              │
│  Search path (bottom-up):                                                    │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │ /home/user/project/src/utils/.editorconfig  (if exists)                │ │
│  │ /home/user/project/src/.editorconfig        (if exists)                │ │
│  │ /home/user/project/.editorconfig            ← Found! (has root=true)   │ │
│  │ /home/user/.editorconfig                    (stop: root=true above)    │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
│  Merge order (later overrides earlier):                                      │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │ 1. Defaults: indent_style=space, indent_size=4, ...                    │ │
│  │ 2. Root .editorconfig [*] section                                      │ │
│  │ 3. Root .editorconfig [*.ts] section                                   │ │
│  │ 4. Nested .editorconfig [*] section (if any)                           │ │
│  │ 5. Nested .editorconfig [*.ts] section (if any)                        │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
│  Result: FileConfig { indent_style: Tab, indent_size: 2, tab_width: 2, ... }│
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Module Structure

```
src/
├── editorconfig/
│   ├── mod.rs           # Module exports
│   ├── parser.rs        # .editorconfig file parser
│   ├── resolver.rs      # Config resolution for a file path
│   ├── matcher.rs       # Glob pattern matching
│   └── cache.rs         # Config cache with file watching
├── model/
│   └── document.rs      # Add FileConfig to Document
└── update/
    └── document.rs      # Apply config on file open
```

---

## Data Structures

### FileConfig

```rust
// In src/editorconfig/mod.rs

/// Resolved configuration for a specific file
#[derive(Debug, Clone, PartialEq)]
pub struct FileConfig {
    /// Indentation style: "space" or "tab"
    pub indent_style: IndentStyle,
    
    /// Number of columns per indentation level
    pub indent_size: IndentSize,
    
    /// Width of a tab character
    pub tab_width: u8,
    
    /// Line ending style
    pub end_of_line: EndOfLine,
    
    /// File character encoding
    pub charset: Charset,
    
    /// Remove trailing whitespace on save
    pub trim_trailing_whitespace: bool,
    
    /// Ensure file ends with newline on save
    pub insert_final_newline: bool,
    
    /// Maximum line length (for soft wrap, rulers)
    pub max_line_length: Option<usize>,
    
    /// Source file path(s) that contributed to this config
    pub sources: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum IndentStyle {
    #[default]
    Space,
    Tab,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IndentSize {
    /// Specific size
    Value(u8),
    /// Use tab_width
    Tab,
}

impl Default for IndentSize {
    fn default() -> Self {
        IndentSize::Value(4)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum EndOfLine {
    #[default]
    Lf,
    Crlf,
    Cr,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Charset {
    #[default]
    Utf8,
    Utf8Bom,
    Utf16Be,
    Utf16Le,
    Latin1,
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            indent_style: IndentStyle::Space,
            indent_size: IndentSize::Value(4),
            tab_width: 4,
            end_of_line: EndOfLine::Lf,
            charset: Charset::Utf8,
            trim_trailing_whitespace: false,
            insert_final_newline: false,
            max_line_length: None,
            sources: Vec::new(),
        }
    }
}
```

### EditorConfigFile

```rust
// In src/editorconfig/parser.rs

/// Parsed .editorconfig file
#[derive(Debug, Clone)]
pub struct EditorConfigFile {
    /// File path
    pub path: PathBuf,
    
    /// Whether this is a root config (stop searching)
    pub root: bool,
    
    /// Sections by glob pattern
    pub sections: Vec<ConfigSection>,
}

/// A section in an .editorconfig file
#[derive(Debug, Clone)]
pub struct ConfigSection {
    /// Glob pattern (e.g., "*.ts", "[*.{js,jsx}]")
    pub pattern: String,
    
    /// Properties in this section
    pub properties: HashMap<String, String>,
}

impl EditorConfigFile {
    /// Parse an .editorconfig file
    pub fn parse(path: &Path) -> io::Result<Self> {
        let content = fs::read_to_string(path)?;
        Self::parse_str(&content, path.to_path_buf())
    }
    
    pub fn parse_str(content: &str, path: PathBuf) -> io::Result<Self> {
        let mut root = false;
        let mut sections = Vec::new();
        let mut current_section: Option<ConfigSection> = None;
        
        for line in content.lines() {
            let line = line.trim();
            
            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            
            // Section header
            if line.starts_with('[') && line.ends_with(']') {
                if let Some(section) = current_section.take() {
                    sections.push(section);
                }
                let pattern = line[1..line.len()-1].to_string();
                current_section = Some(ConfigSection {
                    pattern,
                    properties: HashMap::new(),
                });
                continue;
            }
            
            // Key-value pair
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim().to_lowercase();
                let value = value.trim().to_string();
                
                if key == "root" && value.to_lowercase() == "true" {
                    root = true;
                } else if let Some(ref mut section) = current_section {
                    section.properties.insert(key, value);
                }
            }
        }
        
        if let Some(section) = current_section {
            sections.push(section);
        }
        
        Ok(Self { path, root, sections })
    }
}
```

### ConfigResolver

```rust
// In src/editorconfig/resolver.rs

/// Resolves EditorConfig settings for files
pub struct ConfigResolver {
    /// Cache of parsed .editorconfig files
    cache: HashMap<PathBuf, EditorConfigFile>,
    
    /// File watcher for live reload
    watcher: Option<notify::RecommendedWatcher>,
}

impl ConfigResolver {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            watcher: None,
        }
    }
    
    /// Resolve configuration for a file path
    pub fn resolve(&mut self, file_path: &Path) -> FileConfig {
        let mut config = FileConfig::default();
        let configs = self.find_configs(file_path);
        
        // Apply configs from root to file (root first, nearest last)
        for ec_file in configs.iter().rev() {
            config.sources.push(ec_file.path.clone());
            self.apply_matching_sections(&mut config, ec_file, file_path);
        }
        
        config
    }
    
    /// Find all .editorconfig files from file to root
    fn find_configs(&mut self, file_path: &Path) -> Vec<EditorConfigFile> {
        let mut configs = Vec::new();
        let mut search_dir = file_path.parent();
        
        while let Some(dir) = search_dir {
            let ec_path = dir.join(".editorconfig");
            
            if ec_path.exists() {
                let ec_file = self.cache.entry(ec_path.clone())
                    .or_insert_with(|| {
                        EditorConfigFile::parse(&ec_path).unwrap_or_else(|_| {
                            EditorConfigFile {
                                path: ec_path.clone(),
                                root: false,
                                sections: Vec::new(),
                            }
                        })
                    })
                    .clone();
                
                let is_root = ec_file.root;
                configs.push(ec_file);
                
                if is_root {
                    break;
                }
            }
            
            search_dir = dir.parent();
        }
        
        configs
    }
    
    /// Apply sections that match the file
    fn apply_matching_sections(
        &self,
        config: &mut FileConfig,
        ec_file: &EditorConfigFile,
        file_path: &Path,
    ) {
        let file_name = file_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let relative_path = file_path.strip_prefix(ec_file.path.parent().unwrap())
            .ok()
            .and_then(|p| p.to_str())
            .unwrap_or(file_name);
        
        for section in &ec_file.sections {
            if self.pattern_matches(&section.pattern, file_name, relative_path) {
                self.apply_properties(config, &section.properties);
            }
        }
    }
    
    /// Check if pattern matches file
    fn pattern_matches(&self, pattern: &str, file_name: &str, relative_path: &str) -> bool {
        // Handle common patterns
        if pattern == "*" {
            return true;
        }
        
        // Simple extension matching: *.ext
        if pattern.starts_with("*.") {
            let ext = &pattern[2..];
            return file_name.ends_with(&format!(".{}", ext));
        }
        
        // Brace expansion: *.{js,ts}
        if pattern.contains('{') && pattern.contains('}') {
            // Parse and expand braces
            return self.match_brace_pattern(pattern, file_name);
        }
        
        // Path pattern with **
        if pattern.contains("**") {
            return self.match_glob_pattern(pattern, relative_path);
        }
        
        // Exact match
        pattern == file_name || pattern == relative_path
    }
    
    fn match_brace_pattern(&self, pattern: &str, file_name: &str) -> bool {
        // e.g., "*.{js,ts,jsx,tsx}" -> check each extension
        if let Some(start) = pattern.find('{') {
            if let Some(end) = pattern.find('}') {
                let prefix = &pattern[..start];
                let suffix = &pattern[end+1..];
                let alternatives = pattern[start+1..end].split(',');
                
                for alt in alternatives {
                    let expanded = format!("{}{}{}", prefix, alt.trim(), suffix);
                    if self.pattern_matches(&expanded, file_name, file_name) {
                        return true;
                    }
                }
            }
        }
        false
    }
    
    fn match_glob_pattern(&self, _pattern: &str, _path: &str) -> bool {
        // Use globset crate for full glob matching
        todo!()
    }
    
    /// Apply parsed properties to config
    fn apply_properties(&self, config: &mut FileConfig, props: &HashMap<String, String>) {
        for (key, value) in props {
            match key.as_str() {
                "indent_style" => {
                    config.indent_style = match value.to_lowercase().as_str() {
                        "tab" => IndentStyle::Tab,
                        _ => IndentStyle::Space,
                    };
                }
                "indent_size" => {
                    config.indent_size = if value == "tab" {
                        IndentSize::Tab
                    } else if let Ok(n) = value.parse() {
                        IndentSize::Value(n)
                    } else {
                        config.indent_size
                    };
                }
                "tab_width" => {
                    if let Ok(n) = value.parse() {
                        config.tab_width = n;
                    }
                }
                "end_of_line" => {
                    config.end_of_line = match value.to_lowercase().as_str() {
                        "crlf" => EndOfLine::Crlf,
                        "cr" => EndOfLine::Cr,
                        _ => EndOfLine::Lf,
                    };
                }
                "charset" => {
                    config.charset = match value.to_lowercase().as_str() {
                        "utf-8-bom" => Charset::Utf8Bom,
                        "utf-16be" => Charset::Utf16Be,
                        "utf-16le" => Charset::Utf16Le,
                        "latin1" => Charset::Latin1,
                        _ => Charset::Utf8,
                    };
                }
                "trim_trailing_whitespace" => {
                    config.trim_trailing_whitespace = value.to_lowercase() == "true";
                }
                "insert_final_newline" => {
                    config.insert_final_newline = value.to_lowercase() == "true";
                }
                "max_line_length" => {
                    if value != "off" {
                        config.max_line_length = value.parse().ok();
                    }
                }
                _ => {}
            }
        }
    }
    
    /// Invalidate cache for a specific .editorconfig
    pub fn invalidate(&mut self, path: &Path) {
        self.cache.remove(path);
    }
    
    /// Clear entire cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}
```

### Document Integration

```rust
// In src/model/document.rs

pub struct Document {
    // ... existing fields ...
    
    /// EditorConfig settings for this file
    pub file_config: FileConfig,
}

impl Document {
    /// Apply EditorConfig settings
    pub fn apply_file_config(&mut self, config: FileConfig) {
        self.file_config = config;
        
        // Update internal settings
        self.indent_style = config.indent_style;
        self.indent_size = match config.indent_size {
            IndentSize::Value(n) => n as usize,
            IndentSize::Tab => config.tab_width as usize,
        };
        self.tab_width = config.tab_width as usize;
    }
}
```

---

## Implementation Plan

### Phase 1: Parser

**Estimated effort: 2 days**

1. [ ] Create `src/editorconfig/mod.rs` module
2. [ ] Implement `.editorconfig` file parsing
3. [ ] Handle root=true correctly
4. [ ] Parse all standard properties
5. [ ] Unit tests for parser

**Test:** Parse sample `.editorconfig` files correctly

### Phase 2: Pattern Matching

**Estimated effort: 2 days**

1. [ ] Implement simple glob matching (`*`, `*.ext`)
2. [ ] Implement brace expansion (`{js,ts}`)
3. [ ] Implement `**` path matching
4. [ ] Add `globset` crate for full compatibility
5. [ ] Unit tests for pattern matching

**Test:** Match patterns according to EditorConfig spec

### Phase 3: Resolution

**Estimated effort: 2 days**

1. [ ] Implement config file discovery (walk up to root)
2. [ ] Implement section merging
3. [ ] Implement property cascading
4. [ ] Add caching for parsed files
5. [ ] Integration tests

**Test:** Correct config resolution for nested directories

### Phase 4: Document Integration

**Estimated effort: 2 days**

1. [ ] Add `FileConfig` to `Document`
2. [ ] Resolve config on file open
3. [ ] Apply indent settings to document
4. [ ] Apply end-of-line on save
5. [ ] Apply trim_trailing_whitespace on save
6. [ ] Apply insert_final_newline on save

**Test:** File uses correct settings from `.editorconfig`

### Phase 5: Live Reload

**Estimated effort: 1-2 days**

1. [ ] Add file watcher for `.editorconfig` changes
2. [ ] Invalidate cache on change
3. [ ] Re-resolve config for open documents
4. [ ] Debounce rapid changes

**Test:** Editing `.editorconfig` updates open files

### Phase 6: UI Feedback

**Estimated effort: 1 day**

1. [ ] Show indent style/size in status bar
2. [ ] Show config source on hover (tooltip)
3. [ ] Visual indicator when using editorconfig
4. [ ] Override menu for current file

**Test:** Status bar reflects active configuration

---

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_parse_basic_editorconfig() {
    let content = r#"
root = true

[*]
indent_style = space
indent_size = 4

[*.rs]
indent_size = 4
"#;
    
    let config = EditorConfigFile::parse_str(content, PathBuf::from(".editorconfig")).unwrap();
    assert!(config.root);
    assert_eq!(config.sections.len(), 2);
}

#[test]
fn test_pattern_matching() {
    let resolver = ConfigResolver::new();
    
    assert!(resolver.pattern_matches("*.rs", "main.rs", "main.rs"));
    assert!(resolver.pattern_matches("*.{js,ts}", "app.ts", "app.ts"));
    assert!(!resolver.pattern_matches("*.rs", "main.ts", "main.ts"));
}

#[test]
fn test_property_cascade() {
    // Create resolver with multiple configs
    // Verify later configs override earlier ones
}
```

### Integration Tests

```rust
#[test]
fn test_resolve_real_editorconfig() {
    let temp_dir = tempdir().unwrap();
    
    // Create .editorconfig
    fs::write(temp_dir.path().join(".editorconfig"), r#"
root = true
[*]
indent_size = 2
[*.rs]
indent_size = 4
"#).unwrap();
    
    let mut resolver = ConfigResolver::new();
    
    let rs_config = resolver.resolve(&temp_dir.path().join("test.rs"));
    assert_eq!(rs_config.indent_size, IndentSize::Value(4));
    
    let js_config = resolver.resolve(&temp_dir.path().join("test.js"));
    assert_eq!(js_config.indent_size, IndentSize::Value(2));
}
```

### Manual Testing Checklist

- [ ] `.editorconfig` in project root is detected
- [ ] Nested `.editorconfig` files override
- [ ] `root = true` stops search
- [ ] Indent style applies correctly
- [ ] Tab width applies correctly
- [ ] Trim trailing whitespace works on save
- [ ] Insert final newline works on save
- [ ] Status bar shows current settings
- [ ] Editing `.editorconfig` updates open files

---

## Sample .editorconfig

```ini
# EditorConfig is awesome: https://EditorConfig.org

root = true

[*]
indent_style = space
indent_size = 4
end_of_line = lf
charset = utf-8
trim_trailing_whitespace = true
insert_final_newline = true

[*.md]
trim_trailing_whitespace = false

[*.{rs,toml}]
indent_size = 4

[*.{js,ts,jsx,tsx,json,yaml,yml}]
indent_size = 2

[Makefile]
indent_style = tab

[*.go]
indent_style = tab
```

---

## Dependencies

```toml
# Optional, for better glob matching
globset = "0.4"
```

---

## References

- EditorConfig specification: https://editorconfig.org/
- EditorConfig core library: https://github.com/editorconfig/editorconfig-core-rust
- `globset` crate: https://docs.rs/globset/latest/globset/
