# PRD 016: OCI ARM Deployment

**Status:** DRAFT — PLACEHOLDER
**Created:** 2026-02-26
**Author:** PRD Writer Agent

> **NOTE: This is a placeholder PRD.** The target Oracle Cloud Infrastructure A1 Flex instance has
> not yet been provisioned. It is being requested via a cron-based provisioning job. Once the
> instance is available, this PRD should be fleshed out with concrete paths, usernames, port
> assignments, TLS certificate strategy, and verified acceptance criteria. All acceptance criteria
> below are marked TBD.

---

## Problem Statement

EventfoldDB currently has no defined production deployment target or operational runbook. The
design doc names Fly.io as a reference deployment but the actual target is an Oracle Cloud
Infrastructure (OCI) Always Free A1 Flex instance (4 ARM cores, 24 GB RAM, 200 GB block volume).
Without a concrete deployment, there is no production binary, no process supervision, no backup
strategy, and no way to operate EventfoldDB outside of a developer laptop.

## Goals

- Produce a reproducible, documented deployment of EventfoldDB on an OCI A1 Flex ARM64 instance.
- Ensure the event log is stored on a persistent OCI block volume that survives instance reboots
  and crashes.
- Configure process supervision (systemd or Docker) so the server restarts automatically after
  failure and integrates with the OS health/watchdog mechanism.
- Define and enforce firewall rules that expose only the required ports (gRPC, metrics, SSH).
- Establish a backup strategy for the append-only event log file.
- Integrate with the TLS, health-check, and metrics PRDs (011, 012, 013) once those land.

## Non-Goals

- Multi-node or clustered deployment. EventfoldDB is single-node by design (see `docs/design.md`).
- Automated blue/green or rolling deployments. Manual deploy via SSH is sufficient for an in-house
  service at this scale.
- Container orchestration (Kubernetes, ECS, Nomad). A single systemd unit or single Docker
  container is the target.
- CI/CD pipeline automation (GitHub Actions, etc.). Out of scope for this PRD; may be addressed
  separately.
- Automated certificate rotation. TLS certificate management is owned by PRD 011.
- Cross-region replication or disaster recovery beyond periodic snapshot backups.
- Monitoring dashboards or alerting rules. Metrics exposure is owned by PRD 013; dashboarding is
  out of scope here.

## User Stories

- As an operator, I want EventfoldDB to start automatically when the OCI instance boots so I do
  not need to SSH in after every restart.
- As an operator, I want the event log file to live on a persistent block volume so that a
  instance termination or recreation does not lose data.
- As an operator, I want the server process to be restarted by the supervisor if it crashes so
  that the service recovers without manual intervention.
- As an operator, I want firewall rules to block all ports except SSH, gRPC (2113), and metrics
  (9090) so that EventfoldDB is not inadvertently exposed to the public internet.
- As an operator, I want periodic backups of the event log file to OCI Object Storage so that I
  can recover from accidental deletion or block volume failure.
- As an operator, I want log output from EventfoldDB to be captured in the system journal (or
  Docker logs) and rotated automatically so that logs do not fill the root filesystem.

## Technical Approach

> **TODO: Flesh this section out once the instance is provisioned and the OS is confirmed.**
>
> Decisions pending:
> - OS choice: Oracle Linux 9 (ARM) vs. Ubuntu 24.04 (ARM). Ubuntu is strongly preferred for
>   familiarity; Oracle Linux is the OCI default Always Free image.
> - Process supervision: systemd unit file vs. Docker with `--restart=unless-stopped`. Systemd is
>   preferred to avoid Docker overhead on a memory-constrained free-tier instance.
> - Binary delivery: cross-compile on dev machine for `aarch64-unknown-linux-gnu` via
>   `cross` + Docker, vs. build on instance using `rustup` + `cargo`. Cross-compile is preferred
>   to keep the instance lean (no Rust toolchain required at runtime).
> - Block volume mount point: `/data` is the design doc default. Confirm with OCI volume
>   attachment.
> - TLS termination: in-process (tonic TLS) vs. a reverse proxy (nginx, caddy). Deferred to
>   PRD 011.

### Affected Files (Preliminary)

