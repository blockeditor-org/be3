# nl.be3.pfg.pw: the build server (guides/build_server.md, guides/servers.md), on a Hetzner CPX62 in Helsinki.
#
# NativeLink's cache, scheduler and worker run as one process inside the image buck/tools/defs.bzl's
# worker_properties() names, so actions run there exactly as they would anywhere else. Caddy (Caddyfile) is the only
# way in from outside; re-relay's tunnel-server carries the calls of clients behind a slow proxy; and the dashboard
# (dashboard/) is node_exporter, the OpenTelemetry Collector, Prometheus, nl-stats.py and Grafana. Everything is on the
# server's own disk: the cache, the dashboard's history and Caddy's tokens.
{ modulesPath, pkgs, ... }:
let
  host = "nl.be3.pfg.pw";
  secrets = "/var/lib/nl-secrets";

  # The image actions run in: buck/tools/defs.bzl worker_properties() and nativelink.json5's platform_properties.
  actionImage = "docker.io/library/buildpack-deps@sha256:2607512c685336a441eba9719ab17da07137ab3178ae8b7118dfe1dff7991549";
  prometheusImage = "docker.io/prom/prometheus@sha256:fa3302279710be2f21a8301ec17a8b4f2301fd829bd113839b3b6b02b6f191a2"; # v3.15.1
  otelImage = "docker.io/otel/opentelemetry-collector-contrib@sha256:39923a8e431bd1f57be82411999d389fcfe40857492e4365456d97a4c1f74be6"; # 0.162.0
  grafanaImage = "docker.io/grafana/grafana@sha256:9b031995a5483a44f80a5dd214a70cecb31448b42d765b44d3756e17b855775f"; # 13.0.13

  # NativeLink's own static (musl) release, which runs unchanged inside the action image.
  nativelinkVersion = "1.8.6";
  nativelink = pkgs.runCommand "nativelink-${nativelinkVersion}" {
    src = pkgs.fetchurl {
      url = "https://github.com/TraceMachina/nativelink/releases/download/v${nativelinkVersion}/nativelink-${nativelinkVersion}-x86_64-unknown-linux-musl.tar.gz";
      hash = "sha256-m60QqhwRoA+xVE3dKKWlq4ks+kKBfpamzxjxad9/BHI="; # GitHub's published digest
    };
  } "mkdir -p $out/bin && tar xzf $src -C $out/bin nativelink";

  # The tunnel's server end, from the same source clients run (guides/build_server.md, Behind a proxy).
  re-relay = pkgs.buildGoModule {
    pname = "re-relay";
    version = "0";
    src = ../../scripts/internal/re-relay;
    vendorHash = null;
    env.CGO_ENABLED = 0;
  };

  site = pkgs.runCommand "nl-gate" { } "mkdir $out && cp ${./dashboard/gate.html} $out/gate.html";

  # Grafana's SQLite data source, for nl-stats.py's records of builds (grafana-datasources.yaml).
  sqlitePlugin = pkgs.runCommand "frser-sqlite-datasource-4.0.6" {
    src = pkgs.fetchurl {
      url = "https://grafana.com/api/plugins/frser-sqlite-datasource/versions/4.0.6/download";
      name = "frser-sqlite-datasource-4.0.6.zip";
      hash = "sha256-5c4jVN0mupvqVrN/ntQFDMkXpGuFigoZHhyv2jgr8Yo="; # grafana.com's published digest
    };
    nativeBuildInputs = [ pkgs.unzip ];
  } "mkdir $out && unzip -q $src -d $out && chmod 0755 $out/frser-sqlite-datasource/gpx_*";

  grafanaProvisioning = pkgs.runCommand "nl-grafana-provisioning" { } ''
    mkdir -p $out/datasources $out/dashboards $out/plugins $out/alerting
    cp ${./dashboard/grafana-datasources.yaml} $out/datasources/grafana-datasources.yaml
    cp ${./dashboard/grafana-dashboards.yaml} $out/dashboards/grafana-dashboards.yaml
  '';
  grafanaDashboards = pkgs.runCommand "nl-grafana-dashboards" { } ''
    mkdir $out && cp ${./dashboard/build-server.json} $out/build-server.json
  '';

  # A container run by systemd as `docker run --rm` on the host's network, stopped with the unit.
  container = { description, name, args, after ? [ ], stopTimeout ? 10 }: {
    inherit description;
    after = [ "network-online.target" "docker.service" ] ++ after;
    wants = [ "network-online.target" ];
    requires = [ "docker.service" ];
    wantedBy = [ "multi-user.target" ];
    serviceConfig = {
      ExecStartPre = "-${pkgs.docker}/bin/docker rm -f ${name}";
      ExecStart = "${pkgs.docker}/bin/docker run --rm --name ${name} --network host ${builtins.concatStringsSep " " args}";
      ExecStop = "${pkgs.docker}/bin/docker stop -t ${toString stopTimeout} ${name}";
      Restart = "always";
      RestartSec = 5;
      TimeoutStartSec = "15min"; # the first start pulls the image
    };
  };
