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

    // fn next(&mut self) -> Option<Token> {
    //     self.tokens.next()
    // }
    fn peek(&mut self) -> Option<&Token> {
        self.tokens.peek()
    }
    ///returns true if token was consumed
    fn consume_if(&mut self, c: impl FnOnce(&Token) -> bool) -> bool {
        self.tokens.next_if(c).is_some()
    }

    fn next_if_name(&mut self) -> Result<String, AstError> {
        let token = self.next()?;
        if let Name(name) = token {
            Ok(name)
        } else {
            Err(AstError::unexpeced_token("Name", token))
        }
    }
    fn next(&mut self) -> Result<Token, AstError> {
        self.tokens.next().ok_or(AstError::NoNextToken)
    }
    fn parse_class(&mut self) -> Result<Node, AstError> {
        self.consume_if(|t| t == &Class);
        let class_name = self.next_if_name()?;

        if self.peek() != Some(&LeftBrace) {
            error!("next token was not left brace");
            return Err(AstError::unexpeced_token("{", self.next()?));
        }

        let body = self.parse_sequence_of_exprs(LeftBrace, RightBrace, SemiColon)?;

        Ok(Node::ClassDeclare {
            name: class_name,
            body,
        })
    }
    fn parse_var_declare(&mut self) -> Result<Node, AstError> {
        self.consume_if(|t| t == &Let);

        let mut attrs = VariableAttributes::default();
        if self.consume_if(|t| t == &Static) {
            attrs.is_static = true;
        }

        let name = self.next_if_name()?;
        if !self.consume_if(|t| t == &Equals) {
            error!("not equals after var name");
            return Err(AstError::unexpeced_token("=", self.next()?));
        }
        let value = self.parse_next_expr(&SemiColon)?;

        Ok(Node::VarDeclare {
            attributes: attrs,
            name,
            value: Box::new(value),
        })
    }
    fn parse_expr<'expr, E>(&mut self, token: Token, end_token: &'expr E) -> Result<Node, AstError>
    where
        E: Into<Option<Token>>,
        Option<&'expr Token>: From<&'expr E>,
    {
        let node: Result<Node, AstError> = match token {
            NumberLiteral(_) | Name(_) | StringLiteral(_) | LeftParen | Minus | BoolLiteral(_) => {
                pratt_parser::parse_expression(self, 0, token, end_token.into())
            }
            Return => {
                let return_value = self.parse_next_expr::<Option<Token>>(&None).map(Box::new);
                match return_value {
                    Ok(s) => Ok(Node::Return(Some(s))),
                    Err(e) => Err(e),
                }
            }
            Import => self.parse_import(),
            If => self.parse_if_statement(),
            LeftBracket => self.parse_array(),
            Let => self.parse_var_declare(),
            Fn => self.parse_function(),
            LeftBrace | RightBrace | RightParen | SemiColon | Comma | Colon | WhiteSpace
            | Equals | EOF | Class | Plus | Asterisk | RightBracket | Division | Dot | IsEq
            | Static | Null => {
                error!("unexpected token while parsing expresion: {:?}", token);
                Err(AstError::unexpeced_token(
                    "Number,name,string,bool,return,import,if,let,fn,-,[,(,",
                    token,
                ))
            }
        };
        let end_token: Option<&Token> = end_token.into();
        if let Some(end_token) = end_token
            && self.peek() == Some(end_token)
        {
            self.next()?;
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
            Err(AstError::InvalidExprEnd)
        }
    }

    fn parse_sequence_of_exprs(
        &mut self,
        start: Token,
        end: Token,
        sep: Token,
    ) -> Result<Vec<Node>, AstError> {
        let mut args = Vec::new();
        if !self.consume_if(|t| t == &start) {
            warn!("List does not start with token: {start:?}");
        }
        while let Ok(token) = self.next() {
            debug!("token = {token:?}, end token = {end:?}");
            if token == end {
                break;
            }

            let expr = self.parse_expr(token, &None)?;
            args.push(expr);

            if self.consume_if(|t| t == &sep) {
                continue;
            }
        }

        Ok(args)
    }

    fn parse_next_expr<'expr, E>(&mut self, end: &'expr E) -> Result<Node, AstError>
    where
        E: Into<Option<Token>>,
        Option<&'expr Token>: From<&'expr E>,
    {
        let next = self.next()?;
        self.parse_expr(next, end)
    }

    fn parse_args(&mut self) -> Result<Vec<Node>, AstError> {
        self.parse_sequence_of_exprs(LeftParen, RightParen, Comma)
    }
    fn parse_if_statement(&mut self) -> Result<Node, AstError> {
        self.consume_if(|t| t == &If);
        let con = self.parse_next_expr(&LeftBrace)?;
        let body = self.parse_scope()?;
        Ok(Node::If {
            condition: Box::new(con),
            body,
        })
    }
    fn parse_array(&mut self) -> Result<Node, AstError> {
        Ok(Node::ArrayLiteral(self.parse_sequence_of_exprs(
            LeftBracket,
            RightBracket,
            Comma,
        )?))
    }
    fn parse_import(&mut self) -> Result<Node, AstError> {
        let name = self.next_if_name()?;
        Ok(Node::Import(name))
    }

    fn parse_function(&mut self) -> Result<Node, AstError> {
        self.consume_if(|t| t == &Fn);
        let name = self.next_if_name()?;
        if !self.consume_if(|t| t == &LeftParen) {
            error!("no left paren after function name");
            return Err(AstError::unexpeced_token("(", self.next()?));
        }
        let mut params = Vec::new();

        while let Ok(token) = self.next() {
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

        Ok(Node::FnDeclare {
            name,
            parameters: params,
            body: self.parse_scope()?,
        })
    }
    fn parse_scope(&mut self) -> Result<Vec<Node>, AstError> {
        self.parse_sequence_of_exprs(LeftBrace, RightBrace, SemiColon)
    }
    fn parse_name(&mut self, name: String) -> Result<Node, AstError> {
        let peeked = self.peek();
        match peeked {
            Some(LeftParen) => {
                let args = self.parse_args()?;
                let call = Node::FnCall {
                    name,
                    arguments: args,
                };
                if self.peek() == Some(&Dot) {
                    self.next()?;
                    let name = self.next_if_name()?;
                    let node = self.parse_name(name)?;
                    Ok(Node::FnCallIntoOther {
                        func: Box::new(call),
                        other: Box::new(node),
                    })
                } else {
                    Ok(call)
                }
            }
            Some(Equals) => {
                self.next()?;
                Ok(Node::VarReasign {
                    name,
                    new_value: Box::new(self.parse_next_expr(&None)?),
                })
            }
            //this branch is for assigning instance vars
            Some(Colon) => {
                self.next()?;

                Ok(Node::VarReasign {
                    name,
                    new_value: Box::new(self.parse_next_expr(&None)?),
                })
            }
            Some(Dot) => {
                self.next()?;
                let accessed_name = self.next_if_name()?;
                let peeked = self.peek();
                match peeked {
                    Some(LeftParen) => {
                        let args = self.parse_args()?;
                        Ok(Node::MethodCall {
                            var_name: name,
                            method_name: accessed_name,
                            args,
                        })
                    }
                    Some(Equals) => {
                        self.next()?;
                        Ok(Node::FieldReasign {
                            var_name: name,
                            field_name: accessed_name,
                            new_value: Box::new(self.parse_next_expr(&None)?),
                        })
                    }
                    _ => Ok(Node::FieldAccess {
                        var_name: name,
                        field_name: accessed_name,
                    }),
                }
            }
            _ => Ok(Node::VarRef(name)),
        }
    }

    pub fn build(mut self) -> Result<Vec<Node>, AstError> {
        while let Some(token) = self.peek() {
            let node = match token {
                Class => self.parse_class(),

                Let => self.parse_var_declare(),

                Fn => self.parse_function(),

                EOF => break,
                If => self.parse_if_statement(),

                _ => self.parse_next_expr(&SemiColon),
            };
            if let Ok(node) = node {
                self.tree.push(node);
            } else {
                error!("Some error occured");

                return Err(node.expect_err("checked if Ok"));
            }

            debug!("tokens: {:?}", self.tokens);
            debug!("ast: {:?}", self.tree);
        }

        Ok(self.tree)
    }
}
#[derive(Debug)]
pub enum AstError {
    InvalidExprEnd,
    UnexpectedToken { expected: String, found: Token },
    NoNextToken,
    SyntaxError { _kind: SyntaxError },
}
impl AstError {
    fn unexpeced_token(expected: &str, found: Token) -> Self {
        Self::UnexpectedToken {
            expected: expected.to_string(),
            found,
        }
    }
}

