// Simple test to debug table reading

use turso_parser_pg;

fn main() {
    // Test parsing SELECT * FROM users
    let sql = "SELECT * FROM users";

    println!("Parsing: {}", sql);

    match turso_parser_pg::parse(sql) {
        Ok(result) => {
            println!("Parse successful!");

            // Try to translate
            let translator = turso_parser_pg::translator::PostgreSQLTranslator::new();
            match translator.translate(&result) {
                Ok(ast) => {
                    println!("Translation successful!");
                    println!("AST: {:#?}", ast);
                }
                Err(e) => {
                    println!("Translation error: {:?}", e);
                }
            }
        }
        Err(e) => {
            println!("Parse error: {:?}", e);
        }
    }

    // Also test SELECT with columns
    println!("\n---\n");
    let sql2 = "SELECT id, name FROM users";
    println!("Parsing: {}", sql2);

    match turso_parser_pg::parse(sql2) {
        Ok(result) => {
            println!("Parse successful!");

            let translator = turso_parser_pg::translator::PostgreSQLTranslator::new();
            match translator.translate(&result) {
                Ok(ast) => {
                    println!("Translation successful!");
                    println!("AST: {:#?}", ast);
                }
                Err(e) => {
                    println!("Translation error: {:?}", e);
                }
            }
        }
        Err(e) => {
            println!("Parse error: {:?}", e);
        }
    }
}