# Quire on a VPS, reached over the internet

> **Status:** idea — discussed 29 Oct 2026, not started
> **Goal:** the web server (`quire-server`) running on the user's own VPS, opened in a browser from anywhere over HTTPS.

---

## Summary

Possible today: the server builds without a window system and has per-person workspaces, sealed secrets
(`QUIRE_SECRET_KEY`), the data fence and Python off by default ([web-server-mode.md](web-server-mode.md)). What's
missing is the setup around it (HTTPS, a service, a firewall), a guard against the demo accounts, and a first run on
Linux — it has only run on macOS, and the `Dockerfile` has never been built.

---

## Must happen before it is reachable

1. **Demo passwords.** `admin`, `data`, `design` and `user` all sign in with `quire`, and the sign-in screen offers them
   as buttons. Change every password or delete the demo accounts first. Planned safeguard: the server refuses demo
   passwords and hides the demo buttons whenever it doesn't listen on `127.0.0.1`.
2. **HTTPS.** Quire speaks plain HTTP. Put **Caddy** in front (it gets a Let's Encrypt certificate by itself); Quire
   listens on `127.0.0.1:8766` only and runs with `--secure`. Needs a **domain name** pointing at the VPS.
3. **Firewall.** Open only 22 (SSH), 80 and 443; 8766 stays closed.
4. **Data.** The demo bank database is invented data — fine. No real client data on a public VPS without the bank's
   sign-off.

## Linux: not tried yet

- Build on the VPS needs Rust and a C++ compiler (DuckDB compiles from source, ~5 min) and about **4 GB RAM** while
  building — on a smaller VPS add swap, or build the Docker image elsewhere and copy it over.
- Expect a short round of fixes the first time (fonts, finding `curl` and Python).
- **Capacity:** about 3 CPU-seconds per person opening a report (TESTING.md § 6) — a 2-core VPS serves a couple of
  people opening reports at the same moment; fine for a demo.

## Steps (Debian / Ubuntu)

```sh
# build
sudo apt install build-essential curl git ca-certificates
curl https://sh.rustup.rs -sSf | sh
cargo build --release --no-default-features --bin quire-server

# run (a systemd service in practice)
QUIRE_SECRET_KEY='<long passphrase>' QUIRE_UI=/opt/quire/ui \
  ./target/release/quire-server --host 127.0.0.1 --port 8766 --data /var/lib/quire --secure
```

`/etc/caddy/Caddyfile`:

```
quire.example.com {
    reverse_proxy 127.0.0.1:8766
}
```

## To build when it's picked up

- The demo-password safeguard (above).
- `deploy/`: a systemd service, the Caddyfile, and a setup script (build, a `quire` user, the data folder, start).
- A note on the sign-in page when it is served over plain HTTP from an address that isn't local.

## Questions to answer first

1. The VPS: operating system and version, RAM, cores.
2. A domain name for it (needed for HTTPS)?
3. Build on the VPS, or a Docker image built elsewhere?
