// Dependency-free safe Rust calculator reducer.
//
// State is (value: String, evaluated: bool). `press` returns the new state.

const MAX_LEN: usize = 128;

enum Tok {
    Digit(char),
    Dot,
    Op(char), // '+', '-', '*', '/'
    Eq,
    Clear,
    Back,
    Neg,
    Pct,
    Other,
}

fn tokenize(key: &str) -> Tok {
    match key {
        "=" | "Enter" => Tok::Eq,
        "AC" | "C" => Tok::Clear,
        "\u{232B}" | "Backspace" => Tok::Back, // ⌫
        "\u{00B1}" | "+/-" => Tok::Neg,        // ±
        "%" => Tok::Pct,
        "." => Tok::Dot,
        "*" => Tok::Op('*'),
        "/" => Tok::Op('/'),
        "+" => Tok::Op('+'),
        "-" => Tok::Op('-'),
        "\u{00D7}" => Tok::Op('*'), // ×
        "\u{00F7}" => Tok::Op('/'), // ÷
        s => {
            let mut it = s.chars();
            match (it.next(), it.next()) {
                (Some(c), None) if c.is_ascii_digit() => Tok::Digit(c),
                _ => Tok::Other,
            }
        }
    }
}

fn is_op(c: char) -> bool {
    matches!(c, '+' | '-' | '*' | '/')
}

/// Format a finite f64 cleanly (0.1 + 0.2 -> "0.3").
fn fmt_num(n: f64) -> String {
    // Round ordinary display noise without rounding tiny nonzero values to zero.
    let n = if (1e-10..1e15).contains(&n.abs()) {
        (n * 1e10).round() / 1e10
    } else {
        n
    };
    if n == 0.0 { "0".into() } else { n.to_string() }
}

/// Recursive-descent evaluator over a char slice.
/// expr   := term (('+'|'-') term)*
/// term   := factor (('*'|'/') factor)*
/// factor := ('-')* (number | '(' expr ')')
pub(crate) fn eval(expr: &str) -> Option<f64> {
    if expr.len() > MAX_LEN {
        return None;
    }
    let b: Vec<char> = expr.chars().collect();
    let mut i = 0usize;
    let v = parse_expr(&b, &mut i)?;
    skip_ws(&b, &mut i);
    if i != b.len() {
        return None;
    }
    if v.is_finite() { Some(v) } else { None }
}

fn skip_ws(b: &[char], i: &mut usize) {
    while *i < b.len() && b[*i] == ' ' {
        *i += 1;
    }
}

fn parse_expr(b: &[char], i: &mut usize) -> Option<f64> {
    let mut v = parse_term(b, i)?;
    loop {
        skip_ws(b, i);
        if *i >= b.len() {
            return Some(v);
        }
        match b[*i] {
            '+' => {
                *i += 1;
                v += parse_term(b, i)?;
            }
            '-' => {
                *i += 1;
                v -= parse_term(b, i)?;
            }
            _ => return Some(v),
        }
    }
}

fn parse_term(b: &[char], i: &mut usize) -> Option<f64> {
    let mut v = parse_factor(b, i)?;
    loop {
        skip_ws(b, i);
        if *i >= b.len() {
            return Some(v);
        }
        match b[*i] {
            '*' => {
                *i += 1;
                v *= parse_factor(b, i)?;
            }
            '/' => {
                *i += 1;
                let d = parse_factor(b, i)?;
                if d == 0.0 {
                    return None;
                }
                v /= d;
            }
            _ => return Some(v),
        }
    }
}

fn parse_factor(b: &[char], i: &mut usize) -> Option<f64> {
    skip_ws(b, i);
    if *i >= b.len() {
        return None;
    }
    if b[*i] == '-' {
        *i += 1;
        return Some(-parse_factor(b, i)?);
    }
    if b[*i] == '(' {
        *i += 1;
        let v = parse_expr(b, i)?;
        skip_ws(b, i);
        if *i >= b.len() || b[*i] != ')' {
            return None;
        }
        *i += 1;
        return Some(v);
    }
    if b[*i].is_ascii_digit() || b[*i] == '.' {
        let start = *i;
        let mut dots = 0;
        while *i < b.len() && (b[*i].is_ascii_digit() || b[*i] == '.') {
            if b[*i] == '.' {
                dots += 1;
                if dots > 1 {
                    return None;
                }
            }
            *i += 1;
        }
        let token: String = b[start..*i].iter().collect();
        return token.parse::<f64>().ok();
    }
    None
}

