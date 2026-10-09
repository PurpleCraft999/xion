use std::path::Path;

pub mod ast;
pub mod class;
pub mod functions;
pub mod runtime;
pub mod token;
mod xion_std;
pub use runtime::Value;
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
pub fn parse_and_lex_file(path: impl AsRef<Path>) -> Vec<ast::Node> {
    let file = std::fs::read_to_string(path).unwrap();
    parse_and_lex(&file)
}

pub fn parse_and_lex(code: &str) -> Vec<ast::Node> {
    let lexer = token::Lexer::new(code);
    let mut lexed = lexer.lex();
    lexed.retain(|s| s != &crate::token::Token::WhiteSpace);
    log::debug!("lexed: {:?}", lexed);
    let ast = ast::AstBuilder::new(lexed);
    ast.build()
}
