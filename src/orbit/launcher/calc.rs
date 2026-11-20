const DIGITS: i32 = 12;

pub fn evaluate(text: &str) -> Option<f64> {
    let text = text.trim();
    let text = text.strip_suffix('=').unwrap_or(text);
    let mut parser = Parser {
        chars: text.chars().filter(|c| !c.is_whitespace()).collect(),
        pos: 0,
        operations: 0,
    };
    let value = parser.sum()?;
    let finished = parser.pos == parser.chars.len();
    (finished && parser.operations > 0 && value.is_finite()).then_some(value)
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
    operations: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn take(&mut self, expected: &[char]) -> Option<char> {
        let c = self.peek().filter(|c| expected.contains(c))?;
        self.pos += 1;
        Some(c)
    }

    fn sum(&mut self) -> Option<f64> {
        let mut value = self.product()?;
        while let Some(op) = self.take(&['+', '-', '−']) {
            self.operations += 1;
            let right = self.product()?;
            value = if op == '+' {
                value + right
            } else {
                value - right
            };
        }
        Some(value)
    }

    fn product(&mut self) -> Option<f64> {
        let mut value = self.signed()?;
        while let Some(op) = self.take(&['*', '×', '·', '/', '÷', ':']) {
            self.operations += 1;
            let right = self.signed()?;
            value = match op {
                '/' | '÷' | ':' if right == 0.0 => return None,
                '/' | '÷' | ':' => value / right,
                _ => value * right,
            };
        }
        Some(value)
    }

    fn signed(&mut self) -> Option<f64> {
        match self.take(&['-', '−', '+']) {
            Some('+') => self.signed(),
            Some(_) => Some(-self.signed()?),
            None => self.power(),
        }
    }

    fn power(&mut self) -> Option<f64> {
        let base = self.percent()?;
        if self.take(&['^']).is_some() {
            self.operations += 1;
            let exponent = self.signed()?;
            // Only integer powers: powf pulls the CRT pow (~4 KB) into the binary.
            if exponent.fract() != 0.0 || exponent.abs() > 1000.0 {
                return None;
            }
            return Some(base.powi(exponent as i32));
        }
        Some(base)
    }

    fn percent(&mut self) -> Option<f64> {
        let mut value = self.atom()?;
        while self.take(&['%']).is_some() {
            self.operations += 1;
            value /= 100.0;
        }
        Some(value)
    }

    fn atom(&mut self) -> Option<f64> {
        if self.take(&['(']).is_some() {
            let value = self.sum()?;
            self.take(&[')'])?;
            return Some(value);
        }
        if self.take(&['√']).is_some() {
            self.operations += 1;
            let value = self.percent()?;
            return (value >= 0.0).then(|| value.sqrt());
        }
        self.number()
    }

    fn number(&mut self) -> Option<f64> {
        let start = self.pos;
        let mut text = String::new();
        let mut dot = false;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                text.push(c);
            } else if (c == '.' || c == ',') && !dot {
                dot = true;
                text.push('.');
            } else {
                break;
            }
            self.pos += 1;
        }
        if self.pos == start || text == "." {
            return None;
        }
        text.parse().ok()
    }
}

// Formatted by hand: std float formatting adds tens of KB to the binary.
pub fn format(value: f64, decimal: char, group: Option<char>) -> String {
    let negative = value < 0.0;
    let value = value.abs();
    if value == 0.0 {
        return "0".to_string();
    }
    let mut exponent = decimal_exponent(value);
    let mut out = String::new();
    if (-6..15).contains(&exponent) {
        let decimals = (DIGITS - 1 - exponent).clamp(0, 12) as u32;
        let scaled = (value * 10f64.powi(decimals as i32)).round() as u64;
        let unit = 10u64.pow(decimals);
        let (whole, fraction) = (scaled / unit, scaled % unit);
        if whole == 0 && fraction == 0 {
            return "0".to_string();
        }
        out += &grouped(whole, group);
        push_fraction(&mut out, fraction, decimals, decimal);
    } else {
        let mut mantissa = (value / 10f64.powi(exponent) * 1e11).round() as u64;
        if mantissa >= 1_000_000_000_000 {
            mantissa /= 10;
            exponent += 1;
        }
        let unit = 100_000_000_000u64;
        out += &(mantissa / unit).to_string();
        push_fraction(&mut out, mantissa % unit, 11, decimal);
        out += &format!("e{exponent}");
    }
    if negative {
        out.insert(0, '-');
    }
    out
}

fn decimal_exponent(value: f64) -> i32 {
    let mut exponent = 0;
    let mut scaled = value;
    while scaled >= 10.0 {
        scaled /= 10.0;
        exponent += 1;
    }
    while scaled < 1.0 {
        scaled *= 10.0;
        exponent -= 1;
    }
    exponent
}

fn push_fraction(out: &mut String, fraction: u64, digits: u32, decimal: char) {
    if fraction == 0 {
        return;
    }
    let text = fraction.to_string();
    let padded = "0".repeat(digits as usize - text.len()) + &text;
    out.push(decimal);
    out.push_str(padded.trim_end_matches('0'));
}

fn grouped(number: u64, group: Option<char>) -> String {
    let digits = number.to_string();
    let Some(separator) = group else {
        return digits;
    };
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(separator);
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calc(text: &str) -> Option<String> {
        evaluate(text).map(|v| format(v, '.', None))
    }

    #[test]
    fn evaluates_expressions() {
        assert_eq!(calc("2+2*3").as_deref(), Some("8"));
        assert_eq!(calc("(2+2)*3").as_deref(), Some("12"));
        assert_eq!(calc(" 1,5 + 2.5 ").as_deref(), Some("4"));
        assert_eq!(calc("10/4").as_deref(), Some("2.5"));
        assert_eq!(calc("7:2").as_deref(), Some("3.5"));
        assert_eq!(calc("2^10").as_deref(), Some("1024"));
        assert_eq!(calc("2^3^2").as_deref(), Some("512"));
        assert_eq!(calc("2^-1").as_deref(), Some("0.5"));
        assert_eq!(calc("2^0.5"), None);
        assert_eq!(calc("-2^2").as_deref(), Some("-4"));
        assert_eq!(calc("200*15%").as_deref(), Some("30"));
        assert_eq!(calc("√16+1").as_deref(), Some("5"));
        assert_eq!(calc("0.1+0.2").as_deref(), Some("0.3"));
        assert_eq!(calc("1/3").as_deref(), Some("0.333333333333"));
        assert_eq!(calc("2+2=").as_deref(), Some("4"));
        assert_eq!(calc("3-5").as_deref(), Some("-2"));
    }

    #[test]
    fn ignores_plain_text_and_errors() {
        assert_eq!(calc("42"), None);
        assert_eq!(calc("telegram"), None);
        assert_eq!(calc("2+"), None);
        assert_eq!(calc("(2+3"), None);
        assert_eq!(calc("1/0"), None);
        assert_eq!(calc("1.2.3+1"), None);
        assert_eq!(calc(""), None);
    }

    #[test]
    fn formats_numbers() {
        assert_eq!(format(1234567.5, ',', Some(' ')), "1 234 567,5");
        assert_eq!(format(-0.000123, ',', None), "-0,000123");
        assert_eq!(format(1e20, '.', None), "1e20");
        assert_eq!(format(1.5e-9, '.', None), "1.5e-9");
        assert_eq!(format(999999999999999.0, '.', None), "999999999999999");
        assert_eq!(format(1e15, '.', None), "1e15");
        assert_eq!(format(100.0, '.', Some(' ')), "100");
        assert_eq!(format(-0.0, '.', None), "0");
    }
}
