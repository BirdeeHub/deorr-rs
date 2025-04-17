{ APPNAME
, makeRustPlatform
, fenix
, vulkan-loader
, pkgs
, system
, ...
}: let
APPDRV = (makeRustPlatform fenix.packages.${system}.latest).buildRustPackage {
  pname = APPNAME;
  version = "0.0.0";
  src = ./.;
  buildInputs = with pkgs; [
    vulkan-loader
  ];
  cargoLock = {
    lockFileContents = builtins.readFile ./Cargo.lock;
  };
  postInstall = ''
    patchelf $out/bin/${APPNAME} --add-needed libvulkan.so
    patchelf $out/bin/${APPNAME} --add-rpath ${vulkan-loader}/lib
  '';
};
in
APPDRV
