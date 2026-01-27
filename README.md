# E(X)Hentai Downloader

A **Fast** manga downloader for e-hentai.org and exhentai.org, written in Rust.

## Features

- Multi-threaded downloading for maximum speed
- Automatic retry with verification (up to 5 attempts)
- Supports both e-hentai.org and exhentai.org
- Cross-platform: Linux, macOS, Windows

## Installation

### Download Pre-built Binary

Download the latest release from the [Releases](https://github.com/rniczh/hentai-downloader/releases) page.

| Platform | Download |
|----------|----------|
| Linux (x64) | `hentai-downloader-x86_64-unknown-linux-gnu.tar.gz` |
| macOS (Intel) | `hentai-downloader-x86_64-apple-darwin.tar.gz` |
| macOS (Apple Silicon) | `hentai-downloader-aarch64-apple-darwin.tar.gz` |
| Windows (x64) | `hentai-downloader-x86_64-pc-windows-msvc.zip` |

### Build from Source

Make sure you have [Rust](https://rustup.rs/) installed, then run:

```bash
cargo build --release
```

The executable will be at `target/release/hentai-downloader`.

## Usage

```
hentai-downloader 0.2
Hongsheng Zheng <mathan0203@gmail.com>
Download the Manga from e(x)hentai website.

USAGE:
    hentai-downloader [OPTIONS] --url <url>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -c, --cookie <file>    The cookie file for access exhentai.org
    -u, --url <url>        The url of Manga for which you want to download
```

### Examples

**Download from e-hentai.org:**

```bash
hentai-downloader -u https://e-hentai.org/g/12345/abcdef/
```

**Download from exhentai.org (requires cookie):**

```bash
hentai-downloader -u https://exhentai.org/g/12345/abcdef/ -c cookie.txt
```

Downloaded files will be saved to `tmp{gallery_id}/` directory.

## Cookie Setup (for exhentai.org)

To access exhentai.org, you need to provide your session cookies.

1. Log in to exhentai.org in your browser
2. Open Developer Tools (F12) -> Storage/Application -> Cookies
3. Copy the cookie values and save to `cookie.txt`:

```
ipb_member_id=YOUR_ID; ipb_pass_hash=YOUR_HASH; igneous=YOUR_IGNEOUS
```

## Notice

E(X)Hentai has an implicit image viewing limit per user.
If you exceed this limit, you'll need to wait several hours for it to recover.

## License
GPL-3.0 license
