# sfz

[![Travis build status](https://travis-ci.org/weihanglo/sfz.svg?branch=master)](https://travis-ci.org/weihanglo/sfz) [![Appveyor build status](https://ci.appveyor.com/api/projects/status/github/weihanglo/sfz?svg=true)](https://ci.appveyor.com/project/weihanglo/sfz) [![Dependency status](https://deps.rs/repo/github/weihanglo/sfz/status.svg)](https://deps.rs/repo/github/weihanglo/sfz) [![Lines of code](https://tokei.rs/b1/github/weihanglo/sfz?category=code)](https://github.com/weihanglo/sfz)

[sfz][sfz], or **S**tatic **F**ile **Z**erver, is a simple command-line tool serving static files for you.

![cover](cover.png)

The name **sfz** is derived from a accented note [Sforzando][sforzando] in music, which meaning “suddenly with force.”

[sfz]: https://github.com/weihanglo/sfz
[sforzando]: https://en.wikipedia.org/wiki/Dynamics_(music)#Sudden_changes_and_accented_notes

## Features

- Directory listing
- Partial responses (range requests)
- Conditional requests with cache validations
- Cross-origin resource sharing
- Automatic HTTP compression (Brotli, Gzip, Deflate)
- Respect to `.gitignore` file

## Installation

### Automatic

#### Cargo

If you are a Rust programmer, you can install sfz directly from GitHub via [Cargo][cargo].

```bash
cargo install --git https://github.com/weihanglo/sfz
```

[cargo]: https://doc.rust-lang.org/cargo/

### Manual

#### Prebuilt binaries

Currently unavailable.

#### Build from source

sfz is written in Rust. You need to [install Rust][install-rust] in order to compile it.

```bash
$ git clone https://github.com/weihanglo/sfz.git
$ cd sfz
$ cargo build --release
$ ./target/release/sfz --version
0.1.0
```

[install-rust]: https://www.rust-lang.org/install.html

## Usage

The simplest way to start serving files is to run this command:

```bash
sfz [FLAGS] [OPTIONS] [path]
```

The command above will start serving your current working directory on `127.0.0.1:8888` by default.

If you want to serve another directory, pass `[path]` positional argument in with either absolute or relaitve path.

```bash
sfz /usr/local

# Serve files under `/usr/local` directory.
```

### Flags and Options

sfz aims to be simple but configurable. Here is a list of available options:

| Option             | Default Value             |
| :----------------- | ------------------------- |
| Base directory     | current working directory |
| Address            | 127.0.0.1                 |
| Port               | 8888                      |
| CORS               | `false`                   |
| Caching            | 0 second                  |
| HTTP compression   | `true`                    |
| Serve hidden files | `false`                   |
| Respect .gitignore | `true`                    |

For more infomation, run following command:

```bash
sfz --help
```

## Contributing

Contributions are highly appreciated! Feel free to open issues or send pull requests directly.

## Credits

sfz was originally inspired by another static serving tool [serve][serve], and its directory-listing UI is mainly borrowed from [GitHub][github].

sfz is built on the top of awesome Rust community. Thanks for all Rust and crates contributors.

[serve]: https://github.com/zeit/serve
[github]: https://github.com/

## License

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in sfz by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
