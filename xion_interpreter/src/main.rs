use std::path::PathBuf;

use log::{debug, error};
use xion_interpreter::runtime::Runtime;

fn main() {
    log::set_max_level(log::LevelFilter::Debug);
    log::set_boxed_logger(Box::new(simple_logger::SimpleLogger::new()))
        .expect("only returns error when the logger is already set");

    let main_file;

    #[cfg(debug_assertions)]
    {
        main_file = "xion_interpreter/test_programs/class.xn".to_string();
    }

    #[cfg(not(debug_assertions))]
    {
        let args: Vec<_> = std::env::args().collect();
        main_file = match args.get(1) {
            Some(s) => s.to_string(),
            None => {
                println!("please pass the main file");
                return;
            }
        };
    }
    let path = PathBuf::from(main_file);
    let nodes = xion_interpreter::parse_and_lex(&path);
    debug!("running main");

    let mut runtime = match Runtime::main(nodes) {
        Ok(r) => r,
        Err(e) => {
            error!(target:"main", "{e}");
            return;
        }
    };

    debug!("{runtime:?}");

    match runtime.run() {
        Ok(_) => (),
        Err(e) => log::error!(target:"main", "{e}"),
    }
}
