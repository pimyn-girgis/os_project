const invoke = window.__TAURI__.core.invoke;

    let cpuChart, memoryChart;

    function showCard(cardId) {
        const cards = document.querySelectorAll('.card');
        cards.forEach(card => card.classList.remove('active'));
        document.getElementById(cardId).classList.add('active');
        if(cardId === 'memoryUsageCard') {
            loadSysInfo();
        }
    }
    
    async function loadSystemStats() {
        try {
            const stats = await invoke('show_stats', {
                nprocs: 10,
                sortBy: 'memory',
                descending: true,
                filterBy: '',
                pattern: '',
                exactMatch: false
            });
            document.getElementById('systemOverview').innerHTML = `<pre>${stats}</pre>`;
        } catch (error) {
            console.error('System stats error:', error);
        }
    }

    async function loadSysInfo() {
        const sysInfo = await invoke("get_sysinfo_serialized");

        // Format the information with HTML structure and Graphs
        const infoText = `
            <div style="font-family: Arial, sans-serif; font-size: 14px; line-height: 1.8; padding: 20px; border-radius: 10px; background: #f9f9f9;">
                <h2><i class="fa-solid fa-memory"></i> Memory Usage</h2>
                <div style="display: flex; justify-content: space-between; margin-bottom: 20px;">
                    <div style="width: 48%;">
                        <strong>Total RAM:</strong> ${(sysInfo.total_ram / 1024).toFixed(2)} MB<br>
                        <strong>Free RAM:</strong> ${(sysInfo.free_ram / 1024).toFixed(2)} MB<br>
                        <strong>Swap Memory:</strong> ${(sysInfo.total_swap / 1024).toFixed(2)} MB
                    </div>
                    <div style="width: 48%;">
                        <canvas id="ramUsageChart" height="150"></canvas>
                    </div>
                </div>
                <div style="display: flex; justify-content: space-between; margin-bottom: 20px;">
                    <div style="width: 48%;">
                        <strong>Shared RAM:</strong> ${(sysInfo.shared_ram / 1024).toFixed(2)} MB<br>
                        <strong>Buffered RAM:</strong> ${(sysInfo.buffer_ram / 1024).toFixed(2)} MB<br>
                        <strong>Uptime:</strong> ${sysInfo.uptime} seconds
                    </div>
                    <div style="width: 48%;">
                        <canvas id="loadAveragesChart" height="150"></canvas>
                    </div>
                </div>
                <div style="text-align: center; font-size: 16px;">
                    <strong>Load Averages:</strong><br> 
                    1m: ${sysInfo.load_averages[0]}, 5m: ${sysInfo.load_averages[1]}, 15m: ${sysInfo.load_averages[2]}
                </div>
            </div>
        `;

        // Insert the text and graphs into the container
        document.getElementById("memoryUsageCard").innerHTML = infoText;

        // Create the RAM usage chart (Pie chart example)
        new Chart(document.getElementById("ramUsageChart"), {
            type: 'pie',
            data: {
                labels: ['Free RAM', 'Used RAM'],
                datasets: [{
                    data: [
                        sysInfo.free_ram / sysInfo.total_ram * 100, 
                        (sysInfo.total_ram - sysInfo.free_ram) / sysInfo.total_ram * 100
                    ],
                    backgroundColor: ['#4CAF50', '#FF5733'],
                }]
            },
            options: {
                responsive: true,
                plugins: {
                    legend: {
                        position: 'top',
                    },
                    tooltip: {
                        callbacks: {
                            label: function(tooltipItem) {
                                return tooltipItem.label + ': ' + tooltipItem.raw.toFixed(2) + '%';
                            }
                        }
                    }
                }
            }
        });

        // Update the load averages over time (Line Chart)
        const currentTime = new Date().getTime(); // Get the current timestamp

        // Check if the global chartData variable exists, if not initialize it
        if (!window.chartData) {
            window.chartData = {
                labels: [], // Time (x-values)
                datasets: [{
                    label: 'Load Averages (1m)',
                    data: [],
                    borderColor: '#FF5733',
                    fill: false,
                    tension: 0.1
                }, {
                    label: 'Load Averages (5m)',
                    data: [],
                    borderColor: '#4CAF50',
                    fill: false,
                    tension: 0.1
                }, {
                    label: 'Load Averages (15m)',
                    data: [],
                    borderColor: '#2196F3',
                    fill: false,
                    tension: 0.1
                }]
            };
        }

        // Add the new data to the load averages chart
        window.chartData.labels.push(currentTime);
        window.chartData.datasets[0].data.push(sysInfo.load_averages[0]);
        window.chartData.datasets[1].data.push(sysInfo.load_averages[1]);
        window.chartData.datasets[2].data.push(sysInfo.load_averages[2]);

        // Limit the number of data points to avoid excessive data on the chart (Optional: Keep the last 20 points)
        if (window.chartData.labels.length > 20) {
            window.chartData.labels.shift(); // Remove the oldest time label
            window.chartData.datasets.forEach(dataset => dataset.data.shift()); // Remove the oldest data
        }       
    }
    
    

    async function updateCharts() {
        try {
            const cpuUsage = await invoke('get_cpu_usage');
            
            // CPU Core Usage
            const cpuCoresDiv = document.getElementById('cpuCores');
            cpuCoresDiv.innerHTML = cpuUsage.map((usage, index) => 
                `<div class="cpu-core">Core ${index}: ${usage.toFixed(2)}%</div>`
            ).join('');

            // CPU Chart
            if (!cpuChart) {
                const ctx = document.getElementById('cpuChart').getContext('2d');
                cpuChart = new Chart(ctx, {
                    type: 'line',
                    data: {
                        labels: cpuUsage.map((_, i) => `Core ${i}`),
                        datasets: [{
                            label: 'CPU Usage',
                            data: cpuUsage,
                            borderColor: 'rgb(75, 192, 192)',
                            tension: 0.1
                        }]
                    }
                });
            } else {
                cpuChart.data.datasets[0].data = cpuUsage;
                cpuChart.update();
            }
        } catch (error) {
            console.error('CPU usage error:', error);
        }
    }

    async function loadProcesses() {

        try {
            let processes = await invoke('read_processes');
            const processList = document.getElementById('processList');
            const sortBy = document.getElementById('sortBy').value;
            const filter = document.getElementById('filterProcess').value;
            const filter_by = document.getElementById('filterBy').value;  
            const is_ascending = document.getElementById('is_ascending').checked;           
            if (sortBy) {
                processes.sort((a, b) => {
                    if (sortBy === 'pid') {
                        return a.pid - b.pid;
                    } else if (sortBy === 'name') {
                        return a.name.localeCompare(b.name);
                    } else if (sortBy === 'memory') {
                        return b.memory - a.memory;
                    } else if (sortBy === 'priority') {
                        return b.priority - a.priority;
                    }
                    return is_ascending === 'asc' ? result : -result;
                });
            }
            
            if (filter) {
                console.log(filter);
                if (filter_by === 'name') {
                    processes = processes.filter(p => p.name.toLowerCase().includes(filter.toLowerCase()));
                } else if (filter_by === 'user') {
                    processes = processes.filter(p => p.user.toLowerCase().includes(filter.toLowerCase()));
                } else if (filter_by === 'pid') {
                    processes = processes.filter(p => p.pid === parseInt(filter));
                } else if (filter_by === 'ppid') {
                    processes = processes.filter(p => p.ppid === parseInt(filter));
                } else if (filter_by === 'state') {
                    processes = processes.filter(p => p.state.toLowerCase().includes(filter.toLowerCase()));
                }
            }

            processList.innerHTML = processes.map(p => `
                <tr>
                    <td>${p.pid}</td>
                    <td>${p.ppid}</td>
                    <td>${p.name}</td>
                    <td>${p.state}</td>
                    <td>${(p.memory / 1000).toFixed(2)}</td>
                    <td>${p.thread_count}</td>
                    <td>${(p.virtual_memory / 1000).toFixed(2)}</td>
                    <td>${(p.user_time / 1000).toFixed(2)}</td>
                    <td>${(p.system_time / 1000).toFixed(2)}</td>
                    <td>${p.priority}</td>
                    <td>
    <select  onchange="handleProcessAction(event, ${p.pid})">
        <option value="">Select Action</option>
        <option value="bind">Bind CPU</option>
        <option value="kill">Kill Process</option>
    </select>
</td>
                </tr>
            `).join('');
        } catch (error) {
            console.error('Process load error:', error);
        }
    }

    function handleProcessAction(event, pid) {
        const action = event.target.value; // Get selected value

        if (action === "bind") {
            promptBindToCPU(pid); // Call bind CPU function
        } else if (action === "kill") {
            killProcess(pid); // Call kill process function
        }

        // Reset dropdown to default after action
        event.target.value = "";
    }
    function promptBindToCPU(pid) {
        const cpuInput = prompt(`Enter CPU IDs to bind to process ${pid} (comma-separated):`);
        if (cpuInput) {
            const cpuIDs = cpuInput.split(',').map(id => parseInt(id.trim()));
            try {
    invoke('bind_to_cpu_set', { pid, cpuIds: cpuIDs });
    alert('Process bound successfully to CPUs: ' + cpuIDs.join(', '));
} catch (error) {
    console.error('CPU binding error:', error);
    alert('Failed to bind process: ' + error);
}
            // bindToCPU(pid, cpuIDs);
        }
    }

    async function bindToCPU() {
const pid = parseInt(document.getElementById('bindPid').value);
const cpus = document.getElementById('bindCpus').value
    .split(',')
    .map(cpu => parseInt(cpu.trim()));

try {
    await invoke('bind_to_cpu_set', { pid, cpuIds: cpus });
    alert('Process bound successfully to CPUs: ' + cpus.join(', '));
} catch (error) {
    console.error('CPU binding error:', error);
    alert('Failed to bind process: ' + error);
}
}
    async function killProcess(pid) {
        try {
            await invoke('kill_process', { pid, signal: 15 });
            loadProcesses();
        } catch (error) {
            console.error('Kill process error:', error);
        }
    }

    // Initial load
    loadSystemStats();
    loadProcesses();
    
    // Update data periodically
    setInterval(updateCharts, 2000);
    setInterval(loadProcesses, 2000);
    setInterval(loadSystemStats, 2000);
    // setInterval(loadSysInfo, 2000);