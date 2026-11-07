// Modified by Vabian124 on 2026-11-07 to parse plugin PHP into a Rust skeleton.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.

/// Plugin header fields read from the file comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginHeader {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
}

/// A registration whose tag and callback are PHP string literals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    pub kind: RegistrationKind,
    pub tag: String,
    pub callback: String,
    pub priority: i32,
    /// PHP function source, when the callback is a named function. Not executed.
    pub body: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationKind {
    Action,
    Filter,
    Shortcode,
}

/// A PHP construct this crate does not translate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsupported {
    pub reason: String,
    pub source: String,
}

/// Registrations and reports from one plugin file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginIr {
    pub header: PluginHeader,
    pub registrations: Vec<Registration>,
    pub unsupported: Vec<Unsupported>,
}

/// Parse a plugin file. Unsupported calls are reported and skipped.
pub fn parse_plugin(source: &str) -> PluginIr {
    let functions = function_sources(source);
    let mut registrations = Vec::new();
    let mut unsupported = Vec::new();
    let mut index = 0;

    while index < source.len() {
        index = skip_trivia(source, index);
        if index >= source.len() {
            break;
        }

        let ch = source[index..].chars().next().unwrap();
        if ch == '\'' || ch == '"' {
            index = skip_string(source, index).unwrap_or(source.len());
            continue;
        }

        if is_ident_start(ch) {
            let (word, next) = read_ident(source, index);
            match word {
                "add_action" | "add_filter" | "add_shortcode" => {
                    let after = skip_trivia(source, next);
                    if source[after..].starts_with('(') {
                        let (call_end, call) = read_call(source, index);
                        match parse_call(word, &call) {
                            Ok(mut registration) => {
                                registration.body = functions.get(&registration.callback).cloned();
                                registrations.push(registration);
                            }
                            Err(reason) => unsupported.push(Unsupported {
                                reason,
                                source: call.trim().to_string(),
                            }),
                        }
                        index = call_end;
                    } else {
                        index = next;
                    }
                }
                "include" | "include_once" | "require" | "require_once" | "eval" => {
                    let end = statement_end(source, next);
                    unsupported.push(Unsupported {
                        reason: format!("{word} is not translated"),
                        source: source[index..end].trim().to_string(),
                    });
                    index = end;
                }
                _ => index = next,
            }
            continue;
        }

        index += ch.len_utf8();
    }

    PluginIr {
        header: parse_header(source),
        registrations,
        unsupported,
    }
}

fn parse_header(source: &str) -> PluginHeader {
    let mut header = PluginHeader {
        name: String::new(),
        version: String::new(),
        description: String::new(),
        author: String::new(),
    };
    let head: String = source.chars().take(8_192).collect();
    for line in head.lines() {
        let line = line.trim().trim_start_matches('*').trim();
        if header.name.is_empty() {
            if let Some(value) = field(line, "Plugin Name:") {
                header.name = value;
            }
        }
        if header.description.is_empty() {
            if let Some(value) = field(line, "Description:") {
                header.description = value;
            }
        }
        if header.version.is_empty() {
            if let Some(value) = field(line, "Version:") {
                header.version = value;
            }
        }
        if header.author.is_empty() && !line.starts_with("Author URI:") {
            if let Some(value) = field(line, "Author:") {
                header.author = value;
            }
        }
    }
    header
}

fn field(line: &str, label: &str) -> Option<String> {
    line.strip_prefix(label)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn function_sources(source: &str) -> std::collections::HashMap<String, String> {
    let mut functions = std::collections::HashMap::new();
    let mut index = 0;
    while index < source.len() {
        index = skip_trivia(source, index);
        if index >= source.len() {
            break;
        }
        let ch = source[index..].chars().next().unwrap();
        if ch == '\'' || ch == '"' {
            index = skip_string(source, index).unwrap_or(source.len());
            continue;
        }
        if is_ident_start(ch) {
            let (word, next) = read_ident(source, index);
            if word == "function" {
                let name_at = skip_trivia(source, next);
                if name_at < source.len()
                    && is_ident_start(source[name_at..].chars().next().unwrap())
                {
                    let (name, after_name) = read_ident(source, name_at);
                    if let Some(end) = function_end(source, after_name) {
                        functions.insert(name.to_string(), source[index..end].to_string());
                        index = end;
                        continue;
                    }
                }
            }
            index = next;
            continue;
        }
        index += ch.len_utf8();
    }
    functions
}

fn function_end(source: &str, from: usize) -> Option<usize> {
    let mut index = from;
    let mut depth = 0i32;
    while index < source.len() {
        index = skip_trivia(source, index);
        if index >= source.len() {
            return None;
        }
        let ch = source[index..].chars().next().unwrap();
        if ch == '\'' || ch == '"' {
            index = skip_string(source, index)?;
            continue;
        }
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            '{' if depth <= 0 => {
                return brace_end(source, index);
            }
            _ => {}
        }
        index += ch.len_utf8();
    }
    None
}

fn brace_end(source: &str, open: usize) -> Option<usize> {
    let mut index = open + '{'.len_utf8();
    let mut depth = 1i32;
    while index < source.len() {
        let ch = source[index..].chars().next().unwrap();
        if ch == '\'' || ch == '"' {
            index = skip_string(source, index)?;
            continue;
        }
        if source[index..].starts_with("//") || source[index..].starts_with('#') {
            index = skip_line_comment(source, index);
            continue;
        }
        if source[index..].starts_with("/*") {
            index = skip_block_comment(source, index);
            continue;
        }
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index + '}'.len_utf8());
                }
            }
            _ => {}
        }
        index += ch.len_utf8();
    }
    None
}

