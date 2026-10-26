pub mod ast;
pub mod class;
pub mod functions;
pub mod runtime;
pub mod token;
mod xion_std;
pub use runtime::Value;

use crate::ast::Ast;
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

pub fn lex_and_build_ast(code: &str) -> Result<Ast, LexAndAstError> {
    #[cfg(test)]
    {
        test_log::init_logger();
    }

    let lexer = token::Lexer::new(code);
    let lexed = lexer.lex().map_err(LexAndAstError::Parse)?;
    log::debug!("lexed: {:?}", lexed);
    let ast = ast::AstBuilder::new(lexed);
    ast.build().map_err(LexAndAstError::Ast)
}
#[derive(Debug)]
pub enum LexAndAstError {
    Parse(token::LexerError),
    Ast(ast::AstError),
}