| File | Change |
|---|---|
| `Dockerfile` (new or update) | Multi-stage ARM64 build if Docker path is chosen |
| `deploy/eventfold-db.service` (new) | systemd unit file if systemd path is chosen |
| `deploy/backup.sh` (new) | Periodic backup script to OCI Object Storage |
| `deploy/logrotate.conf` (new) | Log rotation config if not using journald |
| `deploy/README.md` (new) | Step-by-step operator runbook |
| `src/main.rs` | Ensure `EVENTFOLD_DATA`, `EVENTFOLD_LISTEN`, `EVENTFOLD_BROKER_CAPACITY` env vars are documented; no code changes expected |

### Build Target

The production binary must target `aarch64-unknown-linux-gnu`. The cross-compilation command
(using the `cross` tool) is expected to be:

```sh
cross build --release --target aarch64-unknown-linux-gnu
```

The resulting binary is at `target/aarch64-unknown-linux-gnu/release/eventfold-db`.

### Systemd Unit (Draft)

```ini
[Unit]
Description=EventfoldDB event store
After=network.target

[Service]
Type=notify
User=eventfold
Group=eventfold
ExecStart=/opt/eventfold-db/eventfold-db
Restart=on-failure
RestartSec=5s
Environment=EVENTFOLD_DATA=/data/eventfold.log
Environment=EVENTFOLD_LISTEN=[::]:2113
Environment=EVENTFOLD_BROKER_CAPACITY=4096
# Systemd watchdog integration (depends on PRD 012)
# WatchdogSec=30s
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

### Block Volume

- OCI block volume attached and formatted as ext4 (required by `docs/design.md` — ext4 with
  `data=ordered` journaling is the only validated filesystem).
- Mounted at `/data` with entry in `/etc/fstab` for persistence across reboots.
- `EVENTFOLD_DATA=/data/eventfold.log` set in the systemd unit or Docker environment.

### Firewall Rules (OCI Security List + OS-level)

| Port | Protocol | Source | Purpose |
|---|---|---|---|
| 22 | TCP | operator IP range | SSH access |
| 2113 | TCP | application subnet | gRPC (EventfoldDB) |
| 9090 | TCP | monitoring subnet | Prometheus metrics (PRD 013) |
| All others | — | any | DENY |

### Backup Strategy (Preliminary)

A cron job or systemd timer runs a backup script that:

1. Copies `/data/eventfold.log` to OCI Object Storage using the OCI CLI or `rclone`.
2. Retains the last N daily snapshots (N TBD, likely 7-30).
3. Logs success/failure to journald.

Because the event log is append-only and the server fsyncs before acknowledging writes, a
file-level copy taken at rest (server stopped) or via OCI block volume snapshot is
crash-consistent. Online hot backups (with the server running) are safe only if the backup tool
reads a point-in-time consistent view; a block volume snapshot from OCI satisfies this.

### Log Rotation

If using systemd + journald: log rotation is handled automatically by `journald` (size and time
limits configurable in `/etc/systemd/journald.conf`). No additional `logrotate` config is needed.

If using Docker: configure `--log-opt max-size=100m --log-opt max-file=5` on the container.

## Acceptance Criteria

> All criteria are **TBD** pending instance provisioning and OS confirmation.

1. TBD: The `eventfold-db` binary cross-compiles for `aarch64-unknown-linux-gnu` with
   `cross build --release --target aarch64-unknown-linux-gnu` and the resulting binary executes
   on the OCI ARM instance without error.

2. TBD: The OCI block volume is mounted at `/data` (or the confirmed mount path), formatted as
   ext4, and the mount entry is present in `/etc/fstab` such that the mount persists after a
   reboot verified by `sudo reboot` + `mount | grep /data`.

3. TBD: The systemd unit (or Docker container) starts EventfoldDB automatically on instance boot,
   confirmed by `systemctl is-active eventfold-db` returning `active` after a clean reboot
   with no manual intervention.

4. TBD: If the `eventfold-db` process is killed with `kill -9 <pid>`, the supervisor restarts it
   within 10 seconds, confirmed by `systemctl status eventfold-db` showing a restart count > 0.

5. TBD: OCI Security List and OS-level firewall rules allow TCP connections on port 2113 from the
   application subnet and block TCP connections on port 2113 from an external IP outside the
   allowed range, confirmed by a `grpc_cli` connection test from an authorized host and a
   refused-connection test from an unauthorized host.

6. TBD: SSH (port 22) is accessible from the operator IP range and blocked from all other public
   IPs, verified by a successful SSH login from the authorized IP and a connection timeout from
   an unauthorized IP.

7. TBD: EventfoldDB process logs are captured in the system journal and a `journalctl -u
   eventfold-db` command returns structured log lines from the server's `tracing` output.

8. TBD: The backup script copies `/data/eventfold.log` to OCI Object Storage successfully,
   confirmed by a manual run that exits 0 and the object appearing in the OCI console within
   60 seconds.

9. TBD: After a simulated restore (copy the backed-up log file to a fresh `/data/eventfold.log`
   on the instance, restart the server), `ReadAll` returns the same events that were present
   before the backup was taken, in the same global position order.

10. TBD: The gRPC port (2113) is reachable from the application client and responds to a
    `ReadAll` RPC with a valid (possibly empty) response within 2 seconds over the OCI internal
    network.

## Open Questions

- **OS**: Oracle Linux 9 vs. Ubuntu 24.04 ARM64? The OCI Always Free default is Oracle Linux.
  Ubuntu is preferred for tooling familiarity. Confirm after provisioning.
- **Supervision**: systemd unit vs. Docker? Systemd is preferred but Docker simplifies
  cross-architecture image builds. Decide based on available memory after provisioning (Docker
  daemon overhead matters on a shared free-tier instance).
- **Cross-compilation toolchain**: Use `cross` (Docker-based) on the dev machine vs. install
  `rustup` on the instance and build there. Cross-compile avoids Rust on the instance but
  requires Docker on the build machine.
- **TLS termination point**: In-process via tonic TLS (PRD 011) vs. nginx/caddy reverse proxy?
  In-process is simpler; a proxy adds a hop. Decide when PRD 011 is scoped.
- **Block volume snapshot frequency**: OCI block volume snapshots (managed by OCI) vs.
  file-level copies to Object Storage. Snapshots are crash-consistent and easy to automate;
  object storage copies are portable. May use both.
- **Backup retention policy**: How many days of backups to retain? 7 days is a reasonable
  default; confirm with operator requirements.
- **Metrics port exposure**: Port 9090 — should it be internal-only (OCI VCN subnet) or exposed
  to a specific external monitoring IP? Depends on where Prometheus runs (PRD 013 scope).
- **Watchdog interval**: systemd `WatchdogSec` value. Requires PRD 012 (health check) to be
  defined first so the process knows what to probe and how quickly.

## Dependencies

- **PRD 011** (TLS certificates): TLS must be configured before the gRPC port is exposed to
  the public internet. Deployment can proceed on the internal VCN without TLS, but external
  access requires PRD 011.
- **PRD 012** (Health check / systemd watchdog): The systemd unit's `WatchdogSec` directive and
  `Type=notify` rely on the process sending `sd_notify` pings. This requires a health-check
  implementation in `src/main.rs` (PRD 012 scope). The unit file above has `WatchdogSec`
  commented out until PRD 012 lands.
- **PRD 013** (Metrics endpoint): Port 9090 firewall rule and the metrics scrape path depend on
  the Prometheus metrics exporter defined in PRD 013.
- **PRDs 001-009**: A complete, tested EventfoldDB binary is a prerequisite. All prior PRDs must
  be implemented and passing CI before deployment is meaningful.
- **OCI instance**: The actual A1 Flex instance must be provisioned before any acceptance
  criteria can be verified.
- **OCI block volume**: A 200 GB block volume must be attached to the instance and formatted
  before `EVENTFOLD_DATA` can point to a durable path.
- **OCI Object Storage bucket**: Required for the backup strategy (PRD 013 dependency or
  standalone bucket provisioning).
- **`cross` crate** (dev dependency, build tooling): Required for `aarch64-unknown-linux-gnu`
  cross-compilation from an x86_64 dev machine. Not a `Cargo.toml` dependency — installed
  separately via `cargo install cross`.