#[derive(Debug)]
pub enum SyntaxError {
    // NoName,
    InvalidFloat,
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub enum Node {
    ClassDeclare {
        name: String,
        body: Vec<Node>,
    },
    VarDeclare {
        attributes: VariableAttributes,
        name: String,
        value: Box<Node>,
    },
    StringLiteral(String),
    NumberLiteral(i64),
    BoolLiteral(bool),
    ArrayLiteral(Vec<Node>),
    ///name of var
    VarRef(String),
    FnCall {
        name: String,
        arguments: Vec<Node>,
    },
    FnCallIntoOther {
        func: Box<Node>,
        other: Box<Node>,
    },
    VarReasign {
        name: String,
        new_value: Box<Node>,
    },

    FnDeclare {
        name: String,
        parameters: Vec<String>,
        body: Vec<Node>,
    },
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
    Import(String),
    If {
        condition: Box<Node>,
        body: Vec<Node>,
    },
    IsEqual {
        left: Box<Node>,
        right: Box<Node>,
    },
    FloatLiteral(f64),
    Null,
}
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub struct VariableAttributes {
    is_static: bool,
}
impl VariableAttributes {
    pub fn attr_static() -> Self {
        Self {
            is_static: true,
            ..Default::default()
        }
    }
    pub fn is_static(&self) -> bool {
        self.is_static
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq))]
pub enum MathSign {
    Plus,
    Minus,
    Multiply,
    Division,
}
///based on https://github.com/jdvillal/parser
mod pratt_parser {
    use log::{debug, error};

