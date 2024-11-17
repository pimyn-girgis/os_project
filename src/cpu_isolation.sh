#!/bin/bash

# Define the core numbers to isolate
read -p "Enter core numbers to isolate (comma-separated, e.g., 5,11): " CORES

# Validate input
if [[ -z "$CORES" ]]; then
    echo "Error: No cores specified. Exiting."
    exit 1
fi

# Set file paths
GRUB_CONFIG="/etc/default/grub"
ISOLATED_FILE="/sys/devices/system/cpu/isolated"
NOHZ_FULL_FILE="/sys/devices/system/cpu/nohz_full"

# Ensure script is run as root
if [[ $EUID -ne 0 ]]; then
    echo "This script must be run as root. Exiting."
    exit 1
fi

# Backup GRUB config if not already backed up
if [[ ! -f "${GRUB_CONFIG}.bak" ]]; then
    echo "Backing up GRUB configuration to ${GRUB_CONFIG}.bak..."
    cp "$GRUB_CONFIG" "${GRUB_CONFIG}.bak"
fi

# Modify GRUB_CMDLINE_LINUX in GRUB configuration
echo "Updating GRUB configuration..."
if grep -q '^GRUB_CMDLINE_LINUX=' "$GRUB_CONFIG"; then
    sed -i "s/^GRUB_CMDLINE_LINUX=.*/GRUB_CMDLINE_LINUX=\"isolcpus=$CORES nohz_full=$CORES\"/" "$GRUB_CONFIG"
else
    echo "GRUB_CMDLINE_LINUX=\"isolcpus=$CORES nohz_full=$CORES\"" >> "$GRUB_CONFIG"
fi

# Update GRUB
if command -v update-grub &> /dev/null; then
    echo "Running update-grub..."
    update-grub
elif command -v grub2-mkconfig &> /dev/null; then
    echo "Running grub2-mkconfig..."
    grub2-mkconfig -o /boot/grub2/grub.cfg
else
    echo "Error: Unable to find GRUB update command. Please update GRUB manually."
    exit 1
fi

# Reboot prompt
echo "Reboot the system to apply changes and verify isolated cores."
# Uncomment the following line to automatically reboot:
# reboot

# Verification commands (displayed to the user)
echo "After reboot, verify the changes using the following commands:"
echo "1. Check isolated cores: cat $ISOLATED_FILE"
echo "2. Check full tickless operation: cat $NOHZ_FULL_FILE"
echo ""
echo "If full tickless operation is not enabled:"
echo "a) Find your kernel version: uname -a"
echo "b) Edit the kernel config: sudo nano /boot/config-<kernel version>"
echo "c) Enable the CONFIG_NO_HZ_FULL flag under the 'Timers subsystem'."