in
{
  imports = [
    (modulesPath + "/profiles/qemu-guest.nix")
    ./disk.nix
  ];

  networking.hostName = "build";
  system.stateVersion = "26.05";

  boot.loader.grub = {
    enable = true;
    efiSupport = true;
    efiInstallAsRemovable = true;
  };
  boot.initrd.availableKernelModules = [ "ata_piix" "uhci_hcd" "virtio_pci" "virtio_scsi" "sd_mod" "sr_mod" ];

  # Hetzner gives the IPv4 address by DHCP and the IPv6 one only in its console. The second interface is Hetzner's
  # private network, also by DHCP.
  networking.useDHCP = false;
  networking.useNetworkd = true;
  systemd.network.networks."10-wan" = {
    matchConfig.MACAddress = "92:00:0a:7c:a9:7d";
    networkConfig.DHCP = "ipv4";
    address = [ "2a01:4f9:c015:fbaf::1/64" ];
    routes = [ { Gateway = "fe80::1"; } ];
  };
  systemd.network.networks."20-private" = {
    matchConfig.MACAddress = "86:00:00:11:87:d0";
    networkConfig.DHCP = "ipv4";
  };

  # The worker does not schedule by memory, so a large link (be-host's fat LTO link took about 10 GB before it was
  # removed) beside 15 other actions can run out; swap slows that down rather than OOM-killing the link. Swappiness 10 keeps the CAS's files in the page
  # cache rather than swapping out what runs.
  swapDevices = [ { device = "/swapfile"; size = 16384; } ];
  boot.kernel.sysctl."vm.swappiness" = 10;

  networking.firewall = {
    allowedTCPPorts = [ 80 443 ];
    allowedUDPPorts = [ 443 ];
  };

  virtualisation.docker.enable = true;

  # Caddy's tokens, each made once and kept on the disk: NL_TUNNEL_TOKEN is clients' BE3_NL_TOKEN, NL_DASH_TOKEN the
  # dashboard's cookie (guides/build_server.md). Read one with `ssh root@nl.be3.pfg.pw cat /var/lib/nl-secrets/caddy.env`.
  systemd.services.nl-secrets = {
    description = "Caddy's tokens for the build server";
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
    };
    script = ''
      install -d -m 0700 ${secrets}
      token() { head -c 32 /dev/urandom | ${pkgs.coreutils}/bin/base64 | tr -dc 'A-Za-z0-9'; }
      (umask 077; touch ${secrets}/caddy.env
        grep -q '^NL_TUNNEL_TOKEN=.' ${secrets}/caddy.env || echo "NL_TUNNEL_TOKEN=$(token)" >> ${secrets}/caddy.env
        grep -q '^NL_DASH_TOKEN=.' ${secrets}/caddy.env || echo "NL_DASH_TOKEN=$(token)" >> ${secrets}/caddy.env)
    '';
  };

  systemd.tmpfiles.rules = [
    "d /var/lib/nativelink 0755 root root -"
    "d /var/lib/nl-stats 0755 root root -"
    "d /var/lib/prometheus/node-exporter 0755 root root -"
    # The containers' own users own their data.
    "d /var/lib/nl-prometheus 0755 65534 65534 -"
    "d /var/lib/nl-grafana 0755 472 0 -"
  ];

  systemd.services.nativelink = container {
    description = "NativeLink cache, scheduler and worker (be3), in the action image";
    name = "nativelink";
    stopTimeout = 30;
    args = [
      "--ulimit nofile=1048576:1048576"
      "-v ${nativelink}/bin/nativelink:/usr/local/bin/nativelink:ro"
      "-v ${./nativelink.json5}:/etc/nativelink/nativelink.json5:ro"
      "-v /var/lib/nativelink:/work"
      actionImage
      "/usr/local/bin/nativelink /etc/nativelink/nativelink.json5"
    ];
  };

  systemd.services.nl-tunnel = {
    description = "re-relay's tunnel-server: carries a client's calls to NativeLink inside a few long HTTP/1.1 requests";
    after = [ "network-online.target" "nativelink.service" ];
    wants = [ "network-online.target" ];
    wantedBy = [ "multi-user.target" ];
    startLimitIntervalSec = 0;
    serviceConfig = {
      ExecStart = "${re-relay}/bin/re-relay tunnel-server -listen 127.0.0.1:50070 -upstream 127.0.0.1:50052";
      DynamicUser = true;
      Restart = "always";
      RestartSec = 3;
    };
  };

  services.caddy = {
    enable = true;
    configFile = ./Caddyfile;
  };
  systemd.services.caddy = {
    after = [ "nl-secrets.service" "nativelink.service" ];
    requires = [ "nl-secrets.service" ];
    environment = {
      NL_HOST = host;
      NL_SITE = "${site}";
    };
    serviceConfig.EnvironmentFile = "${secrets}/caddy.env";
  };

  # The dashboard (dashboard/, guides/build_server.md, The dashboard).
  services.prometheus.exporters.node = {
    enable = true;
    listenAddress = "127.0.0.1";
    port = 9100;
    enabledCollectors = [ "textfile" ];
    extraFlags = [ "--collector.textfile.directory=/var/lib/prometheus/node-exporter" ];
  };

  systemd.services.nl-stats = {
    description = "The build server's records of builds and count of running actions, for the dashboard (nl-stats.py)";
    after = [ "network-online.target" ];
    wants = [ "network-online.target" ];
    wantedBy = [ "multi-user.target" ];
    serviceConfig = {
      ExecStart = "${pkgs.python3}/bin/python3 ${./dashboard/nl-stats.py}";
      Restart = "always";
      RestartSec = 5;
      ProtectSystem = "strict";
      ReadWritePaths = [ "/var/lib/nl-stats" "/var/lib/prometheus/node-exporter" ];
      ProtectHome = true;
      PrivateTmp = true;
    };
  };

  systemd.services.nl-prometheus = container {
    description = "Prometheus, a year of the build server's metrics for the dashboard (prometheus.yml)";
    name = "nl-prometheus";
    args = [
      "--user 65534:65534"
      "-v ${./dashboard/prometheus.yml}:/etc/prometheus/prometheus.yml:ro"
      "-v /var/lib/nl-prometheus:/prometheus"
      prometheusImage
      "--config.file=/etc/prometheus/prometheus.yml --storage.tsdb.path=/prometheus"
      "--storage.tsdb.retention.time=400d --storage.tsdb.retention.size=20GB --web.listen-address=127.0.0.1:9090"
    ];
  };

  systemd.services.nl-otel = container {
    description = "The OpenTelemetry Collector, which turns NativeLink's metrics into Prometheus's (otel-collector.yaml)";
    name = "nl-otel";
    args = [
      "-v ${./dashboard/otel-collector.yaml}:/etc/otelcol/config.yaml:ro"
      otelImage
      "--config=/etc/otelcol/config.yaml"
    ];
  };

  # Anyone past Caddy's gate is a viewer who can also explore: there is no login. The dashboards are build-server.json.
  systemd.services.nl-grafana = container {
    description = "Grafana, the build server's dashboard, behind Caddy's cookie gate (Caddyfile)";
    name = "nl-grafana";
    after = [ "nl-prometheus.service" "nl-stats.service" ];
    args = [
      "-e GF_SERVER_HTTP_ADDR=127.0.0.1 -e GF_SERVER_HTTP_PORT=3000 -e GF_SERVER_ROOT_URL=https://${host}/"
      "-e GF_AUTH_ANONYMOUS_ENABLED=true -e GF_AUTH_ANONYMOUS_ORG_ROLE=Viewer -e GF_AUTH_DISABLE_LOGIN_FORM=true"
      "-e GF_USERS_VIEWERS_CAN_EDIT=true -e GF_ANALYTICS_REPORTING_ENABLED=false -e GF_ANALYTICS_CHECK_FOR_UPDATES=false"
      "-e GF_ANALYTICS_CHECK_FOR_PLUGIN_UPDATES=false -e GF_NEWS_NEWS_FEED_ENABLED=false"
      "-e GF_DASHBOARDS_DEFAULT_HOME_DASHBOARD_PATH=/etc/grafana/dashboards/build-server.json"
      "-e GF_PATHS_PLUGINS=/etc/grafana/plugins"
      "-v /var/lib/nl-grafana:/var/lib/grafana"
      "-v ${grafanaProvisioning}:/etc/grafana/provisioning:ro"
      "-v ${grafanaDashboards}:/etc/grafana/dashboards:ro"
      "-v ${sqlitePlugin}:/etc/grafana/plugins:ro"
      "-v /var/lib/nl-stats:/var/lib/nl-stats:ro"
      grafanaImage
    ];
  };

  environment.systemPackages = [ pkgs.sqlite pkgs.htop pkgs.strace pkgs.perf ];
}
