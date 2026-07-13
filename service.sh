#!/system/bin/sh
# TeeForge-CD Service Script
# Runs at boot via Magisk service.d

MODDIR=${0%/*}

# Wait for system to boot
while [ "$(getprop sys.boot_completed)" != "1" ]; do
  sleep 1
done

# Additional delay for stability
sleep 5

# Run teeforge
$MODDIR/teeforge --generate

# Log completion
echo "[TeeForge-CD] Boot service completed" >> /data/adb/teeforge/teeforge.log
