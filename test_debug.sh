#\!/bin/bash
export RUST_LOG=warn
./target/debug/tursodb :memory: << 'SQL' 2>&1 | grep -E "DELETE:|INSERT:"
CREATE TABLE tasks (id INTEGER PRIMARY KEY, task TEXT, status TEXT);
INSERT INTO tasks VALUES (1, 'TaskA', 'pending');
CREATE VIEW pending_tasks AS SELECT * FROM tasks WHERE status = 'pending';
SELECT * FROM pending_tasks;
UPDATE tasks SET status = 'completed' WHERE id = 1;
SELECT * FROM pending_tasks;
.exit
SQL