fn parse_call(kind: &str, call: &str) -> Result<Registration, String> {
    let open = call
        .find('(')
        .ok_or_else(|| format!("{kind} is not a call"))?;
    let close = call
        .rfind(')')
        .ok_or_else(|| format!("{kind} is not a call"))?;
    let args = split_args(&call[open + 1..close]);
    let tag = string_arg(args.first().map(String::as_str).unwrap_or(""))
        .ok_or_else(|| format!("{kind} tag is not a string literal"))?;
    let callback = string_arg(args.get(1).map(String::as_str).unwrap_or(""))
        .ok_or_else(|| format!("{kind} callback is not a string literal"))?;
    let priority = if kind == "add_shortcode" {
        10
    } else if let Some(raw) = args.get(2) {
        int_arg(raw).ok_or_else(|| format!("{kind} priority is not an integer literal"))?
    } else {
        10
    };
    if let Some(raw) = args.get(3) {
        if int_arg(raw).is_none() {
            return Err(format!("{kind} accepted_args is not an integer literal"));
        }
    }
    let kind = match kind {
        "add_action" => RegistrationKind::Action,
        "add_filter" => RegistrationKind::Filter,
        _ => RegistrationKind::Shortcode,
    };
    Ok(Registration {
        kind,
        tag,
        callback,
        priority,
        body: None,
    })
}

fn split_args(inner: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut start = 0;
    let mut index = 0;
    let mut depth = 0i32;
    let bytes = inner;
    while index < bytes.len() {
        let ch = bytes[index..].chars().next().unwrap();
        if ch == '\'' || ch == '"' {
            index = skip_string(bytes, index).unwrap_or(bytes.len());
            continue;
        }
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                let arg = bytes[start..index].trim();
                if !arg.is_empty() {
                    args.push(arg.to_string());
                }
                start = index + 1;
            }
            _ => {}
        }
        index += ch.len_utf8();
    }
    let tail = bytes[start..].trim();
    if !tail.is_empty() {
        args.push(tail.to_string());
    }
    args
}

fn string_arg(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let mut chars = raw.chars();
    let quote = chars.next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let mut value = String::new();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') if quote == '"' => value.push('\n'),
                Some('t') if quote == '"' => value.push('\t'),
                Some('r') if quote == '"' => value.push('\r'),
                Some(escaped) => value.push(escaped),
                None => return None,
            }
            continue;
        }
        if ch == quote {
            return if chars.next().is_none() {
                Some(value)
            } else {
                None
            };
        }
        value.push(ch);
    }
    None
}

fn int_arg(raw: &str) -> Option<i32> {
    raw.trim().parse().ok()
}

fn read_call(source: &str, start: usize) -> (usize, String) {
    let open = source[start..].find('(').map(|offset| start + offset);
    let Some(open) = open else {
        return (start, String::new());
    };
    let mut index = open + 1;
    let mut depth = 1i32;
    while index < source.len() {
        let ch = source[index..].chars().next().unwrap();
        if ch == '\'' || ch == '"' {
            index = skip_string(source, index).unwrap_or(source.len());
            continue;
        }
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    let end = index + ')'.len_utf8();
                    let mut stop = end;
                    if source[stop..].starts_with(';') {
                        stop += 1;
                    }
                    return (stop, source[start..stop].to_string());
                }
            }
            _ => {}
        }
        index += ch.len_utf8();
    }
    (source.len(), source[start..].to_string())
}

fn statement_end(source: &str, from: usize) -> usize {
    let mut index = from;
    while index < source.len() {
        let ch = source[index..].chars().next().unwrap();
        if ch == '\'' || ch == '"' {
            index = skip_string(source, index).unwrap_or(source.len());
            continue;
        }
        if ch == ';' {
            return index + 1;
        }
        index += ch.len_utf8();
    }
    source.len()
}

fn skip_trivia(source: &str, mut index: usize) -> usize {
    loop {
        if index >= source.len() {
            return index;
        }
        let rest = &source[index..];
        if rest.starts_with("//") || rest.starts_with('#') {
            index = skip_line_comment(source, index);
            continue;
        }
        if rest.starts_with("/*") {
            index = skip_block_comment(source, index);
            continue;
        }
        let ch = rest.chars().next().unwrap();
        if ch.is_whitespace() {
            index += ch.len_utf8();
            continue;
        }
        return index;
    }
}

fn skip_line_comment(source: &str, index: usize) -> usize {
    source[index..]
        .find('\n')
        .map(|offset| index + offset + 1)
        .unwrap_or(source.len())
}

fn skip_block_comment(source: &str, index: usize) -> usize {
    source[index + 2..]
        .find("*/")
        .map(|offset| index + 2 + offset + 2)
        .unwrap_or(source.len())
}

fn skip_string(source: &str, index: usize) -> Option<usize> {
    let quote = source[index..].chars().next()?;
    let mut cursor = index + quote.len_utf8();
    while cursor < source.len() {
        let ch = source[cursor..].chars().next()?;
        if ch == '\\' {
            cursor += ch.len_utf8();
            if let Some(escaped) = source[cursor..].chars().next() {
                cursor += escaped.len_utf8();
            }
            continue;
        }
        cursor += ch.len_utf8();
        if ch == quote {
            return Some(cursor);
        }
    }
    None
}

fn read_ident(source: &str, index: usize) -> (&str, usize) {
    let mut end = index;
    for ch in source[index..].chars() {
        if end == index {
            if !is_ident_start(ch) {
                break;
            }
        } else if !is_ident_continue(ch) {
            break;
        }
        end += ch.len_utf8();
    }
    (&source[index..end], end)
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}
