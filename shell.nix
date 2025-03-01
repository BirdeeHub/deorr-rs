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
      wasmtoolchain
      (wasm-bindgen-cli.override {
        version = "0.2.99";
        hash = "sha256-1AN2E9t/lZhbXdVznhTcniy+7ZzlaEp/gwLEAucs6EA=";
        cargoHash = "sha256-DbwAh8RJtW38LJp+J9Ht8fAROK9OabaJ85D9C/Vkve4=";
      })
      binaryen
      wasm-tools
      wasm-pack
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
      (pkgs.writeShellScriptBin "build_wasm_package" ''
        if [ -d ./out ]; then
          rm -rf ./out/*
          RUSTFLAGS="--cfg=web_sys_unstable_apis" cargo build --release --target wasm32-unknown-unknown
          wasm-bindgen --no-typescript --out-dir ./out/ --target web ./target/wasm32-unknown-unknown/release/day6vis.wasm
          wasm-opt -Oz -o ./out/day6vis.wasm ./out/day6vis_bg.wasm
          mv ./out/day6vis.wasm ./out/day6vis_bg.wasm
        fi
      '')
    ];
    LD_LIBRARY_PATH = "${lib.makeLibraryPath (with pkgs; [ alsa-lib udev vulkan-loader libxkbcommon])}";
    shellHook = ''
      exec ${shellPkg}
    '';
  };
in
DEVSHELL
