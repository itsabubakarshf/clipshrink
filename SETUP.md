# Setup guide

**What is this?** `compressor` is a small program that makes videos smaller and/or cuts them into parts
(for example 50 MB each). It is one single file — `compressor-windows.exe` on Windows,
`compressor-linux` on Ubuntu, `compressor-macos` on Mac — and it uses **ffmpeg** (a free video tool) to do the work.

You need two things on your computer:

| # | What | Where it comes from |
|---|---|---|
| 1 | **The compressor program** (the `.exe` on Windows) | Downloaded from this GitHub repository — [Part 1](#part-1--get-the-program-the-exe-file) |
| 2 | **ffmpeg** | Installed with one command — [Part 2](#part-2--install-ffmpeg) |

Then [Part 3](#part-3--use-it) shows how to use it and [Part 4](#part-4--settings-what-you-can-change) lists the settings.

There is **nothing to configure or compile** if you use the download. (You *can* build it yourself — see
[Option B](#option-b--build-the-exe-yourself).)

---

## Part 1 — Get the program (the `.exe` file)

### Option A — Download it from GitHub (easiest, no programming tools needed)

GitHub builds the program for Windows, Ubuntu and Mac automatically every time the code changes and puts the
files on the repository's **Releases** page.

1. Open the repository in your browser: <https://github.com/itsabubakarshf/compressor>
2. Open the Releases page: either click **Releases** in the right-hand column of the repo page, or go directly to
   <https://github.com/itsabubakarshf/compressor/releases>
3. Click the newest entry — it is called **Latest build** (or a version number like `v0.1.0` once you publish one).
4. Scroll down to **Assets** and click the file for your computer:

   | Your computer | Download this file |
   |---|---|
   | **Windows** | `compressor-windows.exe` |
   | **Ubuntu / Linux** | `compressor-linux` |
   | **Mac** (M1/M2/M3/M4) | `compressor-macos` |

   (Ignore "Source code (zip)" / "Source code (tar.gz)" — that is the program's code, not the program.)
5. Save it somewhere you will remember. On Windows, create a folder such as `C:\Tools` and put it there.
   You can rename `compressor-windows.exe` to `compressor.exe`.

> **The Releases page is empty / has no "Latest build"?** The build runs automatically after the code is pushed
> and takes about 5–10 minutes. Meanwhile (or if a build failed) you can grab the files from the **Actions** tab:
> open <https://github.com/itsabubakarshf/compressor/actions> → click the newest run with a green ✔ → scroll to
> **Artifacts** at the bottom → click `compressor-windows` / `compressor-linux` / `compressor-macos`. You must be
> logged in to GitHub, and you get a `.zip` — right-click → **Extract All** to get the program out of it.

> **Where does the code live?** Currently on the branch `claude/video-compression-splitting-ii43jb`. To make the
> repo page show it by default, merge that branch into `main` (on GitHub: **Pull requests → New pull request**,
> base `main`, compare `claude/video-compression-splitting-ii43jb`, then **Merge**). The downloads work either way.

#### Windows: the first time you open it
Chrome/Edge may say the file *"isn't commonly downloaded"* → click **Keep**. When you double-click it, Windows may show
**"Windows protected your PC"** (the program is new and not code-signed) → click **More info → Run anyway**.

#### Ubuntu / Mac: make it runnable
Open a terminal in the folder where you saved it and run (use your file's name):
```bash
chmod +x compressor-linux          # Ubuntu
chmod +x compressor-macos          # Mac
xattr -d com.apple.quarantine compressor-macos     # Mac only: removes the "unverified developer" block
```

### Option B — Build the exe yourself

Use this if you want to change the code, or you have an Intel Mac, or the downloads aren't available.
You need **Git** and **Rust** (the programming language the tool is written in). It takes a few minutes.

**Windows** (PowerShell):
```powershell
winget install Git.Git
winget install Rustlang.Rustup
# close PowerShell and open it again, then:
git clone -b claude/video-compression-splitting-ii43jb https://github.com/itsabubakarshf/compressor
cd compressor
cargo build --release
```
Your program is now at `compressor\target\release\compressor.exe` — copy it wherever you like.
(If it complains about a linker, install *Build Tools for Visual Studio* with the **Desktop development with C++**
workload and retry.)

**Ubuntu / Linux**:
```bash
sudo apt update
sudo apt install -y build-essential pkg-config git curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh      # press Enter to accept defaults, then reopen the terminal
git clone -b claude/video-compression-splitting-ii43jb https://github.com/itsabubakarshf/compressor
cd compressor
cargo build --release
```
Program: `target/release/compressor`.

**Mac**:
```bash
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh      # press Enter to accept defaults, then reopen Terminal
git clone -b claude/video-compression-splitting-ii43jb https://github.com/itsabubakarshf/compressor
cd compressor
cargo build --release
```
Program: `target/release/compressor`.

(Once the branch is merged into `main`, you can leave out `-b claude/video-compression-splitting-ii43jb`.
To update later: `git pull` and `cargo build --release` again.)

---

## Part 2 — Install ffmpeg

The compressor needs ffmpeg installed. Do this once.

**Windows** — open **PowerShell** (Start → type `PowerShell`):
```powershell
winget install Gyan.FFmpeg
```
Then **close PowerShell and open it again** and check:
```powershell
ffmpeg -version
```
A version number means it worked. *No `winget`?* Download "release essentials" from
<https://www.gyan.dev/ffmpeg/builds/>, unzip it, and copy `ffmpeg.exe` and `ffprobe.exe` (from its `bin` folder) into the
**same folder as the compressor exe** — the program finds them there automatically.

**Ubuntu / Linux**:
```bash
sudo apt update
sudo apt install -y ffmpeg libxkbcommon-x11-0 libgl1 libegl1
ffmpeg -version
```
(The `lib…` packages are only for the window; the command-line works without them.)

**Mac** — install [Homebrew](https://brew.sh) first if you don't have it, then:
```bash
brew install ffmpeg
ffmpeg -version
```

---

## Part 3 — Use it

### The window (easiest)

- **Windows:** double-click `compressor.exe`.
- **Ubuntu / Mac:** open a terminal in the folder and run `./compressor-linux` (or `./compressor-macos`) with nothing after it.

Then:
1. **Choose a video or folder** — drag it onto the window, paste its path, or (Windows) click *Choose file… / Choose folder…*.
2. **What do you want to do?** — *Compress*, *Split into parts*, or *Compress, then split*.
3. **How much smaller?** — drag the slider or click 30 / 40 / 50 / 60 / 70 %.
4. **Maximum size of each part** — 50 MB by default (only for splitting).
5. Press **Start**. A progress bar shows how it's going; **Cancel** stops it.
6. When it says *Done*, click **Open output folder**. The results are in a folder called `compressor_out` next to your original
   video. Your original is never changed.

### The command line

Open **PowerShell** (Windows) or a terminal (Ubuntu/Mac), go to the folder containing the program
(`cd C:\Tools`), and run it with the path to your video or folder. Windows examples (on Ubuntu/Mac use `./compressor-linux`
or `./compressor-macos` instead of `.\compressor.exe`, and `/` paths):

```powershell
# cut into parts of at most 50 MB (no quality loss, very fast)
.\compressor.exe "C:\Videos\holiday.mp4" --mode split

# make it about 40% smaller, one file
.\compressor.exe "C:\Videos\holiday.mp4" --mode compress --reduce 40%

# 50% smaller, then cut into 25 MB parts if still too big
.\compressor.exe "C:\Videos\holiday.mp4" --reduce 50 --max-size 25MB

# every video in a folder (and its sub-folders)
.\compressor.exe "C:\Videos" --recursive --reduce 40

# list every option
.\compressor.exe --help
```
Tip (Windows): Shift + right-click a file in Explorer → **Copy as path** gives you the quoted path to paste.
Tip (Mac): drag a file from Finder into the Terminal window to paste its path.

*Want to run it from any folder just by typing `compressor`?* Put the program in a folder that is on your PATH
(Linux/Mac: `/usr/local/bin`, e.g. `sudo mv compressor-linux /usr/local/bin/compressor`; Windows: add `C:\Tools` to
the PATH in *Settings → System → About → Advanced system settings → Environment Variables*).

---

## Part 4 — Settings (what you can change)

There is no config file. Everything is chosen in the window, or with options on the command line.

| Window | Command-line option | Meaning |
|---|---|---|
| What do you want to do? | `--mode compress` / `split` / `both` | Compress only / split only / compress then split (default `both`) |
| x% smaller | `--reduce 40` (or `40%`, or a range `30-50`) | How much smaller the video should get. A range aims for the middle |
| Maximum size of each part | `--max-size 50MB` | Size limit of each part (`25MB`, `1.5GB`, …). 1 MB = 1,000,000 bytes |
| Advanced → Codec | `--codec h264` / `h265` | H.264 plays everywhere; H.265 is ~30–40 % smaller but slower |
| Advanced → Use graphics card | `--encoder auto` | Try NVIDIA/Intel/AMD hardware encoding (faster, size control less exact). Default is the CPU |
| Advanced → Max height | `--height 720` | Shrink the picture to at most this many pixels high (never enlarges) |
| Advanced → fixed quality | `--crf 26` | Instead of a percentage, use a quality number (lower = better and bigger) |
| Advanced → Include sub-folders | `--recursive` | Also process videos inside sub-folders |
| Advanced → Replace existing results | `--overwrite` | Redo files that already have results |
| Advanced → Output folder | `--output "D:\Out"` | Where to put results (default `compressor_out` next to the video) |
| — | `--preset fast` | Speed vs. size on the CPU encoder (`ultrafast` … `veryslow`, default `medium`) |
| — | `--ffmpeg "C:\path\ffmpeg.exe"` | Use a specific ffmpeg if it is not found automatically |

**Good starting points**
- *"It must fit in 50 MB"* → mode **Split**, max size 50 MB (fast, no quality loss).
- *"Just make it smaller"* → mode **Compress**, 40–50 % smaller.
- *"Smaller **and** under 50 MB per file"* → **Compress, then split**, 50 % smaller, 50 MB.
- *"It's slow"* → tick *Use graphics card*, or set max height to 720, or `--preset fast`.

---

## Troubleshooting

| Problem | Fix |
|---|---|
| `ffmpeg/ffprobe not found` | ffmpeg isn't installed, or the terminal was opened before installing it. Close and reopen the terminal and check `ffmpeg -version`. Or put `ffmpeg`/`ffprobe` next to the program, or use `--ffmpeg <path>`. |
| Windows: "Windows protected your PC" | **More info → Run anyway** (the program isn't code-signed). |
| Mac: "cannot be opened because the developer cannot be verified" | `xattr -d com.apple.quarantine compressor-macos`, or **System Settings → Privacy & Security → Open Anyway**. |
| Linux/Mac: "Permission denied" | `chmod +x <file>`. |
| Linux: window doesn't open / `libxkbcommon-x11.so could not be loaded` | `sudo apt install libxkbcommon-x11-0 libgl1 libegl1`, and run it inside a desktop session (not plain SSH). |
| Releases page has nothing to download | Wait for the build to finish (Actions tab), or use the Actions artifacts / build it yourself (Option B above). |
| Parts are smaller than the limit | Normal: videos are cut on keyframes, so a part can't be exactly 50 MB. They are never larger than the limit. |
| `skipped: … already exists` | Results from an earlier run are in `compressor_out`. Tick *Replace existing results* / use `--overwrite`. |
| Compression is slow | See "It's slow" above. Hitting an exact size on the CPU encodes the video twice, so it takes about twice as long. |

---

## For the repo owner: publishing a numbered release

Every push already refreshes the **Latest build** release. For a named version, push a tag:
```bash
git tag v0.1.0
git push origin v0.1.0
```
