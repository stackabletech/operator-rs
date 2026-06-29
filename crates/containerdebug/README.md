# Container Support Helper

This is a tool meant to make the life of a support engineer easier when working
with docker containers.

We often face issues where we would have loved to know more details about the
environment a container is running in. This tool is meant to print as much
information as possible to stdout (later possibly also to a log file)
to aid in debugging:

It currently prints:

- CPU core count
- Memory (free, total, available)
- CGroup Limits
- Swap information
- Host name
- Kernel information
- CPU architecture
- Mount points and available space on those
- List of all network interfaces with names and IP addresses
- Reverse Lookup for those IP addresses (IP Address => List of hostnames)
- Forward lookup for those hostnames (Hostname => List of IP addresses)

Here is an example of what it looks like on my virtual machine:

```text
2025-02-03T11:37:47.420479Z  INFO containerdebug: stackable_operator::utils::logging: Starting
2025-02-03T11:37:47.420521Z  INFO containerdebug: stackable_operator::utils::logging: This is version 0.1.1 (Git information: 0.1.1-1-gcf51ae1), built for x86_64-unknown-linux-gnu by rustc 1.84.1 (e71f9a9a9 2025-01-27) at Tue, 1 Jan 1980 00:00:00 +0000
2025-02-03T11:37:47.420544Z  INFO containerdebug:SystemInformation::init: containerdebug::system_information: initializing
2025-02-03T11:37:47.426065Z  INFO containerdebug:SystemInformation::init: containerdebug::system_information: init finished
2025-02-03T11:37:47.426135Z  INFO containerdebug:SystemInformation::collect: containerdebug::system_information: Starting data collection
2025-02-03T11:37:47.426693Z  INFO containerdebug:SystemInformation::collect:Resources::collect: containerdebug::system_information::resources: cpus cpus.physical=8 cpus.cores.physical=8
2025-02-03T11:37:47.426727Z  INFO containerdebug:SystemInformation::collect:Resources::collect: containerdebug::system_information::resources: memory memory.total=24604889088 memory.free=17235124224 memory.available=22974955520 memory.used=1629933568
2025-02-03T11:37:47.426746Z  INFO containerdebug:SystemInformation::collect:Resources::collect: containerdebug::system_information::resources: swap swap.total=0 swap.free=0 swap.used=0
2025-02-03T11:37:47.426857Z  INFO containerdebug:SystemInformation::collect:Resources::collect: containerdebug::system_information::resources: not in a cgroup
2025-02-03T11:37:47.426940Z  INFO containerdebug:SystemInformation::collect:OperatingSystem::collect: containerdebug::system_information::os: operating system os.name="NixOS" os.kernel.version="6.6.46" os.version="Linux 24.11 NixOS" os.host_name="nixos2" os.cpu_arch="x86_64"
2025-02-03T11:37:47.427031Z  INFO containerdebug:SystemInformation::collect:User::collect_current: containerdebug::system_information::user: current user user.name="nat" user.uid="Uid(1000)" user.gid="Uid(1000)"
2025-02-03T11:37:47.427436Z  INFO containerdebug:SystemInformation::collect:Disk::collect_all: containerdebug::system_information::disk: found disk disk.mount_point="/" disk.name="/dev/sda2" disk.space.total=106298343424 disk.space.available=72663478272
2025-02-03T11:37:47.427483Z  INFO containerdebug:SystemInformation::collect:Disk::collect_all: containerdebug::system_information::disk: found disk disk.mount_point="/nix/store" disk.name="/dev/sda2" disk.space.total=106298343424 disk.space.available=72663478272
2025-02-03T11:37:47.427499Z  INFO containerdebug:SystemInformation::collect:Disk::collect_all: containerdebug::system_information::disk: found disk disk.mount_point="/boot" disk.name="/dev/sda1" disk.space.total=1071624192 disk.space.available=1022820352
2025-02-03T11:37:47.428771Z  INFO containerdebug:SystemInformation::collect:SystemNetworkInfo::collect: containerdebug::system_information::network: found network interface network.interface.name="lo" network.interface.address=127.0.0.1
2025-02-03T11:37:47.428821Z  INFO containerdebug:SystemInformation::collect:SystemNetworkInfo::collect: containerdebug::system_information::network: found network interface network.interface.name="enp1s0" network.interface.address=192.168.122.138
2025-02-03T11:37:47.428836Z  INFO containerdebug:SystemInformation::collect:SystemNetworkInfo::collect: containerdebug::system_information::network: found network interface network.interface.name="br-82bbc663b8a3" network.interface.address=172.18.0.1
2025-02-03T11:37:47.428847Z  INFO containerdebug:SystemInformation::collect:SystemNetworkInfo::collect: containerdebug::system_information::network: found network interface network.interface.name="docker0" network.interface.address=172.17.0.1
2025-02-03T11:37:47.428861Z  INFO containerdebug:SystemInformation::collect:SystemNetworkInfo::collect: containerdebug::system_information::network: found network interface network.interface.name="lo" network.interface.address=::1
2025-02-03T11:37:47.428874Z  INFO containerdebug:SystemInformation::collect:SystemNetworkInfo::collect: containerdebug::system_information::network: found network interface network.interface.name="enp1s0" network.interface.address=fe80::5054:ff:fe91:5652
2025-02-03T11:37:47.428885Z  INFO containerdebug:SystemInformation::collect:SystemNetworkInfo::collect: containerdebug::system_information::network: found network interface network.interface.name="br-82bbc663b8a3" network.interface.address=fc00:f853:ccd:e793::1
2025-02-03T11:37:47.428910Z  INFO containerdebug:SystemInformation::collect:SystemNetworkInfo::collect: containerdebug::system_information::network: ip addresses network.addresses.ip={127.0.0.1, 172.17.0.1, 172.18.0.1, 192.168.122.138, ::1, fc00:f853:ccd:e793::1, fe80::5054:ff:fe91:5652}
2025-02-03T11:37:47.429002Z  INFO containerdebug:SystemInformation::collect:SystemNetworkInfo::collect: containerdebug::system_information::network: performed reverse DNS lookup for IP ip=127.0.0.1 hostnames=["localhost."]
2025-02-03T11:37:47.437343Z  INFO containerdebug:SystemInformation::collect:SystemNetworkInfo::collect: containerdebug::system_information::network: performed reverse DNS lookup for IP ip=172.17.0.1 hostnames=["nixos2.", "nixos2.local."]
2025-02-03T11:37:47.439911Z  INFO containerdebug:SystemInformation::collect:SystemNetworkInfo::collect: containerdebug::system_information::network: performed reverse DNS lookup for IP ip=172.18.0.1 hostnames=["nixos2.", "nixos2.local."]
2025-02-03T11:37:47.440526Z  INFO containerdebug:SystemInformation::collect:SystemNetworkInfo::collect: containerdebug::system_information::network: performed reverse DNS lookup for IP ip=192.168.122.138 hostnames=["nixos2.kvm."]
2025-02-03T11:37:47.440678Z  INFO containerdebug:SystemInformation::collect:SystemNetworkInfo::collect: containerdebug::system_information::network: performed reverse DNS lookup for IP ip=::1 hostnames=["localhost."]
2025-02-03T11:37:47.441339Z  WARN containerdebug:SystemInformation::collect:SystemNetworkInfo::collect: containerdebug::system_information::network: reverse DNS lookup failed ip=fc00:f853:ccd:e793::1 error=no record found for Query { name: Name("1.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.3.9.7.e.d.c.c.0.3.5.8.f.0.0.c.f.ip6.arpa."), query_type: PTR, query_class: IN }
2025-02-03T11:37:47.442068Z  INFO containerdebug:SystemInformation::collect:SystemNetworkInfo::collect: containerdebug::system_information::network: performed reverse DNS lookup for IP ip=fe80::5054:ff:fe91:5652 hostnames=["nixos2."]
2025-02-03T11:37:47.442149Z  INFO containerdebug:SystemInformation::collect:SystemNetworkInfo::collect: containerdebug::system_information::network: hostnames network.addresses.hostname={"localhost.", "nixos2.", "nixos2.kvm.", "nixos2.local."}
2025-02-03T11:37:47.442300Z  INFO containerdebug:SystemInformation::collect:SystemNetworkInfo::collect: containerdebug::system_information::network: performed forward DNS lookup for hostname hostname="localhost." ips=[127.0.0.1]
2025-02-03T11:37:47.442755Z  INFO containerdebug:SystemInformation::collect:SystemNetworkInfo::collect: containerdebug::system_information::network: performed forward DNS lookup for hostname hostname="nixos2." ips=[127.0.0.2]
2025-02-03T11:37:47.443144Z  INFO containerdebug:SystemInformation::collect:SystemNetworkInfo::collect: containerdebug::system_information::network: performed forward DNS lookup for hostname hostname="nixos2.kvm." ips=[127.0.0.2]
2025-02-03T11:37:47.443628Z  INFO containerdebug:SystemInformation::collect:SystemNetworkInfo::collect: containerdebug::system_information::network: performed forward DNS lookup for hostname hostname="nixos2.local." ips=[192.168.122.138, 172.18.0.1, 172.17.0.1]
2025-02-03T11:37:47.443837Z  INFO containerdebug:SystemInformation::collect: containerdebug::system_information: Data collection finished
```

