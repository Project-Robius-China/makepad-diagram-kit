//! Streaming-prefix parsing.
//!
//! LLMs emit JSON one token at a time. [`crate::parse_lossy`] attempts to
//! recover a valid [`crate::Diagram`] from a partial body by appending the
//! missing closing brackets / braces and retrying `serde_json::from_str`.

use crate::errors::ParseError;
use crate::types::Diagram;

/// Attempt to parse a JSON body that may be mid-stream.
///
/// Strategy:
/// 1. Try a direct parse. If it succeeds, return.
/// 2. Otherwise, trim the trailing whitespace, drop a trailing comma if
///    present, then close any open brackets/braces in reverse order.
/// 3. Retry. If that still fails, incrementally back up one token at a time
///    from the tail until either a parseable prefix is found or the stream
///    is exhausted.
/// 4. Return `None` if no non-trivial prefix is parseable.
#[must_use]
pub fn parse_lossy(body: &str) -> Option<Diagram> {
    if body.trim().is_empty() {
        return None;
    }

    // Fast path
    if let Ok(d) = try_parse(body)
        && d.validate().is_ok()
    {
        return Some(d);
    }

    // Try closing open brackets
    let closed = close_open(body);
    if !closed.is_empty()
        && let Ok(d) = try_parse(&closed)
        && d.validate().is_ok()
    {
        return Some(d);
    }

    // Back off from the tail one byte at a time (skipping UTF-8 continuation
    // bytes), closing after each backstep. Cap at a reasonable number of
    // retries — we don't want O(n^2) on a 200 KB body.
    let mut end = body.len();
    let mut tries = 0;
    while end > 0 && tries < 4096 {
        end -= 1;
        while end > 0 && !body.is_char_boundary(end) {
            end -= 1;
        }
        let prefix = &body[..end];
        let closed = close_open(prefix);
        if !closed.is_empty()
            && let Ok(d) = try_parse(&closed)
            && d.validate().is_ok()
        {
            return Some(d);
        }
        tries += 1;
    }

    None
}

/// Attempt `serde_json::from_str` and run post-parse validation. Surface as
/// [`ParseError`] so callers above us can unify handling.
fn try_parse(s: &str) -> Result<Diagram, ParseError> {
    let d: Diagram = serde_json::from_str(s)?;
    Ok(d)
}

trait DiagramValidate {
    fn validate(&self) -> Result<(), ParseError>;
}

impl DiagramValidate for Diagram {
    fn validate(&self) -> Result<(), ParseError> {
        match self {
            Diagram::Pyramid(s) => s.validate(),
            Diagram::Quadrant(s) => s.validate(),
            Diagram::Tree(s) => s.validate(),
            Diagram::Layers(s) => s.validate(),
            Diagram::Flowchart(s) => s.validate(),
        }
    }
}

/// Heuristically close an in-progress JSON body. Walks the string tracking
/// bracket depth (skipping contents of strings), drops a trailing comma at the
/// current depth if present, then appends the matching closers.
///
/// Returns an empty string if the body is not salvageable (for example, an
/// unterminated string spanning the entire rest of the buffer).
fn close_open(body: &str) -> String {
    let bytes = body.as_bytes();
    let mut stack: Vec<u8> = Vec::new();
    let mut in_str = false;
    let mut escape = false;

    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if in_str {
            if escape {
                escape = false;
            } else if c == b'\\' {
                escape = true;
            } else if c == b'"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        match c {
            b'"' => in_str = true,
            b'{' => stack.push(b'}'),
            b'[' => stack.push(b']'),
            b'}' | b']' => {
                stack.pop();
            }
            _ => {}
        }
        i += 1;
    }

    // If we ended inside a string or with a trailing escape, the tail isn't
    // recoverable without a richer parser. Give up on this prefix.
    if in_str || escape {
        return String::new();
    }
    // Nothing open? Then the body is already balanced; `serde_json` failed for
    // some other reason (e.g., trailing comma). Return a trimmed-comma version
    // so the caller's retry can succeed.
    if stack.is_empty() {
        return trim_trailing_comma(body);
    }

    let mut out = trim_trailing_comma(body);
    for c in stack.iter().rev() {
        out.push(*c as char);
    }
    out
}

/// Drop a dangling comma right before the current depth's close position.
/// Very conservative: trims `, \n\t\r` from the right.
fn trim_trailing_comma(body: &str) -> String {
    let trimmed = body.trim_end();
    if let Some(rest) = trimmed.strip_suffix(',') {
        rest.trim_end().to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_body_fastpath() {
        let body = r#"{"type":"layers","layers":[{"label":"A"}]}"#;
        let d = parse_lossy(body).unwrap();
        assert!(matches!(d, Diagram::Layers(_)));
    }

    #[test]
    fn test_pyramid_prefix_layout() {
        // Scenario: a 5-level pyramid truncated to after level 2's closing
        // brace. parse_lossy should recover 2 levels.
        let full = r#"{"type":"pyramid","levels":[
            {"label":"L1"},
            {"label":"L2"},
            {"label":"L3"},
            {"label":"L4"},
            {"label":"L5"}
        ]}"#;
        // Cut after the second level's closing brace
        let cut = r#"{"type":"pyramid","levels":[
            {"label":"L1"},
            {"label":"L2"}"#;
        let d = parse_lossy(cut).expect("lossy parse should succeed");
        if let Diagram::Pyramid(p) = d {
            assert_eq!(p.levels.len(), 2);
        } else {
            panic!("expected pyramid");
        }

        // Full body produces 5 levels, verifying monotonic growth.
        let full_d = parse_lossy(full).unwrap();
        if let Diagram::Pyramid(p) = full_d {
            assert_eq!(p.levels.len(), 5);
        }
    }

    #[test]
    fn dangling_comma_recovered() {
        let body = r#"{"type":"layers","layers":[{"label":"A"},"#;
        let d = parse_lossy(body).unwrap();
        if let Diagram::Layers(l) = d {
            assert_eq!(l.layers.len(), 1);
        } else {
            panic!("expected layers");
        }
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(parse_lossy("").is_none());
        assert!(parse_lossy("   ").is_none());
    }

    #[test]
    fn unrecoverable_returns_none() {
        // Just the opening of an unknown-type diagram isn't recoverable.
        assert!(parse_lossy("{").is_none());
    }
}
