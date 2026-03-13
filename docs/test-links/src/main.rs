//! mdBook preprocessor that links documentation examples to their test coverage.
//!
//! Test files annotate their connection to docs using `@doc-link` comments:
//!   // @doc-link-file filtering          — link key to entire file
//!   // @doc-link filtering::null         — link key to the next test function
//!   // @doc-link filtering::comparison {start}  — start of a range
//!   // @doc-link filtering::comparison {end}    — end of a range
//!
//! Usage in markdown:
//!   {{#test_link filtering}}
//!
//! Generates a link like:
//!   [See test](https://github.com/evanjt/crudcrate/blob/v0.7.1/test_suite/tests/comprehensive_filtering_test.rs#L45-L67)

use mdbook_core::book::{Book, BookItem};
use mdbook_core::errors::Error;
use mdbook_preprocessor::{Preprocessor, PreprocessorContext};
use regex::Regex;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process;

/// Test location in the codebase
#[derive(Debug, Clone, Deserialize)]
struct TestLocation {
    file: String,
    start_line: usize,
    end_line: Option<usize>,
    description: String,
}

/// Registry mapping feature names to test locations, built by scanning test files
#[derive(Debug, Default)]
struct TestRegistry {
    tests: HashMap<String, TestLocation>,
}

impl TestRegistry {
    /// Scan all `.rs` files in `test_dir` for `@doc-link` annotations.
    /// File paths in the registry are stored relative to `repo_root`.
    fn from_scan(test_dir: &Path, repo_root: &Path) -> Result<Self, Vec<String>> {
        let mut registry = Self::default();
        let mut errors = Vec::new();

        let entries = match fs::read_dir(test_dir) {
            Ok(e) => e,
            Err(e) => {
                return Err(vec![format!(
                    "Failed to read {}: {}",
                    test_dir.display(),
                    e
                )])
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    errors.push(format!("Failed to read directory entry: {}", e));
                    continue;
                }
            };

            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }

            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    errors.push(format!("Failed to read {}: {}", path.display(), e));
                    continue;
                }
            };

            let rel_path = path.strip_prefix(repo_root).unwrap_or(&path);
            let rel_path_str = rel_path.to_string_lossy().replace('\\', "/");

            if let Err(scan_errors) = registry.scan_file(&content, &rel_path_str) {
                errors.extend(scan_errors);
            }
        }

        if errors.is_empty() {
            Ok(registry)
        } else {
            Err(errors)
        }
    }

    /// Parse a single file's content for `@doc-link` annotations.
    fn scan_file(&mut self, content: &str, file_path: &str) -> Result<(), Vec<String>> {
        let lines: Vec<&str> = content.lines().collect();
        let mut errors = Vec::new();

        // Track range annotations: key → (start_line, end_line)
        let mut ranges: HashMap<String, (Option<usize>, Option<usize>)> = HashMap::new();

        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1; // 1-based
            let trimmed = line.trim();

            // @doc-link-file <key>
            if let Some(key) = trimmed.strip_prefix("// @doc-link-file ") {
                let key = key.trim().to_string();
                if self.tests.contains_key(&key) {
                    errors.push(format!(
                        "Duplicate @doc-link-file '{}' in {} — already defined elsewhere",
                        key, file_path
                    ));
                } else {
                    self.tests.insert(
                        key.clone(),
                        TestLocation {
                            file: file_path.to_string(),
                            start_line: 1,
                            end_line: None,
                            description: format!("{} tests", key.replace("::", " ")),
                        },
                    );
                }
                continue;
            }

            // @doc-link <key> {start} | @doc-link <key> {end} | @doc-link <key>
            if let Some(rest) = trimmed.strip_prefix("// @doc-link ") {
                let rest = rest.trim();

                if let Some(key) = rest.strip_suffix(" {start}") {
                    let key = key.trim().to_string();
                    ranges.entry(key).or_insert((None, None)).0 = Some(line_num);
                    continue;
                }

                if let Some(key) = rest.strip_suffix(" {end}") {
                    let key = key.trim().to_string();
                    ranges.entry(key).or_insert((None, None)).1 = Some(line_num);
                    continue;
                }

                // Simple function annotation — find next test function
                let key = rest.to_string();
                if let Some((fn_start, fn_end)) = find_next_function(&lines, i + 1) {
                    if self.tests.contains_key(&key) {
                        errors.push(format!(
                            "Duplicate @doc-link '{}' in {} at line {} — already defined elsewhere",
                            key, file_path, line_num
                        ));
                    } else {
                        self.tests.insert(
                            key.clone(),
                            TestLocation {
                                file: file_path.to_string(),
                                start_line: fn_start,
                                end_line: Some(fn_end),
                                description: format!("{} test", key.replace("::", " ")),
                            },
                        );
                    }
                } else {
                    errors.push(format!(
                        "No test function found after @doc-link '{}' at {}:{}",
                        key, file_path, line_num
                    ));
                }
                continue;
            }
        }

        // Process range annotations
        for (key, (start, end)) in ranges {
            match (start, end) {
                (Some(s), Some(e)) => {
                    if self.tests.contains_key(&key) {
                        errors.push(format!(
                            "Duplicate range annotation '{}' in {} — already defined elsewhere",
                            key, file_path
                        ));
                    } else {
                        self.tests.insert(
                            key.clone(),
                            TestLocation {
                                file: file_path.to_string(),
                                start_line: s,
                                end_line: Some(e),
                                description: format!("{} tests", key.replace("::", " ")),
                            },
                        );
                    }
                }
                (Some(_), None) => {
                    errors.push(format!(
                        "Missing {{{{end}}}} for range annotation '{}' in {}",
                        key, file_path
                    ));
                }
                (None, Some(_)) => {
                    errors.push(format!(
                        "Missing {{{{start}}}} for range annotation '{}' in {}",
                        key, file_path
                    ));
                }
                (None, None) => unreachable!(),
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn get(&self, key: &str) -> Option<&TestLocation> {
        self.tests.get(key)
    }

    fn keys(&self) -> HashSet<&str> {
        self.tests.keys().map(|k| k.as_str()).collect()
    }
}

/// Find the next test function after `start_idx` and return `(start_line, end_line)` (1-based).
fn find_next_function(lines: &[&str], start_idx: usize) -> Option<(usize, usize)> {
    for i in start_idx..lines.len() {
        let trimmed = lines[i].trim();

        // Skip comments
        if trimmed.starts_with("//") {
            continue;
        }

        // Match function definitions
        if trimmed.contains("fn test_") {
            let fn_start = i + 1; // 1-based

            // Count braces to find function end
            let mut depth: i32 = 0;
            let mut found_open = false;

            for j in i..lines.len() {
                for ch in lines[j].chars() {
                    match ch {
                        '{' => {
                            depth += 1;
                            found_open = true;
                        }
                        '}' => {
                            depth -= 1;
                        }
                        _ => {}
                    }
                }

                if found_open && depth == 0 {
                    return Some((fn_start, j + 1)); // 1-based
                }
            }
        }
    }
    None
}

/// Scan doc markdown files for `{{#test_link key}}` patterns, returning all used keys.
fn scan_doc_keys(doc_dir: &Path) -> Result<HashSet<String>, String> {
    let re = Regex::new(r"\{\{#test_link\s+([a-z_:]+)\s*\}\}").unwrap();
    let mut keys = HashSet::new();
    scan_doc_dir(doc_dir, &re, &mut keys)?;
    Ok(keys)
}

fn scan_doc_dir(dir: &Path, re: &Regex, keys: &mut HashSet<String>) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|e| format!("Failed to read {}: {}", dir.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
            scan_doc_dir(&path, re, keys)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            let content = fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

            for cap in re.captures_iter(&content) {
                keys.insert(cap[1].to_string());
            }
        }
    }

    Ok(())
}