## Log output

The log-style output above is written to stdout.
It can also be output to files by setting the environment variable
`CONTAINERDEBUG_LOG_DIRECTORY=/path/to/logs/directory`.

This file output will be output as _JSON-formatted logs_, in order to
ease ingestion into a log aggregation system (such as
[Vector](https://vector.dev/)).
These log files will also automatically be rotated over time.

## Data output

The containerdebug tool can write its collected data into a JSON dump,
by setting the `--output=/path/to/dump.json` flag.

This is intended to be queried with tools like [jq](https://jqlang.org/).
However, note that the output format is currently not stable, and may
change over time.

For example:

```json
{
  "resources": {
    "cpu_count": 8,
    "physical_core_count": 8,
    "total_memory": 24604889088,
    "free_memory": 17228574720,
    "available_memory": 22974619648,
    "used_memory": 1630269440,
    "total_swap": 0,
    "free_swap": 0,
    "used_swap": 0,
    "total_memory_cgroup": null,
    "free_memory_cgroup": null,
    "free_swap_cgroup": null
  },
  "os": {
    "name": "NixOS",
    "kernel_version": "6.6.46",
    "version": "Linux 24.11 NixOS",
    "host_name": "nixos2",
    "cpu_arch": "x86_64"
  },
  "current_user": {
    "name": "nat",
    "uid": "1000",
    "gid": "100"
  },
  "disks": [
    {
      "name": "/dev/sda2",
      "mount_point": "/",
      "total_space": 106298343424,
      "available_space": 72267354112
    },
    {
      "name": "/dev/sda2",
      "mount_point": "/nix/store",
      "total_space": 106298343424,
      "available_space": 72267354112
    },
    {
      "name": "/dev/sda1",
      "mount_point": "/boot",
      "total_space": 1071624192,
      "available_space": 1022820352
    }
  ],
  "network": {
    "interfaces": {
      "enp1s0": [
        "192.168.122.138",
        "fe80::5054:ff:fe91:5652"
      ],
      "lo": [
        "127.0.0.1",
        "::1"
      ],
      "br-82bbc663b8a3": [
        "172.18.0.1",
        "fc00:f853:ccd:e793::1"
      ],
      "docker0": [
        "172.17.0.1"
      ]
    },
    "reverse_lookups": {
      "fe80::5054:ff:fe91:5652": [
        "nixos2."
      ],
      "127.0.0.1": [
        "localhost."
      ],
      "192.168.122.138": [
        "nixos2.kvm."
      ],
      "::1": [
        "localhost."
      ],
      "172.17.0.1": [
        "nixos2.",
        "nixos2.local."
      ],
      "172.18.0.1": [
        "nixos2.",
        "nixos2.local."
      ]
    },
    "forward_lookups": {
      "nixos2.kvm.": [
        "127.0.0.2"
      ],
      "nixos2.": [
        "127.0.0.2"
      ],
      "nixos2.local.": [
        "192.168.122.138",
        "172.18.0.1",
        "172.17.0.1"
      ],
      "localhost.": [
        "127.0.0.1"
      ]
    }
  }
}
```

## Continuous mode

If given the `--loop` flag, containerdebug will stay in the background and
re-run on a fixed interval. The default interval is `30m` (every 30 minutes), but
it can be customized as desired (e.g. `--loop=30s`).
