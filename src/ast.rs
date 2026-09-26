use std::{iter::Peekable};

use crate::{
    ast::Node::Var,
    token::Token::{self, *},
};

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

        while let Some(token) = self.tokens.next() {
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
        // self.consume_whitespace();
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
        Some(Node::Class(ClassAst { name: class_name }))
    }
    fn parse_var(&mut self) -> Option<Node> {
        // self.consume_whitespace();
        let Some(name) = self.next_if_name() else {
            self.error("no name after let");
            return None;
        };
        // self.consume_whitespace();
        if !self.consume_if(|t| t == &Equals) {
            self.error("not equals after var name");
            return None;
        }

        let value = self.parse_expr()?;

        if !self.consume_if(|t|t==&SemiColon){
            self.error("no semicolon after var");
            return None;
        }

        Some(Var(Box::new(VarAst {
            name,
            value,
        })))
    }
    fn parse_expr(&mut self) -> Option<Node> {
        // self.consume_whitespace();

        let node = match self.next()? {
            StringLiteral(name) => Node::StringLiteral(name),
            Name(name) => {
                if self.peek() == Some(&LeftParen) {
                    Node::FnRef(FnRefAst {
                        name,
                        args: self.parse_args(),
                    })
                } else {
                    Node::VarRef(name)
                }
            }
            _ => {
                self.error("not expresion");
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

            if let Some(expr) = self.parse_expr() {
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

    fn parse_function(&mut self)->Option<Node>{

        let name= self.next_if_name()?;
        if !self.consume_if(|t|t==&LeftParen){
            self.error("no left paren after function name");
            return None;
        }
        let mut params = Vec::new();

        while let Some(token) = self.next(){
            if token==RightParen{
                break;
            }

            if token==Comma{
                continue;
            }
            if let Name(param_name) = token {
                params.push(param_name);
                continue;
            }
            self.error(&format!("unexpected token {:?} while parsing fn header",token));
            break;
        }

        // if !self.consume_if(|t|t==&LeftBrace){
        //     self.error("not left brace after function header");
        //     return None
        // }
        



        Some(Node::Fn(FunctionDefAst { name, paramaters: params,body:self.parse_scope() }))
        

    }
    fn parse_scope(&mut self)->Vec<Node>{
        if !self.consume_if(|t|t==&LeftBrace){
            self.error("not left brace to start scope");
            return Vec::new();
        }

        let mut scope = Vec::new();
        while let Some(token) = self.next(){
            if token==RightBrace{
                break;
            }



            if let Some(var) = self.parse_var(){
                scope.push(var);
            }
            





        }

        scope


    }



    pub fn build(mut self)->Vec<Node> {
        while let Some(token) = self.next() {
            let node = match token {
                Class => self.parse_class(),
                Name(_) => {
                    self.error("unexpeced name");
                    None
                }
                //never meant to be read here
                //whitespace cant be in at this point
                WhiteSpace | Colon | SemiColon | Comma | LeftBrace | RightBrace | Equals
                | LeftParen | RightParen => {
                    self.error("unexpected lang syntax");
                    None
                }
                Let => self.parse_var(),
                StringLiteral(_) => {
                    self.error("string lit in main ast branch");

                    None
                }
                Fn=>self.parse_function(),
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
#[derive(Debug)]
pub enum Node {
    Class(ClassAst),
    Var(Box<VarAst>),
    StringLiteral(String),
    ///name of var
    VarRef(String),
    FnRef(FnRefAst),
    Fn(FunctionDefAst),
}
#[derive(Debug)]
pub struct ClassAst {
    name: String,
}

#[derive(Debug)]
pub struct VarAst {
    name: String,
    value: Node,
}
#[derive(Debug)]
pub struct FnRefAst {
    name: String,
    args: Vec<Node>,
}


#[derive(Debug)]
pub struct FunctionDefAst{
    name:String,
    paramaters:Vec<String>,
    body:Vec<Node>,
}