pub mod ast;
pub mod functions;
pub mod runtime;
pub mod token;
mod xion_std;

#[cfg(test)]
mod test_log {
    use std::sync::Once;

    static LOGGER: Once = Once::new();

    pub fn init_logger() {
        LOGGER.call_once(|| {
            simple_logger::SimpleLogger::new()
                .init()
                .expect("this can only be called once")
        });
    }
}
