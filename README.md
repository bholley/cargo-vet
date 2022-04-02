# cargo-vet

[![crates.io](https://img.shields.io/crates/v/cargo-vet.svg)](https://crates.io/crates/cargo-vet)
[![docs.rs](https://docs.rs/cargo-vet/badge.svg)](https://docs.rs/cargo-vet)
![Rust CI](https://github.com/bholley/cargo-vet/workflows/Rust%20CI/badge.svg?branch=master)

cargo-vet helps you vet your dependencies and rely on the vetting done by other projects.



# Current Default Directory Structure

(As implemented by the CLI, currently divergent from the book.)

* supply-chain/
  * audited.toml
  * trusted.toml
  * untrusted.toml





# cargo-vet CLI manual

> This manual can be regenerated with `cargo vet help-markdown`

Version: `cargo-vet 0.1.0`

Supply-chain security for Rust

### USAGE
cargo-vet [OPTIONS] [SUBCOMMAND]

### OPTIONS
#### `--all-features`
Activate all available features

#### `--exclude <SPEC>`
Exclude packages from being processed

#### `--features <FEATURES>`
Space-separated list of features to activate

#### `-h, --help`
Print help information

#### `--locked`
Do not pull in new "audits"

#### `--log-file <LOG_FILE>`
Instead of stderr, write logs to this file (only used after successful CLI parsing)

#### `--manifest-path <PATH>`
Path to Cargo.toml

#### `--no-default-features`
Do not activate the `default` feature

#### `--output-file <OUTPUT_FILE>`
Instead of stdout, write output to this file

#### `-p, --package <SPEC>`
Package to process (see `cargo help pkgid`)

#### `-v, --verbose <VERBOSE>`
How verbose logging should be (log level)

\[default: warn]
\[possible values: off, error, warn, info, debug, trace]

#### `-V, --version`
Print version information

#### `--workspace`
Process all packages in the workspace

### SUBCOMMANDS
* [audits](#cargo-vet-audits): ??? List audits mechanisms ???
* [certify](#cargo-vet-certify): Mark `$crate $version` as reviewed with `$message`
* [diff](#cargo-vet-diff): Yield a diff against the last reviewed version
* [fetch](#cargo-vet-fetch): Fetch the source of `$crate $version`
* [forbid](#cargo-vet-forbid): Mark `$crate $version` as unacceptable with `$message`
* [help](#cargo-vet-help): Print this message or the help of the given subcommand(s)
* [help-markdown](#cargo-vet-help-markdown): Print --help as markdown (for generating docs)
* [init](#cargo-vet-init): initialize cargo-vet for your project
* [suggest](#cargo-vet-suggest): Suggest some low-hanging fruit to review

<br><br><br>
## cargo vet help 
Print this message or the help of the given subcommand(s)

### cargo vet help USAGE
help [SUBCOMMAND]...

### cargo vet help ARGS
#### `<SUBCOMMAND>...`
The subcommand whose help message to display

<br><br><br>
## cargo vet help-markdown 
Print --help as markdown (for generating docs)

### cargo vet help-markdown USAGE
help-markdown

### cargo vet help-markdown OPTIONS
#### `-h, --help`
Print help information

#### `-V, --version`
Print version information

<br><br><br>
## cargo vet audits 
??? List audits mechanisms ???

### cargo vet audits USAGE
audits

### cargo vet audits OPTIONS
#### `-h, --help`
Print help information

#### `-V, --version`
Print version information

<br><br><br>
## cargo vet suggest 
Suggest some low-hanging fruit to review

### cargo vet suggest USAGE
suggest

### cargo vet suggest OPTIONS
#### `-h, --help`
Print help information

#### `-V, --version`
Print version information

<br><br><br>
## cargo vet forbid 
Mark `$crate $version` as unacceptable with `$message`

### cargo vet forbid USAGE
forbid <KRATE> <VERSION> <MESSAGE>

### cargo vet forbid ARGS
#### `<KRATE>`


#### `<VERSION>`


#### `<MESSAGE>`


### cargo vet forbid OPTIONS
#### `-h, --help`
Print help information

<br><br><br>
## cargo vet certify 
Mark `$crate $version` as reviewed with `$message`

### cargo vet certify USAGE
certify <KRATE> <VERSION> <MESSAGE>

### cargo vet certify ARGS
#### `<KRATE>`


#### `<VERSION>`


#### `<MESSAGE>`


### cargo vet certify OPTIONS
#### `-h, --help`
Print help information

<br><br><br>
## cargo vet diff 
Yield a diff against the last reviewed version

### cargo vet diff USAGE
diff

### cargo vet diff OPTIONS
#### `-h, --help`
Print help information

#### `-V, --version`
Print version information

<br><br><br>
## cargo vet fetch 
Fetch the source of `$crate $version`

### cargo vet fetch USAGE
fetch <KRATE> <VERSION>

### cargo vet fetch ARGS
#### `<KRATE>`


#### `<VERSION>`


### cargo vet fetch OPTIONS
#### `-h, --help`
Print help information

<br><br><br>
## cargo vet init 
initialize cargo-vet for your project

### cargo vet init USAGE
init

### cargo vet init OPTIONS
#### `-h, --help`
Print help information

#### `-V, --version`
Print version information