#[derive(Clone, Copy)]
enum LinkMode {
    Strict,
    Lenient,
}

struct TestLinksPreprocessor {
    registry: TestRegistry,
    mode: LinkMode,
}

impl TestLinksPreprocessor {
    fn new(registry: TestRegistry, mode: LinkMode) -> Self {
        Self { registry, mode }
    }

    fn generate_link(&self, key: &str, version: &str, repo_url: &str) -> Option<String> {
        self.registry.get(key).map(|loc| {
            let line_fragment = match loc.end_line {
                Some(end) => format!("#L{}-L{}", loc.start_line, end),
                None => format!("#L{}", loc.start_line),
            };

            let url = format!(
                "{}/blob/{}/{}{}",
                repo_url, version, loc.file, line_fragment
            );

            format!(
                "<span class=\"test-link\"><a href=\"{}\" target=\"_blank\" title=\"{}\">\u{1f4cb} See test</a></span>",
                url, loc.description
            )
        })
    }

    fn process_content(
        &self,
        content: &str,
        version: &str,
        repo_url: &str,
    ) -> (String, Vec<String>) {
        let re = Regex::new(r"\{\{#test_link\s+([a-z_:]+)\s*\}\}").unwrap();
        let mut errors = Vec::new();
        let is_strict = matches!(self.mode, LinkMode::Strict);

        // First pass: collect errors for missing keys (strict mode only)
        if is_strict {
            for caps in re.captures_iter(content) {
                let key = &caps[1];
                if self.registry.get(key).is_none() {
                    errors.push(format!("Unresolved test link key: {}", key));
                }
            }
        }

        // Second pass: replace all directives
        let result = re
            .replace_all(content, |caps: &regex::Captures| -> String {
                let key = &caps[1];
                self.generate_link(key, version, repo_url)
                    .unwrap_or_else(|| {
                        if is_strict {
                            format!("<!-- Unresolved: {} -->", key)
                        } else {
                            String::new()
                        }
                    })
            })
            .to_string();

        (result, errors)
    }
}

