# AreaNet

A Peer 2 Peer Network Controller.

This is a CLI application to connect, and be connected, with remote peers.
The purpose of this network is just to explore network peers.


## Getting Started

Follow these instructions to get a copy of the project up and running.
This project is in development, and is intended for testing only.

### Prerequisites

This is a rust project, so you'll need to install it:
- [Getting Started with Rust](https://www.rust-lang.org/learn/get-started)

### Installing

You need to download and build the project, using the following instructions:

```
git clone https://github.com/crocme10/area-net.git
cd area-net
cargo build --release
./target/release/area-net -h
```


## Running the tests

Explain how to run the automated tests for this system

### Usage


#### Configuration

The application is configured using layers where we store key/value settings.
Each setting in a layer overrides the corresponding setting in the layers below.
There are 3 layers:

- at the base level, we have the **default layer**.
- then we have the **profile layer**, which can override some settings for a given profile.
- finally we have the **command-line layer**, where the settings present in the command line
  will override the corresponding one below.

The command line is sparse, and contains essentially these three components:

```
area-net -c [CONFIG DIR] -p [PROFILE] -s [KEY1=VALUE] -s [KEY2=VALUE]
```

* A **config directory**: This is required, and provides the application with
  default configuration values.
* A **profile name**: This is optional, but most likely important to get the 
  correct behavior
* Finally, individual configuration settings can be overriden with the command line.

The configuration directory has the following hierarchy:

```
config/
└- network/
   ├ default.toml
   └ <profile>.toml
```

So, for example, if you want to create a profile for testing, which listens to
port 8901, you would create a file `config/network/testing.toml` with the
following content:

```toml
[network.controller.listen]
port = 8901
```

and run the application with the command

```
area-net -c config -p testing
```

There is one additional configuration file, which contains the list of initial peers the node should connect to. This
file is identified by a configuration setting, `config.network.target.path`, eg, in `config/network/bob.toml`:

```toml
[...]

[network.controller.target]
path = "profile/bob.toml"
```

If the path is relative, it is relative to the current working directory, which is the root of the project. So the application
expects to find a file in `profile/bob.toml` containing an array of addresses serialized in JSON format, eg:

```JSON
[
    "[::1]:8090",
    "[::1]:8095"
]
```

#### Example

The project comes with 4 profiles for testing basic functionalities, bob, alice, carol, and dave.
 ## Built With

  - [Tokio](https://tokio.rs/) - This project relies mainly on Tokio for all I/O, async/await runtime, and tracing.
  - [Clap](https://clap.rs/) - Used for command line operations.
  - Serde, Chrono, ...

## Contributing

Coming Soon.

## Versioning

We use [Semantic Versioning](http://semver.org/) for versioning. For the versions
available, see the [tags on this
repository](https://github.com/crocme10/area-net/tags).

## Authors

  - **Matthieu Paindavoine** [crocme10](https://github.com/crocme10)

## License

This project is licensed under the [MIT License](LICENSE.md)

## Acknowledgments

  - Hat tip to anyone whose code is used
  - Inspiration
  - etc

