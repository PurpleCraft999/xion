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
        if !self.consume_if(|t| matches!(t, LeftBrace)) {
            error!("next token was not left brace")
        }

        let fields = self.parse_list(LeftBrace, RightBrace)?;

        Some(Node::Class(ClassAst {
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
        let value = self.parse_expr(next,&SemiColon)?;


        Some(Node::Var(Box::new(VarAst { name, value })))
    }
    fn parse_expr<'expr,E>(&mut self, token: Token,end_token:&'expr E) -> Option<Node> where E:Into<Option<Token>>,Option<&'expr Token>:From<&'expr E>{
        let node  = match token {
            StringLiteral(name) => {
                //strings only support addition
                if self.peek() == Some(&Plus) {
                    self.tree.push(Node::StringLiteral(name));
                    let n = self.next().expect("we just peeked ahead and saw a value");
                    self.parse_expr(n,end_token)
                } else {
                    Some(Node::StringLiteral(name))
                }
            }
            Name(name) => self.parse_name(name),
            NumberLiteral(_) => self.parse_math_equasion(token),
            Return => {
                let token = self.next();
                let return_value = if let Some(value) = token {
                    self.parse_expr(value,end_token).map(Box::new)
                } else {
                    None
                };


                Some(Node::Return(return_value))
            }
            Minus => match self.peek() {
                Some(NumberLiteral(num)) => {
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
            Fn | LeftBrace | RightBrace | LeftParen | RightParen | SemiColon | Comma | Colon
            | WhiteSpace | Equals | EOF | Class | Plus | Asterisk | RightBracket => {
                error!("unexpected expresion while parsing expresion: {:?}", token);
                None
            }
        };
        let end_token: Option<&Token> = end_token.into();
        if let Some(end_token) = end_token && self.peek()==Some(end_token){
            self.next();
            node
        } else if end_token.is_none(){
            node
        } else{
            error!("no semicolon after expresion");
            None
        }
    }
    fn parse_list(&mut self, start: Token, end: Token) -> Option<Vec<Node>> {
        let mut args = Vec::new();
        if !self.consume_if(|t| t == &start) {
            error!("List does not start with token: {start:?}");
            return None;
        }
        let mut depth = 0u16;
        while let Some(token) = self.next() {
            debug!("depth = {depth}, token = {token:?}, end token = {end:?}");
            if token == end {
                if depth == 0 {
                    break;
                } else {
                    depth -= 1;
                }
            }
            if token == start {
                depth += 1;
            }



            if let Some(expr) = self.parse_expr(token,&None) {
                args.push(expr);
            } else {
                error!("list parse error");
                return None;
            }
            if self.consume_if(|t| t == &Comma) {
                continue;
            }
        }

        Some(args)
    }

    fn parse_args(&mut self) -> Option<Vec<Node>> {
        self.parse_list(LeftParen, RightParen)
    }
    fn parse_array(&mut self) -> Option<Node> {
        Some(Node::ArrayLiteral(
            self.parse_list(LeftBracket, RightBracket)?,
        ))
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
    fn parse_scope(&mut self) -> Vec<Node>{
        // self.parse_list(LeftBrace,RightBrace)
        if !self.consume_if(|t| t == &LeftBrace) {
            warn!("not left brace to start scope");
        }

        let mut scope = Vec::new();
        while let Some(token) = self.next() {
            if token == RightBrace {
                break;
            }
            if let Some(var) = self.parse_expr(token,&SemiColon) {
                scope.push(var);
            }
        }

        scope
    }
    fn parse_name(&mut self, name: String) -> Option<Node> {
        if self.peek() == Some(&LeftParen) {
            let args = self.parse_args()?;

            Some(Node::FnCall(FnCallAst { name, args }))
        } else if self.peek() == Some(&Equals) {
            self.next();
            let new_value = self.next()?;
            Some(Node::VarReasign {
                name,
                new_value: Box::new(self.parse_expr(new_value,&SemiColon)?),
            })
        } else if let Some(math) =  self.peek() && (math.is_addition_or_subtraction()||math.is_multiplication_or_division()) {
            self.parse_math_equasion(Token::Name(name))

        } else{
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
                | RightBracket |True|False => {
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

                EOF => break,
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
    pub name: String,
    pub fields: Vec<Node>,
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
