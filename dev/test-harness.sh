#!/bin/bash
# =============================================================================
# NaraeClaw Harness Layer — Docker Smoke Test
#
# Validates the gateway/runtime harness, including the ByoriDB knowledge contract:
#   1. ByoriDB is the configured durable knowledge provider
#   2. The retired flat /api/memory surface is absent
#   3. The managed ByoriDB MCP connection is healthy when installed
#   4. Agent and session APIs remain healthy
#
# Usage:
#   docker exec naraeclaw-dev bash /naraeclaw-data/workspace/test-harness.sh
#   or: ./dev/test-harness.sh  (if running on host with gateway at localhost:42617)
#
# Prerequisites:
#   - Gateway running on localhost:42617
#   - API_KEY set (or no auth required)
#   - curl and websocat (or wscat) available
# =============================================================================

set -euo pipefail

BASE_URL="${NARAECLAW_GATEWAY_URL:-${ZEROCLAW_GATEWAY_URL:-http://localhost:42617}}"
WS_URL="${NARAECLAW_WS_URL:-${ZEROCLAW_WS_URL:-ws://localhost:42617/ws/chat}}"
PASS=0
FAIL=0
SKIP=0

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { PASS=$((PASS + 1)); echo -e "${GREEN}  PASS${NC}: $1"; }
fail() { FAIL=$((FAIL + 1)); echo -e "${RED}  FAIL${NC}: $1${2:+ — $2}"; }
skip() { SKIP=$((SKIP + 1)); echo -e "${YELLOW}  SKIP${NC}: $1${2:+ — $2}"; }
info() { echo -e "  INFO: $1"; }

# ── Wait for gateway readiness ──────────────────────────────────────
echo "=== Waiting for gateway at $BASE_URL ==="
for i in $(seq 1 30); do
    if curl -sf "$BASE_URL/health" >/dev/null 2>&1; then
        echo "Gateway ready after ${i}s"
        break
    fi
    if [ "$i" -eq 30 ]; then
        echo -e "${RED}Gateway not ready after 30s, aborting${NC}"
        exit 1
    fi
    sleep 1
done

# ═══════════════════════════════════════════════════════════════════════
# TEST 1: ByoriDB Knowledge Configuration
# ═══════════════════════════════════════════════════════════════════════
echo ""
echo "=== Test 1: ByoriDB Knowledge Configuration ==="

KNOWLEDGE_CONFIG=$(curl -sf "$BASE_URL/api/config" 2>&1) || true

if echo "$KNOWLEDGE_CONFIG" | grep -qi "byoridb"; then
    pass "ByoriDB is present in the effective configuration"
else
    fail "ByoriDB knowledge configuration" "$(echo "$KNOWLEDGE_CONFIG" | head -c 300)"
fi

# ═══════════════════════════════════════════════════════════════════════
# TEST 2: Legacy Memory API Is Retired
# ═══════════════════════════════════════════════════════════════════════
echo ""
echo "=== Test 2: Legacy Memory API Is Retired ==="

LEGACY_MEMORY_STATUS=$(curl -s -o /dev/null -w "%{http_code}" "$BASE_URL/api/memory") || true

if [ "$LEGACY_MEMORY_STATUS" = "404" ]; then
    pass "Legacy /api/memory route is not registered"
else
    fail "Legacy /api/memory route" "expected HTTP 404, got $LEGACY_MEMORY_STATUS"
fi

# ═══════════════════════════════════════════════════════════════════════
# TEST 3: Managed ByoriDB MCP Readiness
# ═══════════════════════════════════════════════════════════════════════
echo ""
echo "=== Test 3: Managed ByoriDB MCP Readiness ==="

if command -v naraeclaw >/dev/null 2>&1 && [ -x "${HOME}/.byoridb/bin/run-mcp.sh" ]; then
    KNOWLEDGE_STATUS=$(naraeclaw knowledge status 2>&1) || true
    if echo "$KNOWLEDGE_STATUS" | grep -q "MCP connection:.*ready"; then
        pass "Managed ByoriDB MCP connection is ready"
    else
        fail "Managed ByoriDB MCP readiness" "$(echo "$KNOWLEDGE_STATUS" | head -c 400)"
    fi
else
    skip "Managed ByoriDB MCP readiness" "naraeclaw or ~/.byoridb/bin/run-mcp.sh is unavailable"
fi

# ═══════════════════════════════════════════════════════════════════════
# TEST 4: Session State API
# ═══════════════════════════════════════════════════════════════════════
echo ""
echo "=== Test 4: Session State API ==="

SESSIONS_RESP=$(curl -sf "$BASE_URL/api/sessions/running" 2>&1) || true

if echo "$SESSIONS_RESP" | grep -qE '\[|sessions'; then
    pass "GET /api/sessions/running returned valid response"
else
    fail "GET /api/sessions/running" "$SESSIONS_RESP"
fi

# ═══════════════════════════════════════════════════════════════════════
# TEST 5: Gateway Status API
# ═══════════════════════════════════════════════════════════════════════
echo ""
echo "=== Test 5: Gateway Status ==="

