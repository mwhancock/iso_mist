{
  description = "iso_mist - ESP32-C6 firmware";

  inputs = {
    # Pinned so the toolchain and the flashing tools belong to this repository
    # rather than to whatever nixpkgs the machine happens to run.
    nixpkgs.url = "github:NixOS/nixpkgs/1d060086d60cb76a40cf37109f979d88a5bd6396";

    # The base Rust shell from ~/dotfiles/devshells. `follows` makes it run
    # against the pin above, so nixpkgs is still chosen here, not by the
    # shared flake.
    devshells = {
      url = "path:/home/mark/dotfiles/devshells";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { devshells, ... }:
    let
      system = "x86_64-linux";
      lib = devshells.lib.${system};
    in
    {
      devShells.${system}.default = lib.mkRust {
        # The compiler and its riscv32 std source come from the shell, so
        # `cargo build` works without rustup or a rust-toolchain.toml.
        targets = [ "riscv32imac-unknown-none-elf" ];

        extraPackages = with lib.pkgs; [
          # .cargo/config.toml runs binaries through probe-rs, and espflash is
          # the standalone flasher. Both ship with the repository instead of
          # living in the system profile.
          probe-rs-tools
          espflash
        ];

        shellHook = ''
          echo "--- ESP32-C6 Embedded Rust Environment Active ---" >&2
        '';
      };
    };
}
