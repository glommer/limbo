#!/bin/bash

echo "Testing connection benchmarks..."

# Test that symlinks work
echo "Testing symlinked scripts..."
python3 gen-database.py test_shared.db -t 5
if [ -f "test_shared.db" ]; then
    echo "✓ Shared gen-database.py script works"
    rm test_shared.db
else
    echo "✗ Shared gen-database.py script failed"
fi

# Test rusqlite benchmark
echo "Testing rusqlite benchmark..."
cd rusqlite
python3 gen-database.py test_tiny.db -t 5
if cargo build --release; then
    echo "✓ Rusqlite benchmark builds successfully"
    if ./target/release/rusqlite-connection-benchmark test_tiny.db --iterations 3; then
        echo "✓ Rusqlite benchmark runs successfully"
    else
        echo "✗ Rusqlite benchmark failed to run"
    fi
else
    echo "✗ Rusqlite benchmark failed to build"
fi

cd ..

# Test limbo benchmark  
echo "Testing limbo benchmark..."
cd limbo
python3 gen-database.py test_tiny.db -t 5
if cargo build --release; then
    echo "✓ Limbo benchmark builds successfully"
    if ./target/release/limbo-connection-benchmark test_tiny.db --iterations 3; then
        echo "✓ Limbo benchmark runs successfully"
    else
        echo "✗ Limbo benchmark failed to run"
    fi
else
    echo "✗ Limbo benchmark failed to build"
fi

echo "Test complete!"