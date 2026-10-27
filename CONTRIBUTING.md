# Contributing to WhatBreaks

Welcome, and thank you for your interest in contributing to WhatBreaks (WB)!

WhatBreaks is an open-source, deterministic change-impact analysis and topology exploration engine for modern infrastructure. It helps platform engineers, SREs, and DevOps practitioners understand what dependencies could be affected before an operator deletes, updates, scales, or replaces a resource.

Whether you are participating through **NSOC (Nexus Spring of Code.)**, contributing to open source for the first time, or an experienced cloud-native developer, we welcome your contributions.

---

## 1. Project Vision & Architecture

### What WhatBreaks Does
Modern cloud-native infrastructure is deeply interconnected through service selectors, ingress routing, volumes, and runtime references. WhatBreaks collects read-only infrastructure observations, builds an immutable factual evidence graph, and traverses directed dependencies to evaluate the blast radius of proposed operational changes—**before** changes reach production.

### Current Scope & Long-Term Direction
- **Version 1 (Current)**: Focuses on Kubernetes workloads, networking, routing, configuration, and storage across single- and multi-cluster topologies.
- **Future Direction**: WhatBreaks is designed to be cloud- and provider-agnostic. Planned expansions include cloud infrastructure providers (AWS VPC/IAM/RDS, GCP Cloud SQL), Infrastructure-as-Code pre-merge pull request validation (Terraform/Helm), real-time Kubernetes Watch event streaming, and multi-collector concurrent ingestion.

### Core Technical Components
- **Go Platform & Collector (`cmd/wb`, `internal/`)**: Go powers the REST API, lifecycle orchestrator, health probes, and the strictly read-only Kubernetes collector and normalizer.
- **Rust Core Engine (`core-engine/`, `core-engine-server/`)**: Rust powers the high-performance in-memory graph engine, deterministic path traversal, change assessment, and gRPC service.
- **PostgreSQL 16**: Acts as the durable system of record for normalized evidence and semantic relationships, partitioned by tenant workspace UUID.
- **TypeScript / React Dashboard (`web/`)**: React 19 and Vite power the interactive topology visualizer and impact investigation UI.

---

## 2. Ways to Contribute

We welcome many forms of contribution:

1. **Code**: Adding new relationship discovery rules, improving API endpoints, extending collector capabilities, or enhancing the frontend graph interface.
2. **Bug Fixes**: Diagnosing and fixing bugs, edge cases, and crash scenarios.
3. **Tests**: Adding unit tests, concurrency race tests, or integration test scenarios across Go, Rust, or TypeScript.
4. **Documentation**: Improving setup guides, explaining architectural details, fixing typos, or authoring walkthroughs.
5. **Feature Proposals**: Proposing new provider normalizers, propagation heuristics, or analysis capabilities through GitHub Issues.
6. **Code Reviews & Issue Triage**: Helping review incoming pull requests, reproducing reported issues, and assisting fellow community members.

---

## 3. Finding Issues & Getting Started

