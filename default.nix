{ APPNAME
, lib
, makeRustPlatform
, fenix
, vulkan-loader
, makeWrapper
, pkgs
, system
, ...
}: let
APPDRV = (makeRustPlatform fenix.packages.${system}.latest).buildRustPackage {
  pname = APPNAME;
  version = "0.0.0";
  src = ./.;
  nativeBuildInputs = [ makeWrapper ];
  buildInputs = with pkgs; [
    vulkan-loader
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
      --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath [ vulkan-loader ]}
  '';

};
in
APPDRV
