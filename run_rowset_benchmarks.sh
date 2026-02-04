#!/bin/bash

# RowSet Benchmark Runner
# Usage examples:
#
# Run all benchmarks:
#   ./run_rowset_benchmarks.sh
#
# Run specific benchmark group:
#   ./run_rowset_benchmarks.sh --group "RowSet Insert Operations"
#   ./run_rowset_benchmarks.sh --group "RowSet Test Operations"
#   ./run_rowset_benchmarks.sh --group "RowSet Iteration Operations"
#   ./run_rowset_benchmarks.sh --group "RowSet Mixed Workload"
#   ./run_rowset_benchmarks.sh --group "RowSet Serialization"
#
# Run specific size only:
#   ./run_rowset_benchmarks.sh --filter "1000000"
#   ./run_rowset_benchmarks.sh --filter "100000"
#
# Save results to file:
#   ./run_rowset_benchmarks.sh --save-baseline baseline_name
#
# Compare with previous baseline:
#   ./run_rowset_benchmarks.sh --baseline baseline_name

set -e

# Default parameters
BENCHMARK_NAME="rowset_benchmark"
FILTER=""
GROUP=""
BASELINE=""
SAVE_BASELINE=""
EXTRA_ARGS=""

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --group)
            GROUP="$2"
            shift 2
            ;;
        --filter)
            FILTER="$2"
            shift 2
            ;;
        --baseline)
            BASELINE="$2"
            shift 2
            ;;
        --save-baseline)
            SAVE_BASELINE="$2"
            shift 2
            ;;
        --help)
            echo "RowSet Benchmark Runner"
            echo ""
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --group GROUP         Run specific benchmark group"
            echo "  --filter PATTERN      Filter benchmarks by pattern (e.g., size)"
            echo "  --baseline NAME       Load baseline for comparison"
            echo "  --save-baseline NAME  Save results as baseline"
            echo "  --help               Show this help message"
            echo ""
            echo "Available groups:"
            echo "  'RowSet Insert Operations'"
            echo "  'RowSet Test Operations'"
            echo "  'RowSet Iteration Operations'"
            echo "  'RowSet Mixed Workload'"
            echo "  'RowSet Serialization'"
            echo ""
            echo "Size filters: 1000, 10000, 100000, 1000000"
            exit 0
            ;;
        *)
            EXTRA_ARGS="$EXTRA_ARGS $1"
            shift
            ;;
    esac
done

# Build the command
CMD="cargo bench --bench $BENCHMARK_NAME"

# Add filtering
if [ -n "$GROUP" ]; then
    CMD="$CMD -- --exact '$GROUP'"
elif [ -n "$FILTER" ]; then
    CMD="$CMD -- '$FILTER'"
fi

# Add baseline comparison
if [ -n "$BASELINE" ]; then
    CMD="$CMD --load-baseline '$BASELINE'"
fi

# Add baseline saving
if [ -n "$SAVE_BASELINE" ]; then
    CMD="$CMD --save-baseline '$SAVE_BASELINE'"
fi

# Add extra arguments
if [ -n "$EXTRA_ARGS" ]; then
    CMD="$CMD $EXTRA_ARGS"
fi

echo "Running: $CMD"
echo "=========================================="

# Execute the command
eval $CMD

echo ""
echo "Benchmark complete!"
echo ""
echo "Results are saved in: target/criterion/"
echo "Flamegraphs are in: target/criterion/*/profile/"
echo ""
echo "To view HTML reports, open: target/criterion/report/index.html"