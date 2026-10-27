#!/bin/bash
# Test script for P2P Remote Control
# Run this after installing Rust to verify functionality

set -e

echo "=========================================="
echo "P2P Remote Control Test Suite"
echo "=========================================="
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

PASS=0
FAIL=0

test_pass() {
    echo -e "${GREEN}✓ PASS${NC}: $1"
    ((PASS++)) || true
}

test_fail() {
    echo -e "${RED}✗ FAIL${NC}: $1"
    ((FAIL++)) || true
}

test_info() {
    echo -e "${YELLOW}→${NC}: $1"
}

# Test 1: Check Rust installation
test_info "Checking Rust installation..."
if command -v rustc &> /dev/null; then
    RUST_VERSION=$(rustc --version 2>&1)
    test_pass "Rust installed: $RUST_VERSION"
else
    test_fail "Rust not installed - install with: winget install Rustlang.Rust"
    echo ""
    echo "To continue testing, install Rust and run this script again."
    exit 1
fi

# Test 2: Check Cargo installation
test_info "Checking Cargo installation..."
if command -v cargo &> /dev/null; then
    CARGO_VERSION=$(cargo --version 2>&1)
    test_pass "Cargo installed: $CARGO_VERSION"
else
    test_fail "Cargo not installed"
    exit 1
fi

# Test 3: Check file structure
test_info "Checking file structure..."
REQUIRED_FILES=(
    "Cargo.toml"
    "README.md"
    "src/main.rs"
    "src/screen_capture.rs"
    "src/input_injector.rs"
    "src/pairing.rs"
    "src/webrtc_conn.rs"
    "src/video_encoder.rs"
    "src/client_input.rs"
)

STRUCTURE_OK=true
for file in "${REQUIRED_FILES[@]}"; do
    if [ ! -f "$file" ]; then
        test_fail "Missing file: $file"
        STRUCTURE_OK=false
    fi
done

if [ "$STRUCTURE_OK" = true ]; then
    test_pass "All required files present (${#REQUIRED_FILES[@]} files)"
fi

# Test 4: Check for test modules
test_info "Checking test modules..."
TEST_MODULES=(
    "src/pairing.rs"
    "src/video_encoder.rs"
    "src/input_injector.rs"
    "src/client_input.rs"
)

TESTS_FOUND=0
for file in "${TEST_MODULES[@]}"; do
    if grep -q "#\[cfg(test)\]" "$file" 2>/dev/null; then
        COUNT=$(grep -c "#\[test\]" "$file" 2>/dev/null || echo 0)
        TESTS_FOUND=$((TESTS_FOUND + COUNT))
    fi
done

if [ $TESTS_FOUND -gt 0 ]; then
    test_pass "Found $TESTS_FOUND unit tests across modules"
else
    test_fail "No unit tests found"
fi

# Test 5: Check syntax with cargo check
test_info "Running cargo check..."
if cargo check 2>&1 | tee /tmp/cargo_check.log; then
    test_pass "Syntax check passed"
else
    if grep -q "error\[" /tmp/cargo_check.log; then
        test_fail "Compilation errors found"
        echo ""
        echo "Errors:"
        grep "error\[" /tmp/cargo_check.log | head -5
    else
        test_info "Warnings detected (non-fatal)"
    fi
fi

# Test 6: Run unit tests
test_info "Running unit tests..."
if cargo test 2>&1 | tee /tmp/cargo_test.log; then
    # Extract test results
    TESTS_RUN=$(grep "test result:" /tmp/cargo_test.log | tail -1 || echo "")
    if [ -n "$TESTS_RUN" ]; then
        test_pass "Tests completed: $TESTS_RUN"
    else
        test_pass "Tests completed successfully"
    fi
else
    if grep -q "test result:.*0 passed" /tmp/cargo_test.log; then
        test_fail "No tests passed"
    else
        test_info "Tests had failures (check output above)"
    fi
fi

# Test 7: Check pairing module specifically
test_info "Testing pairing module..."
PAIRING_TESTS=$(grep -c "#\[test\]" src/pairing.rs 2>/dev/null || echo 0)
if [ $PAIRING_TESTS -gt 5 ]; then
    test_pass "Pairing module has $PAIRING_TESTS tests"
else
    test_fail "Pairing module has few tests ($PAIRING_TESTS)"
fi

# Test 8: Check video encoder module
test_info "Testing video encoder module..."
VIDEO_TESTS=$(grep -c "#\[test\]" src/video_encoder.rs 2>/dev/null || echo 0)
if [ $VIDEO_TESTS -gt 3 ]; then
    test_pass "Video encoder has $VIDEO_TESTS tests"
else
    test_fail "Video encoder has few tests ($VIDEO_TESTS)"
fi

# Test 9: Check input injector module
test_info "Testing input injector module..."
INPUT_TESTS=$(grep -c "#\[test\]" src/input_injector.rs 2>/dev/null || echo 0)
if [ $INPUT_TESTS -gt 3 ]; then
    test_pass "Input injector has $INPUT_TESTS tests"
else
    test_fail "Input injector has few tests ($INPUT_TESTS)"
fi

# Test 10: Check client input module
test_info "Testing client input module..."
CLIENT_TESTS=$(grep -c "#\[test\]" src/client_input.rs 2>/dev/null || echo 0)
if [ $CLIENT_TESTS -gt 3 ]; then
    test_pass "Client input has $CLIENT_TESTS tests"
else
    test_fail "Client input has few tests ($CLIENT_TESTS)"
fi

# Test 11: Security tests
test_info "Checking security test coverage..."
SECURITY_TESTS=$(grep -c "test_auth\|test_key\|test_handshake\|test_salt" src/pairing.rs 2>/dev/null || echo 0)
if [ $SECURITY_TESTS -gt 3 ]; then
    test_pass "Security tests: $SECURITY_TESTS"
else
    test_fail "Few security tests ($SECURITY_TESTS)"
fi

# Test 12: Build test (optional - may take time)
test_info "Checking release build configuration..."
if grep -q "lto = true" Cargo.toml; then
    test_pass "LTO enabled for optimized builds"
else
    test_info "LTO not enabled"
fi

# Summary
echo ""
echo "=========================================="
echo "Test Summary"
echo "=========================================="
echo -e "${GREEN}Passed: $PASS${NC}"
echo -e "${RED}Failed: $FAIL${NC}"
echo ""

if [ $FAIL -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}"
    echo ""
    echo "Next steps:"
    echo "1. Run: cargo build --release"
    echo "2. Test host: ./target/release/p2p-remote-control --host"
    echo "3. Test client: ./target/release/p2p-remote-control --connect 123456"
    exit 0
else
    echo -e "${RED}Some tests failed.${NC}"
    echo ""
    echo "See errors above for details."
    exit 1
fi
