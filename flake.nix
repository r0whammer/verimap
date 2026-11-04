{
  description = "VeriMap: Rust domain + Charon/Aeneas/Lean proofs + WASM/React/Cesium web client";

  # Applied with `nix develop --accept-flake-config` when the user is trusted
  # (single-user Nix, or the user listed in trusted-users). On a multi-user
  # daemon, restricted settings such as http2 are ignored unless they are in
  # the daemon's nix.conf; see docs/adr/0001-toolchain.md.
  nixConfig = {
    extra-substituters = [ "https://hacl.cachix.org" ];
    extra-trusted-public-keys = [
      "hacl.cachix.org-1:FzsZ2xsByOwKwIWNPII7yMOelJNDZ12mDAj3d1eGX0c="
    ];
    # cache.nixos.org is Fastly in front of S3. Parallel HTTP/2 downloads
    # often die with curl error 55 ("Failed sending data to the peer").
    # Nix then resumes at the byte offset it reached. If that offset equals
    # Content-Length, the CDN answers 416 and, without fallback, the whole
    # shell fails even though the object is intact. HTTP/1.1 avoids the
    # reset; fallback builds the path if a substitute still fails.
    http2 = false;
    fallback = true;
  };

  inputs = {
    # Proof toolchain: Aeneas, the Charon it locks, and Charon's Rust nightly.
    # proofs/lakefile.toml must require the same Aeneas rev (just init-proofs).
    aeneas.url = "github:AeneasVerif/aeneas";
    # General and web tooling (Node, pnpm, Playwright, wasm-bindgen-cli). Kept
    # separate from aeneas/nixpkgs so a proof-toolchain upgrade does not force
    # web tool changes; see docs/adr/0001-toolchain.md.
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    {
      self,
      nixpkgs,
      aeneas,
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system);
    in
    {
      formatter = forAllSystems (system: nixpkgs.legacyPackages.${system}.nixfmt);

      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          aeneasTools = aeneas.packages.${system};
          # The Rust toolchain Charon is built with, plus rust-analyzer and the
          # browser target. One compiler for native, WASM, and extraction.
          rustToolchain = aeneas.inputs.charon.packages.${system}.rustToolchain.override (old: {
            extensions = (old.extensions or [ ]) ++ [ "rust-analyzer" ];
            targets = (old.targets or [ ]) ++ [ "wasm32-unknown-unknown" ];
          });
          # Must equal the `wasm-bindgen` crate version in Cargo.lock
          # (checked by `just toolchain-check`).
          wasmBindgenCli = pkgs.wasm-bindgen-cli_0_2_126;
          # Must equal the `@playwright/test` version in web/package.json.
          playwrightBrowsers = pkgs.playwright-driver.browsers.override {
            withFirefox = false;
            withWebkit = false;
          };
        in
        {
          default = pkgs.mkShell {
            packages = [
              aeneasTools.aeneas
              aeneasTools.charon
              rustToolchain
              wasmBindgenCli
              pkgs.nodejs_24
              pkgs.pnpm_10
              pkgs.elan
              pkgs.git
              pkgs.just
              pkgs.watchexec
              pkgs.jq
              pkgs.nixfmt
              pkgs.pkg-config
            ];
            AENEAS_SOURCE = aeneas.outPath;
            # Locked Git revision of Aeneas; proofs/lakefile.toml must pin the same rev.
            AENEAS_REV = aeneas.rev;
            VERIMAP_WASM_BINDGEN_VERSION = wasmBindgenCli.version;
            VERIMAP_PLAYWRIGHT_VERSION = pkgs.playwright-driver.version;
            PLAYWRIGHT_BROWSERS_PATH = playwrightBrowsers;
            PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD = "1";
            PLAYWRIGHT_SKIP_VALIDATE_HOST_REQUIREMENTS = "true";
            # On stderr, so `nix develop --command ...` output stays clean.
            shellHook = ''
              echo "VeriMap dev shell: Rust $(rustc --version | cut -d' ' -f2-) / Charon / Aeneas / Node $(node --version) / pnpm $(pnpm --version)" >&2
              echo "Aeneas rev: $AENEAS_REV (Lean version pinned in proofs/lean-toolchain)" >&2
            '';
          };
        }
      );
    };
}
