use std::collections::HashMap;

use crate::ast::Node;
use crate::ast::Node::*;



#[derive(Debug,Clone)]
pub struct Runtime {
    nodes: Vec<Node>,
    scope: Scope,
    return_value:Option<Value>,
}
impl Runtime {
    pub fn new(nodes: Vec<Node>) -> Self {
        Self {
            nodes,
            scope: Scope::new(),
            return_value:None,
        }
    }
    fn get_current_scope_mut(&mut self) -> &mut Scope{
        &mut self.scope
    }
    fn get_current_scope(&self) -> &Scope{
        &self.scope
    }

    fn eval(&mut self, node: Node) -> Option<Value> {
        match node {
            Var(var_ast) => {
                if let Some(value) = self.eval(var_ast.value) {
                    let var = Variable {
                        name: var_ast.name,
                        value,
                    };
                    self.get_current_scope_mut().add_var(var);
                } else {
                    println!("cant parse value of var {}", var_ast.name);
                }

                None
            }
            StringLiteral(str) => Some(Value::String(str)),
            VarRef(var) => self.get_current_scope().get_var_value(&var).cloned(),
            NumberLiteral(num) => Some(Value::Number(num)),
            Class(class) => {
                println!("classes not implemented");
                None
            }
            FnDeclare(mut func) => {
                let body = self.child_runtime(func.body);
                self.get_current_scope_mut().add_func(Function {
                    name: func.name,
                    params: func.paramaters,
                    body: body,
                });

                None
            }
            FnCall(func_ast) => {
                let func = self
                    .get_current_scope()
                    .get_function(&func_ast.name).cloned()?;


                // if let Some(func) = self
                //     .get_current_scope()
                //     .get_function(&func_ast.name)
                //     .cloned()
                // {
                    let mut values = Vec::new();
                    for node in func_ast.args {
                        if let Some(node) = self.eval(node) {
                            values.push(node)
                        }
                    }
                // func.call(args)

                    func.call(values)
                // } else {
                //     println!("cannot find function {}",func_ast.name)
                //     None
                // }
// None
                
            }
            Return(value)=>{
                let value = if let Some(value) = value{self.eval(*value)} else {None};
                self.return_value=value;


                None
            }
        }
    }

    fn child_runtime(&self, nodes: Vec<Node>) -> Runtime {
        let scope = Scope::with_parent(self.get_current_scope().clone());
        Runtime { scope, nodes,return_value:None }
    }

    pub fn run(mut self) -> Option<Value>{
        let nodes = std::mem::take(&mut self.nodes);
        for node in nodes {
            self.eval(node);
            // println!("{s:?}");
        }
        println!("{self:?}");
        self.return_value
    }
}
#[derive(Debug, Clone)]
pub enum Value {
    String(String),
    Number(i64),
}

#[derive(Debug,Clone)]
pub struct Scope {
    parent_scope: Option<Box<Scope>>,
    vars: HashMap<String, Variable>,
    functions: HashMap<String, Function>,
}
impl Scope {
    fn new() -> Self {
        Self {
            parent_scope: None,
            vars: HashMap::new(),
            functions: HashMap::new(),
        }
    }
    fn with_parent(scope: Scope) -> Self {
        Self {
            parent_scope: Some(Box::new(scope)),
            vars: HashMap::new(),
            functions: HashMap::new(),
        }
    }

    fn add_var(&mut self, var: Variable) {
        self.vars.insert(var.name.clone(), var);
    }
    fn add_func(&mut self, func: Function) {
        self.functions.insert(func.name.clone(), func);
    }

    pub fn get_var(&self, name: &str) -> Option<&Variable> {
        self.vars.get(name).or_else(||match &self.parent_scope{
            Some(scope)=>scope.get_var(name),
            None=>None
        })
        
    }
    pub fn get_var_value(&self, name: &str) -> Option<&Value> {
        match self.get_var(name) {
            Some(v) => Some(&v.value),
            None => None,
        }
    }
    pub fn get_function(&self, name: &str) -> Option<&Function> {
        self.functions.get(name).or(match &self.parent_scope {
            Some(s) => s.get_function(name),
            None => None,
        })
    }

    // fn add_function
}

#[derive(Debug, Clone)]
pub struct Variable {
    name: String,
    value: Value,
}
#[derive(Debug,Clone)]
pub struct Function {
    name: String,
    params: Vec<String>,
    body: Runtime,
}
impl Function {
    fn call(&self, args: Vec<Value>)->Option<Value> {
        let mut runtime=self.body.clone();
        for (name, value) in self.params.iter().zip(args.into_iter()) {
            runtime.scope.add_var(Variable { name:name.clone(), value });
        }
        runtime.run()

    }
}
