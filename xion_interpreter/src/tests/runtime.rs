use crate::ast::{AstBuilder, VariableAttributes};

use super::*;

fn run(nodes: Vec<Node>) -> (RuntimeReturn, Runtime) {
    crate::test_log::init_logger();
    let mut run = Runtime::main(nodes).expect("test env should not fail");
    let err = run.run();
    if let Err(err) = err {
        panic!("{err}")
    }
    (err.map(|s| s.as_value()), run)
}
use crate::parse_and_lex;
macro_rules! run {
    ($nodes:expr) => {
        run($nodes).0
    };
    ($nodes:expr ; Runtime) => {
        run($nodes).1
    };
    (parse:$p:tt) => {
        run(parse_and_lex($p)).0
    };
    (parse:$p:tt;Runtime) => {
        run(parse_and_lex($p)).1
    };
}

#[test]
fn test_number_and_string_literals() {
    let nodes = vec![
        Node::NumberLiteral(42),
        Node::StringLiteral("hello xion".to_string()),
    ];

    let result = run!(nodes);
    assert_eq!(result, Ok(None));
}

#[test]
fn test_variable_declaration_and_reference() {
    let nodes = vec![
        Node::VarDeclare {
            attributes: VariableAttributes::default(),

            name: "x".to_string(),
            value: Box::new(Node::NumberLiteral(100)),
        },
        Node::VarDeclare {
            attributes: VariableAttributes::default(),

            name: "y".to_string(),
            value: Box::new(Node::VarRef("x".to_string())),
        },
    ];

    let runtime = run!(nodes;Runtime);

    let scope = runtime.get_current_scope();
    assert!(
        scope
            .with_var("y", |var_y| {
                assert_eq!(*var_y.get_value(), Value::Number(100));
            })
            .is_some(),
        "var y should exist"
    );
}

#[test]
fn test_variable_reassignment() {
    let nodes = vec![
        Node::VarDeclare {
            attributes: VariableAttributes::default(),

            name: "x".to_string(),
            value: Box::new(Node::NumberLiteral(10)),
        },
        Node::VarReasign {
            name: "x".to_string(),
            new_value: Box::new(Node::NumberLiteral(20)),
        },
    ];

    let runtime = run!(nodes;Runtime);

    let scope = runtime.get_current_scope();
    assert!(
        scope
            .with_var("x", |var_x| {
                assert_eq!(*var_x.get_value(), Value::Number(20));
            })
            .is_some(),
        "var x should exist"
    )
}

