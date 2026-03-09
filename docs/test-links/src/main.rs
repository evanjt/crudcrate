//! mdBook preprocessor that links documentation examples to their test coverage.
//!
//! Usage in markdown:
//!   {{#test_link filtering}}
//!   {{#test_link filtering::null_field}}
//!
//! Generates a link like:
//!   [📋 See test](https://github.com/evanjt/crudcrate/blob/v0.7.1/test_suite/tests/comprehensive_filtering_test.rs#L45-L67)

use mdbook_core::book::{Book, BookItem};
use mdbook_core::errors::Error;
use mdbook_preprocessor::{Preprocessor, PreprocessorContext};
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::{self, Read};
use std::process;

/// Test location in the codebase
#[derive(Debug, Clone, Deserialize)]
struct TestLocation {
    file: String,
    start_line: usize,
    end_line: Option<usize>,
    description: String,
}

/// Registry mapping feature names to test locations
#[derive(Debug, Default)]
struct TestRegistry {
    tests: HashMap<String, TestLocation>,
}

impl TestRegistry {
    fn new() -> Self {
        let mut registry = Self::default();

        // =================================================================
        // FILTERING TESTS
        // =================================================================
        registry.add("filtering", TestLocation {
            file: "test_suite/tests/comprehensive_filtering_test.rs".into(),
            start_line: 1,
            end_line: None,
            description: "Comprehensive filtering tests".into(),
        });
        registry.add("filtering::null", TestLocation {
            file: "test_suite/tests/comprehensive_filtering_test.rs".into(),
            start_line: 28,
            end_line: Some(55),
            description: "Filter by null field".into(),
        });
        registry.add("filtering::comparison", TestLocation {
            file: "test_suite/tests/comprehensive_filtering_test.rs".into(),
            start_line: 57,
            end_line: Some(120),
            description: "Comparison operators (_gt, _gte, _lt, _lte, _neq)".into(),
        });

        // =================================================================
        // SORTING TESTS
        // =================================================================
        registry.add("sorting", TestLocation {
            file: "test_suite/tests/comprehensive_sorting_test.rs".into(),
            start_line: 1,
            end_line: None,
            description: "Comprehensive sorting tests".into(),
        });

        // =================================================================
        // PAGINATION TESTS
        // =================================================================
        registry.add("pagination", TestLocation {
            file: "test_suite/tests/comprehensive_pagination_test.rs".into(),
            start_line: 1,
            end_line: None,
            description: "Comprehensive pagination tests".into(),
        });
        registry.add("pagination::range", TestLocation {
            file: "test_suite/tests/comprehensive_pagination_test.rs".into(),
            start_line: 25,
            end_line: Some(60),
            description: "Range parameter format".into(),
        });

        // =================================================================
        // FULLTEXT SEARCH TESTS
        // =================================================================
        registry.add("fulltext", TestLocation {
            file: "test_suite/tests/comprehensive_fulltext_test.rs".into(),
            start_line: 1,
            end_line: None,
            description: "Fulltext search tests".into(),
        });

        // =================================================================
        // EXCLUDE TESTS
        // =================================================================
        registry.add("exclude", TestLocation {
            file: "test_suite/tests/exclude_functionality_test.rs".into(),
            start_line: 1,
            end_line: None,
            description: "Field exclusion tests".into(),
        });
        registry.add("exclude::create", TestLocation {
            file: "test_suite/tests/exclude_functionality_test.rs".into(),
            start_line: 58,
            end_line: Some(95),
            description: "exclude(create) tests".into(),
        });
        registry.add("exclude::update", TestLocation {
            file: "test_suite/tests/exclude_functionality_test.rs".into(),
            start_line: 97,
            end_line: Some(140),
            description: "exclude(update) tests".into(),
        });
        registry.add("exclude::one", TestLocation {
            file: "test_suite/tests/exclude_functionality_test.rs".into(),
            start_line: 28,
            end_line: Some(56),
            description: "exclude(one) tests".into(),
        });
        registry.add("exclude::list", TestLocation {
            file: "test_suite/tests/exclude_functionality_test.rs".into(),
            start_line: 142,
            end_line: Some(180),
            description: "exclude(list) tests".into(),
        });

        // =================================================================
        // RELATIONSHIP/JOIN TESTS
        // =================================================================
        registry.add("relationships", TestLocation {
            file: "test_suite/tests/join_functionality_test.rs".into(),
            start_line: 1,
            end_line: None,
            description: "Relationship/join loading tests".into(),
        });
        registry.add("relationships::depth", TestLocation {
            file: "test_suite/tests/relationship_depth_test.rs".into(),
            start_line: 1,
            end_line: None,
            description: "Relationship depth tests".into(),
        });
        registry.add("relationships::recursive", TestLocation {
            file: "test_suite/tests/test_deep_recursion.rs".into(),
            start_line: 1,
            end_line: None,
            description: "Deep recursive join tests".into(),
        });

        // =================================================================
        // HOOK TESTS
        // =================================================================
        registry.add("hooks", TestLocation {
            file: "test_suite/tests/hook_system_test.rs".into(),
            start_line: 1,
            end_line: None,
            description: "Hook system tests".into(),
        });
        registry.add("hooks::create", TestLocation {
            file: "test_suite/tests/hook_system_test.rs".into(),
            start_line: 50,
            end_line: Some(120),
            description: "Create hooks tests".into(),
        });
        registry.add("hooks::update", TestLocation {
            file: "test_suite/tests/hook_system_test.rs".into(),
            start_line: 122,
            end_line: Some(190),
            description: "Update hooks tests".into(),
        });
        registry.add("hooks::delete", TestLocation {
            file: "test_suite/tests/hook_system_test.rs".into(),
            start_line: 192,
            end_line: Some(250),
            description: "Delete hooks tests".into(),
        });

        // =================================================================
        // CRUD TESTS
        // =================================================================
        registry.add("crud", TestLocation {
            file: "test_suite/tests/basic_crud_test.rs".into(),
            start_line: 1,
            end_line: None,
            description: "Basic CRUD operations".into(),
        });
        registry.add("crud::create", TestLocation {
            file: "test_suite/tests/basic_crud_test.rs".into(),
            start_line: 25,
            end_line: Some(60),
            description: "Create operation tests".into(),
        });
        registry.add("crud::read", TestLocation {
            file: "test_suite/tests/basic_crud_test.rs".into(),
            start_line: 62,
            end_line: Some(100),
            description: "Read operation tests".into(),
        });
        registry.add("crud::update", TestLocation {
            file: "test_suite/tests/basic_crud_test.rs".into(),
            start_line: 102,
            end_line: Some(145),
            description: "Update operation tests".into(),
        });
        registry.add("crud::delete", TestLocation {
            file: "test_suite/tests/basic_crud_test.rs".into(),
            start_line: 147,
            end_line: Some(190),
            description: "Delete operation tests".into(),
        });

        // =================================================================
        // AUTO-GENERATED VALUES TESTS
        // =================================================================
        registry.add("on_create", TestLocation {
            file: "test_suite/tests/exclude_functionality_test.rs".into(),
            start_line: 182,
            end_line: Some(220),
            description: "on_create auto-generation tests".into(),
        });
        registry.add("on_update", TestLocation {
            file: "test_suite/tests/exclude_functionality_test.rs".into(),
            start_line: 222,
            end_line: Some(265),
            description: "on_update auto-generation tests".into(),
        });

        registry
    }

