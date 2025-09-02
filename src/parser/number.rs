use std::str::FromStr;

use crate::parser::token_horizontal_ending_position;
use crate::Result;

fn split_leading_number(s: &str) -> Option<(&str, &str)> {
    let mut chars = s.char_indices().peekable();
    let mut has_digit = false;
    let mut has_dot = false;
    let mut exponent_start: Option<usize> = None;
    let mut last_valid_end = 0;

    // 处理可选符号
    if let Some(&(_, c)) = chars.peek() {
        if c == '+' || c == '-' {
            if let Some((idx, _)) = chars.next() {
                last_valid_end = idx;
            }
        }
    }

    while let Some(&(idx, c)) = chars.peek() {
        match c {
            '0'..='9' => {
                has_digit = true;
                last_valid_end = idx + c.len_utf8();
                chars.next();

                // 如果我们在科学计数法中，现在有有效的指数
                if exponent_start.is_some() {
                    exponent_start = None; // 标记为有效
                }
            }
            '.' if !has_dot && exponent_start.is_none() => {
                has_dot = true;
                last_valid_end = idx + c.len_utf8();
                chars.next();
            }
            'e' | 'E' if has_digit && exponent_start.is_none() => {
                // 记录科学计数法开始位置
                exponent_start = Some(idx);
                last_valid_end = idx + c.len_utf8();
                chars.next();

                // 检查科学计数法符号
                if let Some(&(next_idx, next_c)) = chars.peek() {
                    if next_c == '+' || next_c == '-' {
                        last_valid_end = next_idx + next_c.len_utf8();
                        chars.next();
                    }
                }
            }
            _ => break,
        }
    }

    // 如果科学计数法开始了但没有完成，回退到科学计数法之前
    if let Some(exponent_idx) = exponent_start {
        if chars.peek().is_some() {
            // 还有字符，说明科学计数法未完成
            return Some((&s[..exponent_idx], &s[exponent_idx..]));
        }
    }

    if has_digit && last_valid_end > 0 {
        Some((&s[..last_valid_end], &s[last_valid_end..]))
    } else {
        None
    }
}

fn parse_number(s: &str) -> Option<Result<(serde_json::Number, &str)>> {
    match split_leading_number(s) {
        None => None,
        Some((number_str, remains)) => {
            if remains.is_empty() || token_horizontal_ending_position(remains).is_some() {
                let result = serde_json::Number::from_str(&number_str)
                    .map(|n| (n, remains))
                    .map_err(|e| crate::error::Error::from(e));
                Some(result)
            } else {
                None
            }
        }
    }
}
