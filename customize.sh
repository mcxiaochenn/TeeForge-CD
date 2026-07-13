#!/system/bin/sh
# TeeForge-CD Installation Script

ui_print "- Installing TeeForge-CD"

# Create directories
mkdir -p /data/adb/teeforge/keybox
mkdir -p /data/adb/teeforge/logs

# Copy config file if not exists
if [ ! -f /data/adb/teeforge/config.conf ]; then
    cp $MODPATH/config.conf /data/adb/teeforge/config.conf
    ui_print "- Created default config.conf"
else
    ui_print "- Preserved existing config.conf"
fi

# Set permissions
set_perm_recursive $MODPATH 0 0 0755 0644
set_perm $MODPATH/teeforge 0 0 0755

ui_print "- Installation complete"