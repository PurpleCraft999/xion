use std::{iter::Peekable, str::Chars};

use log::warn;

use crate::token::Token::*;

pub struct Lexer<'a> {
    file: Peekable<Chars<'a>>,
    tokens: Vec<Token>,
}
impl<'a> Lexer<'a> {
    pub fn new(file: &'a str) -> Self {
        Self {
            file: file.chars().peekable(),
            tokens: Vec::new(),
        }
    }
    fn next(&mut self) -> Option<char> {
        self.file.next()
    }

    fn consume_until_char_token(&mut self, current: char) -> String {
        let mut next = String::from(current);
        while let Some(c) = &self.file.peek() {
            let c = **c;
            if Token::from_char(c).is_some() {
                break;
            }
            self.next();
            next.push(c);
        }
        next
    }
    ///consumes raw_tokens upto and including s
    fn consume_until(&mut self, s: char) -> String {
        let mut next = String::new();
        while let Some(c) = &self.file.next() {
            let c = *c;

            if c == s {
                break;
            }

            next.push(c);
        }
        if self.file.peek().is_none() {
            warn!("cannot find char \'{s:?}\'");
        }

        next
    }

    fn add_token(&mut self, token: Token) {
        self.tokens.push(token);
    }

    pub fn lex(mut self) -> Vec<Token> {
        while let Some(current) = self.next() {
            //comments
            if current == '#' {
                self.consume_until('\n');
                continue;
            }

            //string literals
            if current == '"' {
                let string_lit = self.consume_until('"');
                self.add_token(Token::StringLiteral(string_lit));
                continue;
            }
            if let Some(peek) = self.file.peek()
                && let Some(token) = Token::from_two_char(current, *peek)
            {
                self.next();
                self.add_token(token);
                continue;
            }

            if let Some(token) = Token::from_char(current) {
                self.add_token(token);
                continue;
            }

            let current_str = self.consume_until_char_token(current);

            if let Some(token) = Token::from_str(&current_str) {
                self.add_token(token);
                continue;
            }

            warn!("unknown token {}", current_str);
        }
        self.add_token(Token::EOF);

        self.tokens
    }
}

const fn is_whitespace(c: char) -> bool {
    c == ' ' || c == '\n' || c == '\t'
}
fn is_name(name: &str) -> bool {
    !name.starts_with(|c: char| c.is_numeric())
        && name.chars().all(|c| c.is_alphanumeric() || c == '_')
}

#[derive(Debug, PartialEq)]
pub enum Token {
    LeftBrace,
    RightBrace,
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    SemiColon,
    Colon,
    Fn,
    Return,
    WhiteSpace,
    Comma,
    Equals,
    EOF,
    Let,
    Name(String),
    StringLiteral(String),
    NumberLiteral(i64),
    Class,
    True,
    False,
    Plus,
    Minus,
    Asterisk,
    Division,
    Dot,
    Import,
    If,
    IsEq,
}

impl Token {
    fn from_str(s: &str) -> Option<Self> {
        let token = match s {
            "let" => Let,
            "class" => Class,
            "fn" => Fn,
            "return" => Return,
            "true" => True,
            "false" => False,
            "import" => Import,
            "if" => If,
            number if let Ok(num) = number.parse::<i64>() => NumberLiteral(num),
            name if is_name(name) => Name(name.to_string()),

            _ => return None,
        };
        Some(token)
    }

    fn from_char(c: char) -> Option<Self> {
        let token = match c {
            '{' => LeftBrace,
            '}' => RightBrace,
            '[' => LeftBracket,
            ']' => RightBracket,
            ';' => SemiColon,
            ':' => Colon,
            ',' => Comma,
            '=' => Equals,
            '(' => LeftParen,
            ')' => RightParen,
            '+' => Plus,
            '-' => Minus,
            '*' => Asterisk,
            '/' => Division,
            '.' => Dot,
            whitespace if is_whitespace(whitespace) => WhiteSpace,
            _ => return None,
        };
        Some(token)
    }
    fn from_two_char(c1: char, c2: char) -> Option<Self> {
        let token = match (c1, c2) {
            ('=', '=') => IsEq,
            _ => return None,
        };
        Some(token)
    }
    pub fn is_name(&self) -> bool {
        matches!(self, Name(_))
    }
    pub fn is_math_sign(&self) -> bool {
        matches!(self, Plus | Minus | Asterisk)
    }
}

#[cfg(test)]
#[path = "tests/lexer.rs"]
mod test;
