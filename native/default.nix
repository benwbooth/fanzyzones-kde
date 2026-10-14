{ lib, stdenv, cmake, kdePackages }:
stdenv.mkDerivation {
  pname = "fanzyzones-input";
  version = "0.3.1";
  src = ./.;
  nativeBuildInputs = [ cmake ];
  buildInputs = [ kdePackages.kwin kdePackages.kdecoration kdePackages.qtdeclarative ];
  dontWrapQtApps = true;
  meta = {
    description = "KWin input bridge for FanzyZones two-finger zoom and rectangle framing";
    license = lib.licenses.mit;
    platforms = lib.platforms.linux;
  };
}
