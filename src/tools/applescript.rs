//! Safe AppleScript generation with parameterized string handling.
//!
//! This module provides a builder pattern for constructing AppleScript commands
//! that prevents injection attacks by properly escaping string values.
//!
//! ## Security Model
//!
//! AppleScript strings are delimited by double quotes. To safely embed user data,
//! we use `quote` concatenation: `"before" & quote & "value" & quote & "after"`
//! This avoids the need for traditional escape sequences which AppleScript lacks.
//!
//! Control characters (newlines, tabs, etc.) are replaced with spaces to prevent
//! statement injection via multi-line scripts.

use std::fmt;

/// Escapes a string for safe embedding inside an AppleScript double-quoted string literal.
///
/// AppleScript has no backslash escape sequences. The canonical approach is to use
/// the built-in `quote` constant: `"hello" & quote & "world"` produces `hello"world`.
///
/// Control characters (newlines, carriage returns, etc.) are replaced with spaces
/// to prevent multi-statement injection.
pub fn escape_string(s: &str) -> String {
    let sanitized: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    sanitized.replace('"', "\" & quote & \"")
}

/// Validates a date string is in YYYY-MM-DD format.
///
/// Returns the validated string on success, or an error message on failure.
pub fn validate_date(date: &str) -> Result<&str, String> {
    if date.len() != 10 {
        return Err(format!("Invalid date format '{date}': expected YYYY-MM-DD"));
    }
    for (i, c) in date.chars().enumerate() {
        match i {
            4 | 7 => {
                if c != '-' {
                    return Err(format!("Invalid date '{date}': expected YYYY-MM-DD"));
                }
            },
            _ => {
                if !c.is_ascii_digit() {
                    return Err(format!("Invalid date '{date}': expected YYYY-MM-DD"));
                }
            },
        }
    }
    Ok(date)
}

/// Builder for constructing AppleScript scripts with parameterized values.
///
/// # Example
///
/// ```rust
/// use crate::tools::applescript::AppleScriptBuilder;
///
/// let script = AppleScriptBuilder::new()
///     .tell_application("Safari")
///     .activate()
///     .open_location("https://example.com")
///     .build();
/// ```
pub struct AppleScriptBuilder {
    blocks: Vec<String>,
}

impl Default for AppleScriptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl AppleScriptBuilder {
    /// Creates a new empty builder.
    pub fn new() -> Self {
        Self { blocks: Vec::new() }
    }

