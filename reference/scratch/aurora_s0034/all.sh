#!/bin/bash
# Aurora S0034 Complete Test Sequence
# Board: Aurora PGA Sonoma Air full size BIM (5 DUTs)
# Generated from SCH#S0034 Rev A (07/13/24)
#
# Usage: ./all.sh [vectors_dir]
# Example: ./all.sh /mnt/vectors/aurora

VECTORS_DIR=${1:-"/mnt/vectors/aurora"}

echo "============================================================"
echo "  Aurora S0034 HTOL Test Sequence"
echo "  5 DUTs, PGA Package"
echo "============================================================"
echo

# Copy PIN_MAP to controller
cp PIN_MAP /mnt/.

# Power on sequence
echo "[1/6] Powering on..."
./PowerOn
sleep 1

# Verify power good
echo "[2/6] Verifying power rails..."
/mnt/bin/ADC32ChPlusStats.elf | grep -E "VOUT|PS[123]"
echo

# Run test vectors if they exist
if [ -d "$VECTORS_DIR" ]; then
    echo "[3/6] Loading scan burnin vectors..."
    /mnt/bin/linux_load_vectors.elf ${VECTORS_DIR}/aurora_scan_burnin.seq ${VECTORS_DIR}/aurora_scan_burnin.hex
    /mnt/bin/linux_run_vector.elf 0 0 1 1 0 0

    echo "[4/6] Loading GPIO toggle vectors..."
    /mnt/bin/linux_load_vectors.elf ${VECTORS_DIR}/aurora_gpio_toggle.seq ${VECTORS_DIR}/aurora_gpio_toggle.hex
    /mnt/bin/linux_run_vector.elf 0 0 1 1 0 0

    echo "[5/6] Loading MBIST vectors..."
    /mnt/bin/linux_load_vectors.elf ${VECTORS_DIR}/aurora_mbist.seq ${VECTORS_DIR}/aurora_mbist.hex
    /mnt/bin/linux_run_vector.elf 0 0 1 1 0 0
else
    echo "[3-5/6] Skipping vectors (directory not found: $VECTORS_DIR)"
fi

# Final ADC readback
echo "[6/6] Final power rail verification..."
/mnt/bin/ADC32ChPlusStats.elf
/mnt/bin/XADC32Ch.elf

# Power off
echo
echo "Test complete. Powering off..."
./PowerOff

echo
echo "============================================================"
echo "  Aurora S0034 Test Sequence Complete"
echo "============================================================"
