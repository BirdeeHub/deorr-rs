{ shellPkg
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
  DEVSHELL = mkShell {
    packages = [];
    inputsFrom = [];
    DEVSHELL = 0;
    inherit APPNAME;
    buildInputs = with pkgs; [
      fenix.packages.${system}.latest.toolchain
      vulkan-loader
    ];
    LD_LIBRARY_PATH = "${lib.makeLibraryPath (with pkgs; [ vulkan-loader ])}";
    shellHook = ''
      exec ${shellPkg}
    '';
  };
in
DEVSHELL
