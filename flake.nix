{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = { nixpkgs, ... }@inputs: let
    forEachSystem = with builtins; f: let # flake-utils.lib.eachSystem
      op = attrs: system: let
        ret = f system;
        op = attrs: key: attrs // {
          ${key} = (attrs.${key} or { })
          // { ${system} = ret.${key}; };
        };
      in foldl' op attrs (attrNames ret);
    in foldl' op { } nixpkgs.lib.platforms.all;

    APPNAME = "deorr";
    appOverlay = final: prev: {
      # any pkgs overrides made here will be inherited in the arguments of default.nix
      # because we used final.callPackage instead of prev.callPackage
      ${APPNAME} = final.callPackage ./native.nix ({ inherit APPNAME; } // inputs);
      day6vis-wasm = final.callPackage ./wasm.nix ({ inherit APPNAME; } // inputs);
    };
  in {
    overlays.default = appOverlay;
  } // (
    forEachSystem (system: let
      pkgs = import nixpkgs { inherit system; overlays = [ appOverlay ]; };
    in{
      packages = {
        default = pkgs.${APPNAME};
        inherit (pkgs) day6vis-wasm;
      };
      devShells = {
        default = pkgs.callPackage ./shell.nix ({
          inherit APPNAME;
          shellPkg = "${pkgs.zsh}/bin/zsh";
        } // inputs);
      };
    })
  );
}
