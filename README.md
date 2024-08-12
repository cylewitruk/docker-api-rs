# stackify-docker-api

❗❗ Forked from [docker-api-rs](https://github.com/vv9k/docker-api-rs) to add support for functions used by [Stackify](https://github.com/cylewitruk/stackify) as the upstream repo appears to be inactive. This fork will only be updated to the extent of features needed by Stackify and should not be generally used.

[![GitHub Actions](https://github.com/cylewitruk/docker-api-rs/workflows/Main/badge.svg)](https://github.com/cylewitruk/docker-api-rs/actions) [![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE) [![Released API docs](https://docs.rs/docker-api/badge.svg)](http://docs.rs/docker-api)

> a rust interface to [Docker](https://www.docker.com/) containers

## Install

Add the following to your `Cargo.toml` file

```toml
[dependencies]
docker-api = "0.14"
```

## Supported API
Default endpoints include:
 - Containers
 - Images
 - Networks
 - Volumes
 - Exec
 - System

To enable swarm endpoints add a `swarm` feature to `Cargo.toml` like so:
```toml
docker-api = { version = "0.14", features = ["swarm"] }
```

Swarm endpoints include:
 - Swarm
 - Nodes
 - Services
 - Tasks
 - Secrets
 - Configs
 - Plugins

Latest stable version of this crate supports API version: **v1.42**
Master branch supports: **v1.43**

## Features

### SSL Connection

To enable HTTPS connection to docker add a `tls` flag to `Cargo.toml`.

### Chrono

To enable chrono DateTime timestamps add a `chrono` feature flag to `Cargo.toml`.

### Default features

By default only `chrono` feature is enabled. To disable it use:
```toml
docker-api = { version = "0.14", default-features = false }
```

## Usage

Examples for most API endpoints can be found in the [examples directory](https://github.com/vv9k/docker-api-rs/tree/master/examples).


## Notice
This crate is a fork of [shiplift](https://github.com/softprops/shiplift).

## License
[MIT](https://raw.githubusercontent.com/vv9k/docker-api-rs/master/LICENSE)
