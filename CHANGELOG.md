# Changelog

This project adheres to [Semantic Versioning](http://semver.org/).  
Every release, along with the migration instructions, is documented on this file and Github [Releases](https://github.com/weihanglo/sfz/releases) page.

## [Unreleased](https://github.com/weihanglo/sfz/compare/0.1.2...HEAD)

## [0.1.2] - 2020-08-28

### [Changes][0.1.2-changes]

- Fix range header off-by-one error ([#39](https://github.com/weihanglo/sfz/issues/39))

[0.1.2]: https://github.com/weihanglo/sfz/releases/tag/0.1.2
[0.1.2-changes]: https://github.com/weihanglo/sfz/compare/0.1.1...0.1.2

## [0.1.1] - 2020-06-04

### [Changes][0.1.1-changes]

- Fix duplicated prefix slash regression issue ([#31](https://github.com/weihanglo/sfz/issues/31))

[0.1.1]: https://github.com/weihanglo/sfz/releases/tag/0.1.1
[0.1.1-changes]: https://github.com/weihanglo/sfz/compare/0.1.0...0.1.1

## [0.1.0] - 2020-05-01

### [Changes][0.1.0-changes]

- Added new flag `--path-prefix` to customize path prefix when serving content (credit to [@jxs](https://github.com/jxs))

[0.1.0]: https://github.com/weihanglo/sfz/releases/tag/0.1.0
[0.1.0-changes]: https://github.com/weihanglo/sfz/compare/0.0.4...0.1.0

## [0.0.4] - 2019-09-07

### [Changes][0.0.4-changes]

- Added new feature: logs request/response by default.
- Added new option flag `--no-log` to disable request/response logging.
- Updated to Rust 2018 edition.
- Upgraded dependency `mime_guess` from 2.0.0-alpha to 2.0.
- Upgraded dependency `percent-encoding` from 1.0 to 2.1.
- Upgraded dependency `brotli` from 1.1 to 3.
- Upgraded dependency `unicase` from 2.1 to 2.5.

[0.0.4]: https://github.com/weihanglo/sfz/releases/tag/0.0.4
[0.0.4-changes]: https://github.com/weihanglo/sfz/compare/0.0.3...0.0.4

## [0.0.3] - 2018-03-07

### [Changes][0.0.3-changes]

- Handled error with some human-readable format.
- Added new command arg `--render--index` to automatically render index file such as `index.html`.
- Updated some command args' short names, default values and descriptions.

[0.0.3]: https://github.com/weihanglo/sfz/releases/tag/0.0.3
[0.0.3-changes]: https://github.com/weihanglo/sfz/compare/0.0.2...0.0.3

## [0.0.2] - 2018-03-03

First release version on [Crates.io][crate-sfz]!

### [Changes][0.0.2-changes]

- Hombrew formula for sfz! You can now donwload sfz via homebrew from GitHub.
- Fixed missing `ETag` and `Last-Modified` header fields.
- Fixed unsecure symlink following.

[0.0.2]: https://github.com/weihanglo/sfz/releases/tag/0.0.2
[0.0.2-changes]: https://github.com/weihanglo/sfz/compare/0.0.1-beta.1...0.0.2

## [0.0.1-beta.1] - 2018-03-02

Beta release.

[0.0.1-beta.1]: https://github.com/weihanglo/sfz/releases/tag/0.0.1-beta.1

[crate-sfz]: https://crates.io/crates/sfz
