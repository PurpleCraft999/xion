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
    ///consumes raw_tokens upto but not including s
    fn consume_until(&mut self, s: char) -> String {
        let mut consumed = String::new();
        while let Some(c) = self.file.peek() {
            let c = *c;

            if c == s {
                break;
            }
            self.next();
            consumed.push(c);
        }
        if self.file.peek().is_none() {
            warn!(target:"lexer", "cannot find char {s:?}");
        }

        consumed
    }
    fn consume_comment(&mut self) {
        while let Some(c) = self.file.peek()
            && *c != '\n'
        {
            self.next();
        }
    }

    fn add_token(&mut self, token: Token) {
        self.tokens.push(token);
    }

    pub fn lex(mut self) -> Vec<Token> {
        while let Some(current) = self.next() {
            //comments
            if current == '#' {
                self.consume_comment();
                continue;
            }

            //string literals
            if current == '"' {
                let string_lit = self.consume_until('"');
                //consume end "
                self.next();
                let string = escape(string_lit);

                self.add_token(Token::StringLiteral(string));
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

            warn!(target:"lexer", "unknown token {}", current_str);
        }
        self.add_token(Token::EOF);

        self.tokens
    }
}

const fn is_whitespace(c: char) -> bool {
    c == ' ' || c == '\t'
}

///turns unicode chars into the value
fn escape(s: String) -> String {
    //FixMe: \\n to be parsed as r"\n" and not \
    s.replace(r"\n", "\n")
}

fn is_name(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_alphabetic())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
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
    NewLine,
    Comma,
    Equals,
    EOF,
    Let,
    Static,
    Name(String),
    StringLiteral(String),
    NumberLiteral(i64),
    BoolLiteral(bool),
    Class,
    Plus,
    Minus,
    Asterisk,
    Division,
    Dot,
    Import,
    If,
    IsEq,
    Null,
    QuestionMark,
}

impl Token {
    fn from_str(s: &str) -> Option<Self> {
        let token = match s {
            "let" => Let,
            "class" => Class,
            "fn" => Fn,
            "return" => Return,
            "true" => BoolLiteral(true),
            "false" => BoolLiteral(false),
            "import" => Import,
            "if" => If,
            "static" => Static,
            "null" => Null,
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
            '?' => QuestionMark,
            '\n' => NewLine,
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
    pub fn span(&self) -> u32 {
        match self {
            EOF => 0,
            LeftBrace | RightBrace | LeftBracket | RightBracket | SemiColon | Colon | Comma
            | Equals | LeftParen | RightParen | Plus | Minus | Asterisk | Division | Dot
            | NewLine | WhiteSpace | QuestionMark => 1,
            Fn | If | IsEq => 2,
            Let => 3,
            BoolLiteral(true) | Null => 4,
            BoolLiteral(false) | Class => 5,
            Static | Return | Import => 6,
            //i guess technicly we should check if len is higher than u32::Max for all three of these
            Name(name) => name.len() as u32,
            //two is added to the length because of quotes
            //use chars().count() because string length is in bytes not chars
            StringLiteral(str_lit) => str_lit.chars().count() as u32 + 2,
            //string length is fine becuase 0..9 is all ascii
            NumberLiteral(int) => int.to_string().len() as u32,
        }
    }
}

#[cfg(test)]
#[path = "tests/lexer.rs"]
mod test;