impl Preprocessor for TestLinksPreprocessor {
    fn name(&self) -> &str {
        "test-links"
    }

    fn run(&self, _ctx: &PreprocessorContext, mut book: Book) -> Result<Book, Error> {
        let repo_url = "https://github.com/evanjt/crudcrate";

        let version = std::env::var("CRUDCRATE_VERSION")
            .or_else(|_| std::env::var("GITHUB_REF_NAME"))
            .unwrap_or_else(|_| "main".to_string());

        let mut all_errors = Vec::new();

        book.for_each_mut(|item| {
            if let BookItem::Chapter(chapter) = item {
                let (content, errors) =
                    self.process_content(&chapter.content, &version, repo_url);
                chapter.content = content;
                all_errors.extend(errors);
            }
        });

        if !all_errors.is_empty() {
            let error_msg = format!(
                "test-links: {} unresolved test link(s):\n{}",
                all_errors.len(),
                all_errors
                    .iter()
                    .map(|e| format!("  - {}", e))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            return Err(Error::msg(error_msg));
        }

        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> Result<bool, Error> {
        Ok(renderer == "html")
    }
}

/// Run validation: cross-check doc keys against annotations.
/// Returns Ok(()) if everything matches, Err with a list of problems otherwise.
fn validate(repo_root: &Path) -> Result<(), Vec<String>> {
    let test_dir = repo_root.join("test_suite/tests");
    let doc_dir = repo_root.join("docs/src");

    let mut errors = Vec::new();

    // Build registry from annotations
    let registry = match TestRegistry::from_scan(&test_dir, repo_root) {
        Ok(r) => r,
        Err(scan_errors) => {
            errors.extend(scan_errors);
            return Err(errors);
        }
    };

    // Scan doc files for used keys
    let doc_keys = match scan_doc_keys(&doc_dir) {
        Ok(k) => k,
        Err(e) => {
            errors.push(e);
            return Err(errors);
        }
    };

    let annotation_keys = registry.keys();

    // Doc keys with no matching annotation
    let mut unresolved: Vec<&str> = doc_keys
        .iter()
        .filter(|k| !annotation_keys.contains(k.as_str()))
        .map(|k| k.as_str())
        .collect();
    unresolved.sort();

    for key in &unresolved {
        errors.push(format!(
            "Doc key '{}' has no matching @doc-link annotation",
            key
        ));
    }

    // Annotations with no matching doc key (orphans)
    let mut orphans: Vec<&str> = annotation_keys
        .iter()
        .filter(|k| !doc_keys.contains(&k.to_string()))
        .copied()
        .collect();
    orphans.sort();

    for key in &orphans {
        errors.push(format!(
            "Annotation '{}' has no matching {{{{#test_link}}}} in docs (orphaned)",
            key
        ));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Handle subcommands
    if args.len() > 1 {
        match args[1].as_str() {
            "supports" => {
                let renderer = args.get(2).map(|s| s.as_str()).unwrap_or("");
                if renderer == "html" {
                    process::exit(0);
                } else {
                    process::exit(1);
                }
            }
            "validate" => {
                let repo_root =
                    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

                eprintln!("Scanning test files for @doc-link annotations...");
                eprintln!("Scanning doc files for {{{{#test_link}}}} directives...");

                match validate(&repo_root) {
                    Ok(()) => {
                        eprintln!(
                            "test-links: All doc keys resolve, no orphaned annotations."
                        );
                        process::exit(0);
                    }
                    Err(errors) => {
                        eprintln!("test-links validation failed ({} error(s)):", errors.len());
                        for error in &errors {
                            eprintln!("  - {}", error);
                        }
                        process::exit(1);
                    }
                }
            }
            _ => {
                // Unknown subcommand — fall through to preprocessor mode
            }
        }
    }

    // Preprocessor mode: read JSON from stdin, process, write to stdout
    let mode = match std::env::var("TEST_LINKS_MODE").as_deref() {
        Ok("lenient") => LinkMode::Lenient,
        _ => LinkMode::Strict,
    };

    let mut input = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut input) {
        eprintln!("Failed to read input: {}", e);
        process::exit(1);
    }

    let (ctx, book): (PreprocessorContext, Book) = match serde_json::from_str(&input) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Failed to parse input JSON: {}", e);
            process::exit(1);
        }
    };

