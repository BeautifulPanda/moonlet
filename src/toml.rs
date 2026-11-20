// A small TOML subset for config.toml. set() edits a single value in place
// and keeps the comments of the file.

use std::collections::BTreeMap;
use std::fmt;

use crate::lang::tr;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Array(Vec<Value>),
}

impl Value {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }

    #[cfg_attr(not(any(test, feature = "orbit")), allow(dead_code))]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    #[cfg_attr(not(any(test, feature = "orbit")), allow(dead_code))]
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParseError {
    pub line: usize,
    pub message: &'static str,
}

impl ParseError {
    pub fn at(line: usize, message: &'static str) -> Self {
        Self { line, message }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} {}: {}",
            tr("строка", "line"),
            self.line,
            self.message
        )
    }
}

#[derive(Debug)]
pub struct Entry {
    pub value: Value,
    pub line: usize,
}

#[derive(Debug, Default)]
pub struct Table {
    entries: BTreeMap<String, Entry>,
}

impl Table {
    pub fn get(&self, section: &str, key: &str) -> Option<&Entry> {
        self.entries.get(&full_key(section, key))
    }
}

fn full_key(section: &str, key: &str) -> String {
    if section.is_empty() {
        key.to_string()
    } else {
        format!("{section}.{key}")
    }
}

fn skip_bom(text: &str) -> &str {
    text.strip_prefix('\u{FEFF}').unwrap_or(text)
}

pub fn parse(text: &str) -> Result<Table, ParseError> {
    let text = skip_bom(text);
    let mut table = Table::default();
    let mut section = String::new();
    let mut lines = text.lines().enumerate();

    while let Some((index, raw)) = lines.next() {
        let line_no = index + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if let Some(header) = line.strip_prefix('[') {
            let name = header
                .strip_suffix(']')
                .ok_or(ParseError::at(
                    line_no,
                    tr(
                        "заголовок раздела должен заканчиваться на ]",
                        "a section header must end with ]",
                    ),
                ))?
                .trim();
            if name.starts_with('[') || !name.split('.').all(is_bare_key) {
                return Err(ParseError::at(
                    line_no,
                    tr("непонятное имя раздела", "invalid section name"),
                ));
            }
            section = name.to_string();
            continue;
        }

        let (key, value_text) = line.split_once('=').ok_or(ParseError::at(
            line_no,
            tr("ожидалось «ключ = значение»", "expected \"key = value\""),
        ))?;
        let key = key.trim();
        if !is_bare_key(key) {
            return Err(ParseError::at(
                line_no,
                tr(
                    "ключ может состоять только из латиницы, цифр, _ и -",
                    "a key may only contain Latin letters, digits, _ and -",
                ),
            ));
        }

        let mut value_text = value_text.trim().to_string();
        while bracket_depth(&value_text) > 0 {
            let (_, next) = lines.next().ok_or(ParseError::at(
                line_no,
                tr(
                    "список не закрыт скобкой ]",
                    "the list is not closed with ]",
                ),
            ))?;
            value_text.push(' ');
            value_text.push_str(strip_comment(next).trim());
        }

        let (value, rest) = parse_value(&value_text).map_err(|m| ParseError::at(line_no, m))?;
        if !rest.trim().is_empty() {
            return Err(ParseError::at(
                line_no,
                tr(
                    "лишние символы после значения",
                    "unexpected characters after the value",
                ),
            ));
        }
        let name = full_key(&section, key);
        if table.entries.contains_key(&name) {
            return Err(ParseError::at(
                line_no,
                tr("этот ключ уже задан выше", "this key is already set above"),
            ));
        }
        table.entries.insert(
            name,
            Entry {
                value,
                line: line_no,
            },
        );
    }
    Ok(table)
}