    use super::Node;
    use crate::{
        ast::{AstBuilder, AstError, MathSign},
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
    ) -> Result<Node, AstError> {
        let mut lhs = match start {
            Token::Name(it) => lexer.parse_name(it)?,
            Token::LeftParen => {
                let next = lexer.next()?;
                let lhs = parse_expression(lexer, 0, next, end_token);
                if !lexer.consume_if(|t| t == &Token::RightParen) {
                    error!("no closing paran");
                    return Err(AstError::unexpeced_token(")", lexer.next()?));
                }
                lhs?
            }
            // Token::RightParen => {
            //     error!("start was a )");

            //     return Err(AstError::unexpeced_token("", RightParen));
            // }
            Token::Null => Node::Null,
            Token::NumberLiteral(num) => {
                if let Some(&Token::Dot) = lexer.peek() {
                    lexer.next()?;
                    if let Some(peek) = lexer.peek()
                        && let Token::NumberLiteral(frac) = *peek
                    {
                        lexer.next()?;
                        //converts to f64
                        Node::FloatLiteral(format!("{num}.{frac}").parse::<f64>().map_err(
                            |_| AstError::SyntaxError {
                                _kind: super::SyntaxError::InvalidFloat,
                            },
                        )?)
                    } else {
                        return Err(AstError::InvalidExprEnd);
                    }
                } else {
                    Node::NumberLiteral(num)
                }
            }
            Token::BoolLiteral(b) => Node::BoolLiteral(b),
            Token::StringLiteral(string) => Node::StringLiteral(string),
            Token::Minus => match lexer.peek() {
                Some(Token::NumberLiteral(num)) => {
                    let num = -*num;
                    lexer.next()?;
                    Node::NumberLiteral(num)
                }
                _ => return Err(AstError::unexpeced_token("-", lexer.next()?)),
            },

            t => {
                error!("bad token: {:?}", t);
                return Err(AstError::unexpeced_token(
                    "Name,null,Number,bool,String,-,(",
                    t,
                ));
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
                Some(Token::IsEq) => {
                    lexer.next()?;
                    let next = lexer.next()?;
                    return Ok(Node::IsEqual {
                        left: Box::new(lhs),
                        right: Box::new(parse_expression(lexer, 0, next, end_token)?),
                    });
                }
                Some(t) => {
                    error!("unexpeced operator: {:?}", t);
                    return Err(AstError::unexpeced_token(
                        "+,-,*,/,==,),},;,','",
                        lexer.next()?,
                    ));
                    // break;
                }
            };
            let (l_bp, r_bp) = infix_binding_power(&op);
            if l_bp < min_bp {
                break;
            }

            lexer.next()?;
            let next = lexer.next()?;
            let rhs = parse_expression(lexer, r_bp, next, end_token)?;
            lhs = super::Node::Math {
                left: Box::new(lhs),
                op,
                right: Box::new(rhs),
            };
        }
        Ok(lhs)
    }
}
#[cfg(test)]
#[path = "tests/ast.rs"]
mod test;
