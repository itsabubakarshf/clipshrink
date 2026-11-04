# compressor

Fast Rust command-line tool (wrapping **ffmpeg**) that can

- **split** a video into parts of at most N MB (default **50 MB**) — lossless, no re-encoding,
- **compress** a video to a smaller file (H.264 or H.265, optional downscale, optional GPU),
- or **both**: compress first, then split only if the result is still too big.

Works on a single file or a whole folder.

## Install

**Step-by-step guide for Windows, Ubuntu and macOS: [SETUP.md](SETUP.md).**

In short: install ffmpeg (`winget install Gyan.FFmpeg` / `sudo apt install ffmpeg` / `brew install ffmpeg`), download
`compressor` for your system from the Releases page (or the Actions artifacts), and run it — with no arguments it
opens a window.

## Usage

### Window (easiest)

Double-click `compressor.exe`. Pick a video or folder (or drag it onto the window), choose **Compress**,
**Split into parts** or **Compress, then split**, drag the **"x% smaller"** slider (or click 30 / 40 / 50 / 60 / 70 %),
set the max part size, and press **Start**. Progress is shown live and there is a Cancel button.

### Command line

```powershell
# split only → parts of ≤ 50 MB
compressor.exe "C:\Videos\holiday.mp4" --mode split

# compress only, no splitting
compressor.exe "C:\Videos\holiday.mp4" --mode compress

# compress, then split if still over 50 MB (default mode)
compressor.exe "C:\Videos\holiday.mp4"

# make it about 40% smaller (a range like 30-50 aims for the middle)
compressor.exe "C:\Videos\holiday.mp4" --mode compress --reduce 40%

# 50% smaller, then cut into 25 MB parts
compressor.exe "C:\Videos\holiday.mp4" --reduce 50 --max-size 25MB

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
| `--reduce` | make the video this much smaller: `40`, `40%`, or `30-50` (aims for the middle). Replaces `--crf` |
| `--crf` | quality, lower = better/bigger (default 26 for h264, 28 for h265) |
| `--preset` | x264/x265 speed preset, default `medium` |
| `--height` | downscale to at most this height (never upscales) |
| `--audio-bitrate` | default `128k` |
| `--encoder` | `cpu` (default, best quality per byte), `auto` (try GPU), `nvenc`, `qsv`, `amf` |
| `--ffmpeg` | path to ffmpeg if it isn't on PATH |

## How "make it X% smaller" works

The tool calculates the bitrate that gives `original size × (1 − X%)` and encodes to it (two passes on the CPU
encoder, so the result is accurate — typically within a couple of percent; GPU encoders use one pass and are
looser). Already heavily compressed videos can't always shrink a lot without looking bad; very high percentages
produce a warning.

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