pub fn set(text: &str, section: &str, key: &str, value: &str) -> String {
    let text = skip_bom(text);
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let mut current = String::new();
    let mut insert_at = if section.is_empty() { Some(0) } else { None };

    let mut i = 0;
    while i < lines.len() {
        let code = strip_comment(&lines[i]).trim().to_string();
        if let Some(header) = code.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
            current = header.trim().to_string();
            if current == section {
                insert_at = Some(i + 1);
            }
            i += 1;
            continue;
        }
        if current == section && !code.is_empty() {
            insert_at = Some(i + 1);
            if let Some((name, old_value)) = code.split_once('=')
                && name.trim() == key
            {
                let mut depth = bracket_depth(old_value);
                while depth > 0 && i + 1 < lines.len() {
                    depth += bracket_depth(strip_comment(&lines[i + 1]));
                    lines.remove(i + 1);
                }
                let raw = &lines[i];
                let comment = raw[strip_comment(raw).len()..].trim();
                let indent: String = raw.chars().take_while(|c| c.is_whitespace()).collect();
                let mut updated = format!("{indent}{key} = {value}");
                if !comment.is_empty() {
                    updated.push(' ');
                    updated.push_str(comment);
                }
                lines[i] = updated;
                return join(&lines, newline);
            }
        }
        i += 1;
    }

    match insert_at {
        Some(at) => lines.insert(at, format!("{key} = {value}")),
        None => {
            if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push(format!("[{section}]"));
            lines.push(format!("{key} = {value}"));
        }
    }
    join(&lines, newline)
}

fn join(lines: &[String], newline: &str) -> String {
    let mut text = lines.join(newline);
    text.push_str(newline);
    text
}

fn is_bare_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn strip_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        match quote {
            Some('"') => {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    quote = None;
                }
            }
            Some(_) => {
                if c == '\'' {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => quote = Some(c),
                '#' => return &line[..i],
                _ => {}
            },
        }
    }
    line
}

fn bracket_depth(text: &str) -> i32 {
    let mut depth = 0;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for c in text.chars() {
        match quote {
            Some('"') => {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    quote = None;
                }
            }
            Some(_) => {
                if c == '\'' {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => quote = Some(c),
                '[' => depth += 1,
                ']' => depth -= 1,
                _ => {}
            },
        }
    }
    depth
}

fn parse_value(text: &str) -> Result<(Value, &str), &'static str> {
    let text = text.trim_start();
    match text.chars().next() {
        None => Err(tr("после = нет значения", "no value after =")),
        Some('"') => parse_basic_string(&text[1..]),
        Some('\'') => {
            let body = &text[1..];
            let end = body.find('\'').ok_or(tr(
                "строка не закрыта кавычкой",
                "the string is not closed with a quote",
            ))?;
            Ok((Value::Str(body[..end].to_string()), &body[end + 1..]))
        }
        Some('[') => parse_array(&text[1..]),
        Some(_) => {
            let end = text
                .find(|c: char| c == ',' || c == ']' || c.is_whitespace())
                .unwrap_or(text.len());
            Ok((parse_scalar(&text[..end])?, &text[end..]))
        }
    }
}

fn parse_basic_string(text: &str) -> Result<(Value, &str), &'static str> {
    let mut out = String::new();
    let mut chars = text.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Ok((Value::Str(out), &text[i + 1..])),
            '\\' => {
                let escaped = match chars.next() {
                    Some((_, 'n')) => '\n',
                    Some((_, 't')) => '\t',
                    Some((_, 'r')) => '\r',
                    Some((_, '"')) => '"',
                    Some((_, '\\')) => '\\',
                    _ => {
                        return Err(tr(
                            "неизвестная последовательность после \\ в строке",
                            "unknown escape sequence after \\ in a string",
                        ));
                    }
                };
                out.push(escaped);
            }
            _ => out.push(c),
        }
    }
    Err(tr(
        "строка не закрыта кавычкой",
        "the string is not closed with a quote",
    ))
}

fn parse_array(mut text: &str) -> Result<(Value, &str), &'static str> {
    let mut items = Vec::new();
    loop {
        text = text.trim_start();
        if let Some(rest) = text.strip_prefix(']') {
            return Ok((Value::Array(items), rest));
        }
        let (item, rest) = parse_value(text)?;
        items.push(item);
        text = rest.trim_start();
        if let Some(rest) = text.strip_prefix(',') {
            text = rest;
        } else if !text.starts_with(']') {
            return Err(tr(
                "в списке ожидалась запятая или ]",
                "expected a comma or ] in the list",
            ));
        }
    }
}

