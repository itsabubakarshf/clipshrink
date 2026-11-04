# Setup guide

Three steps on every system: **(1) install ffmpeg → (2) get `compressor` → (3) run it.**
Pick your system: [Windows](#windows) · [Ubuntu / Linux](#ubuntu--linux) · [macOS](#macos)

> **Where do I download it?**
> - **Releases page** (right side of the repo) — once a version has been published. Files are named
>   `compressor-windows.exe`, `compressor-linux`, `compressor-macos`.
> - Before any release exists: open the **Actions** tab → click the newest green run of the *CI* workflow →
>   scroll to **Artifacts** → download `compressor-windows`, `compressor-linux` or `compressor-macos`
>   (you need to be signed in to GitHub; the download is a `.zip` containing the program).
> - Or build it yourself — see "Build from source" in each section.

---

## Windows

### 1. Install ffmpeg
1. Press **Start**, type `PowerShell`, open **Windows PowerShell**.
2. Run:
   ```powershell
   winget install Gyan.FFmpeg
   ```
3. **Close PowerShell and open it again** (so the new PATH is picked up), then check:
   ```powershell
   ffmpeg -version
   ```
   You should see a version number. (No `winget`? Download the "release essentials" zip from
   <https://www.gyan.dev/ffmpeg/builds/>, unzip it, and copy `ffmpeg.exe` and `ffprobe.exe` from its `bin` folder
   into the same folder as `compressor-windows.exe` — then you can skip the PATH step.)

### 2. Get the program
1. Download `compressor-windows.exe` (see the box above; unzip first if it came from Actions).
2. Put it in a folder you'll remember, e.g. `C:\Tools\`. You may rename it to `compressor.exe`.
3. First launch: Windows SmartScreen may say *"Windows protected your PC"* because the file is new and unsigned.
   Click **More info → Run anyway**.

### 3. Run it
- **Window (easiest):** double-click the exe. Drop a video or folder onto the window, choose what you want,
  set "x% smaller" and/or the max part size (50 MB), press **Start**. Results appear in a `compressor_out` folder next
  to your video; the **Open output folder** button takes you there.
- **Command line:** in PowerShell, `cd C:\Tools`, then for example:
  ```powershell
  .\compressor.exe "C:\Videos\holiday.mp4" --mode split
  .\compressor.exe "C:\Videos\holiday.mp4" --mode compress --reduce 40%
  .\compressor.exe "C:\Videos" --recursive --reduce 50 --max-size 25MB
  .\compressor.exe --help
  ```
  Tip: in File Explorer, Shift + right-click a file → **Copy as path** gives you the quoted path to paste.

### Build from source (optional)
```powershell
winget install Rustlang.Rustup
# reopen PowerShell, then:
git clone https://github.com/itsabubakarshf/compressor
cd compressor
cargo build --release
# program: target\release\compressor.exe
```
(If the build complains about the linker, install "Build Tools for Visual Studio" with the *Desktop development with C++* workload.)

---

## Ubuntu / Linux

### 1. Install ffmpeg (and window libraries)
```bash
sudo apt update
sudo apt install -y ffmpeg libxkbcommon-x11-0 libgl1 libegl1
ffmpeg -version
```
The three `lib…` packages are only needed for the window; the command-line mode works without them.

### 2. Get the program
Download `compressor-linux` (see the box above; if it came from Actions, unzip it first), then:
```bash
mkdir -p ~/bin
mv ~/Downloads/compressor-linux ~/bin/compressor
chmod +x ~/bin/compressor
```
(`~/bin` is on your PATH on most Ubuntu setups after you log out and in again; otherwise run it as `~/bin/compressor`.)

### 3. Run it
- **Window:** run `compressor` with no arguments from a terminal inside your desktop session. Drop a file onto it
  or paste a path, choose the options, press **Start**.
- **Command line:**
  ```bash
  compressor ~/Videos/holiday.mp4 --mode split
  compressor ~/Videos/holiday.mp4 --mode compress --reduce 40%
  compressor ~/Videos --recursive --reduce 50 --max-size 25MB
  compressor --help
  ```
  Results are written to `compressor_out/` next to the input.

### Build from source (optional)
```bash
sudo apt install -y build-essential pkg-config git curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh     # accept the defaults, then reopen the terminal
git clone https://github.com/itsabubakarshf/compressor
cd compressor
cargo build --release
./target/release/compressor --help
```

---

## macOS

### 1. Install ffmpeg
1. Install Homebrew if you don't have it (<https://brew.sh> — one command to paste into Terminal).
2. In **Terminal**:
   ```bash
   brew install ffmpeg
   ffmpeg -version
   ```

### 2. Get the program
1. Download `compressor-macos` (see the box above; if it came from Actions, unzip it first).
   The build is for Apple Silicon (M1/M2/M3…). On an Intel Mac, use "Build from source" below.
2. In Terminal, make it runnable and remove macOS's download quarantine flag (the program isn't signed):
   ```bash
   cd ~/Downloads
   chmod +x compressor-macos
   xattr -d com.apple.quarantine compressor-macos
   ```
   (If you skip the second line, macOS says *"cannot be opened because the developer cannot be verified"*;
   you can alternatively go to **System Settings → Privacy & Security → Open Anyway**.)
