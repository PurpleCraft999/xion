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

    fn next_if_name(&mut self) -> Option<String> {
        self.tokens.next_if_map(|t| match t {
            Token::Name(s) => Ok(s),
            _ => Err(t),
        })
    }
    fn parse_class(&mut self) -> Option<Node> {
        self.consume_if(|t| t == &Class);
        let Some(class_name) = self.next_if_name() else {
            error!("token sould be name");
            return None;
        };
        if self.peek() != Some(&LeftBrace) {
            error!("next token was not left brace");
            return None;
        }

        let fields = self.parse_sequence_of_exprs(LeftBrace, RightBrace, SemiColon)?;

        Some(Node::ClassDeclare(ClassAst {
            name: class_name,
            fields,
        }))
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
        let value = self.parse_expr(next, &SemiColon)?;

        Some(Node::VarDeclare(Box::new(VarAst { name, value })))
    }
    fn parse_expr<'expr, E>(&mut self, token: Token, end_token: &'expr E) -> Option<Node>
    where
        E: Into<Option<Token>>,
        Option<&'expr Token>: From<&'expr E>,
    { 
        let node = match token {
            NumberLiteral(_) | Name(_) | StringLiteral(_) | LeftParen | Minus => {
                pratt_parser::parse_expression(self, 0, token, end_token.into())
            }
            Return => {
                let token = self.next();
                let return_value = if let Some(value) = token {
                    self.parse_expr::<Option<Token>>(value, &None).map(Box::new)
                } else {
                    None
                };

                Some(Node::Return(return_value))
            }
            LeftBracket => self.parse_array(),
            Let => self.parse_var(),
            Fn => self.parse_function(),
            True => Some(Node::BoolLiteral(true)),
            False => Some(Node::BoolLiteral(false)),
            LeftBrace | RightBrace | RightParen | SemiColon | Comma | Colon | WhiteSpace
            | Equals | EOF | Class | Plus | Asterisk | RightBracket | Division | Dot => {
                error!("unexpected token while parsing expresion: {:?}", token);
                None
            }
        };
        let end_token: Option<&Token> = end_token.into();
        if let Some(end_token) = end_token
            && self.peek() == Some(end_token)
        {
            self.next();
            node
        } else if end_token.is_none() {
            node
        } else {
            let end = if let Some(end) = end_token {
                format!("{end:?}").to_lowercase()
            } else {
                "No end was specified".to_string()
            };
            error!("no {end} after expresion {node:?}");
            None
        }
    }

    fn parse_sequence_of_exprs(&mut self, start: Token, end: Token, sep: Token) -> Option<Vec<Node>> {
        let mut args = Vec::new();
        if !self.consume_if(|t| t == &start) {
            warn!("List does not start with token: {start:?}");
        }
        while let Some(token) = self.next() {
            debug!("token = {token:?}, end token = {end:?}");
            if token == end {
                break;
            }

            if let Some(expr) = self.parse_expr(token, &None) {
                args.push(expr);
            } else {
                error!("list parse error");
                return None;
            }
            if self.consume_if(|t| t == &sep) {
                continue;
            }
        }

        Some(args)
    }

    fn parse_args(&mut self) -> Option<Vec<Node>> {
        self.parse_sequence_of_exprs(LeftParen, RightParen, Comma)
    }
    fn parse_array(&mut self) -> Option<Node> {
        
        Some(Node::ArrayLiteral(self.parse_sequence_of_exprs(
            LeftBracket,
            RightBracket,
            Comma,
        )?))
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
            body: self.parse_scope()?,
        }))
    }
    fn parse_scope(&mut self) -> Option<Vec<Node>> {
        self.parse_sequence_of_exprs(LeftBrace, RightBrace, SemiColon)
    }
    fn parse_name(&mut self, name: String) -> Option<Node> {
        let peeked = self.peek();
        match peeked {
            Some(LeftParen) => {
                let args = self.parse_args()?;

                Some(Node::FnCall(FnCallAst { name, args }))
            }
            Some(Equals) => {
                self.next();
                let new_value = self.next()?;
                Some(Node::VarReasign {
                    name,
                    new_value: Box::new(self.parse_expr(new_value, &None)?),
                })
            }
            //this branch is for assigning instance vars
            Some(Colon) => {
                self.next();
                let new_value = self.next()?;

                Some(Node::VarReasign {
                    name,
                    new_value: Box::new(self.parse_expr(new_value, &None)?),
                })
            }
            Some(Dot) => {
                self.next();
                let accessed_name = self.next_if_name()?;
                let peeked = self.peek();
                match peeked {
                    Some(LeftParen) => {
                        let args = self.parse_args()?;
                        Some(Node::MethodCall {
                            var_name: name,
                            method_name: accessed_name,
                            args,
                        })
                    }
                    Some(Equals) => {
                        self.next();
                        let value = self.next()?;
                        Some(Node::FieldReasign {
                            var_name: name,
                            field_name: accessed_name,
                            new_value: Box::new(self.parse_expr(value, &None)?),
                        })
                    }
                    _ => Some(Node::FieldAccess {
                        var_name: name,
                        field_name: accessed_name,
                    }),
                }
            }
            _ => Some(Node::VarRef(name)),
        }
    }

    pub fn build(mut self) -> Vec<Node> {
        while let Some(token) = self.peek() {
            let node = match token {
                Class => self.parse_class(),

                Let => self.parse_var(),

                Fn => self.parse_function(),

                EOF => break,

                _ => {
                    let next = self.next().expect("we already peeked");
                    self.parse_expr(next, &SemiColon)
                }
            };
            if let Some(node) = node {
                self.tree.push(node);
            }

            debug!("tokens: {:?}", self.tokens);
            debug!("ast: {:?}", self.tree);
        }

        self.tree
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub enum Node {
    ClassDeclare(ClassAst),
    VarDeclare(Box<VarAst>),
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
    MethodCall {
        var_name: String,
        method_name: String,
        args: Vec<Node>,
    },
    FieldAccess {
        var_name: String,
        field_name: String,
    },
    FieldReasign {
        var_name: String,
        field_name: String,
        new_value: Box<Node>,
    },
}
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub enum MathSign {
    Plus,
    Minus,
    Multiply,
    Division,
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct ClassAst {
    pub name: String,
    pub fields: Vec<Node>,
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct VarAst {
    pub name: String,
    pub value: Node,
}
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct FnCallAst {
    pub name: String,
    pub args: Vec<Node>,
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub struct FunctionDefAst {
    pub name: String,
    pub paramaters: Vec<String>,
    pub body: Vec<Node>,
}
///based on https://github.com/jdvillal/parser
mod pratt_parser {
    use log::{debug, error, warn};

    use crate::{
        ast::{AstBuilder, MathSign},
        token::Token::{self},
    };

    fn infix_binding_power(op: &MathSign) -> (u8, u8) {
        match op {
            MathSign::Plus | MathSign::Minus => (1, 2),
            MathSign::Multiply | MathSign::Division => (3, 4),
        }
    }

    pub fn parse_expression(
        lexer: &mut AstBuilder,
        min_bp: u8,
        start: Token,
        end_token: Option<&Token>,
    ) -> Option<super::Node> {
        let mut lhs = match start {
            Token::Name(it) => lexer.parse_name(it)?,
            Token::LeftParen => {
                let next = lexer.next()?;
                let lhs = parse_expression(lexer, 0, next, end_token);
                if !lexer.consume_if(|t| t == &Token::RightParen) {
                    error!("no closing paran");
                }
                lhs?
            }
            Token::RightParen => {
                error!("start was a )");

                return None;
            }
            Token::NumberLiteral(num) => super::Node::NumberLiteral(num),
            Token::StringLiteral(string) => super::Node::StringLiteral(string),
            Token::Minus => match lexer.peek() {
                Some(Token::NumberLiteral(num)) => {
                    let num = -*num;
                    lexer.next();
                    super::Node::NumberLiteral(num)
                }
                _ => return None,
            },
            t => {
                error!("bad token: {:?}", t);
                return None;
            }
        };
        loop {
            let peek = lexer.peek();
            if let Some(et) = end_token
                && peek == Some(et)
            {
                debug!("hit end token");
                break;
            }

            let op = match peek {
                Some(Token::RightParen)
                | None
                | Some(Token::RightBracket)
                | Some(Token::SemiColon)
                | Some(Token::Comma) => break,
                Some(Token::Plus) => MathSign::Plus,
                Some(Token::Minus) => MathSign::Minus,
                Some(Token::Asterisk) => MathSign::Multiply,
                Some(Token::Division) => MathSign::Division,

                Some(t) => {
                    warn!("unexpeced operator: {:?}", t);
                    break;
                }
            };
            let (l_bp, r_bp) = infix_binding_power(&op);
            if l_bp < min_bp {
                break;
            }

            lexer.next();
            let next = lexer.next()?;
            let rhs = parse_expression(lexer, r_bp, next, end_token)?;
            lhs = super::Node::Math {
                left: Box::new(lhs),
                op,
                right: Box::new(rhs),
            };
        }
        Some(lhs)
    }
}
#[cfg(test)]
#[path = "tests/ast.rs"]
mod test;
