use super::*;

fn run(nodes: Vec<Node>) -> (RuntimeReturn, Runtime) {
    crate::test_log::init_logger();
    let mut run = Runtime::main(nodes);
    let err = run.run();
    if let Err(err) = err {
        panic!("{err}")
    }
    (err, run)
}
macro_rules! run {
    ($nodes:expr) => {
        run($nodes).0
    };
    ($nodes:expr ; Runtime) => {
        run($nodes).1
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
            name: "x".to_string(),
            value: Box::new(Node::NumberLiteral(100)),
        },
        Node::VarDeclare {
            name: "y".to_string(),
            value: Box::new(Node::VarRef("x".to_string())),
        },
    ];

    let runtime = run!(nodes;Runtime);

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
        Node::VarDeclare {
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
                assert_eq!(*var_x.get(), Value::Number(20));
            })
            .is_some(),
        "var x should exist"
    )
}

#[test]
fn test_math_operations() {
    let nodes = vec![Node::VarDeclare {
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

    let return_val = run!(nodes);

    assert_eq!(return_val, Ok(Some(Value::Number(99))));
}

#[test]
fn test_array_literal_evaluation() {
    let nodes = vec![Node::VarDeclare {
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
                assert_eq!(*var.get(), Value::Number(11));
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
            name: "initial_val".to_string(),
            value: Box::new(Node::NumberLiteral(77)),
        },
        Node::ClassDeclare {
            name: "BoxObj".to_string(),
            body: vec![Node::VarDeclare {
                name: "val".to_string(),
                value: Box::new(Node::NumberLiteral(0)),
            }],
        },
        Node::VarDeclare {
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
                if let Value::Object(obj) = var.get() {
                    let field = obj.get_field("val").expect("field val should exist");
                    assert_eq!(*field.get(), Value::Number(77));
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
            name: "x".to_string(),
            value: Box::new(Node::NumberLiteral(100)),
        },
        Node::ClassDeclare {
            name: "Container".to_string(),
            body: vec![Node::VarDeclare {
                name: "x".to_string(),
                value: Box::new(Node::NumberLiteral(0)),
            }],
        },
        Node::VarDeclare {
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
                if let Value::Object(obj) = var.get() {
                    let field_x = obj.get_field("x").expect("field x should exist");
                    assert_eq!(*field_x.get(), Value::Number(100));
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
                name: "price".to_string(),
                value: Box::new(Node::NumberLiteral(250)),
            }],
        },
        Node::VarDeclare {
            name: "item".to_string(),
            value: Box::new(Node::FnCall {
                name: "Item".to_string(),
                arguments: vec![],
            }),
        },
        Node::VarDeclare {
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
                assert_eq!(*var.get(), Value::Number(250));
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
                name: "balance".to_string(),
                value: Box::new(Node::NumberLiteral(50)),
            }],
        },
        Node::VarDeclare {
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
                if let Value::Object(obj) = var.get() {
                    let balance_field = obj
                        .get_field("balance")
                        .expect("balance field should exist");
                    assert_eq!(*balance_field.get(), Value::Number(150));
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