    fn add(&mut self, key: &str, location: TestLocation) {
        self.tests.insert(key.to_string(), location);
    }

    fn get(&self, key: &str) -> Option<&TestLocation> {
        self.tests.get(key)
    }
}

struct TestLinksPreprocessor {
    registry: TestRegistry,
}

impl TestLinksPreprocessor {
    fn new() -> Self {
        Self {
            registry: TestRegistry::new(),
        }
    }

    fn generate_link(&self, key: &str, version: &str, repo_url: &str) -> String {
        match self.registry.get(key) {
            Some(loc) => {
                let line_fragment = match loc.end_line {
                    Some(end) => format!("#L{}-L{}", loc.start_line, end),
                    None => format!("#L{}", loc.start_line),
                };

                let url = format!(
                    "{}/blob/{}/{}{}",
                    repo_url, version, loc.file, line_fragment
                );

                format!(
                    "<span class=\"test-link\"><a href=\"{}\" target=\"_blank\" title=\"{}\">📋 See test</a></span>",
                    url, loc.description
                )
            }
            None => {
                eprintln!("Warning: Unknown test link key: {}", key);
                format!("<!-- Unknown test link: {} -->", key)
            }
        }
    }

    fn process_content(&self, content: &str, version: &str, repo_url: &str) -> String {
        let re = Regex::new(r"\{\{#test_link\s+([a-z_:]+)\s*\}\}").unwrap();

        re.replace_all(content, |caps: &regex::Captures| {
            let key = &caps[1];
            self.generate_link(key, version, repo_url)
        })
        .to_string()
    }
}

