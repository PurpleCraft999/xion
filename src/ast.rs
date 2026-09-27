use std::iter::Peekable;

use log::{debug, error, warn};

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
    fn next_if_name(&mut self) -> Option<String> {
        self.tokens.next_if_map(|t| match t {
            Token::Name(s) => Ok(s),
            _ => Err(t),
        })
    }
    fn parse_class(&mut self) -> Option<Node> {
        let Some(class_name) = self.next_if_name() else {
            error!("token sould be name");
            return None;
        };
        if !self.consume_if(|t| matches!(t, LeftBrace)) {
            error!("next non whitespace token was not left brace")
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
        self.consume_if(|t| t == &Let);
        let Some(name) = self.next_if_name() else {
            error!("no name after let");
            return None;
        };
        if !self.consume_if(|t| t == &Equals) {
            error!("not equals after var name");
            return None;
        }
        let next = self.next()?;
        let value = self.parse_expr(next)?;

        if !self.consume_if(|t| t == &SemiColon) {
            error!("no semicolon after var");
            return None;
        }

        Some(Node::Var(Box::new(VarAst { name, value })))
    }
    fn parse_expr(&mut self, token: Token) -> Option<Node> {
        match token {
            StringLiteral(name) => {
                //strings only support addition
                if self.peek() == Some(&Plus) {
                    self.tree.push(Node::StringLiteral(name));
                    let n = self.next()?;
                    self.parse_expr(n)
                } else {
                    Some(Node::StringLiteral(name))
                }
            }
            Name(name) => self.parse_name(name),
            NumberLiteral(_) | LeftParen => self.parse_math_equasion(token),
            Return => {
                let token = self.next();
                let return_value = if let Some(value) = token {
                    self.parse_expr(value).map(Box::new)
                } else {
                    None
                };
                if !self.consume_if(|t| t == &SemiColon) {
                    error!("return statement needs semicolon");
                    return None;
                }

                Some(Node::Return(return_value))
            }
            Minus => match self.peek()? {
                NumberLiteral(num) => {
                    let num = -*num;
                    self.next();
                    Some(Node::NumberLiteral(num))
                }
                _ => None,
            },
            LeftBracket => self.parse_array(),

            Let => self.parse_var(),
            True => Some(Node::BoolLiteral(true)),
            False => Some(Node::BoolLiteral(false)),

            Fn | LeftBrace | RightBrace | RightParen | SemiColon | Comma | Colon | WhiteSpace
            | Equals | EOF | Class | Plus | Asterisk | RightBracket => {
                error!("unexpected expresion while parsing expresion: {:?}", token);
                None
            }
        }
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
                error!("parsing args error");
            }
            if self.consume_if(|t| t == &Comma) {
                continue;
            }
        }

        args
    }
    fn parse_array(&mut self) -> Option<Node> {
        self.consume_if(|t| t == &LeftBracket);

        let mut vec = Vec::new();

        while let Some(token) = self.next() {
            if token == RightBracket {
                break;
            }
            if let Some(expr) = self.parse_expr(token) {
                vec.push(expr);
            } else {
                error!("parsing array literal value error");
            }
            if self.consume_if(|t| t == &Comma) {
                continue;
            }
        }

        Some(Node::ArrayLiteral(vec))
    }

    fn parse_function(&mut self) -> Option<Node> {
        self.consume_if(|t| t == &Fn);
        let name = self.next_if_name()?;
        if !self.consume_if(|t| t == &LeftParen) {
            error!("no left paren after function name");
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
            error!("unexpected token {:?} while parsing fn header", token);
            break;
        }

        Some(Node::FnDeclare(FunctionDefAst {
            name,
            paramaters: params,
            body: self.parse_scope(),
        }))
    }
    fn parse_scope(&mut self) -> Vec<Node> {
        if !self.consume_if(|t| t == &LeftBrace) {
            warn!("not left brace to start scope");
        }

        let mut scope = Vec::new();
        while let Some(token) = self.next() {
            if token == RightBrace {
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
                error!("No semicolon after function call");
                return None;
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
                    let name = self
                        .next_if_name()
                        .expect("we are matching the name branch of the node");
                    self.parse_name(name)
                }
                //never meant to be read here
                //whitespace cant be in at this point
                WhiteSpace | Colon | SemiColon | Comma | LeftBrace | RightBrace | Equals
                | LeftParen | RightParen | Return | Plus | Minus | Asterisk | LeftBracket
                | RightBracket => {
                    warn!("unexpected lang syntax {:?}", token);
                    self.next();
                    None
                }
                Let => self.parse_var(),
                StringLiteral(_) | NumberLiteral(_) => {
                    error!("literal in main ast branch");
                    self.next();
                    None
                }

                Fn => self.parse_function(),
                True => {
                    self.next();
                    Some(Node::BoolLiteral(true))
                }
                False => {
                    self.next();
                    Some(Node::BoolLiteral(false))
                }
                EOF => break,
            };
            if let Some(node) = node {
                self.tree.push(node);
            }

            debug!("tokens: {:?}", self.tokens);
        }
        debug!("ast: {:?}", self.tree);
        self.tree
    }
}
//math operator stuff
impl AstBuilder {
    fn parse_math_equasion(&mut self, number: Token) -> Option<Node> {
        let mut left = self.parse_mult_div(number)?;

        while let Some(token) = self.peek() {
            if token.is_addition_or_subtraction() {
                let op = match self.peek()? {
                    Plus => MathSign::Plus,
                    Minus => MathSign::Minus,
                    _ => return None,
                };
                self.next();
                let n = self.next()?;
                let right = self.parse_mult_div(n)?;
                left = Node::Math {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        Some(left)
    }

    fn parse_mult_div(&mut self, start: Token) -> Option<Node> {
        let mut left = self.parse_primary(start)?;

        while let Some(token) = self.peek() {
            if token.is_multiplication_or_division() {
                let op = match self.peek()? {
                    Asterisk => MathSign::Multiply,
                    _ => return None,
                };
                self.next();
                let n = self.next()?;
                let right = self.parse_primary(n)?;
                left = Node::Math {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                };
            } else {
                break;
            }
        }
        Some(left)
    }

    fn parse_primary(&mut self, start: Token) -> Option<Node> {
        match start {
            Token::NumberLiteral(num) => Some(Node::NumberLiteral(num)),
            Token::LeftParen => {
                let expr = self.parse_math_equasion(start)?;
                if !self.consume_if(|t| t == &RightParen) {
                    error!("no closing parenthises for math expresion");
                    return None;
                }
                Some(expr)
            }
            other => {
                error!("Unexpected token: {:?}", other);
                None
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Node {
    Class(ClassAst),
    Var(Box<VarAst>),
    StringLiteral(String),
    NumberLiteral(i64),
    BoolLiteral(bool),
    ArrayLiteral(Vec<Node>),
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
    Multiply,
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
