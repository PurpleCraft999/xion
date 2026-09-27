use std::iter::Peekable;

use crate::token::Token::{self, *};

pub struct AstBuilder {
    tokens: Peekable<std::vec::IntoIter<Token>>,
    tree: Vec<Node>,
}
impl AstBuilder {
    pub fn new(mut tokens: Vec<Token>) -> Self {
        tokens.retain(|t| t != &Token::WhiteSpace);
        Self {
            tokens: tokens.into_iter().peekable(),
            tree: Vec::new(),
        }
    }

    fn next(&mut self) -> Option<Token> {
        self.tokens.next()
    }
    fn peek(&mut self) -> Option<&Token> {
        self.tokens.peek()
    }
    fn previous_node(&self) -> Option<&Node> {
        self.tree.last()
    }

    fn error(&self, err: &str) {
        println!("error: {}", err)
    }
    ///returns true if token was consumed
    fn consume_if(&mut self, c: impl FnOnce(&Token) -> bool) -> bool {
        self.tokens.next_if(c).is_some()
    }
    ///consums upto and including token
    fn consume_until(&mut self, t: Token) {
        let mut tokens = Vec::new();

        for token in self.tokens.by_ref() {
            if t == token {
                break;
            }

            tokens.push(token);
        }
    }
    // fn consume_whitespace(&mut self) {
    //     while let Some(token) = self.tokens.peek() {
    //         if token == &WhiteSpace {
    //             self.tokens.next();
    //         } else {
    //             break;
    //         }
    //     }
    // }
    fn next_if_name(&mut self) -> Option<String> {
        self.tokens.next_if_map(|t| match t {
            Token::Name(s) => Ok(s),
            _ => Err(t),
        })
    }
    fn parse_class(&mut self) -> Option<Node> {
        let Some(class_name) = self.next_if_name() else {
            self.error("token sould be name");
            return None;
        };
        // self.consume_whitespace();
        if !self.consume_if(|t| matches!(t, LeftBrace)) {
            self.error("next non whitespace token was not left brace")
        }
        //class fields
        // while let Some(token) = self.next() && token!=RightBrace{
        //     if token==WhiteSpace{
        //         continue;
        //     }

        // }

        self.consume_until(RightBrace);
        Some(Node::Class(ClassAst { _name: class_name }))
    }
    fn parse_var(&mut self) -> Option<Node> {
        // self.consume_whitespace();
        self.consume_if(|t| t == &Let);
        let Some(name) = self.next_if_name() else {
            self.error("no name after let");
            return None;
        };
        // self.consume_whitespace();
        if !self.consume_if(|t| t == &Equals) {
            self.error("not equals after var name");
            return None;
        }
        let next = self.next()?;
        let value = self.parse_expr(next)?;

        if !self.consume_if(|t| t == &SemiColon) {
            self.error("no semicolon after var");
            return None;
        }

        Some(Node::Var(Box::new(VarAst { name, value })))
    }
    fn parse_expr(&mut self, token: Token) -> Option<Node> {
        let node = match token {
            StringLiteral(name) => {
                //strings only support addition
                if self.peek() == Some(&Plus) {
                    self.tree.push(Node::StringLiteral(name));
                    let n = self.next()?;
                    self.parse_expr(n)?
                } else {
                    Node::StringLiteral(name)
                }
            }
            Name(name) => self.parse_name(name)?,
            NumberLiteral(num) => {
                if let Some(sign) = self.peek()
                    && sign.is_math_sign()
                {
                    self.tree.push(Node::NumberLiteral(num));
                    let n = self.next()?;
                    self.parse_expr(n)?
                } else {
                    Node::NumberLiteral(num)
                }
            }
            Return => {
                let token = self.next();
                let return_value = if let Some(value) = token {
                    self.parse_expr(value).map(Box::new)
                } else {
                    None
                };
                if !self.consume_if(|t| t == &SemiColon) {
                    self.error("return statement needs semicolon");
                    return None;
                }

                Node::Return(return_value)
            }
            Plus => {
                let next_token = self.next()?;
                Node::Math {
                    left: Box::new(self.previous_node()?.clone()),
                    op: MathSign::Plus,
                    right: Box::new(self.parse_expr(next_token)?),
                }
            }

            Let => self.parse_var()?,
            True => Node::BoolLiteral(true),
            False => Node::BoolLiteral(false),

            Fn | LeftBrace | RightBrace | LeftParen | RightParen | SemiColon | Comma | Colon
            | WhiteSpace | Equals | EOF | Class => {
                self.error(&format!("not expresion {:?}", token));
                return None;
            }
        };
        Some(node)
    }
    fn parse_args(&mut self) -> Vec<Node> {
        let mut args = Vec::new();
        self.consume_if(|t| t == &LeftParen);
        while let Some(token) = self.next() {
            if token == RightParen {
                break;
            }

            if let Some(expr) = self.parse_expr(token) {
                args.push(expr);
            } else {
                self.error("parsing args error");
            }
            if self.consume_if(|t| t == &Comma) {
                continue;
            }
        }

        args
    }

