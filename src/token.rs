use std::{iter::Peekable, str::Chars};

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
        while let Some(c) = &self.file.peek() {
            let c = **c;

            if c == s {
                self.next();
                break;
            }

            self.next();
            next.push(c);
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
                // if self.next().is_none() {
                //     println!("unended string literal")
                // }
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

            println!("unknown token {}", current_str);
        }
        self.add_token(Token::EOF);

        self.tokens
    }
}

const fn is_whitespace(c: char) -> bool {
    c == ' ' || c == '\n' || c == '\t'
}

// pub fn lex(file:&str)->Vec<Token>{
//     let mut file = file.split(|c| is_whitespace(c)).peekable();
//     let mut tokens = Vec::new();
//     loop {
//         let Some(current) = file.next() else {break;};

//         if let Some(token) = Token::from_str(current){
//             tokens.push(token);
//         }

//     }
//     tokens.push(Token::EOF);

//     tokens
// }

#[derive(Debug, PartialEq)]
pub enum Token {
    LeftBrace,
    RightBrace,
    LeftParen,
    RightParen,
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
    // Minus,
}

impl Token {
    fn from_str(s: &str) -> Option<Self> {
        let token = match s {
            KEY_WORD_LET => Let,
            KEY_WORD_CLASS => Class,
            "fn" | "func" => Fn,
            "return" => Return,
            "true" => True,
            "false" => False,
            number if let Ok(num) = number.parse::<i64>() => NumberLiteral(num),
            name if !s.starts_with(|c: char| c.is_numeric())
                && s.chars().all(|c| c.is_alphanumeric() || c == '_') =>
            {
                Name(name.to_string())
            }

            _ => return None,
        };
        Some(token)
    }

    fn from_char(c: char) -> Option<Self> {
        let token = match c {
            '{' => LeftBrace,
            '}' => RightBrace,
            ';' => SemiColon,
            ':' => Colon,
            ',' => Comma,
            '=' => Equals,
            '(' => LeftParen,
            ')' => RightParen,
            '+' =>Plus,
            whitespace if is_whitespace(whitespace) => WhiteSpace,
            _ => return None,
        };
        Some(token)
    }
    pub fn is_name(&self) -> bool {
        matches!(self, Name(_))
    }
    pub fn is_math_sign(&self)->bool{
        matches!(self,Plus)
    }
}
pub const KEY_WORD_LET: &str = "let";
pub const KEY_WORD_CLASS: &str = "class";
