// Compare what AST the SQLite parser produces

use turso_parser::parser::Parser;

fn main() {
    // Test what SQLite parser produces for SELECT id, name FROM users
    let sql = "SELECT id, name FROM users";

    println!("SQLite parser for: {}", sql);

    let mut parser = Parser::new(sql.as_bytes());
    match parser.parse_cmd() {
        Ok(cmd) => {
            println!("Parse successful!");
            println!("AST: {:#?}", cmd);
        }
        Err(e) => {
            println!("Parse error: {:?}", e);
        }
    }

    println!("\n---\n");

    // Also test SELECT *
    let sql2 = "SELECT * FROM users";
    println!("SQLite parser for: {}", sql2);

    let mut parser = Parser::new(sql2.as_bytes());
    match parser.parse_cmd() {
        Ok(cmd) => {
            println!("Parse successful!");
            println!("AST: {:#?}", cmd);
        }
        Err(e) => {
            println!("Parse error: {:?}", e);
        }
    }
}