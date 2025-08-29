use proc_macro2::TokenStream;

/// ANSI color codes for syntax highlighting
mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    
    // Syntax highlighting colors
    pub const KEYWORD: &str = "\x1b[34m";        // Blue
    pub const TYPE: &str = "\x1b[36m";           // Cyan  
    pub const STRING: &str = "\x1b[32m";         // Green
    pub const COMMENT: &str = "\x1b[90m";        // Gray
    pub const ATTRIBUTE: &str = "\x1b[35m";      // Magenta
    pub const FUNCTION: &str = "\x1b[33m";       // Yellow
    pub const STRUCT_NAME: &str = "\x1b[96m";    // Bright Cyan
}

/// Pretty prints generated code with syntax highlighting and formatting
pub fn print_debug_output(tokens: &TokenStream, struct_name: &str) {
    print_debug_section("EntityToModels", struct_name, tokens);
}

/// Print debug output for other derive macros
pub fn print_create_model_debug(tokens: &TokenStream, struct_name: &str) {
    let create_name = format!("{}Create", struct_name);
    print_debug_section("ToCreateModel", &create_name, tokens);
}

/// Print debug output for other derive macros  
pub fn print_update_model_debug(tokens: &TokenStream, struct_name: &str) {
    let update_name = format!("{}Update", struct_name);
    print_debug_section("ToUpdateModel", &update_name, tokens);
}

/// Generic debug output printer with section headers
fn print_debug_section(macro_name: &str, struct_name: &str, tokens: &TokenStream) {
    let formatted_code = format_and_colourise_rust_code(tokens);
    
    println!("\n{}{}🔍 {} Debug Output for '{}'{}", 
             colors::BOLD, colors::STRUCT_NAME, macro_name, struct_name, colors::RESET);
    println!("{}{}─────────────────────────────────────────────────────────────{}", 
             colors::DIM, "─".repeat(50), colors::RESET);
    
    for (i, line) in formatted_code.lines().enumerate() {
        let line_num = format!("{:3}", i + 1);
        println!("{}{} │{} {}", colors::DIM, line_num, colors::RESET, line);
    }
    
    println!("{}{}─────────────────────────────────────────────────────────────{}", 
             colors::DIM, "─".repeat(50), colors::RESET);
    println!("{}Generated {} lines of code{}\n", colors::DIM, formatted_code.lines().count(), colors::RESET);
}

/// Format and colourise Rust code using prettyplease for perfect formatting
fn format_and_colourise_rust_code(tokens: &TokenStream) -> String {
    // Parse the tokens back into a syntax tree and format with prettyplease
    #[cfg(feature = "debug")]
    let formatted_code = match syn::parse2::<syn::File>(tokens.clone()) {
        Ok(file) => prettyplease::unparse(&file),
        Err(_) => {
            // Fallback to basic formatting if parsing fails
            tokens.to_string()
        }
    };
    
    #[cfg(not(feature = "debug"))]
    let formatted_code = tokens.to_string();
    
    // Apply syntax highlighting
    colourise_rust_code(&formatted_code)
}

/// Apply syntax highlighting to already-formatted Rust code
fn colourise_rust_code(code: &str) -> String {
    let mut result = String::new();
    let mut current_word = String::new();
    let mut in_string = false;
    let mut in_comment = false;
    let mut chars = code.chars().peekable();
    
    while let Some(ch) = chars.next() {
        match ch {
            // Handle string literals
            '"' if !in_comment => {
                if !current_word.is_empty() {
                    result.push_str(&colourise_word(&current_word));
                    current_word.clear();
                }
                
                if in_string {
                    result.push(ch);
                    result.push_str(colors::RESET);
                    in_string = false;
                } else {
                    result.push_str(colors::STRING);
                    result.push(ch);
                    in_string = true;
                }
            }
            
            // Handle line comments
            '/' if !in_string && chars.peek() == Some(&'/') => {
                if !current_word.is_empty() {
                    result.push_str(&colourise_word(&current_word));
                    current_word.clear();
                }
                result.push_str(colors::COMMENT);
                result.push(ch);
                in_comment = true;
            }
            
            // Handle newlines (reset comment state)
            '\n' => {
                if !current_word.is_empty() {
                    result.push_str(&colourise_word(&current_word));
                    current_word.clear();
                }
                if in_comment {
                    result.push_str(colors::RESET);
                    in_comment = false;
                }
                result.push(ch);
            }
            
            // Handle word boundaries
            ch if ch.is_whitespace() || "(){}[];,<>=!&|+-*/%:".contains(ch) => {
                if !current_word.is_empty() && !in_string && !in_comment {
                    result.push_str(&colourise_word(&current_word));
                    current_word.clear();
                }
                result.push(ch);
            }
            
            // Accumulate word characters
            _ => {
                if in_string || in_comment {
                    result.push(ch);
                } else {
                    current_word.push(ch);
                }
            }
        }
    }
    
    // Handle any remaining word
    if !current_word.is_empty() {
        result.push_str(&colourise_word(&current_word));
    }
    
    // Ensure we end with reset if needed
    if in_string || in_comment {
        result.push_str(colors::RESET);
    }
    
    result
}

/// Colourise individual words based on Rust syntax
fn colourise_word(word: &str) -> String {
    let colored = match word {
        // Rust keywords
        "pub" | "struct" | "impl" | "fn" | "let" | "mut" | "const" | "static" |
        "use" | "mod" | "crate" | "super" | "self" | "Self" | "enum" | "trait" |
        "where" | "for" | "in" | "if" | "else" | "match" | "return" | "async" |
        "await" | "move" | "ref" | "type" | "unsafe" | "extern" | "macro" => {
            format!("{}{}{}", colors::KEYWORD, word, colors::RESET)
        }
        
        // Common types
        "String" | "Vec" | "Option" | "Result" | "Box" | "Arc" | "Rc" | "HashMap" |
        "BTreeMap" | "HashSet" | "BTreeSet" | "u8" | "u16" | "u32" | "u64" | "u128" |
        "i8" | "i16" | "i32" | "i64" | "i128" | "f32" | "f64" | "bool" | "char" |
        "usize" | "isize" | "str" | "&str" | "Uuid" | "DateTime" | "NaiveDateTime" => {
            format!("{}{}{}", colors::TYPE, word, colors::RESET)
        }
        
        // Attributes (words starting with #)
        word if word.starts_with('#') => {
            format!("{}{}{}", colors::ATTRIBUTE, word, colors::RESET)
        }
        
        // Function-like words (followed by parentheses often)
        word if word.ends_with("!") => {
            format!("{}{}{}", colors::FUNCTION, word, colors::RESET)
        }
        
        // Default case
        _ => word.to_string()
    };
    
    colored
}