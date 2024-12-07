# Tauri System Monitoring App Documentation

## Table of Contents

1. [Introduction](#introduction)
2. [Setup Instructions](#setup-instructions)
   - [Prerequisites](#prerequisites)
   - [Installation](#installation)
3. [Contributing](#contributing)

## Introduction

This Tauri app provides a simple yet powerful system monitoring tool that fetches and displays various system metrics like memory usage and load averages. The app updates periodically, displaying real-time information in a user-friendly interface. It includes graphical representations for RAM usage and load averages over time.

## Setup Instructions

### Prerequisites

Before setting up the Tauri app, ensure the following dependencies are installed:

- **Node.js** (version 14 or higher)
- **Rust** (version 1.56 or higher)
- **Tauri CLI** (installed via npm)
  
You can install the required tools using the following commands:

```bash

sudo apt update

# Install Node.js
sudo apt install nodejs npm

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Tauri CLI
cargo install tauri-cli

# Install other dependancies
sudo apt install libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev
```

### Installation

1. **Clone the repository**:
   
   Clone the project repository to your local machine:
   
   ```bash
   git clone https://github.com/pimyn-girgis/os_project
   git checkout tauri_GUI
   npm install
   ```

3. **Build and run the app**:
   
   To start the Tauri application, run the following command:

   ```bash
   npm run tauri dev
   ```

   This will launch the app with the Tauri backend and frontend running.

## Contributing

We welcome contributions to this project! Feel free to fork the repository, submit pull requests, or report issues.

### Steps to Contribute:

1. Fork the repository.
2. Create a feature branch (`git checkout -b feature-name`).
3. Pull the Tauri GUI branch `git pull origin tauri_GUI`
4. Make your changes and commit them (`git commit -m 'Add new feature'`).
5. Push to your fork (`git push origin feature-name`).
6. Open a pull request to the `tauri_GUI` branch of the original repository.