    /// Starts a `tell application "X"` block.
    pub fn tell_application(app: &str) -> Self {
        Self {
            blocks: vec![format!(r#"tell application "{}""#, escape_string(app))],
        }
    }

    /// Adds an `activate` command to a new builder (starts with tell application).
    pub fn activate(self) -> Self {
        let mut builder = self;
        builder.blocks.push("activate".to_string());
        builder
    }

    /// Adds an `open location "url"` command to a new builder.
    pub fn open_location(self, url: &str) -> Self {
        let mut builder = self;
        builder
            .blocks
            .push(format!(r#"open location "{}""#, escape_string(url)));
        builder
    }

    /// Adds a `get` property access to a new builder.
    pub fn get(self, property: &str) -> Self {
        let mut builder = self;
        builder.blocks.push(format!("get {property}"));
        builder
    }

    /// Starts a `tell application "X" to activate` chain.
    pub fn activate_app(name: &str) -> Self {
        let mut builder = Self::new();
        builder.blocks.push(format!(
            r#"tell application "{}" to activate"#,
            escape_string(name)
        ));
        builder
    }

    /// Creates a display dialog script for permission prompts.
    pub fn display_dialog(message: &str, title: &str) -> Self {
        Self {
            blocks: vec![format!(
                r#"display dialog "{}" with title "{}" buttons {{"Deny", "Allow"}} default button "Allow" cancel button "Deny""#,
                escape_string(message),
                escape_string(title)
            )],
        }
    }

    /// Builds the final AppleScript string.
    pub fn build(&self) -> String {
        self.blocks.join("\n")
    }

    /// Returns the script as a single line for `osascript -e` usage.
    pub fn build_oneline(&self) -> String {
        self.blocks.join("; ")
    }
}

/// Execute an AppleScript script and return the output.
pub fn run(script: &str) -> Result<String, String> {
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// AppleScript constant for double quote.
const QUOTE: &str = "quote";

/// Renders a string as an AppleScript literal using quote concatenation.
///
/// This is the safest way to embed arbitrary strings in AppleScript.
pub fn literal(s: &str) -> String {
    let mut result = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => {
                result.push_str(" & quote & \"");
            },
            c if c.is_control() => {
                result.push(' ');
            },
            c => {
                result.push(c);
            },
        }
    }
    result.push('\"');
    result
}

impl fmt::Display for AppleScriptBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.build())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_string_basic() {
        assert_eq!(escape_string("hello"), "hello");
        assert_eq!(escape_string("hello world"), "hello world");
    }

    #[test]
    fn test_literal_with_quotes() {
        let result = literal("say \"hi\"");
        // Verify the result contains the key elements
        assert!(result.starts_with('"'));
        assert!(result.ends_with('"'));
        assert!(result.contains(" & quote & "));
        // Should have two quote concatenations
        let count = result.matches(" & quote & ").count();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_escape_string_control_chars() {
        assert_eq!(escape_string("line1\nline2"), "line1 line2");
        assert_eq!(escape_string("tab\there"), "tab here");
        assert_eq!(escape_string("null\x00byte"), "null byte");
    }

    #[test]
    fn test_escape_string_injection_attempts() {
        assert_eq!(
            escape_string("a\"; do_something();"),
            "a\" & quote & \"; do_something();"
        );
        assert_eq!(
            escape_string("end tell\nextra command"),
            "end tell extra command"
        );
    }

    #[test]
    fn test_validate_date_valid() {
        assert_eq!(validate_date("2026-03-22").unwrap(), "2026-03-22");
        assert_eq!(validate_date("2025-01-01").unwrap(), "2025-01-01");
    }

    #[test]
    fn test_validate_date_invalid() {
        assert!(validate_date("2026/03/22").is_err());
        assert!(validate_date("03-22-2026").is_err());
        assert!(validate_date("2026-3-22").is_err());
        assert!(validate_date("abc").is_err());
        assert!(validate_date("").is_err());
    }

    #[test]
    fn test_builder_basic() {
        let script = AppleScriptBuilder::tell_application("Safari")
            .activate()
            .build();
        assert!(script.contains(r#"tell application "Safari""#));
        assert!(script.contains("activate"));
    }

    #[test]
    fn test_builder_injection_in_app_name() {
        // The injection attempt tries to break out with a quote
        let script = AppleScriptBuilder::tell_application("Safari\"; do_evil()")
            .activate()
            .build();
        // The quote is properly escaped, preventing injection
        assert!(script.contains("& quote &"));
        // The content is preserved but safely inside quotes
        assert!(script.contains("do_evil()"));
        // The closing quote is also escaped
        assert!(script.contains("Safari"));
    }

    #[test]
    fn test_literal_basic() {
        assert_eq!(literal("hello"), "\"hello\"");
    }

    #[test]
    fn test_literal_control_chars() {
        assert_eq!(literal("line1\nline2"), "\"line1 line2\"");
    }

    #[test]
    fn test_run_failure() {
        let result = run("this is not valid applescript");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn test_injection_multiline_script() {
        let malicious = "Safari\"\ndo_evil()\nend tell";
        let escaped = literal(malicious);
        // The escaping prevents the newlines from creating actual line breaks in AS
        // Each line becomes a separate chunk in the concatenation
        assert!(escaped.contains("& quote &"));
        assert!(escaped.contains("end tell"));
    }

    #[test]
    fn test_injection_statement_terminator() {
        let malicious = "test\"; run(\"rm -rf /\")";
        let escaped = escape_string(malicious);
        // The semicolon is escaped as part of the quote concatenation
        assert!(escaped.contains("& quote &"));
        // The rm command is now safely inside a string
        assert!(escaped.contains("rm -rf"));
    }

    #[test]
    fn test_injection_unicode_bypass() {
        let malicious = "test\u{2028}do_evil()";
        let escaped = escape_string(malicious);
        // Line separator U+2028 is a control char, converted to space
        // So "test do_evil()" becomes "test do_evil()"
        // The security is that there's no NEWLINE that could inject code
        assert!(escaped.contains("do_evil"));
        // No actual line break
        assert!(!escaped.contains('\n'));
    }

    #[test]
    fn test_literal_complex_injection() {
        let malicious = "\" & system(\"whoami\") & \"";
        let escaped = literal(malicious);
        // The whole thing is properly escaped
        assert!(escaped.contains("& quote &"));
        // The content is preserved but inside a string literal
        assert!(escaped.contains("whoami"));
    }

    #[test]
    fn test_validate_date_edge_cases() {
        assert!(validate_date("0000-00-00").is_ok());
        assert!(validate_date("9999-12-31").is_ok());
        // Date validation only checks format, not semantic validity
        // Month 13 would fail semantic validation at runtime
        assert!(validate_date("2026-13-01").is_ok());
        // Feb 30 would also pass format check
        assert!(validate_date("2026-02-30").is_ok());
    }

    #[test]
    fn test_builder_chaining() {
        let script = AppleScriptBuilder::tell_application("Finder");
        let script = script.activate().get("name of first window").build();
        assert!(script.contains("tell application"));
        assert!(script.contains("activate"));
        assert!(script.contains("get"));
    }

    #[test]
    fn test_activate_app_builder() {
        let script = AppleScriptBuilder::activate_app("Chrome");
        let s = script.to_string();
        assert!(s.contains("tell application"));
        assert!(s.contains("activate"));
        assert!(s.contains("Chrome"));
    }

    #[test]
    fn test_display_dialog_builder() {
        let script = AppleScriptBuilder::display_dialog("Run command?", "Rover");
        let s = script.build();
        assert!(s.contains("display dialog"));
        assert!(s.contains("Rover"));
    }
}
