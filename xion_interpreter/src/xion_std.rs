use std::time::{Duration, SystemTime};

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
    Some(Value::String(out.trim().to_string()))
}

fn to_str(input: Input) -> Output {
    input.first().map(|v| Value::String(v.to_string()))
}
fn time(_: Input) -> Output {
    match std::time::SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(t) => Some(Value::Float(t.as_secs_f64())),
        Err(s) => {
            error!(
                "system time is behind the unix epoch by {} seconds",
                s.duration().as_secs()
            );

            None
        }
    }
}
/// returns 0 if slept
///
/// returns 1 if type was invalid
///
/// returns 2 if int was negative
///
/// returns 3 if no value was passed
///
/// sleeps for said amount of seconds
fn sleep(input: Input) -> Output {
    if let Some(v) = input.first() {
        match v {
            Value::Number(i) => {
                if *i < 0 {
                    return Some(Value::Number(2));
                }

                std::thread::sleep(Duration::from_secs(*i as u64));

                Some(Value::Number(0))
            }
            Value::Float(f) => {
                if *f < 0.0 {
                    return Some(Value::Number(2));
                }

                std::thread::sleep(Duration::from_secs_f64(*f));

                Some(Value::Number(0))
            }
            _ => Some(Value::Number(1)),
        }
    } else {
        Some(Value::Number(3))
    }
}

pub use lib::get_std_lib;
use log::error;

mod lib {
    use super::*;
    use crate::runtime::RuntimeError;
    use crate::runtime::Scope;
    use std::{collections::HashMap, sync::OnceLock};
    static DEFAULT_LIBS: OnceLock<HashMap<String, Scope>> = OnceLock::new();

    macro_rules! lib {
        ($map:expr, $($name:expr=>$block:expr),+ $(,)?) => {
            $(
                let mut scope = crate::runtime::Scope::new(String::from("std"));
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
            "time"=>|scope|{
                scope.add_native_fn("time", time)?;
                scope.add_native_fn("sleep",sleep)?;
                Ok(())
            }
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
