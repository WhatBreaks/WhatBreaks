# WhatBreaks Installation & Setup Guide

This guide details the complete process for setting up WhatBreaks (WB) on a fresh machine (macOS or Linux). It covers prerequisites, starting required infrastructure services, launching the full stack with the repository's manager script, manual startup instructions, and health verification.

---

## 1. System Prerequisites

Verify that the following tools are installed and present on your `$PATH` before starting:

| Tool | Recommended Version | Required For | Verification Command | Installation Guide |
| :--- | :--- | :--- | :--- | :--- |
| **Go** | `1.22+` | Platform API, Collector, & Probes | `go version` | [golang.org/dl](https://go.dev/dl/) |
| **Rust & Cargo** | `1.75+` | Core Graph Engine | `cargo --version` | [rustup.rs](https://rustup.rs/) |
| **Node.js & npm** | `Node 18+` (20+ or 22+ recommended) | Frontend Dashboard | `node -v && npm -v` | [nodejs.org](https://nodejs.org/) |
| **Docker & Compose** | Docker Desktop 4.25+ / Docker Engine 24+ | PostgreSQL Container | `docker compose version` | [docker.com](https://www.docker.com/products/docker-desktop/) |
| **kubectl** | `v1.28+` | Kubernetes API Proxy | `kubectl version --client` | [kubernetes.io](https://kubernetes.io/docs/tasks/tools/) |
| **Netcat (`nc`)** | Standard BSD or GNU | TCP port readiness checks | `which nc` | Preinstalled on macOS & Linux |
| **k3d** *(optional)* | `v5.6+` | Local testing cluster | `k3d version` | [k3d.io](https://k3d.io/) |

### Quick Package Manager Installation

**macOS (Homebrew):**
```bash
brew install go rust node kubectl docker netcat
```

**Ubuntu / Debian Linux:**
```bash
sudo apt-get update
sudo apt-get install -y golang-go cargo nodejs npm netcat-openbsd kubectl
```

---

## 2. Cloning the Repository

Clone the WhatBreaks repository:

```bash
git clone https://github.com/WhatBreaks/WhatBreaks.git
cd WhatBreaks
```

---

## 3. Architecture & Port Allocations

Before starting services, ensure the following local TCP ports are free:

| Port | Service | Process Name | Default Binding |
| :--- | :--- | :--- | :--- |
| **`5432`** | PostgreSQL 16 Store | `postgres` (Docker or local service) | `127.0.0.1:5432` |
| **`50051`** | Rust Core Engine (gRPC) | `wb-core-server` | `127.0.0.1:50051` |
| **`8001`** | Kubernetes API Proxy | `kubectl proxy` | `127.0.0.1:8001` |
| **`8080`** | Go Platform API | `wb` | `0.0.0.0:8080` |
| **`8081`** | Go Health Probe Server | `wb` | `0.0.0.0:8081` |
| **`3000`** | Frontend Dashboard | Vite Dev Server (`npm run dev`) | `0.0.0.0:3000` |

---

## 4. Quick Start via Stack Manager Script (Recommended)

WhatBreaks provides an automated stack manager script at [`scripts/wb.sh`](../scripts/wb.sh). It manages the entire local developer stack with process tracking, safety invariants, and automatic health checks.

### Start the Stack
```bash
./scripts/wb.sh start
```

### What `scripts/wb.sh start` does:
1. **Tool Prerequisites**: Validates presence of `go`, `cargo`, `node`, `npm`, `docker`, `kubectl`, and `nc`.
2. **Docker Daemon**: Checks if Docker is running. On macOS, if Docker Desktop is installed but stopped, it attempts to launch Docker Desktop and waits with a bounded timeout.
3. **PostgreSQL**: Reuses any healthy instance listening on port 5432, or starts the PostgreSQL 16 container via `docker compose up -d postgres`. Data is stored in the persistent named volume `whatbreaks_postgres_data`.
4. **Rust Core Engine**: Compiles `wb-core-server` if not yet built (`cargo build -p wb-core-server`) and starts the gRPC server on port `50051`, logging to `.run/core-engine.log`.
5. **Kubernetes API Proxy**: Inspects your current kubeconfig context. If accessible, launches an authenticated proxy (`kubectl proxy --port=8001`) in the background, logging to `.run/kubectl-proxy.log`.
6. **Go Platform API**: Compiles `cmd/wb` into `bin/wb` and launches the server with `--api-port=8080` and `--health-port=8081`, logging to `.run/go-api.log`. It verifies DB connectivity and Core Engine gRPC liveness.
7. **Frontend Dashboard**: Starts Vite on port `3000` (`npm --prefix web run dev`), logging to `.run/frontend.log`.
8. **Initial Discovery Sweep**: Triggers an initial discovery sweep (`POST /api/v1/discovery/sync`) and reports the number of discovered resources.

### Open the Application
Open your browser and navigate to:
```text
http://localhost:3000
```

---

## 5. Verifying Service Health

### CLI Status Check
Check process status, port bindings, and ownership:
```bash
./scripts/wb.sh status
```

Expected output:
```text
================================================================================
                           WhatBreaks Stack Status                              
================================================================================
Component                Port     PID (Owner)  Status         URL / Endpoint
--------------------------------------------------------------------------------
PostgreSQL               5432     ...          HEALTHY        localhost:5432
Rust Core Engine         50051    ... (managed) HEALTHY        127.0.0.1:50051
Kubernetes Proxy         8001     ... (managed) READY          http://127.0.0.1:8001/readyz
Go Platform API          8080     ... (managed) HEALTHY        http://127.0.0.1:8080
Go Health Server         8081     ...          READY          http://127.0.0.1:8081/readyz
Frontend Dashboard       3000     ... (managed) HEALTHY        http://localhost:3000
================================================================================
```

### CLI Diagnostic Doctor
Run complete environment and connectivity diagnostics:
```bash
./scripts/wb.sh doctor
```

### HTTP Endpoints Verification
You can also curl the individual health and telemetry endpoints directly:

```bash
# 1. Go Platform API Health
curl -s http://127.0.0.1:8080/health | jq .

# 2. Detailed Component Telemetry
curl -s http://127.0.0.1:8080/api/v1/system/health | jq .

# 3. Kubernetes Controller Readiness Probe
curl -s http://127.0.0.1:8081/readyz | jq .

# 4. Kubernetes Controller Liveness Probe
curl -s http://127.0.0.1:8081/healthz | jq .

# 5. Automated System Diagnostics
curl -s "http://127.0.0.1:8080/api/v1/system/diagnose?workspace_id=00000000-0000-0000-0000-000000000001" | jq .
```

---

## 6. Manual Startup (Step-by-Step Alternative)

If you prefer to run stack components individually in separate terminal windows:

### Step 1: Start PostgreSQL
```bash
docker compose up -d postgres
```
Verify port 5432 is open:
```bash
nc -z 127.0.0.1 5432 && echo "PostgreSQL Ready"
```

### Step 2: Build & Start Rust Core Engine
```bash
cargo build -p wb-core-server
./target/debug/wb-core-server
```
The server will log:
```text
Starting WhatBreaks Core gRPC Discovery Server on 127.0.0.1:50051
```

### Step 3: Start Authenticated Kubernetes Proxy
In a new terminal:
```bash
kubectl proxy --port=8001
```
Verify proxy readiness:
```bash
curl -s http://127.0.0.1:8001/readyz
```

### Step 4: Build & Start Go Platform API
In a new terminal:
```bash
go build -o bin/wb ./cmd/wb
env TEST_DB_PASSWORD=testsecret ./bin/wb \
  --api-port=8080 \
  --health-port=8081 \
  --core-engine-addr=127.0.0.1:50051 \
  --k8s-api-url=http://127.0.0.1:8001 \
  --k8s-cluster-id="$(kubectl config current-context 2>/dev/null || echo 'k8s-cluster')"
```

### Step 5: Start Frontend Dashboard
In a new terminal:
```bash
cd web
npm install
npm run dev
```
Access the dashboard at `http://localhost:3000`.

---

## 7. Stopping Services Cleanly

To stop services managed by `scripts/wb.sh`:
```bash
./scripts/wb.sh stop
```

### Process Management Safety Invariants
- `wb.sh stop` tracks PIDs in `.run/*.pid`. It stops **only** the processes created by the script.
- Pre-existing external processes (such as an existing PostgreSQL database or an already-running `kubectl proxy`) are **never** terminated.
- The PostgreSQL database container is preserved to ensure your discovered graph state is not lost across runs.

### Resetting Stored Data (Complete Teardown)
If you wish to delete all persisted graph records and reset PostgreSQL to a clean state:
```bash
docker compose down -v
```
> [!WARNING]
> This destroys the `whatbreaks_postgres_data` Docker volume and deletes all historical resources and evidence.
