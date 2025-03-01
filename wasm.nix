{ APPNAME
, lib
, makeRustPlatform
, fenix
, pkg-config
, alsa-lib
, udev
, vulkan-loader
, libxkbcommon
, libX11
, libxcb
, makeWrapper
, pkgs
, system
, ...
}: let
#TODO: MAKE IT BE WASM
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
APPDRV = (makeRustPlatform fenix.packages.x86_64-linux.default).buildRustPackage {
  pname = APPNAME;
  version = "0.0.0";
  src = ./.;
  nativeBuildInputs = [ pkg-config makeWrapper ];
  buildInputs = with pkgs; [
    alsa-lib
    udev
    vulkan-loader
    llvmPackages.bintools
    clang
    pkg-config
    libX11
    libxcb
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

  cargoLock = {
    lockFileContents = builtins.readFile ./Cargo.lock;
  };

  postFixup = ''
    wrapProgram "$out/bin/${APPNAME}" \
      --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath [ alsa-lib udev vulkan-loader libxkbcommon]}
  '';

};
in
APPDRV