pub fn press(value: &str, key: &str, evaluated: bool) -> (String, bool) {
    if matches!(key, "AC" | "C") {
        return ("0".into(), false);
    }
    if value.len() > MAX_LEN {
        return ("Error".into(), true);
    }
    let result = press_inner(value, key, evaluated);
    if result.0.len() > MAX_LEN {
        ("Error".into(), true)
    } else {
        result
    }
}

fn press_inner(value: &str, key: &str, evaluated: bool) -> (String, bool) {
    let tok = tokenize(key);

    if evaluated {
        // Digits/dot start a new expression; operators continue the result.
        return match tok {
            Tok::Digit(d) => (d.to_string(), false),
            Tok::Dot => ("0.".to_string(), false),
            Tok::Op(op) => {
                let mut s = if value == "Error" || value.is_empty() {
                    String::from("0")
                } else {
                    value.to_string()
                };
                if s.len() < MAX_LEN {
                    s.push(op);
                }
                (s, false)
            }
            Tok::Neg => negate(value),
            Tok::Pct => percent(value),
            Tok::Clear => ("0".to_string(), false),
            Tok::Eq => (value.to_string(), true),
            Tok::Back => ("0".to_string(), false),
            Tok::Other => (value.to_string(), true),
        };
    }

    match tok {
        Tok::Clear => ("0".to_string(), false),
        Tok::Eq => {
            let trimmed = value.trim();
            if trimmed.is_empty() || trimmed == "0" {
                return ("0".to_string(), true);
            }
            match eval(trimmed) {
                Some(n) if n.is_finite() => (fmt_num(n), true),
                _ => ("Error".to_string(), true),
            }
        }
        Tok::Digit(d) => {
            let mut s = value.to_string();
            if s == "0" || s == "Error" {
                s.clear();
            }
            if s.len() >= MAX_LEN {
                return (s, false);
            }
            s.push(d);
            (s, false)
        }
        Tok::Dot => {
            let mut s = value.to_string();
            if s == "0" || s == "Error" || s.is_empty() {
                return ("0.".to_string(), false);
            }
            let tail: String = s
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            if tail.contains('.') || s.len() >= MAX_LEN {
                return (s, false);
            }
            s.push('.');
            (s, false)
        }
        Tok::Op(op) => {
            let mut s = value.to_string();
            if s == "Error" {
                s.clear();
            }
            if s == "0" && op == '-' {
                s.clear();
            }
            if s.ends_with(' ') {
                s.pop(); // stored unary slot "…- "
            }
            if is_op(s.chars().next_back().unwrap_or(' ')) {
                s.pop();
            }
            if s.len() >= MAX_LEN {
                return (s, false);
            }
            s.push(op);
            (s, false)
        }
        Tok::Back => {
            let mut s = value.to_string();
            if s == "Error" {
                return ("0".to_string(), false);
            }
            s.pop();
            if s.is_empty() || s == "-" {
                s = "0".to_string();
            }
            (s, false)
        }
        Tok::Neg => negate(value),
        Tok::Pct => percent(value),
        Tok::Other => (value.to_string(), false),
    }
}

/// Locate the last numeric token (with its optional unary '-') in `s`.
fn last_num_span(chars: &[char]) -> (usize, usize) {
    let mut end = chars.len();
    while end > 0 && !(chars[end - 1].is_ascii_digit() || chars[end - 1] == '.') {
        end -= 1;
    }
    let mut start = end;
    while start > 0 && (chars[start - 1].is_ascii_digit() || chars[start - 1] == '.') {
        start -= 1;
    }
    // include a unary '-' directly before the number
    if start > 0
        && chars[start - 1] == '-'
        && (start - 1 == 0 || is_op(chars[start - 2]) || chars[start - 2] == '(')
    {
        start -= 1;
    }
    (start, end)
}

fn negate(value: &str) -> (String, bool) {
    if value == "Error" {
        return ("0".to_string(), false);
    }
    let s = value.trim();
    let chars: Vec<char> = s.chars().collect();
    let (start, end) = last_num_span(&chars);
    if start == end {
        return (s.to_string(), false);
    }
    let token: String = chars[start..end].iter().collect();
    let head: String = chars[..start].iter().collect();
    let tail: String = chars[end..].iter().collect();
    let out = if let Some(unsigned) = token.strip_prefix('-') {
        format!("{head}{unsigned}{tail}")
    } else {
        format!("{}-{}{}", head, token, tail)
    };
    if out.is_empty() || out.len() > MAX_LEN {
        return (s.to_string(), false);
    }
    (out, false)
}

