{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      inherit (nixpkgs.lib)
        genAttrs
        importTOML
        licenses
        maintainers
        sourceByRegex
        ;

      eachSystem =
        f:
        genAttrs [
          "aarch64-darwin"
          "aarch64-linux"
          "x86_64-darwin"
          "x86_64-linux"
        ] (system: f nixpkgs.legacyPackages.${system});
    in
    {
      formatter = eachSystem (pkgs: pkgs.nixfmt);

      packages = eachSystem (
        pkgs:
        let
          src = sourceByRegex self [
            "(src|tests)(/.*)?"
            ''Cargo\.(toml|lock)''
            ''build\.rs''
          ];

          inherit (pkgs)
            installShellFiles
            makeWrapper
            rustPlatform
            ;
        in
        {
          default = pkgs.callPackage (
            {
              lib,
              rustPlatform,
              installShellFiles,
              makeWrapper,
              useLix ? true,
              lix,
              nix,
            }:
            rustPlatform.buildRustPackage {
              pname = "flake-du";
              inherit ((importTOML (src + "/Cargo.toml")).package) version;

              inherit src;

              cargoLock = {
                lockFile = src + "/Cargo.lock";
              };

              nativeBuildInputs = [
                installShellFiles
                makeWrapper
              ];

              env = {
                GEN_ARTIFACTS = "artifacts";
              };

              postInstall = ''
                installManPage artifacts/flake-du.1
                installShellCompletion artifacts/flake-du.{bash,fish} --zsh artifacts/_flake-du
                
                # We default to lix to sidestep CppNix's issue where 
                # builtins.fetchTree downloads locked inputs even if they
                # are already cached in the store. Using lix avoids this.
                wrapProgram $out/bin/flake-du \
                  --prefix PATH : ${lib.makeBinPath [ (if useLix then lix else nix) ]}
              '';

              meta = {
                license = licenses.mpl20;
                maintainers = with maintainers; [ kmein ];
              };
            }
          ) { };
        }
      );
    };
}
