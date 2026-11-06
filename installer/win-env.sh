# Environment for building the Windows (x86_64) versions on a Mac.
# Needs: rustup target x86_64-pc-windows-msvc, and the Microsoft CRT/SDK libraries
# downloaded with `xwin --accept-license --arch x86_64 splat --output $XWIN_DIR`.
XWIN_DIR="${XWIN_DIR:-/tmp/avdx/xwin/sdk}"
export CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=rust-lld
export CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS="-C linker-flavor=lld-link -C target-feature=+crt-static -Lnative=$XWIN_DIR/crt/lib/x86_64 -Lnative=$XWIN_DIR/sdk/lib/um/x86_64 -Lnative=$XWIN_DIR/sdk/lib/ucrt/x86_64"
# for any crate that compiles C code
export CC_x86_64_pc_windows_msvc=clang-cl
export CFLAGS_x86_64_pc_windows_msvc="-imsvc $XWIN_DIR/crt/include -imsvc $XWIN_DIR/sdk/include/ucrt -imsvc $XWIN_DIR/sdk/include/um -imsvc $XWIN_DIR/sdk/include/shared"
