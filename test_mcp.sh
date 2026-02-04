#!/bin/bash

# Test script for MCP server
DB_FILE=${1:-$(mktemp /tmp/test_mcp.XXXXXX.db)}

# Create a temporary database with test data if using default
if [ "$#" -eq 0 ]; then
    echo "Creating temporary database: $DB_FILE"
    echo "========================================"
    
    # Create test database with sample data
    ./target/debug/tursodb "$DB_FILE" << 'EOF'
CREATE TABLE users (
    id INTEGER,
    name TEXT,
    email TEXT
);

CREATE TABLE posts (
    id INTEGER,
    user_id INTEGER,
    title TEXT,
    content TEXT
);

INSERT INTO users (id, name, email) VALUES 
    (1, 'Alice Johnson', 'alice@example.com'),
    (2, 'Bob Smith', 'bob@example.com'),
    (3, 'Charlie Brown', 'charlie@example.com');

INSERT INTO posts (id, user_id, title, content) VALUES
    (1, 1, 'First Post', 'Hello world!'),
    (2, 1, 'Second Post', 'This is my second post.'),
    (3, 2, 'Bob''s Post', 'Bob''s first contribution.'),
    (4, 3, 'Charlie''s Thoughts', 'Some random thoughts...');

.quit
EOF
fi

echo ""
echo "Testing MCP server with database: $DB_FILE"
echo "========================================"

# Start server and send test messages
{
    echo '{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "test", "version": "1.0"}}}'
    sleep 0.1
    echo '{"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}'
    sleep 0.1
    echo '{"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "list_tables", "arguments": {}}}'
    sleep 0.1
    echo '{"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "describe_table", "arguments": {"table_name": "users"}}}'
    sleep 0.1
    echo '{"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "execute_query", "arguments": {"query": "SELECT * FROM users LIMIT 3"}}}'
    sleep 0.1
    echo '{"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "execute_query", "arguments": {"query": "SELECT u.name, p.title FROM users u JOIN posts p ON u.id = p.user_id LIMIT 5"}}}'
    sleep 0.1
    echo '{"jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {"name": "insert_data", "arguments": {"query": "INSERT INTO users (id, name, email) VALUES (4, '\''Dave Wilson'\'', '\''dave@example.com'\'')"}}}'
    sleep 0.1
    echo '{"jsonrpc": "2.0", "id": 8, "method": "tools/call", "params": {"name": "execute_query", "arguments": {"query": "SELECT * FROM users WHERE id = 4"}}}'
    sleep 0.1
    echo '{"jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": {"name": "update_data", "arguments": {"query": "UPDATE users SET email = '\''dave.wilson@example.com'\'' WHERE id = 4"}}}'
    sleep 0.1
    echo '{"jsonrpc": "2.0", "id": 10, "method": "tools/call", "params": {"name": "execute_query", "arguments": {"query": "SELECT * FROM users WHERE id = 4"}}}'
    sleep 0.1
    echo '{"jsonrpc": "2.0", "id": 11, "method": "tools/call", "params": {"name": "delete_data", "arguments": {"query": "DELETE FROM users WHERE id = 4"}}}'
    sleep 0.1
    echo '{"jsonrpc": "2.0", "id": 12, "method": "tools/call", "params": {"name": "execute_query", "arguments": {"query": "SELECT COUNT(*) as user_count FROM users"}}}'
    sleep 0.1
} | ./target/debug/tursodb "$DB_FILE" --mcp

# Clean up temporary file if we created it
if [ "$#" -eq 0 ]; then
    echo ""
    echo "Cleaning up temporary database: $DB_FILE"
    rm -f "$DB_FILE"
fi