fn parse_scalar(token: &str) -> Result<Value, &'static str> {
    match token {
        "true" => return Ok(Value::Bool(true)),
        "false" => return Ok(Value::Bool(false)),
        _ => {}
    }
    let digits = token.replace('_', "");
    if let Some(hex) = digits.strip_prefix("0x") {
        return i64::from_str_radix(hex, 16).map(Value::Int).map_err(|_| {
            tr(
                "неверное шестнадцатеричное число",
                "invalid hexadecimal number",
            )
        });
    }
    if let Ok(int) = digits.parse::<i64>() {
        return Ok(Value::Int(int));
    }
    if digits.chars().any(|c| c.is_ascii_digit())
        && let Ok(float) = digits.parse::<f64>()
    {
        return Ok(Value::Float(float));
    }
    Err(tr(
        "непонятное значение (строки пишутся в кавычках)",
        "invalid value (strings must be quoted)",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# комментарий
title = \"Moonlet # не комментарий\"

[orbit]
radius_x = 44   # пиксели
radius_y = 1_9
period = 40.5
clockwise = true
colors = [
    \"#4FD1C5\",  # бирюзовый
    '#F6AD55',
]
";

    #[test]
    fn parses_sections_values_and_comments() {
        let table = parse(SAMPLE).unwrap();
        assert_eq!(
            table.get("", "title").unwrap().value.as_str(),
            Some("Moonlet # не комментарий")
        );
        assert_eq!(
            table.get("orbit", "radius_x").unwrap().value,
            Value::Int(44)
        );
        assert_eq!(
            table.get("orbit", "radius_y").unwrap().value,
            Value::Int(19)
        );
        assert_eq!(
            table.get("orbit", "period").unwrap().value,
            Value::Float(40.5)
        );
        assert_eq!(
            table.get("orbit", "clockwise").unwrap().value,
            Value::Bool(true)
        );
        let colors = table.get("orbit", "colors").unwrap();
        assert_eq!(colors.value.as_array().unwrap().len(), 2);
        assert_eq!(colors.line, 9);
    }

    #[test]
    fn parses_numbers() {
        assert_eq!(parse_scalar("-0.25"), Ok(Value::Float(-0.25)));
        assert_eq!(parse_scalar("1.5e3"), Ok(Value::Float(1500.0)));
        assert_eq!(parse_scalar("0xFF"), Ok(Value::Int(255)));
        assert!(parse_scalar("inf").is_err());
        assert!(parse_scalar("abc").is_err());
    }

    #[test]
    fn reports_line_of_error() {
        let error = parse("[orbit]\nradius_x = 44\nradius_y = abc\n").unwrap_err();
        assert_eq!(error.line, 3);
        assert!(parse("[orbit]\nradius_x = 1\nradius_x = 2\n").is_err());
        assert!(parse("[orbit\n").is_err());
        assert!(parse("colors = [1, 2\n").is_err());
    }

    #[test]
    fn set_replaces_value_and_keeps_comments() {
        let updated = set(SAMPLE, "orbit", "radius_x", "30");
        assert!(updated.contains("radius_x = 30 # пиксели"));
        assert!(updated.contains("# комментарий"));
        assert_eq!(
            parse(&updated)
                .unwrap()
                .get("orbit", "radius_x")
                .unwrap()
                .value,
            Value::Int(30)
        );
    }

    #[test]
    fn set_replaces_multiline_array() {
        let updated = set(SAMPLE, "orbit", "colors", "[\"#FFFFFF\"]");
        let table = parse(&updated).unwrap();
        assert_eq!(
            table
                .get("orbit", "colors")
                .unwrap()
                .value
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn set_adds_missing_key_and_section() {
        let updated = set(SAMPLE, "orbit", "rotate", "false");
        assert_eq!(
            parse(&updated)
                .unwrap()
                .get("orbit", "rotate")
                .unwrap()
                .value,
            Value::Bool(false)
        );
        let updated = set(SAMPLE, "general", "autostart", "true");
        assert_eq!(
            parse(&updated)
                .unwrap()
                .get("general", "autostart")
                .unwrap()
                .value,
            Value::Bool(true)
        );
    }

    #[test]
    fn ignores_utf8_bom() {
        let text = "\u{FEFF}[orbit]\nradius_x = 30\n";
        assert_eq!(
            parse(text).unwrap().get("orbit", "radius_x").unwrap().value,
            Value::Int(30)
        );
        assert_eq!(
            set(text, "orbit", "radius_x", "44"),
            "[orbit]\nradius_x = 44\n"
        );
    }

    #[test]
    fn set_keeps_windows_line_endings() {
        let text = "[a]\r\nx = 1\r\n";
        assert_eq!(set(text, "a", "x", "2"), "[a]\r\nx = 2\r\n");
    }
}
