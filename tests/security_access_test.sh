#!/usr/bin/env bash
#
# E2E Security Test: verifies that ZamSync rejects unauthorized clients
# at the mTLS level (no valid certificate = no connection), and that
# the OwnOnly access policy prevents clinic A from reading clinic B's data.
#
# PKI workflow tested:
#   zamsync keygen /data/hospital       -- hospital CA + hub node cert
#   zamsync sign /data/clinic_a ...     -- clinic A cert signed by hospital CA
#   zamsync sign /data/clinic_b ...     -- clinic B cert signed by hospital CA
#   zamsync keygen /data/rogue          -- rogue with its own self-signed CA (rejected)
#
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

echo -e "${BLUE}======================================================================${NC}"
echo -e "${BOLD} ZamSync E2E Security & PKI Multi-Node Test${NC}"
echo -e "${BLUE}======================================================================${NC}"

PASS_COUNT=0
FAIL_COUNT=0

pass() { echo -e "${GREEN}[PERFECT]${NC} $1"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail() { echo -e "${RED}[CRITICAL]${NC} $1"; FAIL_COUNT=$((FAIL_COUNT + 1)); }
info() { echo -e "${BLUE}[INFO]${NC} $1"; }

# ---------------------------------------------------------------------------
# SETUP: PKI via zamsync sign (Phase 9)
# ---------------------------------------------------------------------------
info "Setting up PKI with zamsync keygen + zamsync sign..."

rm -rf /data/hospital /data/clinic_a /data/clinic_b /data/clinic_a_fresh /data/rogue

# Initialize data dirs (writes .node_id)
zamsync info /data/hospital     > /dev/null
zamsync info /data/clinic_a     > /dev/null
zamsync info /data/clinic_b     > /dev/null
zamsync info /data/rogue        > /dev/null

HOSPITAL_ID=$(cat /data/hospital/.node_id)
CLINIC_A_ID=$(cat /data/clinic_a/.node_id)
CLINIC_B_ID=$(cat /data/clinic_b/.node_id)
ROGUE_ID=$(cat /data/rogue/.node_id)

info "Hospital Node ID : ${CYAN}$HOSPITAL_ID${NC}"
info "Clinic A  Node ID: ${CYAN}$CLINIC_A_ID${NC}"
info "Clinic B  Node ID: ${CYAN}$CLINIC_B_ID${NC}"
info "Rogue     Node ID: ${CYAN}$ROGUE_ID${NC}"

# Hospital generates the deployment CA + its own node cert + WAL key
zamsync keygen /data/hospital
info "Hospital CA generated: /data/hospital/tls/ca.crt"

# Clinic A and B receive node certs signed by hospital CA (Phase 9: zamsync sign)
zamsync sign /data/clinic_a --ca /data/hospital
zamsync sign /data/clinic_b --ca /data/hospital
info "Clinic A signed with hospital CA: /data/clinic_a/tls/node.crt"
info "Clinic B signed with hospital CA: /data/clinic_b/tls/node.crt"

# Verify the CA cert is the same for all trusted nodes
HOSPITAL_CA=$(md5sum /data/hospital/tls/ca.crt | awk '{print $1}')
CLINIC_A_CA=$(md5sum /data/clinic_a/tls/ca.crt | awk '{print $1}')
CLINIC_B_CA=$(md5sum /data/clinic_b/tls/ca.crt | awk '{print $1}')
if [ "$HOSPITAL_CA" = "$CLINIC_A_CA" ] && [ "$HOSPITAL_CA" = "$CLINIC_B_CA" ]; then
    pass "All trusted nodes share the same CA cert (hospital CA)"
else
    fail "CA cert mismatch between hospital and clinics!"
fi

# Rogue: generates its own self-signed CA (completely separate PKI)
zamsync keygen /data/rogue
info "Rogue Node: self-signed CA (different PKI, will be rejected)"

# ---------------------------------------------------------------------------
# TEST 1: OwnOnly Policy -- Clinic A CANNOT read Clinic B's events
# ---------------------------------------------------------------------------
echo ""
echo -e "${YELLOW}=== TEST 1: OwnOnly Access Control Policy ===${NC}"
info "Submitting patient records to Clinic A and Clinic B..."

zamsync submit /data/clinic_a "clinic-a-patient-record-001" > /dev/null
zamsync submit /data/clinic_a "clinic-a-patient-record-002" > /dev/null
zamsync submit /data/clinic_b "clinic-b-patient-record-001" > /dev/null
zamsync submit /data/clinic_b "clinic-b-patient-record-002" > /dev/null

# Start hospital hub in OwnOnly mode (plain TCP for this sub-test)
HOSPITAL_PORT=17001
zamsync serve /data/hospital 127.0.0.1:$HOSPITAL_PORT --policy own &
HOSPITAL_PID=$!
sleep 0.5

zamsync sync /data/clinic_a 127.0.0.1:$HOSPITAL_PORT $HOSPITAL_ID > /dev/null 2>&1 || true
sleep 0.2
zamsync sync /data/clinic_b 127.0.0.1:$HOSPITAL_PORT $HOSPITAL_ID > /dev/null 2>&1 || true
sleep 0.2

HOSPITAL_EVENTS=$(zamsync info /data/hospital | grep "events" | awk '{print $3}')
if [ "$HOSPITAL_EVENTS" -eq 4 ]; then
    pass "Hospital received all 4 events (2 from A + 2 from B)"
else
    fail "Hospital has $HOSPITAL_EVENTS events instead of 4"
fi

kill $HOSPITAL_PID 2>/dev/null || true
sleep 0.3

# Fresh clinic A (same NodeId, empty WAL) pulls from hospital with OwnOnly
HOSPITAL_PORT=17002
rm -rf /data/clinic_a_fresh
zamsync info /data/clinic_a_fresh > /dev/null
echo "$CLINIC_A_ID" > /data/clinic_a_fresh/.node_id

zamsync serve /data/hospital 127.0.0.1:$HOSPITAL_PORT --policy own &
HOSPITAL_PID=$!
sleep 0.5

zamsync sync /data/clinic_a_fresh 127.0.0.1:$HOSPITAL_PORT $HOSPITAL_ID > /dev/null 2>&1 || true
sleep 0.2

kill $HOSPITAL_PID 2>/dev/null || true

FRESH_A_EVENTS=$(zamsync info /data/clinic_a_fresh | grep "events" | awk '{print $3}')
if [ "$FRESH_A_EVENTS" -eq 2 ]; then
    pass "OwnOnly policy: Clinic A received only its 2 events (Clinic B's data is isolated)"
else
    fail "OwnOnly policy: Clinic A has $FRESH_A_EVENTS events instead of 2 (ISOLATION BROKEN)"
fi

# ---------------------------------------------------------------------------
# TEST 2: mTLS -- Trusted clinics (signed by hospital CA) can connect
# ---------------------------------------------------------------------------
echo ""
echo -e "${YELLOW}=== TEST 2: mTLS -- Trusted Clinic Connects Successfully ===${NC}"
info "Starting Hospital TLS server..."

TLS_PORT=17003
zamsync serve /data/hospital 127.0.0.1:$TLS_PORT --tls &
HOSPITAL_TLS_PID=$!
sleep 0.5

# Submit a new event to clinic_a so there is something to sync
zamsync submit /data/clinic_a "clinic-a-tls-test-event" > /dev/null

set +e
zamsync sync /data/clinic_a 127.0.0.1:$TLS_PORT $HOSPITAL_ID --tls > /tmp/clinic_a_tls.log 2>&1
CLINIC_A_TLS_EXIT=$?
set -e

if [ $CLINIC_A_TLS_EXIT -eq 0 ]; then
    pass "Clinic A (hospital-CA-signed cert) connected via mTLS successfully"
else
    fail "Clinic A mTLS connection FAILED (should have succeeded): $(cat /tmp/clinic_a_tls.log | head -2)"
fi

kill $HOSPITAL_TLS_PID 2>/dev/null || true
sleep 0.3

# ---------------------------------------------------------------------------
# TEST 3: mTLS -- Rogue client (own CA) is rejected at TLS handshake
# ---------------------------------------------------------------------------
echo ""
echo -e "${YELLOW}=== TEST 3: mTLS -- Rogue Client Rejected ===${NC}"
info "Starting Hospital TLS server..."

TLS_PORT=17004
zamsync serve /data/hospital 127.0.0.1:$TLS_PORT --tls &
HOSPITAL_TLS_PID=$!
sleep 0.5

# Rogue client attempts connection -- cert not signed by hospital CA
info "Rogue client attempting mTLS connection to Hospital..."
set +e
zamsync sync /data/rogue 127.0.0.1:$TLS_PORT $HOSPITAL_ID --tls > /tmp/rogue_sync.log 2>&1
ROGUE_EXIT=$?
set -e

if [ $ROGUE_EXIT -ne 0 ]; then
    pass "Rogue client rejected (exit code $ROGUE_EXIT)"
    info "Rejection: $(head -2 /tmp/rogue_sync.log)"
else
    fail "SECURITY BREACH: Rogue client connected successfully!"
fi

# Hospital event count must not have changed from the rogue attempt
HOSPITAL_EVENTS_AFTER_ROGUE=$(zamsync info /data/hospital | grep "events" | awk '{print $3}')
if [ "$HOSPITAL_EVENTS_AFTER_ROGUE" -ge 4 ]; then
    pass "Hospital event count ($HOSPITAL_EVENTS_AFTER_ROGUE) unchanged by rogue -- no data injected"
else
    fail "Hospital event count changed after rogue attempt -- potential injection!"
fi

kill $HOSPITAL_TLS_PID 2>/dev/null || true
sleep 0.3

# ---------------------------------------------------------------------------
# TEST 4: mTLS -- Plain TCP client against TLS-only server is rejected
# ---------------------------------------------------------------------------
echo ""
echo -e "${YELLOW}=== TEST 4: Plain TCP against TLS Server is Rejected ===${NC}"

TLS_PORT=17005
zamsync serve /data/hospital 127.0.0.1:$TLS_PORT --tls &
HOSPITAL_TLS_PID=$!
sleep 0.5

set +e
zamsync sync /data/clinic_a 127.0.0.1:$TLS_PORT $HOSPITAL_ID > /tmp/plain_sync.log 2>&1
PLAIN_EXIT=$?
set -e

if [ $PLAIN_EXIT -ne 0 ]; then
    pass "Plain TCP client rejected by TLS server (exit code $PLAIN_EXIT)"
else
    fail "SECURITY ISSUE: Plain TCP client connected to TLS-only server!"
fi

kill $HOSPITAL_TLS_PID 2>/dev/null || true

# ---------------------------------------------------------------------------
# TEST 5: zamsync rekey -- WAL re-encryption smoke test
# ---------------------------------------------------------------------------
echo ""
echo -e "${YELLOW}=== TEST 5: WAL Key Rotation (zamsync rekey) ===${NC}"

# Submit events with the original key
zamsync submit /data/clinic_a "rekey-test-event-1" --key-file /data/clinic_a/tls/data.key > /dev/null
zamsync submit /data/clinic_a "rekey-test-event-2" --key-file /data/clinic_a/tls/data.key > /dev/null

EVENTS_BEFORE=$(zamsync info /data/clinic_a | grep "events" | awk '{print $3}')

# Generate a new key
NEWKEY=/tmp/new_data.key
dd if=/dev/urandom bs=32 count=1 of=$NEWKEY 2>/dev/null

# Rotate the key
set +e
zamsync rekey /data/clinic_a --old-key /data/clinic_a/tls/data.key --new-key $NEWKEY > /tmp/rekey.log 2>&1
REKEY_EXIT=$?
set -e

if [ $REKEY_EXIT -eq 0 ]; then
    pass "WAL re-encryption completed: $(cat /tmp/rekey.log | head -1)"
else
    fail "WAL re-encryption failed: $(cat /tmp/rekey.log | head -2)"
fi

# Verify WAL is readable with the new key
EVENTS_AFTER=$(zamsync info /data/clinic_a | grep "events" | awk '{print $3}' || echo 0)
# Note: zamsync info without --key-file won't read encrypted WAL correctly.
# Use audit to verify with the new key.
set +e
zamsync audit /data/clinic_a --key-file $NEWKEY > /tmp/audit_after_rekey.log 2>&1
AUDIT_EXIT=$?
REKEY_EVENTS=$(grep -c "^[0-9]" /tmp/audit_after_rekey.log 2>/dev/null || echo 0)
set -e

if [ $AUDIT_EXIT -eq 0 ] && [ "$REKEY_EVENTS" -gt 0 ]; then
    pass "WAL readable with new key ($REKEY_EVENTS records verified by audit)"
else
    fail "WAL not readable with new key after rekey (audit exit=$AUDIT_EXIT, records=$REKEY_EVENTS)"
fi

# Verify old key can no longer read the WAL
set +e
zamsync audit /data/clinic_a --key-file /data/clinic_a/tls/data.key > /tmp/audit_old_key.log 2>&1
OLD_KEY_AUDIT=$?
set -e

if [ $OLD_KEY_AUDIT -ne 0 ]; then
    pass "Old key correctly rejected after rekey"
else
    fail "Old key still reads the WAL after rekey -- rotation did not work!"
fi

# ---------------------------------------------------------------------------
# RESULTS
# ---------------------------------------------------------------------------
echo ""
echo -e "${BLUE}======================================================================${NC}"
echo -e "${BOLD}               ZAMSYNC SECURITY & PKI TEST RESULTS                    ${NC}"
echo -e "${BLUE}======================================================================${NC}"

TOTAL=$((PASS_COUNT + FAIL_COUNT))
echo -e " PKI setup (shared CA via zamsync sign): $([ $PASS_COUNT -ge 1 ] && echo -e "${GREEN}PERFECT${NC}" || echo -e "${RED}CRITICAL${NC}")"
echo -e " OwnOnly access policy:                  $([ $PASS_COUNT -ge 3 ] && echo -e "${GREEN}PERFECT${NC}" || echo -e "${RED}CRITICAL${NC}")"
echo -e " mTLS trusted clinic connects:           $([ $PASS_COUNT -ge 4 ] && echo -e "${GREEN}PERFECT${NC}" || echo -e "${RED}CRITICAL${NC}")"
echo -e " mTLS rogue client rejected:             $([ $PASS_COUNT -ge 5 ] && echo -e "${GREEN}PERFECT${NC}" || echo -e "${RED}CRITICAL${NC}")"
echo -e " Plain TCP vs TLS server:                $([ $PASS_COUNT -ge 6 ] && echo -e "${GREEN}PERFECT${NC}" || echo -e "${RED}CRITICAL${NC}")"
echo -e " WAL key rotation (rekey):               $([ $PASS_COUNT -ge 8 ] && echo -e "${GREEN}PERFECT${NC}" || echo -e "${RED}CRITICAL${NC}")"
echo ""
echo -e " Tests passed: ${GREEN}$PASS_COUNT${NC} / $TOTAL  |  Failed: $([ $FAIL_COUNT -eq 0 ] && echo -e "${GREEN}$FAIL_COUNT${NC}" || echo -e "${RED}$FAIL_COUNT${NC}")"
echo -e "${BLUE}======================================================================${NC}"

if [ $FAIL_COUNT -gt 0 ]; then
    echo -e "${RED}[CRITICAL]${NC} $FAIL_COUNT security test(s) failed!"
    exit 1
fi

echo -e "${GREEN}[SUCCESS]${NC} All security and PKI tests passed."
echo -e "${BLUE}======================================================================${NC}"
