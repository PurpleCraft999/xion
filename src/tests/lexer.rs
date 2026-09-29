use super::{
    Lexer,
    Token::{self, *},
};
// use crate::token::{Lexer,Token::{self,WhiteSpace}};
// use xion::token::Lexer;
// #[cfg(test)]
fn lex(str: &str) -> Vec<Token> {
    crate::test_log::init_logger();
    let mut v = Lexer::new(str).lex();
    v.retain(|t| t != &WhiteSpace);
    v.pop_if(|e| e == &EOF);
    v
}
#[allow(non_snake_case)]
fn Name(name: &str) -> Token {
    Token::Name(name.to_string())
}

#[test]
fn parse_var_assignment() {
    let lexed = lex("let var_name = 50");
    assert_eq!(
        vec![Let, Name("var_name"), Equals, NumberLiteral(50)],
        lexed
    );
}

#[test]
fn parse_function_define() {
    let lexed = lex("fn func(){}");
    assert_eq!(
        vec![
            Fn,
            Name("func"),
            LeftParen,
            RightParen,
            LeftBrace,
            RightBrace
        ],
        lexed
    );
}
#[test]
fn parse_function_with_args() {
    let lexed = lex("fn func(param1,param2){}");
    assert_eq!(
        vec![
            Fn,
            Name("func"),
            LeftParen,
            Name("param1"),
            Comma,
            Name("param2"),
            RightParen,
            LeftBrace,
            RightBrace
        ],
        lexed
    );
}

#[test]
fn parse_chars() {
    let lexed = lex("= / * + - ; { } [ ] , ( )");
    assert_eq!(
        vec![
            Equals,
            Division,
            Asterisk,
            Plus,
            Minus,
            SemiColon,
            LeftBrace,
            RightBrace,
            LeftBracket,
            RightBracket,
            Comma,
            LeftParen,
            RightParen
        ],
        lexed
    )
}

#[test]
fn parse_literals() {
    let lexed = lex("true false 50 \"string\"");
    assert_eq!(
        vec![
            True,
            False,
            NumberLiteral(50),
            StringLiteral("string".to_string())
        ],
        lexed
    )
}