3. Optional — put it somewhere permanent:
   ```bash
   sudo mkdir -p /usr/local/bin && sudo mv compressor-macos /usr/local/bin/compressor
   ```

### 3. Run it
- **Window:** in Terminal run `compressor` (or `./compressor-macos`) with no arguments. Drop a file onto the
  window or paste a path, choose the options, press **Start**.
- **Command line:**
  ```bash
  compressor ~/Movies/holiday.mp4 --mode split
  compressor ~/Movies/holiday.mp4 --mode compress --reduce 40%
  compressor ~/Movies --recursive --reduce 50 --max-size 25MB
  ```
  Tip: you can drag a file from Finder into the Terminal window to paste its path.

### Build from source (optional, also for Intel Macs)
```bash
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh     # accept the defaults, then reopen Terminal
git clone https://github.com/itsabubakarshf/compressor
cd compressor
cargo build --release
./target/release/compressor --help
```

---

## Making a release (for the repo owner)

Publishing a tag makes CI build all three programs and attach them to a GitHub Release:
```bash
git tag v0.1.0
git push origin v0.1.0
```

## Troubleshooting

| Problem | Fix |
|---|---|
| `ffmpeg/ffprobe not found` | ffmpeg isn't installed or the terminal was opened before installing it. Reopen the terminal and check `ffmpeg -version`. Or pass `--ffmpeg <path to ffmpeg>`, or put `ffmpeg`/`ffprobe` next to the program. |
| Windows: "Windows protected your PC" | Click **More info → Run anyway** (the program isn't code-signed). |
| macOS: "cannot be opened because the developer cannot be verified" | Run `xattr -d com.apple.quarantine <file>` (see above). |
| Linux/macOS: "Permission denied" | `chmod +x <file>`. |
| Linux: window doesn't open / `libxkbcommon-x11.so could not be loaded` | `sudo apt install libxkbcommon-x11-0 libgl1 libegl1`, and run it inside a desktop session (not over plain SSH). |
| Parts are smaller than the limit | Normal: videos are cut on keyframes, so a part can't be exactly 50 MB. They are never larger than the limit. |
| "skipped: … already exists" | Results from an earlier run are in `compressor_out`; tick **Replace existing results** / use `--overwrite`. |
| Compression is slow | Use the GPU option (Advanced → *Use graphics card*, or `--encoder auto`), a faster `--preset` (e.g. `fast`), or `--height 720`. Two-pass size targeting on the CPU takes about twice as long as one pass. |
