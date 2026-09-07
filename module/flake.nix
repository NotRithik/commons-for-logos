{
  description = "Commons Logos Lambda Prize Basecamp UI module";

  inputs = {
    logos-module-builder.url = "github:logos-co/logos-module-builder/1c2532b2de614c0cd2fc68544fc7b1efed944363";
    commons_logos_sdk_src = {
      url = "path:../sdk";
      flake = false;
    };
  };

  outputs = inputs@{ logos-module-builder, commons_logos_sdk_src, ... }:
    logos-module-builder.lib.mkLogosQmlModule {
      src = ./.;
      configFile = ./metadata.json;
      flakeInputs = inputs;
      preConfigure = ''
        rm -rf sdk
        cp -R ${commons_logos_sdk_src} sdk
        chmod -R u+w sdk
      '';
    };
}
