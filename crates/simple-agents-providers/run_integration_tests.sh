#!/bin/bash
#
# Run integration tests for simple-agents-providers
#
# Usage:
#   ./run_integration_tests.sh              # Run all integration tests
#   ./run_integration_tests.sh connection   # Run only connection test
#   ./run_integration_tests.sh multiple     # Run multiple requests test
#   ./run_integration_tests.sh error        # Run error handling test
#   ./run_integration_tests.sh temperature  # Run temperature test
#   ./run_integration_tests.sh conversation # Run conversation test

set -e  # Exit on error

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Configuration
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ENV_FILE="$ROOT_DIR/.env"
if [ -f "$ENV_FILE" ]; then
    set -a
    # shellcheck source=/dev/null
    . "$ENV_FILE"
    set +a
fi

API_BASE="${CUSTOM_API_BASE:?CUSTOM_API_BASE environment variable not set}"
API_KEY="${CUSTOM_API_KEY:?CUSTOM_API_KEY environment variable not set}"
MODEL="${CUSTOM_API_MODEL:?CUSTOM_API_MODEL environment variable not set}"

echo -e "${YELLOW}=== SimpleAgents Provider Integration Tests ===${NC}"
echo ""
echo "Configuration:"
echo "  API Base: $API_BASE"
echo "  API Key: $API_KEY"
echo "  Model: $MODEL"
echo ""

# Check if server is reachable
echo -e "${YELLOW}Checking if server is reachable...${NC}"
if curl -s --connect-timeout 2 "$API_BASE" > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Server is reachable${NC}"
else
    echo -e "${RED}✗ Cannot connect to $API_BASE${NC}"
    echo ""
    echo "Please ensure your LLM proxy server is running at $API_BASE"
    echo ""
    echo "Example: If using LiteLLM proxy:"
    echo "  litellm --model $MODEL --port 4000"
    echo ""
    exit 1
fi
echo ""

# Determine which test to run
case "${1:-all}" in
    connection)
        echo -e "${YELLOW}Running connection test...${NC}"
        cargo test -p simple-agents-providers test_local_proxy_connection -- --ignored --nocapture
        ;;

    multiple)
        echo -e "${YELLOW}Running multiple requests test...${NC}"
        cargo test -p simple-agents-providers test_local_proxy_multiple_requests -- --ignored --nocapture
        ;;

    error)
        echo -e "${YELLOW}Running error handling test...${NC}"
        cargo test -p simple-agents-providers test_local_proxy_invalid_model -- --ignored --nocapture
        ;;

    temperature)
        echo -e "${YELLOW}Running temperature variations test...${NC}"
        cargo test -p simple-agents-providers test_local_proxy_temperature_variations -- --ignored --nocapture
        ;;

    conversation)
        echo -e "${YELLOW}Running conversation test...${NC}"
        cargo test -p simple-agents-providers test_local_proxy_conversation -- --ignored --nocapture
        ;;

    all|*)
        echo -e "${YELLOW}Running all integration tests...${NC}"
        cargo test -p simple-agents-providers --test openai_integration -- --ignored --nocapture
        ;;
esac

echo ""
echo -e "${GREEN}=== Tests Complete ===${NC}"
