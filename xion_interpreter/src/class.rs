use crate::runtime::ParentScope;
use crate::{
    Value,
    functions::Function,
    runtime::{RuntimeError, RuntimeReturn, Scope, Variable},
};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct RuntimeClass {
    pub(crate) name: String,
    pub(crate) fields: HashMap<String, Variable>,
    pub(crate) methods: HashMap<String, Function>,
}
impl RuntimeClass {
    pub fn new(
        name: String,
        fields: HashMap<String, Variable>,
        methods: HashMap<String, Function>,
    ) -> Self {
        Self {
            name,
            fields,
            methods,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn instantiate(self: &Arc<Self>) -> ClassInstance {
        ClassInstance {
            class: Arc::clone(self),
            fields: self.fields.clone(),
        }
    }
}
impl PartialEq for RuntimeClass {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

#[derive(Debug, Clone)]
pub struct ClassInstance {
    class: Arc<RuntimeClass>,
    fields: HashMap<String, Variable>,
}
impl ClassInstance {
    pub fn call_method(&mut self, name: &str, args: Vec<Value>) -> RuntimeReturn {
        let Some(method) = self.class.methods.get(name) else {
            return Err(RuntimeError::DoesNotExist(name.to_owned()));
        };
        let mut scope = Scope::new();
        scope.vars = std::mem::take(&mut self.fields);
        let scope = Arc::new(Mutex::new(scope));

        let scope = Scope::with_mutable_parent(scope);
        let result = method.call(args, scope.clone());
        match scope.parent_scope {
            Some(p) => match p {
                ParentScope::Mut(m) => {
                    self.fields = {
                        let loc = m.lock().expect("we are already panicing");
                        loc.vars.clone()
                    }
                }
                ParentScope::Normal(_) => panic!("parent was set to mut"),
            },
            None => panic!("we set a parent"),
        };
        result
    }
    pub fn get_field(&self, name: &str) -> Option<&Variable> {
        self.fields.get(name)
    }
    pub fn get_field_mut(&mut self, name: &str) -> Option<&mut Variable> {
        self.fields.get_mut(name)
    }
    pub fn set_fields(&mut self, new: HashMap<String, Variable>) {
        self.fields = new;
    }
}

impl PartialEq for ClassInstance {
    fn eq(&self, other: &Self) -> bool {
        self.class == other.class && map_equal(&self.fields, &other.fields)
    }
}

impl std::fmt::Display for ClassInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut fields = String::new();
        for (name, var) in &self.fields {
            fields += name;
            fields.push(':');
            fields += &var.get().to_string();
            fields.push(',');
        }
        fields.pop();

        write!(f, "{}Instance {{ fields:[{fields}] }}", self.class.name)
    }
}

fn map_equal<K: Eq + Hash, V: PartialEq>(one: &HashMap<K, V>, two: &HashMap<K, V>) -> bool {
    one.iter()
        .all(|(k, v)| two.get(k).filter(|v2| *v2 == v).is_some())
}

pub trait NativeClass {}
