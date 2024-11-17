#!/bin/bash

echo "Verifying CPU affinity and process isolation..."

isolated_cpu=2  # Replace with your isolated CPU
expected_mask=$((1 << isolated_cpu))

echo "Processes bound to CPU $isolated_cpu:"
for pid in $(ps -e -o pid=); do
    mask=$(taskset -p $pid | awk '{print $NF}' | sed 's/^0x//')
    if ((0x$mask & expected_mask)); then
        echo "PID $pid ($(ps -p $pid -o comm=)) is bound to CPU $isolated_cpu with mask $mask."
    fi
done
