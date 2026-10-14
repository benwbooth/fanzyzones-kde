{ stdenv, cmake, kdePackages }:
stdenv.mkDerivation {
  pname = "fanzyzones-test-input";
  version = "1";
  src = ./.;
  nativeBuildInputs = [ cmake ];
  buildInputs = [ kdePackages.kwin kdePackages.kdecoration kdePackages.qtdeclarative ];
  dontWrapQtApps = true;
}
