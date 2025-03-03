{ shellPkg
, pkg-config
, fenix
, APPNAME
, mkShell
, pkgs
, lib
, system
, ...
}: let
# dev shells should not contain the final program.
# They should have the environment
# needed to BUILD (and run) the final program.
  wasmtoolchain = with fenix.packages.${system}; combine [
    (latest.withComponents [
      "rustc"
      "cargo"
      "rustfmt"
      "clippy"
      "rust-src"
    ])
    targets.wasm32-unknown-unknown.latest.rust-std
  ];
  DEVSHELL = mkShell {
    packages = [];
    inputsFrom = [];
    DEVSHELL = 0;
    inherit APPNAME;
    nativeBuildInputs = [ pkg-config ];
    buildInputs = with pkgs; [
      # fenix.packages.x86_64-linux.latest.toolchain
      fenix.packages.${system}.latest.toolchain
      cargo-edit
      alsa-lib
      udev
      vulkan-loader
      llvmPackages.bintools
      clang
      rustup
      lldb
      cargo-watch
      pkg-config
      xorg.libX11
      xorg.libXcursor
      xorg.libXrandr
      xorg.libXi
      xorg.libxkbfile
      libxkbcommon
      vulkan-tools
      vulkan-headers
      vulkan-loader
      vulkan-validation-layers
    ];
    LD_LIBRARY_PATH = "${lib.makeLibraryPath (with pkgs; [ alsa-lib udev vulkan-loader libxkbcommon])}";
    shellHook = ''
      exec ${shellPkg}
    '';
  };
in
DEVSHELL
