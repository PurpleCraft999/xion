use crate::Value;
use crate::functions::NativeFunction;
use crate::runtime::FromValue;
use crate::runtime::RuntimeError;
use std::time::{Duration, SystemTime};
use xion_interpreter_proc_macros::native_function;
#[native_function]
fn print(values: Vec<Value>) {
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
}
#[native_function]
fn input() -> String {
    let mut out = String::new();
    match std::io::stdin().read_line(&mut out) {
        Ok(_) => (),
        Err(err) => println!("Error reading input steam {err}"),
    }
    out.trim().to_string()
}
#[native_function]
fn to_str(input: Value) -> String {
    input.to_string()
}

///returns -1.0 if system time is before unix_epoch
/// returns current time in seconds otherwise
#[native_function]
fn time() -> f64 {
    match std::time::SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Ok(t) => t.as_secs_f64(),
        Err(s) => {
            error!(
                "system time is behind the unix epoch by {} seconds",
                s.duration().as_secs()
            );

            -1f64
        }
    }
}
/// returns 0 if slept
///
/// returns 1 if type was invalid
///
/// returns 2 if int was negative
///
/// sleeps for said amount of seconds
#[native_function]
fn sleep(input: Value) -> i64 {
    match input {
        Value::Number(i) => {
            if i < 0 {
                return 2;
            }

            std::thread::sleep(Duration::from_secs(i as u64));

            0
        }
        Value::Float(f) => {
            if f < 0.0 {
                return 2;
            }

            std::thread::sleep(Duration::from_secs_f64(f));

            0
        }
        _ => 1,
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
            scope.add_native_fn("print", PrintFn)?;
            scope.add_native_fn("str",ToStrFn)?;




            Ok(())
            },
            "io"=>|scope|{


                scope.add_native_fn("input", InputFn)?;

                Ok(())
            },
            "time"=>|scope|{
                scope.add_native_fn("time", TimeFn)?;
                scope.add_native_fn("sleep",SleepFn)?;
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