impl Preprocessor for TestLinksPreprocessor {
    fn name(&self) -> &str {
        "test-links"
    }

    fn run(&self, _ctx: &PreprocessorContext, mut book: Book) -> Result<Book, Error> {
        // Use default repo URL
        let repo_url = "https://github.com/evanjt/crudcrate";

        // Try to get version from environment or config
        let version = std::env::var("CRUDCRATE_VERSION")
            .or_else(|_| std::env::var("GITHUB_REF_NAME"))
            .unwrap_or_else(|_| "main".to_string());

        // Process each chapter
        book.for_each_mut(|item| {
            if let BookItem::Chapter(chapter) = item {
                chapter.content = self.process_content(&chapter.content, &version, repo_url);
            }
        });

        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> Result<bool, Error> {
        Ok(renderer == "html")
    }
}

fn main() {
    let preprocessor = TestLinksPreprocessor::new();

    // Check if we're being called to see if we support a renderer
    if std::env::args().nth(1).as_deref() == Some("supports") {
        let renderer = std::env::args().nth(2).unwrap_or_default();
        match preprocessor.supports_renderer(&renderer) {
            Ok(true) => process::exit(0),
            Ok(false) => process::exit(1),
            Err(e) => {
                eprintln!("Error checking renderer support: {}", e);
                process::exit(1);
            }
        }
    }

    // Read input from stdin
    let mut input = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut input) {
        eprintln!("Failed to read input: {}", e);
        process::exit(1);
    }

    // Parse input as JSON
    let (ctx, book): (PreprocessorContext, Book) = match serde_json::from_str(&input) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Failed to parse input JSON: {}", e);
            process::exit(1);
        }
    };

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
    fn test_link_generation() {
        let preprocessor = TestLinksPreprocessor::new();

        let result = preprocessor.generate_link("filtering", "v0.7.1", "https://github.com/evanjt/crudcrate");
        assert!(result.contains("comprehensive_filtering_test.rs"));
        assert!(result.contains("v0.7.1"));
    }

    #[test]
    fn test_content_replacement() {
        let preprocessor = TestLinksPreprocessor::new();

        let content = "Some text {{#test_link filtering}} more text";
        let result = preprocessor.process_content(content, "main", "https://github.com/evanjt/crudcrate");

        assert!(!result.contains("{{#test_link"));
        assert!(result.contains("See test"));
    }

    #[test]
    fn test_unknown_key() {
        let preprocessor = TestLinksPreprocessor::new();

        let result = preprocessor.generate_link("unknown_feature", "main", "https://github.com/evanjt/crudcrate");
        assert!(result.contains("Unknown test link"));
    }
}
