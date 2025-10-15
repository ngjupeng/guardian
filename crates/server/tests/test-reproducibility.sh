#!/bin/bash
set -e

# Test reproducibility of the server binary
# This script builds the server twice in Docker and compares the resulting binaries

echo "================================================"
echo "Testing Reproducible Build for PSM Server"
echo "================================================"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Create temp directories for builds
BUILD_DIR_1=$(mktemp -d)
BUILD_DIR_2=$(mktemp -d)

echo "Build directories:"
echo "  Build 1: $BUILD_DIR_1"
echo "  Build 2: $BUILD_DIR_2"
echo ""

cleanup() {
    echo "Cleaning up..."
    rm -rf "$BUILD_DIR_1" "$BUILD_DIR_2"
    docker rmi psm-repro-test-1 psm-repro-test-2 2>/dev/null || true
}

trap cleanup EXIT

# Build 1
echo "==> Build 1: Building Docker image..."
docker build -t psm-repro-test-1 . --no-cache

echo "==> Build 1: Extracting binary..."
docker create --name psm-temp-1 psm-repro-test-1
docker cp psm-temp-1:/app/server "$BUILD_DIR_1/server"
docker rm psm-temp-1

# Small delay to ensure different build timestamp if timestamps are embedded
sleep 2

# Build 2
echo ""
echo "==> Build 2: Building Docker image..."
docker build -t psm-repro-test-2 . --no-cache

echo "==> Build 2: Extracting binary..."
docker create --name psm-temp-2 psm-repro-test-2
docker cp psm-temp-2:/app/server "$BUILD_DIR_2/server"
docker rm psm-temp-2

# Compare binaries
echo ""
echo "================================================"
echo "Comparing Binaries"
echo "================================================"

# Get file sizes
SIZE_1=$(wc -c < "$BUILD_DIR_1/server")
SIZE_2=$(wc -c < "$BUILD_DIR_2/server")

echo "Binary sizes:"
echo "  Build 1: $SIZE_1 bytes"
echo "  Build 2: $SIZE_2 bytes"
echo ""

# Calculate hashes
HASH_1=$(sha256sum "$BUILD_DIR_1/server" | awk '{print $1}')
HASH_2=$(sha256sum "$BUILD_DIR_2/server" | awk '{print $1}')

echo "SHA256 hashes:"
echo "  Build 1: $HASH_1"
echo "  Build 2: $HASH_2"
echo ""

# Compare
if [ "$HASH_1" = "$HASH_2" ]; then
    echo -e "${GREEN}✓ SUCCESS: Binaries are identical!${NC}"
    echo -e "${GREEN}The build is reproducible.${NC}"
    exit 0
else
    echo -e "${RED}✗ FAILURE: Binaries differ!${NC}"
    echo -e "${RED}The build is NOT reproducible.${NC}"
    echo ""

    # Additional analysis
    echo "==> Detailed Analysis"
    echo ""

    # Binary diff stats
    if command -v cmp &> /dev/null; then
        echo "Byte differences:"
        cmp -l "$BUILD_DIR_1/server" "$BUILD_DIR_2/server" | wc -l | xargs echo "  Different bytes:"

        # Show first few differences
        echo ""
        echo "First 10 byte differences (offset, build1 value, build2 value):"
        cmp -l "$BUILD_DIR_1/server" "$BUILD_DIR_2/server" | head -10
    fi

    # Check if debug info might be the issue
    echo ""
    if command -v file &> /dev/null; then
        echo "Binary info:"
        file "$BUILD_DIR_1/server"
    fi

    if command -v strings &> /dev/null; then
        echo ""
        echo "Checking for embedded paths or timestamps..."

        # Look for absolute paths
        PATHS_1=$(strings "$BUILD_DIR_1/server" | grep -c "^/" || true)
        PATHS_2=$(strings "$BUILD_DIR_2/server" | grep -c "^/" || true)
        echo "  Absolute paths in Build 1: $PATHS_1"
        echo "  Absolute paths in Build 2: $PATHS_2"

        # Look for potential timestamps (common patterns)
        echo ""
        echo "Sample strings from Build 1 (first 5 lines with /app or /usr):"
        strings "$BUILD_DIR_1/server" | grep -E "/(app|usr|home)" | head -5 || echo "  None found"
    fi

    echo ""
    echo -e "${YELLOW}Tip: Check for:${NC}"
    echo "  - Embedded timestamps"
    echo "  - Absolute file paths in debug info"
    echo "  - Non-deterministic ordering"
    echo "  - Environment-specific data"

    exit 1
fi
