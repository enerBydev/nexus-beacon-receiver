# NEXUS Beacon Receiver

NEXUS Beacon Receiver is a Cloudflare Worker that receives daily telemetry beacons from NEXUS AI Gateway instances, stores them in a D1 database, and serves global statistics about usage patterns.

## What It Does

The NEXUS Beacon Receiver collects anonymized usage statistics from NEXUS AI Gateway deployments worldwide. Each day, NEXUS instances send telemetry beacons containing non-identifiable usage data which is then aggregated and made available through public APIs for transparency and community insight.

## Architecture

The receiver is built as a Cloudflare Worker using Rust and compiled to WASM. It uses D1 (Cloudflare's serverless SQL database) for storage and provides RESTful endpoints for data ingestion and retrieval.

For detailed architecture information, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/beacon` | POST | Receive telemetry beacon from NEXUS instance |
| `/v1/stats` | GET | Retrieve detailed statistics |
| `/v1/stats/summary` | GET | Retrieve summary statistics |

## Tech Stack

- **Language**: Rust
- **Runtime**: Cloudflare Workers (WASM)
- **Framework**: workers-rs
- **Database**: Cloudflare D1 (SQLite)
- **Deployment**: Wrangler CLI
- **Build System**: Taskfile

## Quick Start

### Prerequisites

- Rust and Cargo installed
- Wrangler CLI (`npm install -g wrangler`)
- Cloudflare account with D1 beta access

### Setup

```bash
# Clone the repository
git clone https://github.com/enerBydev/nexus-beacon-receiver.git
cd nexus-beacon-receiver

# Setup git hooks
task setup-hooks

# Run tests
task test
```

### Development

```bash
# Start development server
task dev

# Format code
task fmt

# Lint code
task lint

# Run all checks
task check
```

### Deployment

```bash
# Deploy to Cloudflare
task deploy

# View logs
task tail
```

## Project Structure

```
nexus-beacon-receiver/
├── src/                 # Rust source code
├── docs/                # Documentation
├── scripts/             # Helper scripts
├── Taskfile.yaml        # Task runner configuration
├── wrangler.toml        # Cloudflare configuration
├── Cargo.toml           # Rust dependencies
├── VERSION              # Version file
├── CHANGELOG.md         # Release history
└── README.md           # This file
```

## Configuration

### Environment Variables

| Variable | Description | Required |
|----------|-------------|----------|
| `BEACON_AUTH_TOKEN` | Authentication token for beacon submission | Yes |
| `D1_DATABASE_ID` | D1 database identifier | Yes |

### Secrets

Secrets are managed through Cloudflare's secret store:

```bash
# Set beacon authentication token
task secret-set
```

## Version Management

This project uses a 2-file version synchronization system:

- `VERSION` file contains the single source of truth for the version
- `Cargo.toml` contains the version for Rust package management

Run `task version-check` to verify synchronization.

## CI/CD

The project uses GitHub Actions for continuous integration and deployment. The workflow includes:

- Code formatting and linting checks
- Unit and integration tests
- Automatic version bumping based on conventional commits
- Deployment to Cloudflare Workers

For detailed CI/CD information, see [docs/CI-CD.md](docs/CI-CD.md).

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Author

enerBydev