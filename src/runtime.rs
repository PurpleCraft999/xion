use std::collections::HashMap;

use crate::ast::Node;
use crate::ast::Node::*;
use crate::xion_std;

#[derive(Debug, Clone)]
pub struct Runtime {
    nodes: Vec<Node>,
    scope: Scope,
    return_value: Option<Value>,
}
impl Runtime {
    pub fn start(nodes: Vec<Node>) -> Self {
        let mut scope = Scope::new();
        scope.attach_std_lib();



        Self::with_scope_and_nodes(nodes, scope)
    }
    pub fn with_scope_and_nodes(nodes: Vec<Node>,scope:Scope)->Self{
        Self{
            nodes,
            scope,
            return_value:None,
        }
    }



    fn get_current_scope_mut(&mut self) -> &mut Scope {
        &mut self.scope
    }
    fn get_current_scope(&self) -> &Scope {
        &self.scope
    }

    fn eval(&mut self, node: Node,errors:&mut ErrorLog) -> Option<Value> {
        match node {
            Var(var_ast) => {
                if let Some(value) = self.eval(var_ast.value,errors) {
                    let var = Variable {
                        name: var_ast.name.clone(),
                        value,
                    };
                     if let Err(_) = self.get_current_scope_mut().add_var(var){
                        errors.error_string(format!("variable {} already exists cannot crease",var_ast.name));
                     }
                } else {
                    errors.error_string(format!("cant parse value of var {}", var_ast.name));
                }

                None
            }
            StringLiteral(str) => Some(Value::String(str)),
            VarRef(var) => self.get_current_scope().get_var_value(&var).cloned(),
            NumberLiteral(num) => Some(Value::Number(num)),
            Class(_class) => {
                println!("classes not implemented");
                None
            }
            FnDeclare(func) => {
                let body = self.child_runtime(func.body);
                if let Err(_) = self.get_current_scope_mut().add_func(NonNativeFunction {
                    name:func.name.clone(),
                    params: func.paramaters,
                    body: body,
                }){
                    errors.error_string(format!("function {} already exists cannot create",func.name));



                }

                None
            }
            FnCall(func_ast) => {
                let func = self
                    .get_current_scope()
                    .get_function(&func_ast.name)
                    .cloned()?;

                // if let Some(func) = self
                //     .get_current_scope()
                //     .get_function(&func_ast.name)
                //     .cloned()
                // {
                let mut values = Vec::new();
                for node in func_ast.args {
                    if let Some(node) = self.eval(node,errors) {
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
            Return(value) => {
                let value = if let Some(value) = value {
                    self.eval(*value,errors)
                } else {
                    None
                };
                self.return_value = value;

                None
            }
        }
    }

    fn child_runtime(&self, nodes: Vec<Node>) -> Runtime {
        let scope = Scope::with_parent(self.get_current_scope().clone());
        Self::with_scope_and_nodes(nodes, scope)
    }

    pub fn run(mut self) -> Option<Value> {
        let nodes = std::mem::take(&mut self.nodes);
        let mut error_handler = ErrorLog::new();
        for node in nodes {
            self.eval(node,&mut error_handler);
            if error_handler.has_new_error(){
                println!("Error: {}",error_handler.get_last())
            }
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

#[derive(Debug, Clone)]
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
    fn attach_std_lib(&mut self){

        self.add_native_fn("print", xion_std::print);






    }
    fn add_native_fn(&mut self,name: &str,func:NativeFunctionHeader){

        self.functions.insert(name.to_owned(), Function::Native(NativeFunction { func }));


    }






    fn with_parent(scope: Scope) -> Self {
        Self {
            parent_scope: Some(Box::new(scope)),
            vars: HashMap::new(),
            functions: HashMap::new(),
        }
    }

    fn add_var(&mut self, var: Variable)->Result<(),AlreadyExists> {
        if self.vars.contains_key(&var.name){

            return Err(AlreadyExists);
        }



        self.vars.insert(var.name.clone(), var);
        Ok(())
    }
    fn add_func(&mut self, func: NonNativeFunction) ->Result<(), AlreadyExists>{
        if self.functions.contains_key(&func.name){

            return Err(AlreadyExists);
        }


        self.functions.insert(func.name.clone(), Function::NonNative(func));
        Ok(())
    }

    pub fn get_var(&self, name: &str) -> Option<&Variable> {
        self.vars.get(name).or_else(|| match &self.parent_scope {
            Some(scope) => scope.get_var(name),
            None => None,
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

#[derive(Debug)]
struct AlreadyExists;


struct ErrorLog{
    errors:Vec<String>,
    changed:bool
}
impl ErrorLog{
    fn new()->Self{
        Self { errors: Vec::new(),changed:false }
    }
    // fn error(&mut self,error:&str){
    //     self.errors.push(error.to_owned());
    //     self.changed=true
    // }
    fn error_string(&mut self,error:String){
        self.errors.push(error);
        self.changed=true
    }
    fn has_new_error(&mut self)->bool{
        if self.changed{

            self.changed=false;
            true
        } else{
            false
        }
    }
    fn get_last(&self)->&String{
        self.errors.last().unwrap()
    }




}



#[derive(Debug,Clone)]
pub enum Function{
    NonNative(NonNativeFunction),
    Native(NativeFunction),
}
impl Function{
    fn call(&self,args:Vec<Value>)->Option<Value>{
        match self{
            Self::NonNative(f)=>f.call(args),
            Self::Native(f)=>(f.func)(args)
        }


    }



}




#[derive(Debug, Clone)]
pub struct Variable {
    name: String,
    value: Value,
}


pub type NativeFunctionHeader = fn(Vec<Value>)->Option<Value>;

#[derive(Debug,Clone)]
pub struct NativeFunction{
    func:NativeFunctionHeader
}





#[derive(Debug, Clone)]
pub struct NonNativeFunction {
    name: String,
    params: Vec<String>,
    body: Runtime,
}
impl NonNativeFunction {
    fn call(&self, args: Vec<Value>) -> Option<Value> {
        let mut runtime = self.body.clone();
        for (name, value) in self.params.iter().zip(args.into_iter()) {
            runtime.scope.add_var(Variable {
                name: name.clone(),
                value,
            }).expect("this is safe because these are the first vars made");
        }
        runtime.run()
    }
}