fn percent(value: &str) -> (String, bool) {
    if value == "Error" {
        return ("0".to_string(), false);
    }
    let s = value.trim();
    let chars: Vec<char> = s.chars().collect();
    let (start, end) = last_num_span(&chars);
    if start == end {
        return (s.to_string(), false);
    }
    let token: String = chars[start..end].iter().collect();
    let n: f64 = match token.parse() {
        Ok(n) => n,
        Err(_) => return ("Error".to_string(), true),
    };
    let r = n / 100.0;
    if !r.is_finite() {
        return ("Error".to_string(), true);
    }
    let head: String = chars[..start].iter().collect();
    let tail: String = chars[end..].iter().collect();
    let out = format!("{}{}{}", head, fmt_num(r), tail);
    if out.len() > MAX_LEN {
        return (s.to_string(), false);
    }
    (out, false)
}

#[cfg(test)]
mod tests {
    use super::press;

    fn type_expr(keys: &[&str]) -> (String, bool) {
        let mut st = ("0".to_string(), false);
        for k in keys {
            st = press(&st.0, k, st.1);
        }
        st
    }

    #[test]
    fn calculator_sequences() {
        assert_eq!(type_expr(&["0", "+", "5", "="]).0, "5");
        assert_eq!(type_expr(&["0", "*", "5", "="]).0, "0");
        assert_eq!(press("0.00000000001", "=", false).0, "0.00000000001");
        assert_eq!(press(&"9".repeat(129), "=", false), ("Error".into(), true));
        assert!(press(&"9".repeat(128), "=", false).0.len() <= 128);
        // 7 + 8 = 15
        let (v, e) = type_expr(&["7", "+", "8", "="]);
        assert_eq!((v.as_str(), e), ("15", true));

        // 2 + 3 × 4 = 14 (precedence)
        let (v, _) = type_expr(&["2", "+", "3", "×", "4", "="]);
        assert_eq!(v, "14");
        let (v, _) = type_expr(&["2", "+", "3", "*", "4", "="]);
        assert_eq!(v, "14");

        // decimals: 0.1 + 0.2 = 0.3
        let (v, _) = type_expr(&["0", ".", "1", "+", "0", ".", "2", "="]);
        assert_eq!(v, "0.3");
        // single dot per number
        let (v, _) = type_expr(&["1", ".", "2", ".", "3"]);
        assert_eq!(v, "1.23");

        // division by zero -> Error; digit recovers
        let (v, e) = type_expr(&["5", "÷", "0", "="]);
        assert_eq!((v.as_str(), e), ("Error", true));
        let (v, e) = press(&v, "7", e);
        assert_eq!((v.as_str(), e), ("7", false));

        // backspace deletes one char, empties to 0
        let (v, _) = type_expr(&["1", "2", "3", "\u{232B}"]);
        assert_eq!(v, "12");
        let (v, _) = type_expr(&["5", "Backspace"]);
        assert_eq!(v, "0");

        // ± negates LAST numeric operand, toggles
        let (v, _) = type_expr(&["3", "+", "4", "\u{00B1}"]);
        assert_eq!(v, "3+-4");
        let (v, _) = type_expr(&["3", "+", "4", "\u{00B1}", "+/-"]);
        assert_eq!(v, "3+4");
        // initial negative via '-' then digits
        let (v, _) = type_expr(&["-", "5", "="]);
        assert_eq!(v, "-5");

        // percent divides LAST operand by 100
        let (v, _) = type_expr(&["5", "0", "%"]);
        assert_eq!(v, "0.5");
        let (v, _) = type_expr(&["2", "0", "0", "+", "5", "0", "%", "="]);
        assert_eq!(v, "200.5");

        // repeated operators replace preceding operator
        let (v, _) = type_expr(&["7", "+", "-", "*"]);
        assert_eq!(v, "7*");

        // typing a digit after equals starts a new expression
        let (v, _) = type_expr(&["7", "+", "8", "=", "9"]);
        assert_eq!(v, "9");
        // operator after equals continues the result
        let (v, _) = type_expr(&["7", "+", "8", "=", "+", "1", "="]);
        assert_eq!(v, "16");

        // oversized values are capped at 128 chars
        let long = "9".repeat(140);
        let mut st = ("0".to_string(), false);
        for c in long.chars() {
            st = press(&st.0, &c.to_string(), st.1);
        }
        assert_eq!(st.0.len(), 128);

        // clear
        let (v, _) = type_expr(&["7", "+", "8", "AC"]);
        assert_eq!(v, "0");
        let (v, _) = type_expr(&["7", "+", "8", "C"]);
        assert_eq!(v, "0");

        // invalid expression -> Error
        let (v, e) = type_expr(&["1", "+", "="]);
        assert_eq!((v.as_str(), e), ("Error", true));
    }
}
