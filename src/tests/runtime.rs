use super::*;
use crate::ast::VarAst;

#[test]
fn test_number_and_string_literals() {
    let nodes = vec![
        Node::NumberLiteral(42),
        Node::StringLiteral("hello xion".to_string()),
    ];

    let mut runtime = Runtime::start(nodes);
    let result = runtime.run();
    assert_eq!(result, None);
}

#[test]
fn test_variable_declaration_and_reference() {
    let nodes = vec![
        Node::VarDeclare(Box::new(VarAst {
            name: "x".to_string(),
            value: Node::NumberLiteral(100),
        })),
        Node::VarDeclare(Box::new(VarAst {
            name: "y".to_string(),
            value: Node::VarRef("x".to_string()),
        })),
    ];

    let mut runtime = Runtime::start(nodes);
    runtime.run();

    let scope = runtime.get_current_scope();
    assert!(
        scope
            .with_var("y", |var_y| {
                assert_eq!(*var_y.get(), Value::Number(100));
            })
            .is_some(),
        "var y should exist"
    );
}

#[test]
fn test_variable_reassignment() {
    let nodes = vec![
        Node::VarDeclare(Box::new(VarAst {
            name: "x".to_string(),
            value: Node::NumberLiteral(10),
        })),
        Node::VarReasign {
            name: "x".to_string(),
            new_value: Box::new(Node::NumberLiteral(20)),
        },
    ];

    let mut runtime = Runtime::start(nodes);
    runtime.run();

    let scope = runtime.get_current_scope();
    assert!(
        scope
            .with_var("x", |var_x| {
                assert_eq!(*var_x.get(), Value::Number(20));
            })
            .is_some(),
        "var x should exist"
    )
}

#[test]
fn test_math_operations() {
    let nodes = vec![Node::VarDeclare(Box::new(VarAst {
        name: "result".to_string(),
        value: Node::Math {
            left: Box::new(Node::Math {
                left: Box::new(Node::Math {
                    left: Box::new(Node::NumberLiteral(10)),
                    op: MathSign::Plus,
                    right: Box::new(Node::NumberLiteral(5)),
                }),
                op: MathSign::Multiply,
                right: Box::new(Node::NumberLiteral(2)),
            }),
            op: MathSign::Minus,
            right: Box::new(Node::Math {
                left: Box::new(Node::NumberLiteral(4)),
                op: MathSign::Division,
                right: Box::new(Node::NumberLiteral(2)),
            }),
        },
    }))];

    let mut runtime = Runtime::start(nodes);
    runtime.run();

    let scope = runtime.get_current_scope();

    assert!(
        scope
            .with_var("result", |var_x| {
                assert_eq!(*var_x.get(), Value::Number(28));
            })
            .is_some(),
        "var result should exist"
    )
}

#[test]
fn test_return_statement() {
    let nodes = vec![
        Node::Return(Some(Box::new(Node::NumberLiteral(99)))),
        Node::Return(Some(Box::new(Node::NumberLiteral(100)))),
    ];

    let mut runtime = Runtime::start(nodes);
    let return_val = runtime.run();

    assert_eq!(return_val, Some(Value::Number(99)));
}

#[test]
fn test_array_literal_evaluation() {
    let nodes = vec![Node::VarDeclare(Box::new(VarAst {
        name: "arr".to_string(),
        value: Node::ArrayLiteral(vec![
            Node::NumberLiteral(1),
            Node::StringLiteral("test".to_string()),
            Node::BoolLiteral(true),
        ]),
    }))];

    let mut runtime = Runtime::start(nodes);
    runtime.run();

    let scope = runtime.get_current_scope();
    assert!(
        scope
            .with_var("arr", |arr_var| match &*arr_var.get() {
                Value::Array(vec) => {
                    assert_eq!(vec.len(), 3);
                    assert_eq!(vec[0], Value::Number(1));
                    assert_eq!(vec[1], Value::String("test".to_string()));
                    assert_eq!(vec[2], Value::Bool(true));
                }
                _ => panic!("Expected Value::Array"),
            })
            .is_some()
    );
}