#[test]
fn test_math_operations() {
    let nodes = vec![Node::VarDeclare {
        attributes: VariableAttributes::default(),

        name: "result".to_string(),
        value: Box::new(Node::Math {
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
        }),
    }];

    let runtime = run!(nodes;Runtime);

    let scope = runtime.get_current_scope();

    assert!(
        scope
            .with_var("result", |var_x| {
                assert_eq!(*var_x.get_value(), Value::Number(28));
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

    let return_val = run!(nodes);

    assert_eq!(return_val, Ok(Some(Value::Number(99))));
}

#[test]
fn test_array_literal_evaluation() {
    let nodes = vec![Node::VarDeclare {
        attributes: VariableAttributes::default(),

        name: "arr".to_string(),
        value: Box::new(Node::ArrayLiteral(vec![
            Node::NumberLiteral(1),
            Node::StringLiteral("test".to_string()),
            Node::BoolLiteral(true),
        ])),
    }];

    let runtime = run!(nodes;Runtime);

    let scope = runtime.get_current_scope();
    assert!(
        scope
            .with_var("arr", |arr_var| match &*arr_var.get_value() {
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

#[test]
fn test_function_declare() {
    let nodes = vec![Node::FnDeclare {
        name: "my_func".to_string(),
        parameters: vec!["a".to_string()],
        body: vec![],
    }];

    let runtime = run!(nodes; Runtime);
    let scope = runtime.get_current_scope();

    assert!(
        scope.with_function("my_func", |_| {}).is_some(),
        "function my_func should be declared in the scope"
    );
}

#[test]
fn test_function_call() {
    let nodes = vec![
        Node::FnDeclare {
            name: "add_one".to_string(),
            parameters: vec!["n".to_string()],
            body: vec![Node::Return(Some(Box::new(Node::Math {
                left: Box::new(Node::VarRef("n".to_string())),
                op: MathSign::Plus,
                right: Box::new(Node::NumberLiteral(1)),
            })))],
        },
        Node::VarDeclare {
            attributes: VariableAttributes::default(),

            name: "res".to_string(),
            value: Box::new(Node::FnCall {
                name: "add_one".to_string(),
                arguments: vec![Node::NumberLiteral(10)],
            }),
        },
    ];

    let runtime = run!(nodes; Runtime);
    let scope = runtime.get_current_scope();

    assert!(
        scope
            .with_var("res", |var| {
                assert_eq!(*var.get_value(), Value::Number(11));
            })
            .is_some(),
        "variable res should equal 11"
    );
}

#[test]
fn test_class_declare() {
    let nodes = vec![Node::ClassDeclare {
        name: "Point".to_string(),
        body: vec![Node::VarDeclare {
            attributes: VariableAttributes::default(),

            name: "x".to_string(),
            value: Box::new(Node::NumberLiteral(5)),
        }],
    }];

    let runtime = run!(nodes; Runtime);
    let scope = runtime.get_current_scope();

    assert!(
        scope.get_class("Point").is_some(),
        "Class Point should be declared"
    );
}

#[test]
fn test_constructor_var_assigned_to_field() {
    let nodes = vec![
        Node::VarDeclare {
            attributes: VariableAttributes::default(),

            name: "initial_val".to_string(),
            value: Box::new(Node::NumberLiteral(77)),
        },
        Node::ClassDeclare {
            name: "BoxObj".to_string(),
            body: vec![Node::VarDeclare {
                attributes: VariableAttributes::default(),

                name: "val".to_string(),
                value: Box::new(Node::NumberLiteral(0)),
            }],
        },
        Node::VarDeclare {
            attributes: VariableAttributes::default(),

            name: "obj".to_string(),
            value: Box::new(Node::FnCall {
                name: "BoxObj".to_string(),
                arguments: vec![Node::VarReasign {
                    name: "val".to_string(),
                    new_value: Box::new(Node::VarRef("initial_val".to_string())),
                }],
            }),
        },
    ];

    let runtime = run!(nodes; Runtime);
    let scope = runtime.get_current_scope();

    assert!(
        scope
            .with_var("obj", |var| {
                if let Value::Object(obj) = var.get_value() {
                    let field = obj.get_field("val").expect("field val should exist");
                    assert_eq!(*field.get_value(), Value::Number(77));
                } else {
                    panic!("Expected Value::Object");
                }
            })
            .is_some(),
        "instance obj should have field val set to 77"
    );
}

#[test]
fn test_constructor_var_assigned_to_field_shared_name() {
    let nodes = vec![
        Node::VarDeclare {
            attributes: VariableAttributes::default(),

            name: "x".to_string(),
            value: Box::new(Node::NumberLiteral(100)),
        },
        Node::ClassDeclare {
            name: "Container".to_string(),
            body: vec![Node::VarDeclare {
                attributes: VariableAttributes::default(),

                name: "x".to_string(),
                value: Box::new(Node::NumberLiteral(0)),
            }],
        },
        Node::VarDeclare {
            attributes: VariableAttributes::default(),

            name: "cnt".to_string(),
            value: Box::new(Node::FnCall {
                name: "Container".to_string(),
                arguments: vec![Node::VarReasign {
                    name: "x".to_string(),
                    new_value: Box::new(Node::VarRef("x".to_string())),
                }],
            }),
        },
    ];

    let runtime = run!(nodes; Runtime);
    let scope = runtime.get_current_scope();

    assert!(
        scope
            .with_var("cnt", |var| {
                if let Value::Object(obj) = var.get_value() {
                    let field_x = obj.get_field("x").expect("field x should exist");
                    assert_eq!(*field_x.get_value(), Value::Number(100));
                } else {
                    panic!("Expected Value::Object");
                }
            })
            .is_some(),
        "cnt instance should be initialized properly with shared names"
    );
}

#[test]
fn test_field_access() {
    let nodes = vec![
        Node::ClassDeclare {
            name: "Item".to_string(),
            body: vec![Node::VarDeclare {
                attributes: VariableAttributes::default(),

                name: "price".to_string(),
                value: Box::new(Node::NumberLiteral(250)),
            }],
        },
        Node::VarDeclare {
            attributes: VariableAttributes::default(),

            name: "item".to_string(),
            value: Box::new(Node::FnCall {
                name: "Item".to_string(),
                arguments: vec![],
            }),
        },
        Node::VarDeclare {
            attributes: VariableAttributes::default(),

            name: "fetched_price".to_string(),
            value: Box::new(Node::FieldAccess {
                var_name: "item".to_string(),
                field_name: "price".to_string(),
            }),
        },
    ];

    let runtime = run!(nodes; Runtime);
    let scope = runtime.get_current_scope();

    assert!(
        scope
            .with_var("fetched_price", |var| {
                assert_eq!(*var.get_value(), Value::Number(250));
            })
            .is_some(),
        "fetched_price should equal 250"
    );
}

#[test]
fn test_field_reassignment() {
    let nodes = vec![
        Node::ClassDeclare {
            name: "Account".to_string(),
            body: vec![Node::VarDeclare {
                attributes: VariableAttributes::default(),

                name: "balance".to_string(),
                value: Box::new(Node::NumberLiteral(50)),
            }],
        },
        Node::VarDeclare {
            attributes: VariableAttributes::default(),

            name: "acc".to_string(),
            value: Box::new(Node::FnCall {
                name: "Account".to_string(),
                arguments: vec![],
            }),
        },
        Node::FieldReasign {
            var_name: "acc".to_string(),
            field_name: "balance".to_string(),
            new_value: Box::new(Node::NumberLiteral(150)),
        },
    ];

    let runtime = run!(nodes; Runtime);
    let scope = runtime.get_current_scope();

    assert!(
        scope
            .with_var("acc", |var| {
                if let Value::Object(obj) = var.get_value() {
                    let balance_field = obj
                        .get_field("balance")
                        .expect("balance field should exist");
                    assert_eq!(*balance_field.get_value(), Value::Number(150));
                } else {
                    panic!("Expected Value::Object");
                }
            })
            .is_some(),
        "acc balance should be updated to 150 via field reassignment"
    );
}

#[test]
fn test_method_call() {
    let nodes = vec![
        Node::ClassDeclare {
            name: "Wallet".to_string(),
            body: vec![
                Node::VarDeclare {
                    attributes: VariableAttributes::default(),

                    name: "total".to_string(),
                    value: Box::new(Node::NumberLiteral(10)),
                },
                Node::FnDeclare {
                    name: "deposit".to_string(),
                    parameters: vec!["amount".to_string()],
                    body: vec![Node::VarReasign {
                        name: "total".to_string(),
                        new_value: Box::new(VarRef("amount".to_string())),
                    }],
                },
            ],
        },
        Node::VarDeclare {
            attributes: VariableAttributes::default(),
            name: "w".to_string(),
            value: Box::new(Node::FnCall {
                name: "Wallet".to_string(),
                arguments: vec![],
            }),
        },
        Node::MethodCall {
            var_name: "w".to_string(),
            method_name: "deposit".to_string(),
            args: vec![Node::NumberLiteral(50)],
        },
    ];

    let runtime = run!(nodes; Runtime);
    let scope = runtime.get_current_scope();

    assert!(
        scope.with_var("w", |_| {}).is_some(),
        "method call statement should execute"
    );
}

#[test]
fn floating_point_math() {
    let s = crate::token::Lexer::new(
        r#"
let a =(5.0*1.1);
let b =(5.0/1.1);
let c =(5.0+1.1);
let d =(5.0-1.1);
let e =(5.0*2);
let f =(5.0/2);
let g =(5.0+2);
let h =(5.0-2);
let i =(5*2.0);
let j =(5/2.0);
let k =(5+2.0);
let l =(5-2.0);
let m =(5.0+"");
let n =(""+5.0);"#,
    )
    .lex();

    let s = AstBuilder::new(s).build();

    let run = run!(s;Runtime);

    let expected = HashMap::from([
        ("a", 5.5),
        ("b", 4.545454545454545),
        ("c", 6.1),
        ("d", 3.9),
        ("e", 10.),
        ("f", 2.5),
        ("g", 7.0),
        ("h", 3.0),
        ("i", 10.),
        ("j", 2.5),
        ("k", 7.0),
        ("l", 3.0),
    ]);
    let mut expected: HashMap<String, Variable> = expected
        .into_iter()
        .map(|(k, v)| (k.to_string(), Variable::new(Value::Float(v))))
        .collect();
    expected.insert(
        String::from("m"),
        Variable::new(Value::String("5".to_string())),
    );
    expected.insert(
        String::from("n"),
        Variable::new(Value::String("5".to_string())),
    );

    assert!(
        run.current_scope
            .variables
            .into_iter()
            .all(|(k, v)| expected.get(&k).unwrap() == &v)
    );
}

#[test]
fn test_nested_return() {
    let nodes = vec![
        If {
            condition: Box::new(Node::BoolLiteral(true)),
            body: vec![Return(Some(Box::new(Node::BoolLiteral(true))))],
        },
        Node::Return(Some(Box::new(Node::BoolLiteral(false)))),
    ];
    let r = run!(nodes);
    assert_eq!(Ok(Some(Value::Bool(true))), r);
}

#[test]
fn test_class_static_var_access() {
    let nodes = vec![
        ClassDeclare {
            name: "Test".to_string(),
            body: vec![Node::VarDeclare {
                attributes: VariableAttributes::attr_static(),
                name: "test".to_string(),
                value: Box::new(NumberLiteral(3)),
            }],
        },
        Node::Return(Some(Box::new(FieldAccess {
            var_name: "Test".into(),
            field_name: "test".into(),
        }))),
    ];
    let r = run!(nodes);
    assert_eq!(Ok(Some(Value::Number(3))), r);
}

#[test]
fn use_constructor_in_class_body() {
    let s = run!(parse:"class Test{ let static test = Test();   }");
    assert_eq!(Ok(None), s)
}
