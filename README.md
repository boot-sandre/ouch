# ouch

`ouch` is the Obvious Unified Compression (and decompression) Helper. 


| Supported formats | .tar | .zip | .tar.{.lz, .lzma, .gz, .bz}  | .zip.{.lz, .lzma, .gz, .bz}  | .bz | .gz | .lz, .lzma |
|-------------------|------|------|------------------------------|------------------------------|-----|-----|------------|
| Decompression     |   ✓  |   ✓  |               ✓              |               ✓              |  ✓  |  ✓  |      ✓     |
| Compression       |   ✗  |   ✗  |               ✗              |               ✗              |  ✗  |  ✗  |      ✗     |

## How does it work?

`ouch` infers commands from the extensions of its command-line options.

```
ouch 0.1.0
ouch is a unified compression & decompression utility

USAGE:
    ouch [OPTIONS] --input <input>...

FLAGS:
    -h, --help       Displays this message and exits
    -V, --version    Prints version information

OPTIONS:
    -i, --input <input>...    Input files (TODO description)
    -o, --output <output>     Output file (TODO description)
```

### Examples

#### Decompressing a bunch of files

```bash
$ ouch -i file{1..5}.zip another_file.tar.gz yet_another_file.tar.bz
```

When no output file is supplied, `ouch` infers that it must decompress all of its input files. This will error if any of the input files are not decompressible.

#### Decompressing a bunch of files into a folder

```bash
$ ouch -i file{1..5}.tar.gz -o some-folder
info: attempting to decompress input files into single_folder
info: done!
```

When the output file is not a compressed file, `ouch` will check if all input files are decompressible and infer that it must decompress them into the output file.

#### Compressing files 

```bash
$ ouch -i file{1..20} -o archive.tar
info: trying to compress input files into 'archive.tar'
info: done!
```

### Error scenarios

#### No clear decompression algorithm

```bash
$ ouch -i some-file -o some-folder
error: file 'some-file' is not decompressible.
```

`ouch` might (TODO!) be able to sniff a file's compression format if it isn't supplied in the future, but that is not currently implemented.

## Installation

### Runtime dependencies

`ouch` depends on a few widespread libraries:
* libbz2
* liblzma

Both should be already installed in any mainstream Linux distribution.

If they're not, then:

* On Debian-based distros

`sudo apt install liblzma-dev libbz2-dev`

* On Arch-based distros

`sudo pacman -S xz bzip2`

The last dependency is a recent [Rust](https://www.rust-lang.org/) toolchain. If you don't have one installed, follow the instructions at [rustup.rs](https://rustup.rs/).

### Build process

Once the dependency requirements are met:

```bash
git clone https://github.com/vrmiguel/jacarex   # Clone the repo.
cargo install --path ouch # .. and install it 
```