    // Derive repo root from book root (book.toml is in docs/, repo root is one level up)
    let repo_root = ctx.root.join("..");
    let test_dir = repo_root.join("test_suite/tests");

    let registry = match TestRegistry::from_scan(&test_dir, &repo_root) {
        Ok(r) => r,
        Err(errors) => {
            for error in &errors {
                eprintln!("test-links scan error: {}", error);
            }
            match mode {
                LinkMode::Strict => process::exit(1),
                LinkMode::Lenient => TestRegistry::default(),
            }
        }
    };

    let preprocessor = TestLinksPreprocessor::new(registry, mode);

    let processed = match preprocessor.run(&ctx, book) {
        Ok(book) => book,
        Err(e) => {
            eprintln!("Failed to process book: {}", e);
            process::exit(1);
        }
    };

    if let Err(e) = serde_json::to_writer(io::stdout(), &processed) {
        eprintln!("Failed to write output: {}", e);
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_next_function_basic() {
        let content = "// comment\n#[tokio::test]\nasync fn test_something() {\n    let x = 1;\n    if x > 0 {\n        println!(\"yes\");\n    }\n}\n";
        let lines: Vec<&str> = content.lines().collect();
        let result = find_next_function(&lines, 0);
        assert!(result.is_some());
        let (start, end) = result.unwrap();
        assert_eq!(start, 3); // fn line is line 3 (1-based)
        assert_eq!(end, 8); // closing } is line 8
    }

    #[test]
    fn test_find_next_function_indented() {
        let content =
            "    #[tokio::test]\n    #[serial]\n    async fn test_foo() {\n        let x = 1;\n    }\n";
        let lines: Vec<&str> = content.lines().collect();
        let result = find_next_function(&lines, 0);
        assert!(result.is_some());
        let (start, end) = result.unwrap();
        assert_eq!(start, 3); // fn line
        assert_eq!(end, 5); // closing }
    }

    #[test]
    fn test_scan_file_level_annotation() {
        let mut registry = TestRegistry::default();
        let content = "// @doc-link-file filtering\nuse something;\n";
        registry
            .scan_file(content, "test_suite/tests/filtering_test.rs")
            .unwrap();

        let loc = registry.get("filtering").unwrap();
        assert_eq!(loc.file, "test_suite/tests/filtering_test.rs");
        assert_eq!(loc.start_line, 1);
        assert_eq!(loc.end_line, None);
    }

    #[test]
    fn test_scan_function_annotation() {
        let mut registry = TestRegistry::default();
        let content = "// @doc-link hooks::create\n#[tokio::test]\nasync fn test_create_hooks_called() {\n    let x = 1;\n}\n";
        registry
            .scan_file(content, "test_suite/tests/hooks_test.rs")
            .unwrap();

        let loc = registry.get("hooks::create").unwrap();
        assert_eq!(loc.start_line, 3); // fn line
        assert_eq!(loc.end_line, Some(5)); // closing }
    }

    #[test]
    fn test_scan_range_annotation() {
        let mut registry = TestRegistry::default();
        let content = "// @doc-link filtering::comparison {start}\n#[tokio::test]\nasync fn test_one() {\n    let x = 1;\n}\n\n#[tokio::test]\nasync fn test_two() {\n    let x = 2;\n}\n// @doc-link filtering::comparison {end}\n";
        registry
            .scan_file(content, "test_suite/tests/filtering_test.rs")
            .unwrap();

        let loc = registry.get("filtering::comparison").unwrap();
        assert_eq!(loc.start_line, 1); // {start} line
        assert_eq!(loc.end_line, Some(11)); // {end} line
    }

    #[test]
    fn test_duplicate_annotation_error() {
        let mut registry = TestRegistry::default();
        registry
            .scan_file(
                "// @doc-link-file filtering\n",
                "test_suite/tests/file1.rs",
            )
            .unwrap();

        let result = registry.scan_file(
            "// @doc-link-file filtering\n",
            "test_suite/tests/file2.rs",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_range_end() {
        let mut registry = TestRegistry::default();
        let result = registry.scan_file(
            "// @doc-link foo::bar {start}\nfn something() {}\n",
            "test.rs",
        );
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors[0].contains("{end}"));
    }

    #[test]
    fn test_link_generation() {
        let mut registry = TestRegistry::default();
        registry.tests.insert(
            "filtering".to_string(),
            TestLocation {
                file: "test_suite/tests/comprehensive_filtering_test.rs".to_string(),
                start_line: 1,
                end_line: None,
                description: "filtering tests".to_string(),
            },
        );

        let preprocessor = TestLinksPreprocessor::new(registry, LinkMode::Strict);
        let result = preprocessor.generate_link(
            "filtering",
            "v0.7.1",
            "https://github.com/evanjt/crudcrate",
        );
        assert!(result.is_some());
        let link = result.unwrap();
        assert!(link.contains("comprehensive_filtering_test.rs"));
        assert!(link.contains("v0.7.1"));
        assert!(link.contains("#L1"));
    }

    #[test]
    fn test_link_generation_with_range() {
        let mut registry = TestRegistry::default();
        registry.tests.insert(
            "filtering::comparison".to_string(),
            TestLocation {
                file: "test_suite/tests/comprehensive_filtering_test.rs".to_string(),
                start_line: 15,
                end_line: Some(320),
                description: "filtering comparison tests".to_string(),
            },
        );

        let preprocessor = TestLinksPreprocessor::new(registry, LinkMode::Strict);
        let result = preprocessor.generate_link(
            "filtering::comparison",
            "main",
            "https://github.com/evanjt/crudcrate",
        );
        assert!(result.is_some());
        let link = result.unwrap();
        assert!(link.contains("#L15-L320"));
    }

    #[test]
    fn test_content_replacement() {
        let mut registry = TestRegistry::default();
        registry.tests.insert(
            "filtering".to_string(),
            TestLocation {
                file: "test_suite/tests/comprehensive_filtering_test.rs".to_string(),
                start_line: 1,
                end_line: None,
                description: "filtering tests".to_string(),
            },
        );

        let preprocessor = TestLinksPreprocessor::new(registry, LinkMode::Strict);
        let content = "Some text {{#test_link filtering}} more text";
        let (result, errors) = preprocessor.process_content(
            content,
            "main",
            "https://github.com/evanjt/crudcrate",
        );
        assert!(errors.is_empty());
        assert!(!result.contains("{{#test_link"));
        assert!(result.contains("See test"));
    }

    #[test]
    fn test_unknown_key_strict_mode() {
        let registry = TestRegistry::default();
        let preprocessor = TestLinksPreprocessor::new(registry, LinkMode::Strict);
        let content = "Text {{#test_link unknown_feature}} more";
        let (_, errors) = preprocessor.process_content(
            content,
            "main",
            "https://github.com/evanjt/crudcrate",
        );
        assert!(!errors.is_empty());
        assert!(errors[0].contains("unknown_feature"));
    }

    #[test]
    fn test_unknown_key_lenient_mode() {
        let registry = TestRegistry::default();
        let preprocessor = TestLinksPreprocessor::new(registry, LinkMode::Lenient);
        let content = "Text {{#test_link unknown_feature}} more";
        let (result, errors) = preprocessor.process_content(
            content,
            "main",
            "https://github.com/evanjt/crudcrate",
        );
        assert!(errors.is_empty());
        assert!(!result.contains("{{#test_link"));
        assert!(!result.contains("unknown_feature"));
    }

    #[test]
    fn test_indented_annotation() {
        let mut registry = TestRegistry::default();
        let content =
            "    // @doc-link hooks::create\n    #[tokio::test]\n    async fn test_create() {\n        let x = 1;\n    }\n";
        registry
            .scan_file(content, "test_suite/tests/hooks_test.rs")
            .unwrap();

        let loc = registry.get("hooks::create").unwrap();
        assert_eq!(loc.start_line, 3);
        assert_eq!(loc.end_line, Some(5));
    }
}
