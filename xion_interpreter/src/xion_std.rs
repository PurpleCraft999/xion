use crate::Value;
type Input = Vec<Value>;
type Output = Option<Value>;

fn print(values: Vec<Value>) -> Option<Value> {
    if values.len() == 1 {
        println!("{}", values[0])
    } else if values.is_empty() {
        println!();
    } else {
        for (i, value) in values.iter().enumerate() {
            print!("{value}");
            if i < values.len() - 1 {
                print!(",")
            }
        }
        println!()
    }
    None
}

fn input(_: Vec<Value>) -> Option<Value> {
    let mut out = String::new();
    match std::io::stdin().read_line(&mut out) {
        Ok(_) => (),
        Err(err) => println!("Error reading input steam {err}"),
    }
    Some(Value::String(out))
}

fn to_str(input: Input) -> Output {
    input.first().map(|v| Value::String(v.to_string()))
}

pub fn instantiate(_: Input) -> Output {
    log::error!("instantiate was called directly");
    None
}

pub use lib::get_std_lib;

mod lib {
    use super::*;
    use crate::runtime::RuntimeError;
    use crate::runtime::Scope;
    use std::{collections::HashMap, sync::OnceLock};
    static DEFAULT_LIBS: OnceLock<HashMap<String, Scope>> = OnceLock::new();

    macro_rules! lib {
        ($map:expr, $($name:expr=>$block:expr),+ $(,)?) => {
            $(
                let mut scope = crate::runtime::Scope::new();
                // $block(&mut scope).expect("no names are the same in the same module");
                let block:fn(&mut Scope)-> Result<(), RuntimeError> = $block;
                block(&mut scope).expect("all names are unique");
                $map.insert($name.to_string(),scope);


            )+
        };
    }

    fn build_default_libs() -> HashMap<String, crate::runtime::Scope> {
        let mut map = HashMap::new();
        lib! {map,"lang"=>|scope|{
            scope.add_native_fn("print", print)?;
            scope.add_native_fn("str",to_str)?;
            Ok(())
            },
            "io"=>|scope|{

                scope.add_native_fn("input", input)?;

                Ok(())
            },
        }
        map
    }

    fn get_libs() -> &'static HashMap<String, Scope> {
        DEFAULT_LIBS.get_or_init(build_default_libs)
    }

    pub fn get_std_lib(name: &str) -> Option<Scope> {
        get_libs().get(name).cloned()
    }
}
