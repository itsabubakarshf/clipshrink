# compressor

Fast Rust command-line tool (wrapping **ffmpeg**) that can

- **split** a video into parts of at most N MB (default **50 MB**) — lossless, no re-encoding,
- **compress** a video to a smaller file (H.264 or H.265, optional downscale, optional GPU),
- or **both**: compress first, then split only if the result is still too big.

Works on a single file or a whole folder.

## Setup (Windows)

1. Install ffmpeg: `winget install Gyan.FFmpeg` (or download from <https://www.gyan.dev/ffmpeg/builds/>
   and put `ffmpeg.exe` + `ffprobe.exe` next to `compressor.exe`).
2. Get `compressor.exe` from the **Actions** tab (artifact `compressor-windows`) or the **Releases** page,
   or build it yourself: `cargo build --release` → `target\release\compressor.exe`.

## Usage

Double-click `compressor.exe` (or run it with no arguments) for an interactive prompt — paste or drag in a
path, pick a mode, done. Or use flags:

```powershell
# split only → parts of ≤ 50 MB
compressor.exe "C:\Videos\holiday.mp4" --mode split

# compress only, no splitting
compressor.exe "C:\Videos\holiday.mp4" --mode compress

# compress, then split if still over 50 MB (default mode)
compressor.exe "C:\Videos\holiday.mp4"

# a whole folder, including sub-folders, 25 MB parts
compressor.exe "C:\Videos" --recursive --max-size 25MB

# smaller + faster: 720p, H.265, GPU if available
compressor.exe "C:\Videos\holiday.mp4" --mode compress --height 720 --codec h265 --encoder auto
```

Results go to `compressor_out\` next to the input (change with `--output`). Parts are named
`name_part001.mp4`, `name_part002.mp4`, … Existing outputs are skipped unless you pass `--overwrite`.

| Option | Meaning |
|---|---|
| `-m, --mode` | `compress`, `split`, or `both` (default) |
| `-s, --max-size` | max part size, e.g. `50MB`, `1.5GB` (1 MB = 1,000,000 bytes, so parts are also under 50 MiB) |
| `-o, --output` | output folder |
| `-r, --recursive` | include sub-folders |
| `--codec` | `h264` (default) or `h265` |
| `--crf` | quality, lower = better/bigger (default 26 for h264, 28 for h265) |
| `--preset` | x264/x265 speed preset, default `medium` |
| `--height` | downscale to at most this height (never upscales) |
| `--audio-bitrate` | default `128k` |
| `--encoder` | `cpu` (default, best quality per byte), `auto` (try GPU), `nvenc`, `qsv`, `amf` |
| `--ffmpeg` | path to ffmpeg if it isn't on PATH |

## How splitting works

Split mode copies the streams (no quality loss, very fast), cutting at keyframes so each part is under the
limit. Because cuts land on keyframes, parts can be somewhat smaller than the limit. After cutting, every part's
size is checked; if one is too big the cut length is reduced and retried. If a video has too few keyframes to
cut small enough, the tool tells you and re-encodes with forced keyframes for that file instead.

Only the first video stream and all audio streams are kept (subtitles/data tracks are dropped).

## Development

```bash
cargo test   # unit tests + end-to-end tests (need ffmpeg on PATH)
```
