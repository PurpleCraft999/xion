use log::debug;
use xion_interpreter::runtime::Runtime;

fn main() {
    log::set_max_level(log::LevelFilter::Info);
    log::set_boxed_logger(Box::new(simple_logger::SimpleLogger::new()))
        .expect("only returns error when the logger is already set");

    let nodes = xion_interpreter::parse_and_lex("xion_interpreter/test_programs/mod.xn");
    debug!("running main");
    match Runtime::main(nodes).run(){
        Ok(_)=>(),
        Err(e)=>log::error!("{e}")
    }
}
