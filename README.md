# rust-gba-multiboot-test
Code sample/proof of concept for sending multiboot games using the Rust GBA crate

- `main.rs` - An example GBA program with the multiboot initiation code injected in
- `gba_multiboot.rs` - Defines the multiboot protocol as must be implemented
   on the host cartridge
- `gba_multiboot_bios.rs` - Defines the multiboot syscall and its associated
   data structures.

# Prerequisite Files

To run this project, you need two files, neither of which I can legally supply:

1. If running in emulator, a genuine GBA BIOS. Your emulator should be configured to
   load this whether or not it is a booting a game, since the Multiboot protocol is
   implemented in the BIOS. (If running on real hardware, you do not need this, though
   you will need a way to get the compiled ROM running on it.)
2. A multiboot ROM payload. This should be a GBA ROM that has been compiled to run in
   Multiboot mode (i.e. to run from the external work RAM instead of the cartridge).
   DevkitPro includes functionality to build in Multiboot mode, and many commercial
   GBA and Gamecube games included test payloads.

# Build Instructions

1. Install the prerequisites for GBA development, including the Rust `nightly` toolchain,
   `binutils-arm-none-eabi`, and the Cargo packages `cargo-make` and `gbafix`.
1. Provide a multiboot ROM payload and save it as `./assets/payload.mb.gba`.
1. Build and test the crate using `cargo build` and `cargo run`. The default emulator is set
   to `mgba-qt` in `.cargo/config.toml`, but you can change it to an emulator of choice.
1. When you wish to export to a real GBA, run these commands to produce the final cartridge ROM:

    arm-none-eabi-objcopy -O binary target/thumbv4t-none-eabi/release/gba-multiboot-test gba-multiboot-test.gba
    gbafix gba-multiboot-test.gba

# Test Instructions

1. After starting the ROM on the first GBA, connect a second GBA and boot into the BIOS
   with no cartridge.
   - In MGBA, use File > New multiplayer window, followed by File > Boot BIOS in the new window.
     Use similar steps in other emulators.
   - On real hardware, boot the second GBA either with no cartridge inserted, or
     while holding START + SELECT with a valid cartridge inserted. Use a GBA link cable
     to connect them, with the small purple end in the first GBA and the large grey end
     in the second GBA.
   - Note that this code sample works with up to 3 GBAs connected, either with multiple
     multiplayer windows or daisy-chained Game Boy Advance Link Cables.
2. The test program on the host GBA allows you to paint the screen using the D-Pad
   and change the color using L and R (taken from the [hello-world example](
   https://github.com/rust-console/gba/blob/main/examples/hello_world.rs) in the `gba` crate).
3. Press START on the host GBA to initiate a multiboot session.
   - If multiboot succeeds, the client GBA(s) will receive the payload and the host GBA's
     drawing dot will turn green.
   - If multiboot fails, the host GBA's drawing dot will change to a different color.