    fn parse_function(&mut self) -> Option<Node> {
        self.consume_if(|t| t == &Fn);
        let name = self.next_if_name()?;
        if !self.consume_if(|t| t == &LeftParen) {
            self.error("no left paren after function name");
            return None;
        }
        let mut params = Vec::new();

        while let Some(token) = self.next() {
            if token == RightParen {
                break;
            }

            if token == Comma {
                continue;
            }
            if let Name(param_name) = token {
                params.push(param_name);
                continue;
            }
            self.error(&format!(
                "unexpected token {:?} while parsing fn header",
                token
            ));
            break;
        }

        // if !self.consume_if(|t|t==&LeftBrace){
        //     self.error("not left brace after function header");
        //     return None
        // }

        Some(Node::FnDeclare(FunctionDefAst {
            name,
            paramaters: params,
            body: self.parse_scope(),
        }))
    }
    fn parse_scope(&mut self) -> Vec<Node> {
        if !self.consume_if(|t| t == &LeftBrace) {
            self.error("not left brace to start scope");
            return Vec::new();
        }

        let mut scope = Vec::new();
        while let Some(token) = self.next() {
            if token == RightBrace {
                println!("THE SCOPE IS:     {:?}", scope);
                break;
            }
            if let Some(var) = self.parse_expr(token) {
                scope.push(var);
            }
        }

        scope
    }
    fn parse_name(&mut self, name: String) -> Option<Node> {
        if self.peek() == Some(&LeftParen) {
            let args = self.parse_args();

            if !self.consume_if(|t| t == &SemiColon) {
                self.error("No semicolon after function call");
            }
            Some(Node::FnCall(FnCallAst { name, args }))
        } else if self.peek() == Some(&Equals) {
            self.next();
            let new_value = self.next()?;
            Some(Node::VarReasign {
                name,
                new_value: Box::new(self.parse_expr(new_value)?),
            })
        } else {
            Some(Node::VarRef(name))
        }
    }

    pub fn build(mut self) -> Vec<Node> {
        while let Some(token) = self.peek() {
            let node = match token {
                Class => self.parse_class(),
                Name(_) => {
                    // self.tokens.next_back()
                    let name = self
                        .next_if_name()
                        .expect("we are matching the name branch of the node");
                    self.parse_name(name)

                    // self.error("unexpeced name");
                }
                //never meant to be read here
                //whitespace cant be in at this point
                WhiteSpace | Colon | SemiColon | Comma | LeftBrace | RightBrace | Equals
                | LeftParen | RightParen | Return | Plus => {
                    // self.error(&format!("unexpected lang syntax {:?}", token));
                    self.next();
                    None
                }
                Let => self.parse_var(),
                StringLiteral(_) | NumberLiteral(_) => {
                    self.error("literal in main ast branch");
                    self.next();
                    None
                }

                Fn => self.parse_function(),
                True => Some(Node::BoolLiteral(true)),
                False => Some(Node::BoolLiteral(false)),
                EOF => break,
            };
            if let Some(node) = node {
                self.tree.push(node);
            }
            // else{
            //     self.error(&format!("unexpeced token {:?}",token));
            // }

            println!("tokens: {:?}", self.tokens);
        }
        println!("ast: {:?}", self.tree);
        self.tree
    }
}
#[derive(Debug, Clone)]
pub enum Node {
    Class(ClassAst),
    Var(Box<VarAst>),
    StringLiteral(String),
    NumberLiteral(i64),
    BoolLiteral(bool),
    ///name of var
    VarRef(String),
    FnCall(FnCallAst),
    VarReasign {
        name: String,
        new_value: Box<Node>,
    },

    FnDeclare(FunctionDefAst),
    Return(Option<Box<Node>>),

    Math {
        left: Box<Node>,
        op: MathSign,
        right: Box<Node>,
    },
}
#[derive(Debug, Clone)]
pub enum MathSign {
    Plus,
    Minus,
}

#[derive(Debug, Clone)]
pub struct ClassAst {
    _name: String,
}

#[derive(Debug, Clone)]
pub struct VarAst {
    pub name: String,
    pub value: Node,
}
#[derive(Debug, Clone)]
pub struct FnCallAst {
    pub name: String,
    pub args: Vec<Node>,
}

#[derive(Debug, Clone)]
pub struct FunctionDefAst {
    pub name: String,
    pub paramaters: Vec<String>,
    pub body: Vec<Node>,
}