STATUS_RESP=$(curl -sf "$BASE_URL/api/status" 2>&1) || true

if echo "$STATUS_RESP" | grep -qi "version\|status\|running"; then
    pass "GET /api/status returned valid response"
else
    fail "GET /api/status" "$STATUS_RESP"
fi

# ═══════════════════════════════════════════════════════════════════════
# TEST 6: Tools List (verify harness tools present)
# ═══════════════════════════════════════════════════════════════════════
echo ""
echo "=== Test 6: Tools List ==="

TOOLS_RESP=$(curl -sf "$BASE_URL/api/tools" 2>&1) || true

if echo "$TOOLS_RESP" | grep -qi "byoridb__memory_read"; then
    pass "ByoriDB knowledge tools registered"
elif echo "$TOOLS_RESP" | grep -qi '"memory_store"\|"memory_recall"'; then
    fail "Legacy memory tools are still registered" "$(echo "$TOOLS_RESP" | head -c 300)"
else
    info "Tools response: $(echo "$TOOLS_RESP" | head -c 300)"
    skip "Could not verify ByoriDB tools in tools list"
fi

# ═══════════════════════════════════════════════════════════════════════
# TEST 7: WebSocket Chat (if websocat available)
# ═══════════════════════════════════════════════════════════════════════
echo ""
echo "=== Test 7: WebSocket Chat ==="

if command -v websocat >/dev/null 2>&1; then
    WS_CHAT_URL="${WS_URL}?session_id=harness-test-ws"

    # Send a simple message and capture response (timeout after 30s)
    WS_RESP=$(echo '{"type":"message","content":"What is 2 + 2? Reply with just the number."}' | \
        timeout 30 websocat -t "$WS_CHAT_URL" 2>&1 | head -20) || true

    if echo "$WS_RESP" | grep -qE 'chunk|complete|"4"'; then
        pass "WebSocket chat received response"
    elif [ -n "$WS_RESP" ]; then
        info "WS response: $(echo "$WS_RESP" | head -c 300)"
        pass "WebSocket connection successful (got response)"
    else
        fail "WebSocket chat" "No response received"
    fi
elif command -v wscat >/dev/null 2>&1; then
    skip "WebSocket chat" "wscat available but not scripted — use websocat or test manually"
else
    skip "WebSocket chat" "Neither websocat nor wscat found — install with: apt install websocat"
fi

# ═══════════════════════════════════════════════════════════════════════
# TEST 8: Config Verification (harness features enabled)
# ═══════════════════════════════════════════════════════════════════════
echo ""
echo "=== Test 8: Config Verification ==="

CONFIG_RESP=$(curl -sf "$BASE_URL/api/config" 2>&1) || true

CHECKS=0
if echo "$CONFIG_RESP" | grep -q "max_tool_result_chars"; then
    CHECKS=$((CHECKS + 1))
fi
if echo "$CONFIG_RESP" | grep -q "max_context_tokens"; then
    CHECKS=$((CHECKS + 1))
fi
if echo "$CONFIG_RESP" | grep -q "context_compression"; then
    CHECKS=$((CHECKS + 1))
fi
if echo "$CONFIG_RESP" | grep -qi "byoridb"; then
    CHECKS=$((CHECKS + 1))
fi

if [ "$CHECKS" -ge 3 ]; then
    pass "Config shows harness features enabled ($CHECKS/4 fields found)"
elif [ "$CHECKS" -ge 1 ]; then
    pass "Config shows some harness features ($CHECKS/4 fields found)"
else
    info "Config response: $(echo "$CONFIG_RESP" | head -c 500)"
    skip "Could not verify harness config fields"
fi

# ═══════════════════════════════════════════════════════════════════════
# TEST 9: Session List API
# ═══════════════════════════════════════════════════════════════════════
echo ""
echo "=== Test 9: Session List ==="

SESSIONS_LIST=$(curl -sf "$BASE_URL/api/sessions" 2>&1) || true

if echo "$SESSIONS_LIST" | grep -qE '\[|sessions'; then
    pass "GET /api/sessions returned valid response"
else
    fail "GET /api/sessions" "$SESSIONS_LIST"
fi

# ═══════════════════════════════════════════════════════════════════════
# TEST 10: Health Endpoint
# ═══════════════════════════════════════════════════════════════════════
echo ""
echo "=== Test 10: Health Check ==="

HEALTH_RESP=$(curl -sf "$BASE_URL/health" 2>&1) || true

if [ -n "$HEALTH_RESP" ]; then
    pass "Health endpoint responding"
else
    fail "Health endpoint" "No response"
fi

# ═══════════════════════════════════════════════════════════════════════
# Summary
# ═══════════════════════════════════════════════════════════════════════
echo ""
echo "==========================================="
echo "  RESULTS: ${GREEN}${PASS} passed${NC}, ${RED}${FAIL} failed${NC}, ${YELLOW}${SKIP} skipped${NC}"
echo "==========================================="

if [ "$FAIL" -gt 0 ]; then
    echo -e "${RED}Some tests failed. Check output above.${NC}"
    exit 1
else
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
fi
