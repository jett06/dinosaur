# dinosaur 🦖
Yet another DyKnow monitoring software killer :)

For educational purposes, of course.

## Build Process
Building is easiest on Linux platforms. First, install [cargo-xwin](https://github.com/rust-cross/cargo-xwin.git). Then, this command should work:
```sh
$ cargo +nightly xwin build -Zbuild-std=std,panic_abort -Zbuild-std-features=panic_immediate_abort --target=x86_64-pc-windows-msvc --release
```

You will find the program as `umpdc.dll` in `./target/x86_64-pc-windows-msvc/release/`.

## Usage
This program was built in mind with an intended workflow of DLL-hijacking the official Windows `DeviceCensus.exe` program (in order to evade traditional system monitoring techniques). Basically, you should find that program and copy it to a folder where you have full read-write access. Then, copy this `umpdc.dll` to that same directory. Now, when you launch `DeviceCensus` it should create a tray icon where you can start/stop/quit the DyKnow killer program.

---

Credit to [Good Ware](https://www.flaticon.com/authors/good-ware) for the dinosaur icon base!
