# Get the number of CPUs
num_cpus=20

# Set the CPU range for taskset
cpu_range="0-$((num_cpus - 1))"

# Loop through each PID in /proc and set CPU affinity
for pid in $(ls /proc | grep '^[0-9]\+$'); do
    # Use taskset to set the CPU affinity for each process
    sudo taskset -cp "$cpu_range" "$pid" 2>/dev/null
done

echo "CPU affinity has been reset to use all CPUs for each process."