1. **Explore the Issue Tracker**: Visit [GitHub Issues](https://github.com/WhatBreaks/WhatBreaks/issues) to see ongoing work.
2. **Beginner-Friendly Issues**: Check issues tagged with [`good first issue`](https://github.com/WhatBreaks/WhatBreaks/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22). These tasks have a well-defined scope and are ideal for newcomers.
3. **Help Wanted Tasks**: Check issues tagged with [`help wanted`](https://github.com/WhatBreaks/WhatBreaks/issues?q=is%3Aissue+is%3Aopen+label%3A%22help+wanted%22) for community priorities.
4. **Discussing Larger Changes**: For non-trivial features, architectural modifications, or schema changes, **please open an issue to discuss your proposal first** before spending time writing code. This ensures alignment with project roadmap and saves you rework.
5. **Where to Ask Questions**: If you are unsure where to start or need clarification on an issue, leave a comment directly on the relevant GitHub issue. Maintainers and community members will respond as capacity permits.

---

## 4. Local Development Environment Setup

### Prerequisites
Ensure your local development workstation has the following tools installed:
- **Go**: `1.22+` (`go version`)
- **Rust & Cargo**: `1.75+` (`cargo --version`)
- **Node.js & npm**: Node `18+` and npm (`node -v && npm -v`)
- **Docker & Docker Compose**: For running the local PostgreSQL container (`docker compose version`)
- **kubectl**: `v1.28+` (`kubectl version --client`)
- **netcat (`nc`)**: Preinstalled on macOS and Linux

### Step 1: Fork and Clone
Fork the repository on GitHub, then clone your fork locally:
```bash
git clone https://github.com/<your-username>/WhatBreaks.git
cd WhatBreaks

# Add upstream remote
git remote add upstream https://github.com/WhatBreaks/WhatBreaks.git
git fetch upstream
```

### Step 2: Launch the Development Stack
WhatBreaks provides an automated local stack management script [`scripts/wb.sh`](scripts/wb.sh):
```bash
# Start all services (PostgreSQL, Rust Core Engine, Go API, kubectl proxy, and Web UI)
./scripts/wb.sh start
```

### Step 3: Verify Service Health
Check process status and health probes:
```bash
# Check running processes and port bindings
./scripts/wb.sh status

# Run complete connectivity diagnostics
./scripts/wb.sh doctor
```
Once started, the web dashboard is accessible at `http://localhost:3000`.

### Step 4: Stop the Stack Cleanly
When you are done developing, stop the local processes cleanly:
```bash
./scripts/wb.sh stop
```
`wb.sh stop` terminates only the background processes tracked in `.run/*.pid`, preserving your PostgreSQL container volume data.

---

## 5. Building, Linting, & Testing Components

Always verify that your changes compile cleanly and pass existing verification suites before submitting a pull request.

### A. The Unified Test Suite (Recommended)
Run the project's automated verification script from repository root:
```bash
./scripts/test.sh
```
This script checks Go unit tests, Go concurrency race detector, Rust workspace tests, frontend linting, frontend production build, and git whitespace.

### B. Go Subsystem (`cmd/wb`, `internal/`)
- **Build the Go CLI binary**:
  ```bash
  go build -trimpath -o bin/wb ./cmd/wb
  ./bin/wb --version
  ```
- **Run Go unit & integration tests**:
  ```bash
  go test -count=1 ./...
  ```
- **Run the Go concurrency race detector**:
  ```bash
  go test -race -count=1 ./internal/api ./internal/answer ./internal/state
  ```
- **Formatting**: Format modified Go files with `gofmt -w <filename>` before committing.

### C. Rust Subsystem (`core-engine/`, `core-engine-server/`)
- **Check Rust crates**:
  ```bash
  cargo check --workspace
  ```
- **Run Rust test suite**:
  ```bash
  cargo test --workspace
  ```
- **Build the Rust gRPC server**:
  ```bash
  cargo build -p wb-core-server
  ```
  The binary is built at `./target/debug/wb-core-server`.
- **Formatting**: Format modified Rust code with `cargo fmt --all`.

### D. Frontend Subsystem (`web/`)
In the `web/` directory:
```bash
cd web

# Install dependencies
npm install

# Run the Oxlint linter
npm run lint

# Run frontend tests
npm test

# Build production bundle (tsc -b && vite build)
npm run build
```

### E. Git Whitespace Verification
Verify that your changes introduce no trailing whitespace or merge conflict markers:
```bash
git diff --check
```

### F. Updating Protobuf Contracts
The Go ↔ Rust gRPC protocol definition is located in [`proto/wb/core/v1/core.proto`](proto/wb/core/v1/core.proto).
If you modify `core.proto`:
- **Regenerate Go code**:
  ```bash
  ./proto/gen.sh
  ```
  (Updates `gen/go/wb/core/v1/`).
- **Regenerate Rust code**:
  ```bash
  cargo build -p wb-core-proto
  ```
  (Handled automatically by `core-engine-proto/build.rs`).

---

## 6. Real Kubernetes Acceptance Testing (Safety Guards)

WhatBreaks includes optional end-to-end acceptance tests that run against a real Kubernetes cluster ([`test/acceptance/k8s_e2e_test.go`](test/acceptance/k8s_e2e_test.go)).

> [!CAUTION]
> **Strict Safety Guard**:
> To guarantee that real acceptance tests never modify existing developer or production clusters:
> - Acceptance tests will **NEVER** run against existing clusters (such as `k3d-license-service`).
> - Acceptance tests strictly require an explicit opt-in flag (`--with-k8s`) and a dedicated kubeconfig context named `whatbreaks-e2e`.

To execute real acceptance tests safely using `k3d`:
```bash
# 1. Create a dedicated throwaway test cluster
k3d cluster create whatbreaks-e2e --wait

# 2. Run acceptance tests with explicit opt-in flag
./scripts/test.sh --with-k8s

# 3. Clean up the throwaway cluster
k3d cluster delete whatbreaks-e2e
```

---

## 7. Pull Request Expectations

To ensure efficient reviews, please follow these guidelines when opening a Pull Request (PR):

1. **Create a Dedicated Branch**:
   Create a descriptive branch off `main`:
   ```bash
   git checkout -b feature/describe-your-feature
   # or
   git checkout -b fix/describe-the-bug
   ```
2. **Keep PRs Focused**:
   Submit small, atomic PRs that address a single concern. Avoid bundling unrelated formatting, refactoring, and feature changes together.
3. **Include Tests**:
   Add unit or integration tests verifying your changes and preventing regressions.
4. **Update Documentation**:
   If your change modifies flags, configuration, API endpoints, or user workflows, update the corresponding documentation files in `docs/` and `README.md`.
5. **Write Clear Commit Messages**:
   Use concise, descriptive commit messages in the imperative mood (e.g., `Add Ingress-to-Service port matching normalizer`).
6. **PR Title and Description**:
   Provide a clear PR title and description explaining:
   - What problem this PR solves.
   - How the change was implemented.
   - How you tested the change locally (including test command output).
7. **Address Review Feedback**:
   Participate actively in code review discussions. If changes are requested, push new commits to your branch. Maintainers will review as capacity permits.

---

## 8. Reporting Bugs

If you encounter unexpected behavior or bugs:

1. **Search Existing Issues**: Check [GitHub Issues](https://github.com/WhatBreaks/WhatBreaks/issues) to ensure the problem has not already been reported.
2. **Open a Detailed Bug Report**: Include:
   - A clear, descriptive issue title.
   - Step-by-step reproduction instructions.
   - Expected behavior vs. actual behavior.
   - Environment details: Operating System, Go version, Rust version, Node version, Docker version, and Kubernetes distribution (e.g., k3d, kind, EKS).
   - Sanitized terminal or component logs.
3. **Sanitize Your Logs**:
   Never include real passwords, API tokens, internal server URLs, private keys, or sensitive customer information in issue descriptions. Replace sensitive values with `<REDACTED>`.

---

## 9. Feature Requests

We welcome ideas for improving WhatBreaks:

1. **Open an Issue**: Use GitHub Issues to submit feature proposals.
2. **Explain the Problem**: Describe the real-world scenario or operational bottleneck you are experiencing.
3. **Describe the Proposed Solution**: Outline the expected workflow, API changes, or heuristics.
4. **Respect Project Safety Invariants**:
   - WhatBreaks is strictly **read-only** against monitored infrastructure; it never mutates or remediates workloads.
   - WhatBreaks adheres to **Zero Secret Custody**; it never reads, stores, or logs secret payload data.
   - Tenant isolation must be preserved across multi-workspace boundaries.

---

## 10. Security & Responsible Disclosure

If you discover a potential security vulnerability, **do not open a public GitHub issue**. Please follow our responsible disclosure guidelines outlined in [`SECURITY.md`](SECURITY.md).

---

## 11. Code of Conduct

All contributors, maintainers, and community participants are expected to adhere to our [Code of Conduct](CODE_OF_CONDUCT.md). Please treat all members of the community with empathy, kindness, and professional respect.

---

Thank you for helping make infrastructure changes safer for everyone!
