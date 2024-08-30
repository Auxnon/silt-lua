{ pkgs ? import <nixpkgs> { } }:
let
  unstable = import (fetchTarball https://channels.nixos.org/nixos-unstable/nixexprs.tar.xz) { };
  list1 = with pkgs.buildPackages; [
    cargo
    lua-language-server
    unstable.rustc
  ];
in
pkgs.mkShell {
  nativeBuildInputs = with pkgs; [
    pkg-config
    cmake
  ]
  ++ list1;
  buildInputs = with pkgs; [ systemd ];
  dbus = pkgs.dbus;
}
