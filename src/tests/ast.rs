use crate::{
    ast::{
        AstBuilder, ClassAst, FnCallAst, FunctionDefAst,
        Node::{self, StringLiteral, *},
        VarAst,
    },
    token::Token::{self, *},
};

fn ast(vec: Vec<Token>) -> Vec<Node> {
    crate::test_log::init_logger();
    AstBuilder::new(vec).build()
}
#[allow(non_snake_case)]
fn Name(name: &str) -> Token {
    Token::Name(name.to_string())
}

#[test]
fn var_declaration() {
    let ast = ast(vec![
        Let,
        Name("var_name"),
        Equals,
        Token::NumberLiteral(50),
        SemiColon,
    ]);
    assert_eq!(
        vec![VarDeclare(Box::new(VarAst {
            name: "var_name".to_string(),
            value: Node::NumberLiteral(50)
        }))],
        ast
    )
}
#[test]
fn number_literal() {
    let ast = ast(vec![Token::NumberLiteral(50), SemiColon]);
    assert_eq!(vec![Node::NumberLiteral(50)], ast);
}

#[test]
fn string_literal() {
    let ast = ast(vec![Token::StringLiteral("hello".to_string()), SemiColon]);
    assert_eq!(vec![Node::StringLiteral("hello".to_string())], ast);
}

#[test]
fn boolean_literal() {
    let ast = ast(vec![True, SemiColon]);
    assert_eq!(vec![Node::BoolLiteral(true)], ast);
}

#[test]
fn array_literal() {
    let ast = ast(vec![
        LeftBracket,
        Token::NumberLiteral(1),
        Comma,
        Token::NumberLiteral(2),
        RightBracket,
        SemiColon,
    ]);
    assert_eq!(
        vec![Node::ArrayLiteral(vec![
            Node::NumberLiteral(1),
            Node::NumberLiteral(2)
        ])],
        ast
    );
}

#[test]
fn var_reassignment() {
    let ast = ast(vec![Name("x"), Equals, Token::NumberLiteral(10), SemiColon]);
    assert_eq!(
        vec![Node::VarReasign {
            name: "x".to_string(),
            new_value: Box::new(Node::NumberLiteral(10)),
        }],
        ast
    );
}

#[test]
fn fn_call() {
    let ast = ast(vec![
        Name("foo"),
        LeftParen,
        Token::NumberLiteral(5),
        RightParen,
        SemiColon,
    ]);
    assert_eq!(
        vec![Node::FnCall(FnCallAst {
            name: "foo".to_string(),
            args: vec![Node::NumberLiteral(5)],
        })],
        ast
    );
}

#[test]
fn fn_declaration() {
    let ast = ast(vec![
        Fn,
        Name("add"),
        LeftParen,
        Name("a"),
        Comma,
        Name("b"),
        RightParen,
        LeftBrace,
        Token::Return,
        Token::NumberLiteral(42),
        SemiColon,
        RightBrace,
    ]);
    assert_eq!(
        vec![Node::FnDeclare(FunctionDefAst {
            name: "add".to_string(),
            paramaters: vec!["a".to_string(), "b".to_string()],
            body: vec![Node::Return(Some(Box::new(Node::NumberLiteral(42))))],
        })],
        ast
    );
}

#[test]
fn class_declaration() {
    let ast = ast(vec![Class, Name("MyClass"), LeftBrace, RightBrace]);
    assert_eq!(
        vec![Node::ClassDeclare(ClassAst {
            name: "MyClass".to_string(),
            fields: vec![],
        })],
        ast
    );
}

#[test]
fn class_delcaration_with_variable() {
    let ast = ast(vec![
        Class,
        Name("MyClass"),
        LeftBrace,
        Let,
        Name("field"),
        Equals,
        Token::StringLiteral("hello world".to_string()),
        SemiColon,
        RightBrace,
    ]);
    assert_eq!(
        vec![Node::ClassDeclare(ClassAst {
            name: "MyClass".to_string(),
            fields: vec![VarDeclare(Box::new(VarAst {
                name: "field".to_string(),
                value: StringLiteral("hello world".to_string())
            }))]
        })],
        ast
    );
}
//just an empty function call without value
#[test]
fn class_constructor_with_value() {
    let ast = ast(vec![
        Name("Class"),
        LeftParen,
        Name("x"),
        Colon,
        Token::NumberLiteral(50),
        RightParen,
        SemiColon,
    ]);
    assert_eq!(
        vec![FnCall(FnCallAst {
            name: "Class".to_string(),
            args: vec![VarReasign {
                name: "x".to_string(),
                new_value: Box::new(Node::NumberLiteral(50))
            }]
        })],
        ast
    );
}
#[test]
fn class_field_access() {
    let ast = ast(vec![Name("point"), Dot, Name("x"), SemiColon]);
    assert_eq!(
        vec![FieldAccess {
            var_name: "point".to_string(),
            field_name: "x".to_string()
        }],
        ast
    )
}

#[test]
fn class_method_call() {
    let ast = ast(vec![
        Name("point"),
        Dot,
        Name("moveX"),
        LeftParen,
        Token::NumberLiteral(50),
        RightParen,
        SemiColon,
    ]);
    assert_eq!(
        vec![MethodCall {
            var_name: "point".to_string(),
            method_name: "moveX".to_string(),
            args: vec![Node::NumberLiteral(50)]
        }],
        ast
    )
}
