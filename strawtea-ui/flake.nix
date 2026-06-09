{
  description = "Strawtea frontend development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            nodejs_24
            corepack
            typescript-language-server
            svelte-language-server
            prettier
          ];

          env = {
            COREPACK_ENABLE_DOWNLOAD_PROMPT = "0";
          };
        };
      }
    );
}
